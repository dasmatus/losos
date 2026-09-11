/* Step 3 — Write down your recovery code.
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
import { getRecovery, isMissingRoute } from "./api";
import { Callout, StepText } from "./parts";

export interface StepRecoveryProps {
  boxName: string;
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

export function StepRecovery({ boxName, saved, onSaved, onUnavailable }: StepRecoveryProps) {
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
      <StepText>
        A factory reset or a reinstall erases this box's disk, and it comes back with a brand
        new identity. To every other box it works with, that looks like a stranger claiming a
        name they already know, and they refuse it. This code is the only proof that the new box
        is the old one, so it cannot live on the disk it is meant to recover. Put it on paper,
        or in a password manager.
      </StepText>

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
              Recovery code for {boxName}
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
                This box minted it earlier. It is the same code as before, and it will not
                change.
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
              Copy
            </Button>
            <Button variant="secondary" onClick={print}>
              <HugeiconsIcon
                icon={PrinterIcon}
                size={18}
                strokeWidth={1.5}
                color="currentColor"
                aria-hidden="true"
              />
              Print
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
                Taken off the screen.
              </span>
            )}
          </div>

          {copyFailed && (
            <Callout tone="warn" icon={Alert02Icon} title="This browser would not copy it">
              <p className="mt-1">
                The code above is selected. Press Ctrl+C, or Cmd+C on a Mac. Printing works
                either way and counts as saved.
              </p>
            </Callout>
          )}

          {/* Present in the document at all times, shown only on paper: the
              print rules in ./wizard.css hide everything else on the page and
              force this subtree to black on white. */}
          <RecoverySheet boxName={boxName} code={query.code} />
        </>
      )}

      {query.kind === "not-implemented" && (
        <Callout tone="info" icon={InformationCircleIcon} title="No code on this box yet">
          <p className="mt-1">
            The software on this box cannot produce a recovery code yet. You can continue
            without one; come back to this page after the box has updated itself and write the
            code down then.
          </p>
        </Callout>
      )}

      {query.kind === "failed" && (
        <div className="flex flex-col gap-3">
          <Callout tone="crit" icon={Alert02Icon} title="Could not read the code">
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
              Try again
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
 * wizard around it. Hence the date and the full sentence. */
function RecoverySheet({ boxName, code }: { boxName: string; code: string }) {
  return (
    <section className="wizard-print-sheet hidden print:block">
      <h1 className="text-lg font-semibold">Recovery code for {boxName}</h1>
      <p className="mt-1 text-sm">
        Printed {new Date().toLocaleDateString(undefined, { dateStyle: "long" })}.
      </p>
      <p className="numeric mt-6 text-2xl break-all">{code}</p>
      <p className="mt-6 max-w-prose text-sm leading-relaxed">
        Keep this. If {boxName} is ever reset or reinstalled, this code is the only proof that
        the rebuilt box is the same one. Without it the boxes it works with will treat it as a
        stranger and refuse it. There is no copy anywhere else that survives a reset.
      </p>
      <p className="mt-4 flex items-center gap-2 text-sm">
        <HugeiconsIcon
          icon={Key01Icon}
          size={16}
          strokeWidth={1.5}
          color="currentColor"
          aria-hidden="true"
        />
        Treat it like a key to the box, because that is what it is.
      </p>
    </section>
  );
}

/* Copy, by whichever route this browser allows.
 *
 * navigator.clipboard exists only in a secure context, and step 1 of this very
 * wizard is what makes the connection secure — so on the first pass through,
 * over http, it is routinely absent. The fallback selects the code element
 * that is already on screen and asks the document to copy the selection: no
 * synthetic textarea to position (which would need a style attribute the CSP
 * refuses) and the owner can see what was taken. */
async function copyText(text: string, element: HTMLElement | null): Promise<boolean> {
  if (window.isSecureContext && navigator.clipboard !== undefined) {
    try {
      await navigator.clipboard.writeText(text);
      return true;
    } catch {
      /* Permission refused, or no focus. Fall through. */
    }
  }
  return selectAndCopy(element);
}

function selectAndCopy(element: HTMLElement | null): boolean {
  if (element === null) return false;
  const selection = window.getSelection();
  if (selection === null) return false;
  const range = document.createRange();
  range.selectNodeContents(element);
  selection.removeAllRanges();
  selection.addRange(range);
  try {
    // Deprecated, and the only copy this browser has on a plain http page.
    return document.execCommand("copy");
  } catch {
    return false;
  }
}

function describe(error: unknown): string {
  if (error instanceof Error && error.message.length > 0) return error.message;
  return "This box did not answer.";
}
