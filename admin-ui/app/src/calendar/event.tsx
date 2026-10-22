/* One event of LosOS Calendar: what it is and how to join it (EventDetails),
 * and the dialog that writes one (EventEditor).
 *
 * Guests come from the person's contacts and the people on this box through
 * the people picker every app shares (contacts/suggest.tsx). Their Matrix
 * IDs, where the picker knows them, are kept on the event
 * (X-LOSOS-MATRIX), so a guest's copy can start a chat or a call with the
 * organizer and the other way round. Invitations go out through LosOS Mail
 * as iCalendar (METHOD:REQUEST), which Mail here, Thunderbird, Apple
 * Calendar and Outlook all know how to answer. */

import * as React from "react";
import { useNavigate } from "react-router-dom";
import { HugeiconsIcon, type IconSvgElement } from "@hugeicons/react";
import {
  Alert01Icon,
  BubbleChatIcon,
  Calendar03Icon,
  Clock01Icon,
  Delete02Icon,
  Location01Icon,
  Mail01Icon,
  MailSend01Icon,
  Note01Icon,
  Notification01Icon,
  PencilEdit02Icon,
  RepeatIcon,
  UserGroupIcon,
  Video01Icon,
} from "@hugeicons/core-free-icons";
import { PersonAvatar } from "@/components/person";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Dialog, DialogBody, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Field, FieldDescription, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { NativeSelect, NativeSelectOption } from "@/components/ui/native-select";
import { Spinner } from "@/components/ui/spinner";
import { Switch } from "@/components/ui/switch";
import { Textarea } from "@/components/ui/textarea";
import { toast } from "@/components/ui/toast";
import {
  ApiError,
  calendarInvite,
  deleteCalendarEvent,
  getCalendarEvent,
  getMail,
  mailTextFile,
  saveCalendarEvent,
  sendMail,
  sharedApp,
  suggestContacts,
  type CalendarEvent,
  type CalendarEventFields,
  type CalendarFreq,
  type CalendarInfo,
  type CalendarPerson,
  type CalendarRepeat,
} from "@/lib/api";
import { useLocale, useT } from "@/lib/i18n-react";
import type { MessageKey } from "@/lib/i18n";
import { chatWithHref, composeMailHref } from "@/lib/intents";
import { cn } from "@/lib/utils";
import { PeopleField } from "@/contacts/suggest";
import { colorProps } from "./colors";
import { WEEKDAYS, addDays, dayDate, instant, localInput, offsetOn, span, viewerZone, weekdayCode } from "./time";

export type T = ReturnType<typeof useT>;

export type Draft =
  | { kind: "new"; start: string; allDay: boolean; calendar: string }
  | { kind: "edit"; event: CalendarEvent };

