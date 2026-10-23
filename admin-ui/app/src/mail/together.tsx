/* The open message's ties to the other apps: a chat or a call with the
 * sender when they are on this box, the box's AI on the message, and an
 * issue in LosOS Git's tracker made of it.
 *
 * Kept in its own block of the reader's toolbar, so Mail's own buttons and
 * these grow apart. Each one is drawn only while the app it leads to is
 * live for the person; a box that does not share has no Chat to send them
 * to, and a button there could only fail. */

import { HugeiconsIcon, type IconSvgElement } from "@hugeicons/react";
import { BubbleChatIcon, Call02Icon, TaskAdd01Icon } from "@hugeicons/core-free-icons";
import { Button } from "@/components/ui/button";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { AskAiButton } from "@/components/ask-ai";
import type { MailHome, MailMessage } from "@/lib/api";
import { useT } from "@/lib/i18n-react";
import { canHandTo, canMakeIssue, chatWithHref, newIssueHref, quotedFrom } from "@/lib/intents";

/** The sign-in name behind an address on this box, or null for anyone else. */
export function localName(address: string, home: Pick<MailHome, "address">): string | null {
  const domain = home.address.slice(home.address.lastIndexOf("@") + 1).toLowerCase();
  const at = address.lastIndexOf("@");
  if (at <= 0 || domain === "" || address.slice(at + 1).toLowerCase() !== domain) return null;
  const name = address.slice(0, at).toLowerCase();
  return /^[a-z0-9][a-z0-9._-]*$/.test(name) ? name : null;
}

/** What the AI reads of a message: who wrote it, to whom, and the text. */
export function messageSource(m: MailMessage): { title: string; text: string } {
  const who = (list: MailMessage["from"]) => list.map((a) => (a.name ? `${a.name} <${a.email}>` : a.email)).join(", ");
  return {
    title: m.subject || "(no subject)",
    text: [`From: ${who(m.from)}`, `To: ${who(m.to)}`, `Date: ${m.receivedAt}`, `Subject: ${m.subject}`, "", m.text || m.preview].join("\n"),
  };
}

export function PersonLinks({ message: m, home }: { message: MailMessage; home: MailHome }) {
  const t = useT();
  const sender = m.from[0] === undefined ? null : localName(m.from[0].email, home);
  const me = localName(home.address, home);
  const chat = canHandTo("chat") && sender !== null && sender !== me;

  const link = (icon: IconSvgElement, label: string, href: string, action: string) => (
    <Tooltip>
      <TooltipTrigger
        render={
          <Button
            variant="ghost"
            size="icon-sm"
            aria-label={label}
            data-action={action}
            onClick={() => (action === "make-issue" ? window.open(href, "_blank", "noopener") : window.location.assign(href))}
          />
        }
      >
        <HugeiconsIcon icon={icon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
      </TooltipTrigger>
      <TooltipContent side="bottom">{label}</TooltipContent>
    </Tooltip>
  );

  return (
    <>
      <span aria-hidden="true" className="mx-1 h-5 w-px bg-line" />
      <AskAiButton
        compact
        label={t("together.ai.summarise")}
        sources={() => [messageSource(m)]}
        question={t("together.ai.summary")}
        context="mail"
      />
      {chat && sender !== null && (
        <>
          {link(BubbleChatIcon, t("together.mail.chat"), chatWithHref(sender), "chat-sender")}
          {link(Call02Icon, t("together.mail.call"), chatWithHref(sender, { call: "voice" }), "call-sender")}
        </>
      )}
      {canMakeIssue() &&
        link(
          TaskAdd01Icon,
          t("together.issue.make"),
          newIssueHref({
            title: m.subject,
            body: quotedFrom(m.text || m.preview, t("together.issue.fromMail", { name: m.from[0]?.name || m.from[0]?.email || "", subject: m.subject })),
          }),
          "make-issue",
        )}
    </>
  );
}
