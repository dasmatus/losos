/* The recovery-code step — NOT in the wizard at the moment.
 *
 * Hidden on 2026-10-06 (see ./steps.ts): nothing can accept the code after a
 * reinstall yet, so the step asked for a piece of paper that proves nothing.
 * The file stays as it was so the step can be put back by re-listing it in
 * STEP_IDS and re-wiring Wizard.tsx; its i18n keys and GET /api/recovery
 * are still shipped. The spare admin key, which this step's printed sheet
 * used to carry, prints from step 2 now (StepSignIn.tsx, AdminKey).
 *
 * What follows is the step as it was.
 *
 * The one piece of paper this appliance asks for. backend/src/recovery.rs
 * explains the mechanism at length; the short version, and the version the
 * owner is shown, is that a reinstall gives the box a new identity and the
 * other boxes then refuse it as a stranger using a name they already know.
 * This code is the only evidence of continuity that survives the wipe, which
 * is exactly why it cannot be stored on the thing it recovers.
 *
 * So Continue is gated on the owner having taken it off the screen — copied
 * or printed. The gate is the point of the step; a Continue button that was
 * always live would make this a page people skip.
 */

import * as React from "react";
import { HugeiconsIcon } from "@hugeicons/react";
import {
  Alert02Icon,
  CheckmarkCircle02Icon,
  Copy01Icon,
  InformationCircleIcon,
  Key01Icon,
  PrinterIcon,
  RefreshIcon,
} from "@hugeicons/core-free-icons";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { intlTag, t } from "@/lib/i18n";
import { useT } from "@/lib/i18n-react";
import { getRecovery, isMissingRoute } from "./api";
import { copyText } from "./copy";
import { Callout, StepText } from "./parts";

export interface StepRecoveryProps {
  boxName: string;
  /** The admin key step 2 received from the claim, printed on the sheet
   *  under the code. Null when this tab did not claim the box (a second
   *  visit to the wizard, or a password changed through set-password). */
  adminKey: string | null;
  /** True once the owner has copied or printed it. Owned by the wizard. */
  saved: boolean;
  onSaved: () => void;
  /** The box cannot produce a code. Unblocks Continue, with a changed label. */
  onUnavailable: () => void;
}

type CodeQuery =
  | { kind: "loading" }
  | { kind: "ready"; code: string; minted: boolean }
  /** GET /api/recovery is not served by this box's software — see ./api.ts. */
  | { kind: "not-implemented" }
  | { kind: "failed"; message: string };

