/* Step 1 — Trust this box.
 *
 * The box signs its own certificate (modules/tls.nix), so every browser on
 * the network treats its https address as untrusted until the owner installs
 * it. This step hands over the certificate and the fingerprint to check it
 * against, and asks them to come back on the encrypted address.
 *
 * It is the one step that can be skipped honestly: the certificate may already
 * be installed, or `losos.tls.enable` may be off, and either way the rest of
 * the wizard still works over plain http.
 *
 * Whether skipping costs the passkey is a question about the BROWSER, not
 * about the box, and this step used to get that wrong — it read the box's
 * `tls` flag and promised "the passkey option will not be offered", while step
 * 2 read `window.isSecureContext` and offered it anyway. The two part company
 * on `http://localhost` and `http://127.0.0.1`, which are secure contexts
 * whatever the box is doing. Both now read `passkeysPossibleHere()`.
 */

import { HugeiconsIcon } from "@hugeicons/react";
import {
  Alert02Icon,
  Certificate01Icon,
  CheckmarkCircle02Icon,
  Download01Icon,
  GlobeIcon,
  InformationCircleIcon,
  LinkSquare02Icon,
  SecurityLockIcon,
} from "@hugeicons/core-free-icons";
import { buttonVariants } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { intlTag } from "@/lib/i18n";
import { useT } from "@/lib/i18n-react";
import { cn } from "@/lib/utils";
import { ReadoutRow, StepText } from "./parts";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { HelpLink } from "@/components/ui/help-link";
import { passkeysPossibleHere } from "./passkey";
import { TrustCommand } from "./TrustCommand";
import type { SetupQuery } from "./useSetupState";

export function StepTrust({ query }: { query: SetupQuery }) {
  const t = useT();
  return (
    <div className="flex flex-col gap-5">
      <StepText>{t("wizard.trust.intro")}</StepText>

      {query.kind === "loading" && <LoadingTrust />}
      {query.kind === "ready" && <ReadyTrust query={query} />}

      {query.kind === "blocked" && (
        <Alert variant="warn">
          <HugeiconsIcon icon={Alert02Icon} size={19} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
          <AlertTitle>{t("wizard.trust.blocked.title")}</AlertTitle>
          <AlertDescription>
            <p>{t("wizard.trust.blocked.body")}</p>
            <HelpLink entry="certificate-warning" />
          </AlertDescription>
        </Alert>
      )}

      {query.kind === "absent" && (
        <Alert variant="default">
          <HugeiconsIcon icon={InformationCircleIcon} size={19} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
          <AlertTitle>{t("wizard.trust.nothing.title")}</AlertTitle>
          <AlertDescription>
            <p>
              {passkeysPossibleHere()
                ? t("wizard.trust.absent")
                : t("wizard.trust.absentNoPasskey")}
            </p>
          </AlertDescription>
        </Alert>
      )}

      {query.kind === "failed" && (
        <Alert variant="crit">
          <HugeiconsIcon icon={Alert02Icon} size={19} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
          <AlertTitle>{t("wizard.trust.failed.title")}</AlertTitle>
          <AlertDescription>
            <p className="break-words">{query.message}</p>
            <p>{t("wizard.trust.failed.body")}</p>
            <HelpLink entry="certificate-warning" />
          </AlertDescription>
        </Alert>
      )}
    </div>
  );
}

function LoadingTrust() {
  return (
    <div className="flex flex-col gap-3" aria-busy="true">
      <Skeleton className="h-9 w-56" />
      <Skeleton className="h-16 w-full" />
    </div>
  );
}

