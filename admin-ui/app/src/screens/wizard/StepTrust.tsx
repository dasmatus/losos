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
import { cn } from "@/lib/utils";
import { Callout, ReadoutRow, StepText } from "./parts";
import { passkeysPossibleHere } from "./passkey";
import type { SetupQuery } from "./useSetupState";

export function StepTrust({ query }: { query: SetupQuery }) {
  return (
    <div className="flex flex-col gap-5">
      <StepText>
        This box signs its own certificate, so your browser has never seen it before and will
        not trust it yet. Install the certificate, then reopen this page at the address below.
        Everything after this step travels over that connection. That includes the password you
        are about to set and the passkey you may want, and neither should cross the network in
        the clear.
      </StepText>

      {query.kind === "loading" && <LoadingTrust />}
      {query.kind === "ready" && <ReadyTrust query={query} />}

      {query.kind === "blocked" && (
        <Callout tone="warn" icon={Alert02Icon} title="Not available from here">
          <p className="mt-1">
            The certificate is only handed out to devices on the same network as this box.
            Open this page from that network to install it.
          </p>
        </Callout>
      )}

      {query.kind === "absent" && (
        <Callout tone="info" icon={InformationCircleIcon} title="Nothing to install">
          <p className="mt-1">
            This box is not serving an encrypted address, so there is no certificate to
            trust. You can continue{passkeysPossibleHere() ? "" : "; the passkey option on the next step will not be offered"}.
          </p>
        </Callout>
      )}

      {query.kind === "failed" && (
        <Callout tone="crit" icon={Alert02Icon} title="Could not read the setup details">
          <p className="mt-1 break-words">{query.message}</p>
          <p className="mt-1">
            You can continue without installing the certificate, or reload this page to try
            again.
          </p>
        </Callout>
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

function ReadyTrust({ query }: { query: Extract<SetupQuery, { kind: "ready" }> }) {
  const { fqdn, tls, certificate } = query.state;
  const secure = window.isSecureContext && window.location.protocol === "https:";
  const onTheRightName = secure && window.location.hostname === fqdn;

  if (!tls || certificate === null) {
    return (
      <Callout tone="info" icon={InformationCircleIcon} title="Nothing to install">
        <p className="mt-1">
          {query.state.hostName} is not serving an encrypted address, so there is no
          certificate to trust. You can continue
          {passkeysPossibleHere() ? "" : "; the passkey option on the next step will not be offered"}.
        </p>
      </Callout>
    );
  }

  return (
    <div className="flex flex-col gap-5">
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
          Get the certificate
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
            You are already on the encrypted address.
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
            Reopen at https://{fqdn}
          </a>
        )}
      </div>

      <div className="flex flex-col gap-4 rounded-control border border-line bg-sunk px-4 py-3.5">
        <ReadoutRow label="Check this fingerprint">
          <code className="numeric text-[12.5px] leading-relaxed break-all text-ink select-all">
            {certificate.fingerprintDisplay}
          </code>
        </ReadoutRow>
        <p className="text-[12.5px] leading-snug text-muted">
          Your browser shows the same fingerprint when it asks whether to trust the certificate.
          If the two do not match character for character, stop: something between you and{" "}
          {query.state.hostName} is not this box.
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
            Valid until {formatExpiry(certificate.expires)}. This box mints a new one before
            then on its own.
          </p>
        )}
      </div>

      {!secure && (
        <Callout tone="warn" icon={SecurityLockIcon} title="This page is not encrypted yet">
          <p className="mt-1">
            You can continue either way. On an unencrypted address no browser will offer to
            make a passkey, so step 2 will ask you for a password only.
          </p>
        </Callout>
      )}

      {secure && !onTheRightName && (
        <Callout tone="info" icon={GlobeIcon} title="Encrypted, but under another name">
          <p className="mt-1">
            The certificate is issued for {fqdn}. Reopen this page there so your browser
            accepts it.
          </p>
        </Callout>
      )}
    </div>
  );
}

/* The document carries ISO 8601 in UTC. Shown in the reader's own locale,
 * because a date is the one field here nobody has to compare character for
 * character — unlike the fingerprint two elements up. */
function formatExpiry(iso: string): string {
  const when = new Date(iso);
  if (Number.isNaN(when.getTime())) return iso;
  return when.toLocaleDateString(undefined, { dateStyle: "long" });
}
