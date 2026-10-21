/* The guest link over a call: the address the edge made, with the call's
 * guest secret after `#` (guests.ts), and a button that copies it. */

import * as React from "react";
import { HugeiconsIcon } from "@hugeicons/react";
import { Cancel01Icon, Copy01Icon, LockKeyIcon } from "@hugeicons/core-free-icons";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Field, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { useT } from "@/lib/i18n-react";

export interface InvitePanelProps {
  link: string | null;
  onAnother(): void;
  onClose(): void;
}

export function InvitePanel(p: InvitePanelProps) {
  const t = useT();
  const [copied, setCopied] = React.useState(false);
  React.useEffect(() => setCopied(false), [p.link]);

  const copy = (): void => {
    if (p.link === null) return;
    void navigator.clipboard
      .writeText(p.link)
      .then(() => setCopied(true))
      .catch(() => undefined);
  };

  return (
    <section
      role="dialog"
      aria-label={t("calls.invite")}
      data-invite-panel=""
      className="absolute inset-x-3 top-16 z-10 mx-auto flex max-w-xl flex-col gap-4 rounded-card border border-line bg-surface p-4 shadow-pop sm:inset-x-6"
    >
      <header className="flex items-start justify-between gap-3">
        <div>
          <h3 className="font-medium text-ink">{t("calls.invite")}</h3>
          <p className="text-[13px] text-muted">{t("calls.inviteIntro")}</p>
        </div>
        <Button variant="ghost" size="sm" className="size-8 p-0" aria-label={t("calls.close")} onClick={p.onClose}>
          <HugeiconsIcon icon={Cancel01Icon} size={16} />
        </Button>
      </header>

      <Field>
        <FieldLabel htmlFor="invite-link">{t("calls.inviteLink")}</FieldLabel>
        <div className="flex gap-2">
          <Input
            id="invite-link"
            readOnly
            spellCheck={false}
            className="font-mono text-[12px]"
            value={p.link ?? t("calls.inviteMaking")}
            data-invite-link={p.link ?? undefined}
            onFocus={(e) => e.currentTarget.select()}
          />
          <Button disabled={p.link === null} onClick={copy}>
            <HugeiconsIcon icon={Copy01Icon} size={16} />
            {copied ? t("calls.inviteCopied") : t("calls.inviteCopy")}
          </Button>
        </div>
      </Field>

      <Alert>
        <HugeiconsIcon icon={LockKeyIcon} />
        <AlertDescription>{t("calls.inviteSecret")}</AlertDescription>
      </Alert>

      <div className="flex justify-end">
        <Button variant="ghost" disabled={p.link === null} onClick={p.onAnother}>
          {t("calls.inviteAnother")}
        </Button>
      </div>
    </section>
  );
}