function ReadyTrust({
  query,
}: {
  query: Extract<SetupQuery, { kind: "ready" }>;
}) {
  const t = useT();
  const { fqdn, tls, certificate } = query.state;
  const secure =
    window.isSecureContext && window.location.protocol === "https:";
  const onTheRightName = secure && window.location.hostname === fqdn;

  if (!tls || certificate === null) {
    return (
      <Alert variant="default">
        <HugeiconsIcon icon={InformationCircleIcon} size={19} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
        <AlertTitle>{t("wizard.trust.nothing.title")}</AlertTitle>
        <AlertDescription>
          <p>
            {passkeysPossibleHere()
              ? t("wizard.trust.noTls", { name: query.state.hostName })
              : t("wizard.trust.noTlsNoPasskey", { name: query.state.hostName })}
          </p>
        </AlertDescription>
      </Alert>
    );
  }

  // The one-line installer leads, the download follows as the manual route.
  // Not on a page the browser already trusts: the step is done there, and a
  // panel asking to install what is installed reads as "something is wrong".
  const quick = secure ? null : certificate.install;

  return (
    <div className="flex flex-col gap-5">
      {quick !== null && (
        <TrustCommand
          install={quick}
          fqdn={fqdn}
          address={query.state.address}
        />
      )}

      {quick !== null && (
        <p className="text-[13px] leading-snug text-muted">
          {t("wizard.trust.byHand")}
        </p>
      )}

      <div className="flex flex-wrap items-center gap-2.5">
        {/* An anchor, not a button: this is a download, and the browser's own
            handling of the content type is what opens Firefox's import dialog
            and iOS' profile flow. No `download` attribute for that reason —
            it would force a plain save on browsers that would otherwise
            offer to install it. */}
        <a
          href={certificate.url}
          className={cn(buttonVariants({ variant: "primary" }))}
          rel="noopener"
        >
          <HugeiconsIcon
            icon={Download01Icon}
            size={18}
            strokeWidth={1.5}
            color="currentColor"
            aria-hidden="true"
          />
          {t("wizard.trust.getCert")}
        </a>

        {onTheRightName ? (
          <span className="inline-flex items-center gap-1.5 text-[13px] text-ok">
            <HugeiconsIcon
              icon={CheckmarkCircle02Icon}
              size={18}
              strokeWidth={1.5}
              color="currentColor"
              aria-hidden="true"
            />
            {t("wizard.trust.alreadyEncrypted")}
          </span>
        ) : (
          <a
            href={`https://${fqdn}${window.location.pathname}${window.location.search}`}
            className={cn(buttonVariants({ variant: "secondary" }))}
          >
            <HugeiconsIcon
              icon={LinkSquare02Icon}
              size={18}
              strokeWidth={1.5}
              color="currentColor"
              aria-hidden="true"
            />
            {t("wizard.trust.reopen", { fqdn })}
          </a>
        )}
      </div>

      <div className="flex flex-col gap-4 rounded-control border border-line bg-sunk px-4 py-3.5">
        <ReadoutRow label={t("wizard.trust.checkFingerprint")}>
          <code className="numeric text-[12.5px] leading-relaxed break-all text-ink select-all">
            {certificate.fingerprintDisplay}
          </code>
        </ReadoutRow>
        <p className="text-[12.5px] leading-snug text-muted">
          {t("wizard.trust.fingerprintBody", { name: query.state.hostName })}
        </p>
        {certificate.expires.length > 0 && (
          <p className="flex items-center gap-1.5 text-[12.5px] text-faint">
            <HugeiconsIcon
              icon={Certificate01Icon}
              size={16}
              strokeWidth={1.5}
              color="currentColor"
              aria-hidden="true"
            />
            {t("wizard.trust.validUntil", {
              date: formatExpiry(certificate.expires),
            })}
          </p>
        )}
      </div>

      {!secure && (
        <Alert variant="warn">
          <HugeiconsIcon icon={SecurityLockIcon} size={19} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
          <AlertTitle>{t("wizard.trust.unencrypted.title")}</AlertTitle>
          <AlertDescription>
            <p>{t("wizard.trust.unencrypted.body", { fqdn })}</p>
          </AlertDescription>
        </Alert>
      )}

      {secure && !onTheRightName && (
        <Alert variant="default">
          <HugeiconsIcon icon={GlobeIcon} size={19} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
          <AlertTitle>{t("wizard.trust.otherName.title")}</AlertTitle>
          <AlertDescription>
            <p>{t("wizard.trust.otherName.body", { fqdn })}</p>
          </AlertDescription>
        </Alert>
      )}
    </div>
  );
}

/* The document carries ISO 8601 in UTC. Shown in the reader's own locale,
 * because a date is the one field here nobody has to compare character for
 * character — unlike the fingerprint two elements up. The reader's locale is
 * the interface language, so the date reads in the same language as the
 * sentence around it. */
function formatExpiry(iso: string): string {
  const when = new Date(iso);
  if (Number.isNaN(when.getTime())) return iso;
  return when.toLocaleDateString(intlTag(), { dateStyle: "long" });
}