function message(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

/* The person's own mail address, once per page: who an invitation is from
 * and which guest they are in one. Null where Mail does not run. */
let address: Promise<string | null> | null = null;
export function myAddress(): Promise<string | null> {
  if (!sharedApp("mail")) return Promise.resolve(null);
  address ??= getMail()
    .then((home) => home.address.toLowerCase())
    .catch(() => {
      address = null;
      return null;
    });
  return address;
}

export function useMyAddress(): string | null {
  const [mine, setMine] = React.useState<string | null>(null);
  React.useEffect(() => {
    let live = true;
    void myAddress().then((a) => {
      if (live) setMine(a);
    });
    return () => {
      live = false;
    };
  }, []);
  return mine;
}

/** "Tuesday 22 October, 08:00 – 09:30", in the viewer's zone. */
export function whenText(e: { start: string; end: string; zone?: string; allDay: boolean }, locale: string, t: T): string {
  const [a, b] = span(e);
  if (e.allDay) {
    const day = new Intl.DateTimeFormat(locale, { weekday: "long", day: "numeric", month: "long", year: "numeric" });
    const last = addDays(e.end.slice(0, 10), -1);
    return last <= e.start.slice(0, 10)
      ? `${day.format(dayDate(e.start.slice(0, 10)))} · ${t("calendar.allDay")}`
      : day.formatRange(dayDate(e.start.slice(0, 10)), dayDate(last));
  }
  return new Intl.DateTimeFormat(locale, { weekday: "long", day: "numeric", month: "long", hour: "numeric", minute: "2-digit" }).formatRange(
    new Date(a),
    new Date(b),
  );
}

/** How an event repeats, in a few words. */
export function repeatText(r: CalendarRepeat | undefined, repeats: boolean, locale: string, t: T): string | null {
  if (r === undefined) return repeats ? t("calendar.repeat.custom") : null;
  const every = t(`calendar.repeat.every.${r.freq}` as MessageKey, { count: r.interval ?? 1 });
  const names = new Intl.DateTimeFormat(locale, { weekday: "short" });
  const days =
    r.freq === "weekly" && (r.days?.length ?? 0) > 0
      ? ` (${WEEKDAYS.filter((d) => r.days?.includes(d))
          .map((d) => names.format(new Date(2026, 9, 19 + WEEKDAYS.indexOf(d))))
          .join(", ")})`
      : "";
  const until =
    r.until !== undefined
      ? `, ${t("calendar.repeat.untilText", { day: new Intl.DateTimeFormat(locale, { dateStyle: "medium" }).format(dayDate(r.until)) })}`
      : r.count !== undefined
        ? `, ${t("calendar.repeat.countText", { count: r.count })}`
        : "";
  return `${every}${days}${until}`;
}

function statusBadge(status: string | undefined, t: T) {
  switch (status) {
    case "ACCEPTED":
      return <Badge variant="ok">{t("calendar.status.accepted")}</Badge>;
    case "DECLINED":
      return <Badge variant="crit">{t("calendar.status.declined")}</Badge>;
    case "TENTATIVE":
      return <Badge variant="warn">{t("calendar.status.tentative")}</Badge>;
    default:
      return <Badge variant="secondary">{t("calendar.status.waiting")}</Badge>;
  }
}

/** The people in an event other than `me`, organizer first. */
export function othersIn(e: { organizer?: CalendarPerson; attendees: CalendarPerson[] }, me: string | null): CalendarPerson[] {
  const all = [...(e.organizer !== undefined ? [e.organizer] : []), ...e.attendees];
  const seen = new Set<string>();
  return all.filter((p) => {
    const k = p.email.toLowerCase();
    if (k === me || seen.has(k)) return false;
    seen.add(k);
    return true;
  });
}

/** Mail every guest the event's invitation. */
export async function sendInvitations(e: CalendarEvent, me: string | null, locale: string, t: T): Promise<number> {
  const to = e.attendees.map((a) => a.email).filter((a) => a.toLowerCase() !== me);
  if (to.length === 0) return 0;
  const ics = await calendarInvite(e.id);
  const lines = [
    t("calendar.invite.body", { summary: e.summary }),
    "",
    whenText(e, locale, t),
    ...(e.location !== undefined ? [e.location] : []),
    ...(e.call === "video" || e.call === "voice" ? ["", t("calendar.invite.call")] : []),
    ...(e.description !== undefined ? ["", e.description] : []),
  ];
  await sendMail({
    to,
    cc: [],
    subject: t("calendar.invite.subject", { summary: e.summary }),
    text: lines.join("\n"),
    attachments: [mailTextFile("invite.ics", "text/calendar", ics)],
  });
  return to.length;
}

function Row({ icon, children }: { icon: IconSvgElement; children: React.ReactNode }) {
  return (
    <li className="flex items-start gap-3 py-2">
      <HugeiconsIcon icon={icon} size={18} strokeWidth={1.5} className="mt-0.5 shrink-0 text-muted" aria-hidden="true" />
      <div className="min-w-0 flex-1 text-sm break-words">{children}</div>
    </li>
  );
}

export function EventDetails({
  event,
  calendar,
  onClose,
  onEdit,
  onChanged,
}: {
  event: CalendarEvent;
  calendar: CalendarInfo | undefined;
  onClose: () => void;
  onEdit: (full: CalendarEvent) => void;
  onChanged: () => void;
}) {
  const t = useT();
  const locale = useLocale();
  const navigate = useNavigate();
  const me = useMyAddress();
  const [full, setFull] = React.useState<CalendarEvent | null>(null);
  const [busy, setBusy] = React.useState<string | null>(null);
  const [problem, setProblem] = React.useState<string | null>(null);
  const [asking, setAsking] = React.useState(false);

  React.useEffect(() => {
    getCalendarEvent(event.id)
      .then(setFull)
      .catch(() => setFull(event));
  }, [event]);

  const chat = sharedApp("chat");
  const mail = sharedApp("mail");
  const others = othersIn(event, me);
  const callWith = others.find((p) => p.matrix !== undefined && p.matrix !== "");
  const isCall = event.call === "video" || event.call === "voice";
  const organizing = event.organizer === undefined || (me !== null && event.organizer.email.toLowerCase() === me);
  const repeat = repeatText(full?.repeat, event.repeats, locale, t);
  const guests = event.attendees;

  const remove = (occurrence?: string) => {
    setBusy("delete");
    setProblem(null);
    deleteCalendarEvent(event.id, occurrence)
      .then(() => {
        toast.done(occurrence !== undefined ? t("calendar.done.deletedOne") : t("calendar.done.deleted"));
        onChanged();
      })
      .catch((error: unknown) => {
        setProblem(message(error));
        setBusy(null);
      });
  };

  return (
    <Dialog open onOpenChange={(next) => !next && busy === null && onClose()} labelledBy="calendar-event-title" dialogClassName="w-[min(32rem,calc(100vw-2rem))]">
      <DialogHeader>
        <div className="flex items-start gap-3">
          <span {...colorProps(calendar?.color)} aria-hidden="true" className="mt-1.5 size-3 shrink-0 rounded-full bg-person" />
          <div className="min-w-0 flex-1">
            <DialogTitle id="calendar-event-title" className="break-words">
              {event.summary || t("calendar.untitled")}
            </DialogTitle>
            <DialogDescription>{whenText(event, locale, t)}</DialogDescription>
          </div>
        </div>
      </DialogHeader>
      <DialogBody className="flex flex-col gap-3">
        {isCall && (
          <div className="flex flex-wrap items-center gap-2 rounded-card border border-line bg-sunk px-3 py-2.5">
            <HugeiconsIcon icon={Video01Icon} size={18} strokeWidth={1.5} className="text-accent" aria-hidden="true" />
            <p className="min-w-0 flex-1 text-sm">{event.call === "voice" ? t("calendar.call.voiceLead") : t("calendar.call.videoLead")}</p>
            {chat && callWith?.matrix !== undefined && (
              <Button size="sm" onClick={() => window.location.assign(chatWithHref(callWith.matrix ?? "", { call: event.call === "voice" ? "voice" : "video" }))}>
                {t("calendar.call.join")}
              </Button>
            )}
          </div>
        )}
        <ul className="divide-y divide-hair">
          {repeat !== null && (
            <Row icon={RepeatIcon}>
              {repeat}
              {event.occurrence !== undefined && <p className="text-[12.5px] text-muted">{t("calendar.repeat.editAll")}</p>}
            </Row>
          )}
          <Row icon={Calendar03Icon}>
            {calendar?.name ?? event.calendar}
            {event.readOnly && <p className="text-[12.5px] text-muted">{t("calendar.readOnly")}</p>}
          </Row>
          {event.location !== undefined && <Row icon={Location01Icon}>{event.location}</Row>}
          {event.alarms.length > 0 && (
            <Row icon={Notification01Icon}>{event.alarms.map((m) => alarmText(m, t)).join(", ")}</Row>
          )}
          {event.description !== undefined && (
            <Row icon={Note01Icon}>
              <p className="whitespace-pre-line">{event.description}</p>
            </Row>
          )}
          {(guests.length > 0 || event.organizer !== undefined) && (
            <Row icon={UserGroupIcon}>
              <ul className="flex flex-col gap-1.5">
                {[...(event.organizer !== undefined ? [{ ...event.organizer, organizer: true }] : []), ...guests.map((g) => ({ ...g, organizer: false }))]
                  .filter((p, i, all) => all.findIndex((q) => q.email === p.email) === i)
                  .map((p) => (
                    <li key={p.email} className="flex items-center gap-2">
                      <PersonAvatar person={{ name: p.email, displayName: p.name || p.email }} size="xs" />
                      <span className="min-w-0 flex-1">
                        <span className="block truncate">{p.name || p.email}</span>
                        {p.name !== undefined && p.name !== "" && <span className="block truncate text-[12px] text-muted">{p.email}</span>}
                      </span>
                      {p.organizer ? <Badge variant="secondary">{t("calendar.organizer")}</Badge> : statusBadge(p.status, t)}
                      {chat && p.matrix !== undefined && p.matrix !== "" && p.email.toLowerCase() !== me && (
                        <Button
                          variant="ghost"
                          size="icon-sm"
                          aria-label={t("calendar.act.chatWith", { name: p.name || p.email })}
                          title={t("calendar.act.chatWith", { name: p.name || p.email })}
                          onClick={() => window.location.assign(chatWithHref(p.matrix ?? ""))}
                        >
                          <HugeiconsIcon icon={BubbleChatIcon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
                        </Button>
                      )}
                    </li>
                  ))}
              </ul>
            </Row>
          )}
        </ul>
        {problem !== null && (
          <Alert variant="crit" role="alert">
            <HugeiconsIcon icon={Alert01Icon} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
            <AlertDescription>{problem}</AlertDescription>
          </Alert>
        )}
        {asking && (
          <Alert role="alert">
            <AlertDescription>
              <p>{t("calendar.deleteRepeat")}</p>
              <div className="mt-2 flex flex-wrap gap-2">
                <Button size="sm" variant="destructive" disabled={busy !== null} onClick={() => remove(event.occurrence)}>
                  {t("calendar.act.deleteOne")}
                </Button>
                <Button size="sm" variant="secondary" disabled={busy !== null} onClick={() => remove()}>
                  {t("calendar.act.deleteAll")}
                </Button>
              </div>
            </AlertDescription>
          </Alert>
        )}
      </DialogBody>
      <DialogFooter className="flex-wrap">
        {!event.readOnly && (
          <Button
            variant="ghost"
            className="mr-auto text-crit"
            disabled={busy !== null}
            onClick={() => (event.repeats && event.occurrence !== undefined ? setAsking(true) : remove())}
          >
            <HugeiconsIcon icon={Delete02Icon} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
            {t("calendar.act.delete")}
          </Button>
        )}
        {mail && others.length > 0 && (
          <Button
            variant="secondary"
            onClick={() =>
              navigate(composeMailHref({ to: others.map((p) => p.email), subject: event.summary }))
            }
          >
            <HugeiconsIcon icon={Mail01Icon} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
            {t("calendar.act.mailGuests")}
          </Button>
        )}
        {mail && organizing && guests.length > 0 && !event.readOnly && (
          <Button
            variant="secondary"
            disabled={busy !== null}
            onClick={() => {
              setBusy("invite");
              setProblem(null);
              sendInvitations(full ?? event, me, locale, t)
                .then((n) => toast.done(t("calendar.done.invited", { count: n })))
                .catch((error: unknown) => setProblem(message(error)))
                .finally(() => setBusy(null));
            }}
          >
            {busy === "invite" ? <Spinner className="size-4" /> : <HugeiconsIcon icon={MailSend01Icon} strokeWidth={1.5} color="currentColor" aria-hidden="true" />}
            {t("calendar.act.invite")}
          </Button>
        )}
        {!event.readOnly && (
          <Button disabled={busy !== null || full === null} onClick={() => full !== null && onEdit(full)}>
            <HugeiconsIcon icon={PencilEdit02Icon} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
            {t("calendar.act.edit")}
          </Button>
        )}
        {event.readOnly && <Button onClick={onClose}>{t("calendar.close")}</Button>}
      </DialogFooter>
    </Dialog>
  );
}

export function alarmText(minutes: number, t: T): string {
  if (minutes <= 0) return t("calendar.alarm.atStart");
  if (minutes % 1440 === 0) return t("calendar.alarm.days", { count: minutes / 1440 });
  if (minutes % 60 === 0) return t("calendar.alarm.hours", { count: minutes / 60 });
  return t("calendar.alarm.minutes", { count: minutes });
}

const ALARMS = [-1, 0, 5, 10, 15, 30, 60, 1440] as const;
const FREQS: (CalendarFreq | "none")[] = ["none", "daily", "weekly", "monthly", "yearly"];

/* The editor's own state: the viewer's wall clock, split for the inputs. */
interface Form {
  calendar: string;
  summary: string;
  allDay: boolean;
  startDay: string;
  startTime: string;
  endDay: string;
  endTime: string;
  location: string;
  description: string;
  guests: string;
  freq: CalendarFreq | "none";
  days: string[];
  until: string;
  alarm: number;
  call: boolean;
  invite: boolean;
}

function formOf(draft: Draft): Form {
  if (draft.kind === "new") {
    const day = draft.start.slice(0, 10);
    const time = draft.start.length > 10 ? draft.start.slice(11, 16) : "09:00";
    const end = localInput(instant(`${day}T${time}`) + 3_600_000);
    return {
      calendar: draft.calendar,
      summary: "",
      allDay: draft.allDay,
      startDay: day,
      startTime: time,
      endDay: draft.allDay ? day : end.slice(0, 10),
      endTime: end.slice(11, 16),
      location: "",
      description: "",
      guests: "",
      freq: "none",
      days: [weekdayCode(day)],
      until: "",
      alarm: 10,
      call: false,
      invite: true,
    };
  }
  const e = draft.event;
  const [a, b] = span(e);
  const start = e.allDay ? `${e.start.slice(0, 10)}T09:00` : localInput(a);
  const end = e.allDay ? `${addDays(e.end.slice(0, 10), -1)}T10:00` : localInput(b);
  return {
    calendar: e.calendar,
    summary: e.summary,
    allDay: e.allDay,
    startDay: start.slice(0, 10),
    startTime: start.slice(11, 16),
    endDay: end.slice(0, 10),
    endTime: end.slice(11, 16),
    location: e.location ?? "",
    description: e.description ?? "",
    guests: e.attendees.map((p) => p.email).join(", "),
    freq: e.repeat?.freq ?? "none",
    days: e.repeat?.days ?? [weekdayCode(start.slice(0, 10))],
    until: e.repeat?.until ?? "",
    alarm: e.alarms[0] ?? -1,
    call: e.call === "video" || e.call === "voice",
    invite: false,
  };
}

/* What the picker knows about each guest's address: a name and a Matrix ID. */
async function people(emails: string[], before: CalendarPerson[]): Promise<CalendarPerson[]> {
  return Promise.all(
    emails.map(async (email) => {
      const known = before.find((p) => p.email.toLowerCase() === email.toLowerCase());
      if (known !== undefined && known.matrix !== undefined) return known;
      try {
        const found = (await suggestContacts(email, "email", { limit: 4 })).find((s) => s.emails.some((e) => e.toLowerCase() === email.toLowerCase()));
        return {
          email,
          ...(found !== undefined ? { name: found.name } : known?.name !== undefined ? { name: known.name } : {}),
          ...(found?.matrixId !== undefined ? { matrix: found.matrixId } : {}),
        };
      } catch {
        return known ?? { email };
      }
    }),
  );
}

export function EventEditor({
  draft,
  calendars,
  onClose,
  onSaved,
}: {
  draft: Draft;
  calendars: CalendarInfo[];
  onClose: () => void;
  onSaved: (e: CalendarEvent) => void;
}) {
  const t = useT();
  const locale = useLocale();
  const me = useMyAddress();
  const [form, setForm] = React.useState<Form>(() => formOf(draft));
  const [busy, setBusy] = React.useState(false);
  const [problem, setProblem] = React.useState<string | null>(null);
  const set = <K extends keyof Form>(key: K, value: Form[K]) => setForm((f) => ({ ...f, [key]: value }));
  const editing = draft.kind === "edit" ? draft.event : null;
  // A repeat the editor cannot show (a rule of its own) is kept as it is.
  const custom = editing !== null && editing.repeats && editing.repeat === undefined;
  const chat = sharedApp("chat");
  const mail = sharedApp("mail");
  const guestList = form.guests
    .split(/[,;]/)
    .map((s) => s.trim())
    .filter((s) => s !== "");
  const before = editing?.attendees ?? [];
  const newGuests = guestList.filter((g) => !before.some((p) => p.email.toLowerCase() === g.toLowerCase()));

  const save = async (event: React.FormEvent) => {
    event.preventDefault();
    setBusy(true);
    setProblem(null);
    try {
      const zone = viewerZone();
      let repeat: CalendarRepeat | undefined;
      if (form.freq !== "none") {
        repeat = { freq: form.freq, interval: 1 };
        if (form.freq === "weekly") repeat.days = form.days.length > 0 ? form.days : [weekdayCode(form.startDay)];
        if (form.until !== "") {
          repeat.until = form.until;
          repeat.offset = offsetOn(form.until);
        }
      }
      const attendees = await people(guestList, before);
      const fields: CalendarEventFields & { keepRepeat?: boolean } = {
        calendar: form.calendar,
        summary: form.summary.trim(),
        location: form.location.trim(),
        description: form.description.trim(),
        start: form.allDay ? form.startDay : `${form.startDay}T${form.startTime}`,
        end: form.allDay ? addDays(form.endDay < form.startDay ? form.startDay : form.endDay, 1) : `${form.endDay}T${form.endTime}`,
        ...(form.allDay ? {} : { zone }),
        ...(repeat !== undefined ? { repeat } : {}),
        ...(custom && form.freq === "none" ? { keepRepeat: true } : {}),
        attendees,
        ...(attendees.length > 0 && me !== null
          ? { organizer: editing?.organizer ?? { email: me } }
          : editing?.organizer !== undefined
            ? { organizer: editing.organizer }
            : {}),
        alarms: form.alarm < 0 ? [] : [form.alarm],
        call: form.call ? "video" : "",
      };
      const saved = await saveCalendarEvent(editing?.id ?? null, fields, editing?.etag);
      let invited = 0;
      if (form.invite && mail && newGuests.length > 0) {
        try {
          // Only the new guests get it now; the others had theirs.
          invited = await sendInvitations(
            { ...saved, attendees: saved.attendees.filter((p) => newGuests.some((g) => g.toLowerCase() === p.email.toLowerCase())) },
            me,
            locale,
            t,
          );
        } catch (error) {
          toast.error(t("calendar.inviteFailed"), message(error), { help: "calendar" });
        }
      }
      toast.done(invited > 0 ? t("calendar.done.savedInvited", { count: invited }) : t("calendar.done.saved"));
      onSaved(saved);
    } catch (error) {
      setProblem(error instanceof ApiError && error.status === 412 ? t("calendar.changedElsewhere") : message(error));
      setBusy(false);
    }
  };

  const id = (k: string) => `calendar-edit-${k}`;
  return (
    <Dialog
      open
      onOpenChange={(next) => !next && !busy && onClose()}
      labelledBy="calendar-edit-title"
      dialogClassName="w-[min(34rem,calc(100vw-2rem))]"
    >
      <form onSubmit={(e) => void save(e)}>
        <DialogHeader>
          <DialogTitle id="calendar-edit-title">{editing !== null ? t("calendar.edit.title") : t("calendar.new")}</DialogTitle>
        </DialogHeader>
        <DialogBody className="flex max-h-[min(70dvh,40rem)] flex-col gap-4 overflow-y-auto">
          <Field>
            <FieldLabel htmlFor={id("summary")}>{t("calendar.field.summary")}</FieldLabel>
            <Input id={id("summary")} value={form.summary} onChange={(e) => set("summary", e.target.value)} required autoFocus maxLength={500} />
          </Field>
          <Field orientation="horizontal" className="items-center justify-between">
            <FieldLabel htmlFor={id("allday")}>{t("calendar.allDay")}</FieldLabel>
            <Switch id={id("allday")} isSelected={form.allDay} onChange={(v) => set("allDay", v)} />
          </Field>
          <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
            <Field>
              <FieldLabel htmlFor={id("start")}>{t("calendar.field.start")}</FieldLabel>
              <div className="flex gap-2">
                <Input
                  id={id("start")}
                  type="date"
                  value={form.startDay}
                  required
                  onChange={(e) => {
                    const day = e.target.value;
                    setForm((f) => {
                      // The end moves with the start, keeping the length.
                      const shift = Math.round((dayDate(day).getTime() - dayDate(f.startDay).getTime()) / 86_400_000);
                      return { ...f, startDay: day, endDay: addDays(f.endDay, shift), days: f.freq === "weekly" ? f.days : [weekdayCode(day)] };
                    });
                  }}
                />
                {!form.allDay && (
                  <Input
                    type="time"
                    aria-label={t("calendar.field.startTime")}
                    value={form.startTime}
                    required
                    onChange={(e) => {
                      const time = e.target.value;
                      setForm((f) => {
                        const length = instant(`${f.endDay}T${f.endTime}`) - instant(`${f.startDay}T${f.startTime}`);
                        const end = localInput(instant(`${f.startDay}T${time}`) + Math.max(length, 0));
                        return { ...f, startTime: time, endDay: end.slice(0, 10), endTime: end.slice(11, 16) };
                      });
                    }}
                  />
                )}
              </div>
            </Field>
            <Field>
              <FieldLabel htmlFor={id("end")}>{t("calendar.field.end")}</FieldLabel>
              <div className="flex gap-2">
                <Input id={id("end")} type="date" value={form.endDay} min={form.startDay} required onChange={(e) => set("endDay", e.target.value)} />
                {!form.allDay && (
                  <Input type="time" aria-label={t("calendar.field.endTime")} value={form.endTime} required onChange={(e) => set("endTime", e.target.value)} />
                )}
              </div>
            </Field>
          </div>
          <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
            <Field>
              <FieldLabel htmlFor={id("repeat")}>{t("calendar.field.repeat")}</FieldLabel>
              <NativeSelect id={id("repeat")} value={form.freq} onChange={(e) => set("freq", e.target.value as Form["freq"])}>
                {FREQS.map((f) => (
                  <NativeSelectOption key={f} value={f}>
                    {f === "none" && custom ? t("calendar.repeat.keep") : t(`calendar.repeat.${f}` as MessageKey)}
                  </NativeSelectOption>
                ))}
              </NativeSelect>
            </Field>
            {form.freq !== "none" && (
              <Field>
                <FieldLabel htmlFor={id("until")}>{t("calendar.field.until")}</FieldLabel>
                <Input id={id("until")} type="date" value={form.until} min={form.startDay} onChange={(e) => set("until", e.target.value)} />
                <FieldDescription>{t("calendar.field.untilHint")}</FieldDescription>
              </Field>
            )}
          </div>
          {form.freq === "weekly" && (
            <fieldset className="flex flex-wrap gap-1.5">
              <legend className="sr-only">{t("calendar.field.days")}</legend>
              {WEEKDAYS.map((d, i) => {
                const on = form.days.includes(d);
                return (
                  <Button
                    key={d}
                    type="button"
                    size="sm"
                    variant={on ? "default" : "secondary"}
                    aria-pressed={on}
                    className="w-11"
                    onClick={() => set("days", on ? form.days.filter((x) => x !== d) : [...form.days, d])}
                  >
                    {new Intl.DateTimeFormat(locale, { weekday: "short" }).format(new Date(2026, 9, 19 + i))}
                  </Button>
                );
              })}
            </fieldset>
          )}
          <Field>
            <FieldLabel htmlFor={id("calendar")}>{t("calendar.field.calendar")}</FieldLabel>
            <NativeSelect id={id("calendar")} value={form.calendar} onChange={(e) => set("calendar", e.target.value)}>
              {calendars.map((c) => (
                <NativeSelectOption key={c.uri} value={c.uri}>
                  {c.name}
                </NativeSelectOption>
              ))}
            </NativeSelect>
          </Field>
          <Field>
            <FieldLabel htmlFor={id("guests")}>{t("calendar.field.guests")}</FieldLabel>
            <PeopleField id={id("guests")} value={form.guests} onChange={(v) => set("guests", v)} want="email" multiple placeholder={t("calendar.field.guestsHint")} />
            {newGuests.length > 0 && mail && (
              <label className="mt-1 flex items-center gap-2 text-[13px] text-muted">
                <input type="checkbox" checked={form.invite} onChange={(e) => set("invite", e.target.checked)} className="size-4 accent-accent" />
                {t("calendar.field.sendInvites", { count: newGuests.length })}
              </label>
            )}
          </Field>
          {chat && (
            <Field orientation="horizontal" className="items-center justify-between">
              <div>
                <FieldLabel htmlFor={id("call")} className="flex items-center gap-2">
                  <HugeiconsIcon icon={Video01Icon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
                  {t("calendar.field.call")}
                </FieldLabel>
                <FieldDescription>{t("calendar.field.callHint")}</FieldDescription>
              </div>
              <Switch id={id("call")} isSelected={form.call} onChange={(v) => set("call", v)} />
            </Field>
          )}
          <Field>
            <FieldLabel htmlFor={id("alarm")} className="flex items-center gap-2">
              <HugeiconsIcon icon={Clock01Icon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
              {t("calendar.field.alarm")}
            </FieldLabel>
            <NativeSelect id={id("alarm")} value={String(form.alarm)} onChange={(e) => set("alarm", Number(e.target.value))}>
              {[...new Set([...ALARMS, form.alarm])].map((m) => (
                <NativeSelectOption key={m} value={String(m)}>
                  {m < 0 ? t("calendar.alarm.none") : alarmText(m, t)}
                </NativeSelectOption>
              ))}
            </NativeSelect>
          </Field>
          <Field>
            <FieldLabel htmlFor={id("location")}>{t("calendar.field.location")}</FieldLabel>
            <Input id={id("location")} value={form.location} onChange={(e) => set("location", e.target.value)} maxLength={500} />
          </Field>
          <Field>
            <FieldLabel htmlFor={id("description")}>{t("calendar.field.description")}</FieldLabel>
            <Textarea id={id("description")} value={form.description} onChange={(e) => set("description", e.target.value)} rows={3} />
          </Field>
          {problem !== null && (
            <Alert variant="crit" role="alert">
              <HugeiconsIcon icon={Alert01Icon} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
              <AlertDescription>{problem}</AlertDescription>
            </Alert>
          )}
        </DialogBody>
        <DialogFooter>
          <Button type="button" variant="ghost" onClick={onClose} disabled={busy}>
            {t("calendar.cancel")}
          </Button>
          <Button type="submit" disabled={busy || form.summary.trim() === "" || calendars.length === 0} className={cn(busy && "cursor-progress")}>
            {busy && <Spinner className="size-4" />}
            {t("calendar.save")}
          </Button>
        </DialogFooter>
      </form>
    </Dialog>
  );
}