export function StepRecovery({
  boxName,
  adminKey,
  saved,
  onSaved,
  onUnavailable,
}: StepRecoveryProps) {
  const t = useT();
  const [query, setQuery] = React.useState<CodeQuery>({ kind: "loading" });
  const [attempt, setAttempt] = React.useState(0);
  const [copyFailed, setCopyFailed] = React.useState(false);
  const codeRef = React.useRef<HTMLElement>(null);

  React.useEffect(() => {
    const controller = new AbortController();
    let live = true;

    void (async () => {
      try {
        const recovery = await getRecovery({ signal: controller.signal });
        if (live) setQuery({ kind: "ready", code: recovery.code, minted: recovery.minted });
      } catch (error) {
        if (!live || controller.signal.aborted) return;
        if (isMissingRoute(error)) setQuery({ kind: "not-implemented" });
        else setQuery({ kind: "failed", message: describe(error) });
      }
    })();

    return () => {
      live = false;
      controller.abort();
    };
  }, [attempt]);

  /* A code that cannot be produced must not trap the owner on this step. The
   * wizard is told separately from `onSaved` so its Continue button can say
   * "without a code" — an unblocked gate that still reads as unfinished. */
  React.useEffect(() => {
    if (query.kind === "not-implemented" || query.kind === "failed") onUnavailable();
  }, [query.kind, onUnavailable]);

  const copy = async (): Promise<void> => {
    if (query.kind !== "ready") return;
    if (await copyText(query.code, codeRef.current)) {
      setCopyFailed(false);
      onSaved();
    } else {
      setCopyFailed(true);
    }
  };

  const print = (): void => {
    if (query.kind !== "ready") return;
    // Counted as saved on the call, not on completion: nothing tells a page
    // whether the owner pressed Print or Cancel in the browser's own dialog.
    onSaved();
    window.print();
  };

  return (
    <div className="flex flex-col gap-5">
      <StepText>{t("wizard.recovery.intro")}</StepText>

      {query.kind === "loading" && (
        <div aria-busy="true" className="flex flex-col gap-3">
          <Skeleton className="h-20 w-full" />
          <Skeleton className="h-9 w-64" />
        </div>
      )}

      {query.kind === "ready" && (
        <>
          <div className="flex flex-col gap-3 rounded-card border border-line bg-sunk px-4 py-5">
            <span className="text-[12px] font-medium tracking-wide text-faint uppercase">
              {t("wizard.recovery.codeFor", { name: boxName })}
            </span>
            {/* select-all: one click takes the whole code, so a manual
                Ctrl+C works even where the clipboard API is unavailable. */}
            <code
              ref={codeRef}
              className="numeric text-xl leading-snug break-all text-ink select-all sm:text-2xl"
            >
              {query.code}
            </code>
            {!query.minted && (
              <p className="text-[12.5px] text-faint">
                {t("wizard.recovery.notMinted")}
              </p>
            )}
          </div>

          <div className="flex flex-wrap items-center gap-2.5">
            <Button variant={saved ? "secondary" : "primary"} onClick={() => void copy()}>
              <HugeiconsIcon
                icon={Copy01Icon}
                size={18}
                strokeWidth={1.5}
                color="currentColor"
                aria-hidden="true"
              />
              {t("wizard.recovery.copy")}
            </Button>
            <Button variant="secondary" onClick={print}>
              <HugeiconsIcon
                icon={PrinterIcon}
                size={18}
                strokeWidth={1.5}
                color="currentColor"
                aria-hidden="true"
              />
              {t("wizard.recovery.print")}
            </Button>
            {saved && (
              <span
                role="status"
                className="inline-flex items-center gap-1.5 text-[13px] text-ok"
              >
                <HugeiconsIcon
                  icon={CheckmarkCircle02Icon}
                  size={18}
                  strokeWidth={1.5}
                  color="currentColor"
                  aria-hidden="true"
                />
                {t("wizard.recovery.saved")}
              </span>
            )}
          </div>

          {copyFailed && (
            <Callout tone="warn" icon={Alert02Icon} title={t("wizard.recovery.copyFailed.title")}>
              <p className="mt-1">{t("wizard.recovery.copyFailed.body")}</p>
            </Callout>
          )}

          {/* Present in the document at all times, shown only on paper: the
              print rules in ./wizard.css hide everything else on the page and
              force this subtree to black on white. */}
          <RecoverySheet boxName={boxName} code={query.code} adminKey={adminKey} />
        </>
      )}

      {query.kind === "not-implemented" && (
        <Callout tone="info" icon={InformationCircleIcon} title={t("wizard.recovery.notYet.title")}>
          <p className="mt-1">{t("wizard.recovery.notYet.body")}</p>
        </Callout>
      )}

      {query.kind === "failed" && (
        <div className="flex flex-col gap-3">
          <Callout tone="crit" icon={Alert02Icon} title={t("wizard.recovery.failed.title")}>
            <p className="mt-1 break-words">{query.message}</p>
          </Callout>
          <div>
            <Button variant="secondary" onClick={() => setAttempt((n) => n + 1)}>
              <HugeiconsIcon
                icon={RefreshIcon}
                size={18}
                strokeWidth={1.5}
                color="currentColor"
                aria-hidden="true"
              />
              {t("wizard.recovery.retry")}
            </Button>
          </div>
        </div>
      )}
    </div>
  );
}

/* What comes out of the printer.
 *
 * Its own markup rather than a print stylesheet over the card above, because
 * the two say different things: the screen version is a step in a flow, the
 * paper version has to still make sense in a drawer in two years, with no
 * wizard around it. Hence the date and the full sentence. It prints in the
 * interface language, date included, so the sheet reads as one language. */
function RecoverySheet({
  boxName,
  code,
  adminKey,
}: {
  boxName: string;
  code: string;
  adminKey: string | null;
}) {
  const t = useT();
  return (
    <section className="wizard-print-sheet hidden print:block">
      <h1 className="text-lg font-semibold">
        {t("wizard.recovery.codeFor", { name: boxName })}
      </h1>
      <p className="mt-1 text-sm">
        {t("wizard.recovery.printed", {
          date: new Date().toLocaleDateString(intlTag(), { dateStyle: "long" }),
        })}
      </p>
      <p className="numeric mt-6 text-2xl break-all">{code}</p>
      <p className="mt-6 max-w-prose text-sm leading-relaxed">
        {t("wizard.recovery.sheetBody", { name: boxName })}
      </p>
      {/* The admin key goes on the same sheet because it has the same
          property: the box shows it once and keeps no copy a browser can ask
          for. One piece of paper, two things that must leave the box. */}
      {adminKey !== null && (
        <>
          <h2 className="mt-8 text-base font-semibold">
            {t("wizard.recovery.sheetAdminKey", { name: boxName })}
          </h2>
          <p className="numeric mt-2 text-base break-all">{adminKey}</p>
          <p className="mt-2 max-w-prose text-sm leading-relaxed">
            {t("wizard.recovery.sheetAdminKeyBody")}
          </p>
        </>
      )}
      <p className="mt-4 flex items-center gap-2 text-sm">
        <HugeiconsIcon
          icon={Key01Icon}
          size={16}
          strokeWidth={1.5}
          color="currentColor"
          aria-hidden="true"
        />
        {t("wizard.recovery.sheetKey")}
      </p>
    </section>
  );
}

function describe(error: unknown): string {
  if (error instanceof Error && error.message.length > 0) return error.message;
  return t("wizard.noAnswer");
}
