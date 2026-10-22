/* An invitation in LosOS Mail: a message carrying an .ics (text/calendar)
 * shows what it asks above its text. An invitation (METHOD:REQUEST) gets
 * Yes, Maybe and No; the answer puts the event in the person's calendar
 * with it, and mails the organizer a METHOD:REPLY, so their calendar shows
 * who comes. A reply or a cancellation that reaches the organizer or a
 * guest is put on their own copy as soon as the message is opened.
 *
 * Mail draws this only while LosOS Calendar is listed for the person
 * (sharedApp("calendar")): the answer is kept in LosOS Cloud. */

import * as React from "react";
import { HugeiconsIcon } from "@hugeicons/react";
import { Alert01Icon, Calendar03Icon, CheckmarkCircle02Icon, Video01Icon } from "@hugeicons/core-free-icons";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Spinner } from "@/components/ui/spinner";
import { toast } from "@/components/ui/toast";
import {
  answerInvitation,
  applyInvitation,
  mailAttachmentUrl,
  mailTextFile,
  readInvitation,
  sendMail,
  type CalendarInvitation,
  type MailAttachment,
} from "@/lib/api";
import { useLocale, useT } from "@/lib/i18n-react";
import { cn } from "@/lib/utils";
import { whenText } from "./event";

/** Whether a message's attachment is a calendar file. */
export function isCalendarFile(a: MailAttachment): boolean {
  const type = a.type.toLowerCase();
  return type.startsWith("text/calendar") || type === "application/ics" || /\.ics$/i.test(a.name);
}

type Answer = "ACCEPTED" | "TENTATIVE" | "DECLINED";

