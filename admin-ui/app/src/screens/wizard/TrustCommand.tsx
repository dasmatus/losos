/* The quick way through step 1: one line, pasted into a terminal.
 *
 * The box renders two installers from its certificate (modules/setup.nix,
 * from modules/setup/trust.{sh,ps1}) and serves them next to it. This panel
 * builds the line that fetches the right one for the computer the owner is
 * sitting at and pipes it into its shell, and makes the case for running it.
 *
 * The case has to be made honestly, because "paste this into a terminal" is
 * exactly what every security page tells people never to do. So the panel
 * says what the line does and does not do, points out that the script comes
 * from the box at the address the page is on rather than from the internet,
 * and links the script itself as plain text: the thing to read before
 * pressing Enter is one click away, and it is short.
 *
 * Always `http://`, whatever this page is on. The box answers on :80 whether
 * or not :443 is trusted, and the certificate that would let curl verify the
 * https address is what the script is about to install.
 */

import * as React from "react";
import { HugeiconsIcon } from "@hugeicons/react";
import { CheckmarkCircle02Icon, Copy01Icon } from "@hugeicons/core-free-icons";
import { Button } from "@/components/ui/button";
import { useT } from "@/lib/i18n-react";
import { cn } from "@/lib/utils";
import type { SetupInstallers } from "./api";
import { copyText } from "./copy";

export type Platform = "unix" | "windows";

/** Where this page is being read. Phones get no terminal, so they get no
 *  line; everything else is one of the two scripts. The user agent is a
 *  hint for the default only — the owner can switch. */
export function detectPlatform(): Platform | "phone" {
  const ua = navigator.userAgent;
  if (/iPhone|iPad|iPod|Android/i.test(ua)) return "phone";
  if (/Windows/i.test(ua)) return "windows";
  return "unix";
}

/** The box, as this browser reaches it, on plain http. The port rides along
 *  only when the page itself is on http with one (a VM's port forward); an
 *  https port would be the wrong one for :80. */
function boxBase(): string {
  const { protocol, host, hostname } = window.location;
  return `http://${protocol === "http:" ? host : hostname}`;
}

export function commandFor(
  platform: Platform,
  install: SetupInstallers,
): string {
  const base = boxBase();
  return platform === "windows"
    ? `irm ${base}${install.ps1} | iex`
    : `curl -fsSL ${base}${install.sh} | sh`;
}

export function TrustCommand({
  install,
  fqdn,
}: {
  install: SetupInstallers;
  fqdn: string;
}) {
  const t = useT();
  const detected = React.useMemo(detectPlatform, []);
  const [platform, setPlatform] = React.useState<Platform>(
    detected === "windows" ? "windows" : "unix",
  );
  const [copied, setCopied] = React.useState(false);
  const lineRef = React.useRef<HTMLElement>(null);

  const command = commandFor(platform, install);
  const scriptUrl =
    boxBase() + (platform === "windows" ? install.ps1 : install.sh);

  const copy = async () => {
    setCopied(await copyText(command, lineRef.current));
  };

  return (
    <section
      aria-labelledby="trust-quick-title"
      className="flex flex-col gap-3 rounded-control border border-accent/30 bg-accent-wash px-4 py-3.5"
      data-testid="trust-command"
    >
      <div className="flex flex-wrap items-center justify-between gap-2">
        <p id="trust-quick-title" className="text-[14px] font-medium text-ink">
          {t("wizard.trust.quick.title")}
        </p>
        <div
          role="group"
          aria-label={t("wizard.trust.quick.osLabel")}
          className="inline-flex rounded-control border border-line bg-surface p-0.5 text-[12.5px]"
        >
          {(["unix", "windows"] as const).map((p) => (
            <button
              key={p}
              type="button"
              aria-pressed={platform === p}
              onClick={() => {
                setPlatform(p);
                setCopied(false);
              }}
              className={cn(
                "rounded-[inherit] px-2.5 py-1 transition-colors",
                platform === p
                  ? "bg-accent text-surface"
                  : "text-muted hover:text-ink",
              )}
            >
              {p === "unix"
                ? t("wizard.trust.quick.unix")
                : t("wizard.trust.quick.windows")}
            </button>
          ))}
        </div>
      </div>

      <p className="text-[13px] leading-snug text-muted">
        {platform === "windows"
          ? t("wizard.trust.quick.bodyWindows")
          : detected === "phone"
            ? t("wizard.trust.quick.bodyPhone")
            : t("wizard.trust.quick.bodyUnix")}
      </p>

      <div className="flex items-center gap-2">
        <code
          ref={lineRef}
          data-testid="trust-command-line"
          className="numeric min-w-0 flex-1 rounded-control border border-line bg-surface px-3 py-2 text-[12.5px] leading-relaxed break-all text-ink select-all"
        >
          {command}
        </code>
        <Button variant="secondary" size="sm" onClick={copy} aria-live="polite">
          <HugeiconsIcon
            icon={copied ? CheckmarkCircle02Icon : Copy01Icon}
            size={16}
            strokeWidth={1.5}
            color="currentColor"
            aria-hidden="true"
          />
          {copied
            ? t("wizard.trust.quick.copied")
            : t("wizard.trust.quick.copy")}
        </Button>
      </div>

      <ul className="flex list-disc flex-col gap-1 pl-4 text-[12.5px] leading-snug text-muted">
        <li>{t("wizard.trust.quick.what")}</li>
        <li>{t("wizard.trust.quick.where")}</li>
        <li>
          <a
            href={scriptUrl}
            target="_blank"
            rel="noopener"
            className="text-accent underline-offset-4 hover:underline"
          >
            {t("wizard.trust.quick.read")}
          </a>{" "}
          {t("wizard.trust.quick.readTail")}
        </li>
        <li>{t("wizard.trust.quick.then", { fqdn })}</li>
      </ul>
    </section>
  );
}
