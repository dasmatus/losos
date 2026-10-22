/* LosOS Calendar's dialogs for calendars themselves: subscribing to one
 * at a webcal:// or https address (a tax office's deadlines, a timetable
 * from EduPage), importing an .ics file once, making a new calendar, and
 * one calendar's settings: its name and colour, how often a subscription
 * is read, and the address a published calendar is read at by any
 * calendar app. */

import * as React from "react";
import { HugeiconsIcon } from "@hugeicons/react";
import { Alert01Icon, Copy01Icon, Delete02Icon, Refresh01Icon, Tick02Icon, Upload04Icon } from "@hugeicons/core-free-icons";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Dialog, DialogBody, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Field, FieldDescription, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { NativeSelect, NativeSelectOption } from "@/components/ui/native-select";
import { Spinner } from "@/components/ui/spinner";
import { Switch } from "@/components/ui/switch";
import { toast } from "@/components/ui/toast";
import {
  ACCENTS,
  changeCalendar,
  createCalendar,
  deleteCalendar,
  importCalendar,
  publishCalendar,
  refreshCalendar,
  subscribeCalendar,
  type CalendarInfo,
  type CalendarReport,
} from "@/lib/api";
import { useLocale, useT } from "@/lib/i18n-react";
import { cn } from "@/lib/utils";
import { CALENDAR_COLORS, accentOf, nextColor } from "./colors";