function message(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

export function InviteCard({ attachment, address, subject }: { attachment: MailAttachment; address: string; subject: string }) {
  const t = useT();
  const locale = useLocale();
  const [ics, setIcs] = React.useState<string | null>(null);
  const [invite, setInvite] = React.useState<CalendarInvitation | null>(null);
  const [problem, setProblem] = React.useState<string | null>(null);
  const [busy, setBusy] = React.useState<Answer | null>(null);
  const [applied, setApplied] = React.useState<"reply" | "cancel" | null>(null);

  React.useEffect(() => {
    let live = true;
    setInvite(null);
    setProblem(null);
    setApplied(null);
    (async () => {
      const response = await fetch(mailAttachmentUrl(attachment), { cache: "no-store" });
      if (!response.ok) throw new Error(t("calendar.invite.unreadable"));
      const text = await response.text();
      const read = await readInvitation(text, address);
      if (!live) return;
      setIcs(text);
      setInvite(read);
      // A reply or a cancellation goes onto the copy the person keeps.
      if ((read.method === "REPLY" || read.method === "CANCEL") && read.stored !== undefined) {
        try {
          await applyInvitation(text);
          if (live) setApplied(read.method === "REPLY" ? "reply" : "cancel");
        } catch {
          /* the card still says what the message says */
        }
      }
    })().catch((error: unknown) => {
      if (live) setProblem(message(error));
    });
    return () => {
      live = false;
    };
  }, [attachment, address, t]);

  const answer = (status: Answer) => {
    if (ics === null || invite === null) return;
    setBusy(status);
    setProblem(null);
    answerInvitation({ ics, email: address, status })
      .then(async (done) => {
        if (done.to !== "") {
          await sendMail({
            to: [done.to],
            cc: [],
            subject: done.subject,
            text: t(`calendar.invite.replyBody.${status}`, { summary: invite.summary }),
            attachments: [mailTextFile("reply.ics", "text/calendar", done.reply)],
          });
        }
        setInvite({ ...invite, status, ...(done.event !== undefined ? { stored: done.event } : {}) });
        toast.done(t(`calendar.invite.done.${status}`, { summary: invite.summary }));
      })
      .catch((error: unknown) => setProblem(message(error)))
      .finally(() => setBusy(null));
  };

  if (problem !== null && invite === null) {
    return (
      <Alert variant="crit" role="alert" className="mx-5 mb-3">
        <HugeiconsIcon icon={Alert01Icon} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
        <AlertDescription>{t("calendar.invite.failed", { detail: problem })}</AlertDescription>
      </Alert>
    );
  }
  if (invite === null) {
    return (
      <div className="mx-5 mb-3 flex items-center gap-2 rounded-card border border-line px-4 py-3 text-sm text-muted">
        <Spinner size={16} />
        {t("calendar.invite.reading")}
      </div>
    );
  }

  const isRequest = invite.method === "REQUEST";
  const who = invite.organizer?.name || invite.organizer?.email || "";
  const replier = invite.method === "REPLY" ? invite.attendees[0] : undefined;
  const answered = invite.status === "ACCEPTED" || invite.status === "TENTATIVE" || invite.status === "DECLINED" ? (invite.status as Answer) : null;

  return (
    <section
      aria-label={t("calendar.invite.label")}
      className="mx-5 mb-3 flex flex-col gap-3 rounded-card border border-line bg-sunk/60 px-4 py-3"
    >
      <div className="flex items-start gap-3">
        <span className="grid size-10 shrink-0 place-items-center rounded-card bg-accent-wash text-accent">
          <HugeiconsIcon icon={Calendar03Icon} size={20} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
        </span>
        <div className="min-w-0 flex-1">
          <p className="text-[12px] text-muted">
            {invite.method === "CANCEL"
              ? t("calendar.invite.cancelled")
              : invite.method === "REPLY"
                ? t("calendar.invite.reply")
                : t("calendar.invite.from", { name: who })}
          </p>
          <h3 className={cn("text-[15px] font-semibold tracking-tight break-words", invite.method === "CANCEL" && "line-through")}>
            {invite.summary || subject}
          </h3>
          <p className="text-[13px] text-muted">{whenText(invite, locale, t)}</p>
          {invite.location !== undefined && <p className="text-[13px] text-muted">{invite.location}</p>}
          {invite.repeats && <p className="text-[13px] text-muted">{t("calendar.repeat.custom")}</p>}
        </div>
      </div>
      {replier !== undefined && (
        <p className="flex items-center gap-2 text-sm">
          <HugeiconsIcon icon={CheckmarkCircle02Icon} size={16} strokeWidth={1.5} className="text-ok" aria-hidden="true" />
          {t(`calendar.invite.answered.${replier.status === "ACCEPTED" || replier.status === "TENTATIVE" || replier.status === "DECLINED" ? replier.status : "ACCEPTED"}`, {
            name: replier.name || replier.email,
          })}
          {applied === "reply" && <Badge variant="secondary">{t("calendar.invite.recorded")}</Badge>}
        </p>
      )}
      {applied === "cancel" && <p className="text-sm">{t("calendar.invite.cancelRecorded")}</p>}
      {isRequest && (
        <div className="flex flex-wrap items-center gap-2">
          {(["ACCEPTED", "TENTATIVE", "DECLINED"] as const).map((s) => (
            <Button
              key={s}
              size="sm"
              variant={answered === s ? "default" : "secondary"}
              aria-pressed={answered === s}
              disabled={busy !== null}
              onClick={() => answer(s)}
            >
              {busy === s && <Spinner className="size-4" />}
              {t(`calendar.invite.answer.${s}`)}
            </Button>
          ))}
          {invite.stored !== undefined && (
            <Button size="sm" variant="ghost" onClick={() => window.location.assign(`/calendar?event=${encodeURIComponent(invite.stored?.id ?? "")}`)}>
              {invite.stored.call === "video" && <HugeiconsIcon icon={Video01Icon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />}
              {t("calendar.invite.open")}
            </Button>
          )}
        </div>
      )}
      {problem !== null && (
        <Alert variant="crit" role="alert">
          <HugeiconsIcon icon={Alert01Icon} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
          <AlertDescription>{problem}</AlertDescription>
        </Alert>
      )}
    </section>
  );
}
