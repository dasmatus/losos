import * as React from "react";
import { HugeiconsIcon } from "@hugeicons/react";
import { SecurityLockIcon } from "@hugeicons/core-free-icons";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { HelpLink } from "@/components/ui/help-link";
import { getOptions, hasToken, type OptionsResponse } from "@/lib/api";
import { useT } from "@/lib/i18n-react";

/* The warning a box installed without a TPM shows its owner.
 *
 * Such a box unlocks its disk from a key file baked into the initrd, which
 * sits on the unencrypted boot partition (modules/boot.nix), so whoever has
 * the disk has the data. The installer said so once on a console nobody may
 * have watched; this says it where the owner looks. wiki/TPM.md and the
 * handbook page the link opens carry the long form.
 *
 * The mode comes from the option document: `tpm.enable` is one of the
 * installer's three, and its `current` is what the box runs with. Only an
 * explicit `false` warns. A box serving no document (an older lososd), or
 * one still loading, shows nothing rather than a guess. */

/** The box runs in keyfile mode, by the option document. */
export function runsWithoutTpm(options: OptionsResponse | null): boolean {
  if (options === null || !Array.isArray(options.options)) return false;
  return options.options.find((o) => o.name === "tpm.enable")?.current === false;
}

/* For a screen that has no settings form to read the document from: the
 * wizard, once the claim has put a token in this tab. `ready` is the moment
 * the token exists; before that the route answers 401 and there is nothing
 * to ask. */
export function useRunsWithoutTpm(ready: boolean): boolean {
  const [noTpm, setNoTpm] = React.useState(false);

  React.useEffect(() => {
    if (!ready || !hasToken()) return;
    const controller = new AbortController();
    getOptions({ signal: controller.signal })
      .then((doc) => setNoTpm(runsWithoutTpm(doc)))
      .catch(() => {
        // A warning is not worth an error of its own: the Security pane
        // shows it again once the document answers.
      });
    return () => controller.abort();
  }, [ready]);

  return noTpm;
}

export function NoTpmNotice({ className }: { className?: string }) {
  const t = useT();
  return (
    <Alert variant="warn" className={className} data-notice="no-tpm">
      <HugeiconsIcon icon={SecurityLockIcon} size={19} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
      <AlertTitle>{t("ui.noTpm.title")}</AlertTitle>
      <AlertDescription>
        <p>{t("ui.noTpm.body")}</p>
        <HelpLink entry="tpm" label={t("ui.noTpm.link")} />
      </AlertDescription>
    </Alert>
  );
}