function message(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function Failed({ text }: { text: string }) {
  return (
    <Alert variant="crit" role="alert">
      <HugeiconsIcon icon={Alert01Icon} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
      <AlertDescription>{text}</AlertDescription>
    </Alert>
  );
}

/* The six colours as swatches, one of them picked. */
function Colors({ value, onChange, label }: { value: string; onChange: (c: string) => void; label: string }) {
  const t = useT();
  const picked = accentOf(value);
  return (
    <fieldset className="flex flex-wrap gap-2">
      <legend className="mb-1.5 text-sm font-medium">{label}</legend>
      {ACCENTS.map((a) => (
        <button
          key={a}
          type="button"
          data-accent={a}
          aria-pressed={picked === a}
          aria-label={t(`calendar.color.${a}`)}
          title={t(`calendar.color.${a}`)}
          onClick={() => onChange(CALENDAR_COLORS[a])}
          className={cn(
            "grid size-8 place-items-center rounded-full bg-person text-surface transition-[scale,box-shadow] active:scale-[0.92] focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent",
            picked === a && "ring-2 ring-person ring-offset-2 ring-offset-surface",
          )}
        >
          {picked === a && <HugeiconsIcon icon={Tick02Icon} size={16} strokeWidth={2} color="currentColor" aria-hidden="true" />}
        </button>
      ))}
    </fieldset>
  );
}

function reportText(r: CalendarReport, t: ReturnType<typeof useT>): string {
  return t("calendar.report", { added: r.added, updated: r.updated, removed: r.removed });
}

const REFRESH = [3600, 6 * 3600, 86_400, 7 * 86_400] as const;

function refreshText(secs: number, t: ReturnType<typeof useT>): string {
  if (secs % 604_800 === 0) return t("calendar.refresh.weeks", { count: secs / 604_800 });
  if (secs % 86_400 === 0) return t("calendar.refresh.days", { count: secs / 86_400 });
  if (secs % 3600 === 0) return t("calendar.refresh.hours", { count: secs / 3600 });
  return t("calendar.refresh.minutes", { count: Math.round(secs / 60) });
}

export function SubscribeDialog({ calendars, onClose, onDone }: { calendars: CalendarInfo[]; onClose: () => void; onDone: () => void }) {
  const t = useT();
  const [url, setUrl] = React.useState("");
  const [name, setName] = React.useState("");
  const [color, setColor] = React.useState(() => nextColor(calendars.map((c) => c.color)));
  const [refresh, setRefresh] = React.useState<number>(86_400);
  const [busy, setBusy] = React.useState(false);
  const [problem, setProblem] = React.useState<string | null>(null);

  const submit = (event: React.FormEvent) => {
    event.preventDefault();
    setBusy(true);
    setProblem(null);
    subscribeCalendar({ url: url.trim(), color, refresh, ...(name.trim() !== "" ? { name: name.trim() } : {}) })
      .then(({ calendar, report }) => {
        toast.done(t("calendar.done.subscribed", { name: calendar.name }), reportText(report, t));
        onDone();
      })
      .catch((error: unknown) => {
        setProblem(message(error));
        setBusy(false);
      });
  };

  return (
    <Dialog open onOpenChange={(next) => !next && !busy && onClose()} labelledBy="calendar-subscribe-title">
      <form onSubmit={submit}>
        <DialogHeader>
          <DialogTitle id="calendar-subscribe-title">{t("calendar.subscribeTitle")}</DialogTitle>
          <DialogDescription>{t("calendar.subscribeLead")}</DialogDescription>
        </DialogHeader>
        <DialogBody className="flex flex-col gap-4">
          <Field>
            <FieldLabel htmlFor="calendar-subscribe-url">{t("calendar.field.address")}</FieldLabel>
            <Input
              id="calendar-subscribe-url"
              type="text"
              inputMode="url"
              value={url}
              onChange={(e) => setUrl(e.target.value)}
              placeholder="webcal://"
              required
              autoFocus
              spellCheck={false}
            />
            <FieldDescription>{t("calendar.field.addressHint")}</FieldDescription>
          </Field>
          <Field>
            <FieldLabel htmlFor="calendar-subscribe-name">{t("calendar.field.name")}</FieldLabel>
            <Input id="calendar-subscribe-name" value={name} onChange={(e) => setName(e.target.value)} placeholder={t("calendar.field.nameFromFeed")} />
          </Field>
          <Field>
            <FieldLabel htmlFor="calendar-subscribe-refresh">{t("calendar.field.refresh")}</FieldLabel>
            <NativeSelect id="calendar-subscribe-refresh" value={String(refresh)} onChange={(e) => setRefresh(Number(e.target.value))}>
              {REFRESH.map((s) => (
                <NativeSelectOption key={s} value={String(s)}>
                  {refreshText(s, t)}
                </NativeSelectOption>
              ))}
            </NativeSelect>
          </Field>
          <Colors value={color} onChange={setColor} label={t("calendar.field.color")} />
          {problem !== null && <Failed text={problem} />}
        </DialogBody>
        <DialogFooter>
          <Button type="button" variant="ghost" onClick={onClose} disabled={busy}>
            {t("calendar.cancel")}
          </Button>
          <Button type="submit" disabled={busy || url.trim() === ""}>
            {busy && <Spinner className="size-4" />}
            {busy ? t("calendar.subscribing") : t("calendar.subscribe")}
          </Button>
        </DialogFooter>
      </form>
    </Dialog>
  );
}

export function NewCalendarDialog({ calendars, onClose, onDone }: { calendars: CalendarInfo[]; onClose: () => void; onDone: () => void }) {
  const t = useT();
  const [name, setName] = React.useState("");
  const [color, setColor] = React.useState(() => nextColor(calendars.map((c) => c.color)));
  const [busy, setBusy] = React.useState(false);
  const [problem, setProblem] = React.useState<string | null>(null);
  return (
    <Dialog open onOpenChange={(next) => !next && !busy && onClose()} labelledBy="calendar-new-title">
      <form
        onSubmit={(event) => {
          event.preventDefault();
          setBusy(true);
          createCalendar({ name: name.trim(), color })
            .then(() => {
              toast.done(t("calendar.done.created", { name: name.trim() }));
              onDone();
            })
            .catch((error: unknown) => {
              setProblem(message(error));
              setBusy(false);
            });
        }}
      >
        <DialogHeader>
          <DialogTitle id="calendar-new-title">{t("calendar.newCalendar")}</DialogTitle>
        </DialogHeader>
        <DialogBody className="flex flex-col gap-4">
          <Field>
            <FieldLabel htmlFor="calendar-new-name">{t("calendar.field.name")}</FieldLabel>
            <Input id="calendar-new-name" value={name} onChange={(e) => setName(e.target.value)} required autoFocus maxLength={100} />
          </Field>
          <Colors value={color} onChange={setColor} label={t("calendar.field.color")} />
          {problem !== null && <Failed text={problem} />}
        </DialogBody>
        <DialogFooter>
          <Button type="button" variant="ghost" onClick={onClose} disabled={busy}>
            {t("calendar.cancel")}
          </Button>
          <Button type="submit" disabled={busy || name.trim() === ""}>
            {busy && <Spinner className="size-4" />}
            {t("calendar.create")}
          </Button>
        </DialogFooter>
      </form>
    </Dialog>
  );
}

export function ImportDialog({ calendars, onClose, onImported }: { calendars: CalendarInfo[]; onClose: () => void; onImported: () => void }) {
  const t = useT();
  const input = React.useRef<HTMLInputElement>(null);
  const [into, setInto] = React.useState("");
  const [busy, setBusy] = React.useState<string | null>(null);
  const [done, setDone] = React.useState<{ name: string; report: CalendarReport } | null>(null);
  const [problem, setProblem] = React.useState<string | null>(null);
  const [over, setOver] = React.useState(false);

  const take = (file: File) => {
    setBusy(file.name);
    setProblem(null);
    setDone(null);
    importCalendar(file, into)
      .then(({ calendar, report }) => {
        setDone({ name: calendar.name, report });
        onImported();
      })
      .catch((error: unknown) => setProblem(message(error)))
      .finally(() => setBusy(null));
  };

  return (
    <Dialog open onOpenChange={(next) => !next && busy === null && onClose()} labelledBy="calendar-import-title">
      <DialogHeader>
        <DialogTitle id="calendar-import-title">{t("calendar.importTitle")}</DialogTitle>
        <DialogDescription>{t("calendar.importLead")}</DialogDescription>
      </DialogHeader>
      <DialogBody className="flex flex-col gap-3">
        <Field>
          <FieldLabel htmlFor="calendar-import-into">{t("calendar.field.into")}</FieldLabel>
          <NativeSelect id="calendar-import-into" value={into} onChange={(e) => setInto(e.target.value)}>
            <NativeSelectOption value="">{t("calendar.field.intoNew")}</NativeSelectOption>
            {calendars.map((c) => (
              <NativeSelectOption key={c.uri} value={c.uri}>
                {c.name}
              </NativeSelectOption>
            ))}
          </NativeSelect>
        </Field>
        <input
          ref={input}
          type="file"
          accept=".ics,.ical,.ifb,text/calendar"
          hidden
          onChange={(event) => {
            const file = event.target.files?.[0];
            if (file !== undefined) take(file);
            event.target.value = "";
          }}
        />
        <div
          onDragOver={(event) => {
            event.preventDefault();
            setOver(true);
          }}
          onDragLeave={() => setOver(false)}
          onDrop={(event) => {
            event.preventDefault();
            setOver(false);
            const file = event.dataTransfer.files[0];
            if (file !== undefined && busy === null) take(file);
          }}
          className={cn(
            "flex flex-col items-center gap-2 rounded-card border border-dashed border-line px-4 py-8 text-center transition-colors",
            over && "border-accent bg-accent-wash",
          )}
        >
          {busy !== null ? (
            <p className="flex items-center gap-2 text-sm text-muted">
              <Spinner size={16} />
              {t("calendar.importing", { file: busy })}
            </p>
          ) : (
            <>
              <HugeiconsIcon icon={Upload04Icon} size={24} strokeWidth={1.5} className="text-muted" aria-hidden="true" />
              <p className="text-sm text-muted">{t("calendar.importDrop")}</p>
              <Button variant="secondary" size="sm" onClick={() => input.current?.click()}>
                {t("calendar.importChoose")}
              </Button>
            </>
          )}
        </div>
        {done !== null && (
          <Alert role="status">
            <AlertDescription>
              <p className="font-medium">{t("calendar.importDone", { name: done.name })}</p>
              <p>{reportText(done.report, t)}</p>
              {done.report.skipped + done.report.leftOut > 0 && <p>{t("calendar.importSkipped", { count: done.report.skipped + done.report.leftOut })}</p>}
            </AlertDescription>
          </Alert>
        )}
        {problem !== null && <Failed text={problem} />}
      </DialogBody>
      <DialogFooter>
        <Button variant={done !== null ? "default" : "ghost"} onClick={onClose} disabled={busy !== null}>
          {t("calendar.close")}
        </Button>
      </DialogFooter>
    </Dialog>
  );
}

/** The address a published calendar is read at, as phones want it. */
export function feedAddress(path: string, scheme: "https" | "webcal"): string {
  const host = window.location.host;
  return `${scheme === "webcal" ? "webcal" : window.location.protocol.replace(":", "")}://${host}${path}`;
}

function Copyable({ value, label }: { value: string; label: string }) {
  const t = useT();
  return (
    <div className="flex gap-2">
      <Input readOnly value={value} aria-label={label} className="font-mono text-[12.5px]" onFocus={(e) => e.target.select()} />
      <Button
        type="button"
        variant="secondary"
        size="icon"
        aria-label={`${t("calendar.act.copy")}: ${label}`}
        title={t("calendar.act.copy")}
        onClick={() =>
          void navigator.clipboard
            .writeText(value)
            .then(() => toast.done(t("calendar.copied")))
            .catch(() => undefined)
        }
      >
        <HugeiconsIcon icon={Copy01Icon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
      </Button>
    </div>
  );
}

export function CalendarSettings({
  calendar,
  last,
  onClose,
  onChanged,
}: {
  calendar: CalendarInfo;
  last: boolean;
  onClose: () => void;
  onChanged: (c: CalendarInfo | null) => void;
}) {
  const t = useT();
  const locale = useLocale();
  const [name, setName] = React.useState(calendar.name);
  const [color, setColor] = React.useState(calendar.color ?? CALENDAR_COLORS.teal);
  const [refresh, setRefresh] = React.useState(calendar.feed?.refresh ?? 86_400);
  const [busy, setBusy] = React.useState<string | null>(null);
  const [problem, setProblem] = React.useState<string | null>(null);
  const [confirming, setConfirming] = React.useState(false);

  const run = (what: string, job: () => Promise<unknown>) => {
    setBusy(what);
    setProblem(null);
    job()
      .catch((error: unknown) => setProblem(message(error)))
      .finally(() => setBusy(null));
  };

  const changed = name.trim() !== calendar.name || color !== (calendar.color ?? "") || (calendar.feed !== undefined && refresh !== calendar.feed.refresh);
  const feed = calendar.feed;
  const when = (ms: number) => new Intl.DateTimeFormat(locale, { dateStyle: "medium", timeStyle: "short" }).format(new Date(ms));

  return (
    <Dialog open onOpenChange={(next) => !next && busy === null && onClose()} labelledBy="calendar-settings-title" dialogClassName="w-[min(30rem,calc(100vw-2rem))]">
      <DialogHeader>
        <DialogTitle id="calendar-settings-title">{calendar.name}</DialogTitle>
        <DialogDescription>{calendar.readOnly ? t("calendar.settings.subscriptionLead") : t("calendar.settings.lead", { count: calendar.count })}</DialogDescription>
      </DialogHeader>
      <DialogBody className="flex flex-col gap-4">
        <Field>
          <FieldLabel htmlFor="calendar-settings-name">{t("calendar.field.name")}</FieldLabel>
          <Input id="calendar-settings-name" value={name} onChange={(e) => setName(e.target.value)} maxLength={100} />
        </Field>
        <Colors value={color} onChange={setColor} label={t("calendar.field.color")} />
        {feed !== undefined && (
          <>
            <Field>
              <FieldLabel htmlFor="calendar-settings-url">{t("calendar.field.address")}</FieldLabel>
              <Copyable value={feed.url} label={t("calendar.field.address")} />
            </Field>
            <Field>
              <FieldLabel htmlFor="calendar-settings-refresh">{t("calendar.field.refresh")}</FieldLabel>
              <NativeSelect id="calendar-settings-refresh" value={String(refresh)} onChange={(e) => setRefresh(Number(e.target.value))}>
                {[...new Set([...REFRESH, feed.refresh])].map((s) => (
                  <NativeSelectOption key={s} value={String(s)}>
                    {refreshText(s, t)}
                  </NativeSelectOption>
                ))}
              </NativeSelect>
              <FieldDescription>
                {feed.ok !== undefined ? t("calendar.settings.lastRead", { when: when(feed.ok) }) : t("calendar.settings.neverRead")}
              </FieldDescription>
            </Field>
            {feed.error !== "" && <Failed text={t("calendar.settings.readFailed", { detail: feed.error })} />}
            <div>
              <Button
                variant="secondary"
                size="sm"
                disabled={busy !== null}
                onClick={() =>
                  run("refresh", async () => {
                    const { calendar: c, report } = await refreshCalendar(calendar.uri);
                    toast.done(t("calendar.done.refreshed", { name: c.name }), reportText(report, t));
                    onChanged(c);
                  })
                }
              >
                {busy === "refresh" ? <Spinner className="size-4" /> : <HugeiconsIcon icon={Refresh01Icon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />}
                {t("calendar.act.refresh")}
              </Button>
            </div>
          </>
        )}
        {!calendar.readOnly && (
          <div className="flex flex-col gap-2 rounded-card border border-line p-3">
            <Field orientation="horizontal" className="items-center justify-between">
              <div>
                <FieldLabel htmlFor="calendar-settings-publish">{t("calendar.settings.publish")}</FieldLabel>
                <FieldDescription>{t("calendar.settings.publishHint")}</FieldDescription>
              </div>
              <Switch
                id="calendar-settings-publish"
                isSelected={calendar.published !== undefined}
                isDisabled={busy !== null}
                onChange={(on) =>
                  run("publish", async () => {
                    const c = await publishCalendar(calendar.uri, on);
                    toast.done(on ? t("calendar.done.published") : t("calendar.done.unpublished"));
                    onChanged(c);
                  })
                }
              />
            </Field>
            {calendar.published !== undefined && (
              <>
                <Copyable value={feedAddress(calendar.published, "webcal")} label={t("calendar.settings.webcal")} />
                <p className="text-[12.5px] text-muted">{t("calendar.settings.publishedNote")}</p>
              </>
            )}
          </div>
        )}
        {confirming && (
          <Alert role="alert">
            <AlertDescription>
              <p>{calendar.readOnly ? t("calendar.settings.confirmUnsubscribe") : t("calendar.settings.confirmDelete", { count: calendar.count })}</p>
              <div className="mt-2 flex gap-2">
                <Button
                  size="sm"
                  variant="destructive"
                  disabled={busy !== null}
                  onClick={() =>
                    run("delete", async () => {
                      await deleteCalendar(calendar.uri);
                      toast.done(t("calendar.done.removed", { name: calendar.name }));
                      onChanged(null);
                    })
                  }
                >
                  {calendar.readOnly ? t("calendar.act.unsubscribe") : t("calendar.act.deleteCalendar")}
                </Button>
                <Button size="sm" variant="ghost" onClick={() => setConfirming(false)}>
                  {t("calendar.cancel")}
                </Button>
              </div>
            </AlertDescription>
          </Alert>
        )}
        {problem !== null && <Failed text={problem} />}
      </DialogBody>
      <DialogFooter>
        <Button variant="ghost" className="mr-auto text-crit" disabled={busy !== null || last} title={last ? t("calendar.settings.lastOne") : undefined} onClick={() => setConfirming(true)}>
          <HugeiconsIcon icon={Delete02Icon} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
          {calendar.readOnly ? t("calendar.act.unsubscribe") : t("calendar.act.deleteCalendar")}
        </Button>
        <Button variant="ghost" onClick={onClose} disabled={busy !== null}>
          {t("calendar.close")}
        </Button>
        <Button
          disabled={busy !== null || !changed || name.trim() === ""}
          onClick={() =>
            run("save", async () => {
              const c = await changeCalendar(calendar.uri, {
                name: name.trim(),
                color,
                ...(calendar.feed !== undefined ? { refresh } : {}),
              });
              toast.done(t("calendar.done.savedCalendar"));
              onChanged(c);
              onClose();
            })
          }
        >
          {busy === "save" && <Spinner className="size-4" />}
          {t("calendar.save")}
        </Button>
      </DialogFooter>
    </Dialog>
  );
}
