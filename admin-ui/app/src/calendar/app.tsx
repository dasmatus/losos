/* LosOS Calendar: a screen of the admin UI at /calendar, inside its top bar
 * and sidebar like Mail, over LosOS Cloud's /cloud/api/calendar
 * (backend/src/cloud/calendar.rs). The calendars are the person's in LosOS
 * Cloud, the ones their phone syncs over CalDAV, so Calendar exists
 * wherever LosOS Cloud does.
 *
 * One card holds the calendars on the left (the person's own, and the
 * subscriptions the box reads from webcal:// and https addresses) and the
 * week, the month or the list of what is coming on the right. An event
 * opens in a dialog with what it takes to join it: a call or a chat in
 * LosOS Chat with the people in it, a message to them in LosOS Mail, and
 * its invitations, sent as iCalendar so any calendar app can answer. Those
 * buttons follow the apps, which run only where the box shares. */

import * as React from "react";
import { HugeiconsIcon } from "@hugeicons/react";
import {
  Add01Icon,
  ArrowLeft01Icon,
  ArrowRight01Icon,
  Calendar03Icon,
  CalendarAdd01Icon,
  Link01Icon,
  MoreHorizontalIcon,
  RssIcon,
  Upload04Icon,
  Video01Icon,
  ViewIcon,
  ViewOffIcon,
} from "@hugeicons/core-free-icons";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from "@/components/ui/empty";
import { Spinner } from "@/components/ui/spinner";
import { PaneHeader } from "@/screens/settings/rows";
import {
  ApiError,
  getCalendarEvents,
  getCalendars,
  loadMe,
  signInUrl,
  type CalendarEvent,
  type CalendarInfo,
} from "@/lib/api";
import { useLocale, useT } from "@/lib/i18n-react";
import { contextMenu, type MenuBuilder } from "@/lib/context-menu";
import { cn } from "@/lib/utils";
import { colorProps } from "./colors";
import { EventDetails, EventEditor, type Draft } from "./event";
import { CalendarSettings, ImportDialog, NewCalendarDialog, SubscribeDialog } from "./settings";
import { DAY_MS, addDays, addMonths, dayDate, dayOf, daysCovered, monthStart, span, weekStart } from "./time";

type Loading =
  | { kind: "loading" }
  | { kind: "signIn" }
  | { kind: "missing" }
  | { kind: "failed"; message: string }
  | { kind: "ready"; calendars: CalendarInfo[] };

export type View = "week" | "month" | "agenda";

function message(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

/* Which calendars the person hid, kept per browser. */
const HIDDEN_KEY = "losos.calendar.hidden";
function readHidden(): Set<string> {
  try {
    const raw = window.localStorage.getItem(HIDDEN_KEY);
    return new Set(raw === null ? [] : (JSON.parse(raw) as string[]));
  } catch {
    return new Set();
  }
}
function writeHidden(hidden: Set<string>): void {
  try {
    window.localStorage.setItem(HIDDEN_KEY, JSON.stringify([...hidden]));
  } catch {
    /* the choice lasts this visit only */
  }
}

export default function CalendarScreen() {
  const t = useT();
  const [state, setState] = React.useState<Loading>({ kind: "loading" });

  const load = React.useCallback(async () => {
    try {
      const me = await loadMe();
      if (me === null) {
        setState({ kind: "signIn" });
        return;
      }
      setState({ kind: "ready", calendars: await getCalendars() });
    } catch (error) {
      if (error instanceof ApiError && error.status === 404) setState({ kind: "missing" });
      else if (error instanceof ApiError && (error.status === 401 || error.status === 403)) setState({ kind: "signIn" });
      else setState({ kind: "failed", message: message(error) });
    }
  }, []);

  React.useEffect(() => {
    void load();
  }, [load]);

  if (state.kind === "ready") {
    return <Planner calendars={state.calendars} onCalendars={(calendars) => setState({ kind: "ready", calendars })} />;
  }
  if (state.kind === "loading") {
    return (
      <div className="grid min-h-48 place-items-center text-muted">
        <Spinner label={t("calendar.loading")} />
      </div>
    );
  }
  return (
    <Empty>
      <EmptyHeader>
        <EmptyMedia variant="icon">
          <HugeiconsIcon icon={Calendar03Icon} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
        </EmptyMedia>
        <EmptyTitle>{t("calendar.title")}</EmptyTitle>
        <EmptyDescription>
          {state.kind === "signIn"
            ? t("calendar.signIn.text")
            : state.kind === "missing"
              ? t("calendar.missing")
              : t("calendar.failed", { detail: state.message })}
        </EmptyDescription>
      </EmptyHeader>
      {state.kind === "signIn" && <Button onClick={() => window.location.assign(signInUrl())}>{t("calendar.signIn.button")}</Button>}
      {state.kind === "failed" && (
        <Button variant="secondary" onClick={() => void load()}>
          {t("calendar.retry")}
        </Button>
      )}
    </Empty>
  );
}

/* The window a view shows: its first day and the day after its last. */
function windowOf(view: View, focus: string): [string, string] {
  if (view === "week") {
    const from = weekStart(focus);
    return [from, addDays(from, 7)];
  }
  if (view === "month") {
    const from = weekStart(monthStart(focus));
    return [from, addDays(from, 42)];
  }
  return [focus, addDays(focus, 42)];
}

function Planner({ calendars, onCalendars }: { calendars: CalendarInfo[]; onCalendars: (c: CalendarInfo[]) => void }) {
  const t = useT();
  const locale = useLocale();
  const today = dayOf(new Date());
  const [view, setView] = React.useState<View>(() => (window.matchMedia?.("(max-width: 767px)").matches ? "agenda" : "week"));
  const [focus, setFocus] = React.useState(today);
  const [hidden, setHidden] = React.useState<Set<string>>(readHidden);
  const [events, setEvents] = React.useState<CalendarEvent[] | null>(null);
  const [problem, setProblem] = React.useState<string | null>(null);
  const [open, setOpen] = React.useState<CalendarEvent | null>(null);
  const [editing, setEditing] = React.useState<Draft | null>(null);
  const [dialog, setDialog] = React.useState<"subscribe" | "import" | "new" | null>(null);
  const [settingsOf, setSettingsOf] = React.useState<CalendarInfo | null>(null);
  const [column, setColumn] = React.useState<"list" | "view">("view");
  const [stamp, setStamp] = React.useState(0);

  const [from, to] = windowOf(view, focus);

  React.useEffect(() => {
    const controller = new AbortController();
    setProblem(null);
    getCalendarEvents(from, to, { signal: controller.signal })
      .then(setEvents)
      .catch((error: unknown) => {
        if (!controller.signal.aborted) setProblem(message(error));
      });
    return () => controller.abort();
  }, [from, to, stamp]);

  const reload = React.useCallback(async () => {
    setStamp((s) => s + 1);
    try {
      onCalendars(await getCalendars());
    } catch {
      /* the list stays as it was; the events say what failed */
    }
  }, [onCalendars]);

  // /calendar?event=<id> opens that event (a link from a reminder or Mail).
  React.useEffect(() => {
    const params = new URLSearchParams(window.location.search);
    const id = params.get("event");
    const day = params.get("day");
    if (id === null && day === null) return;
    window.history.replaceState(window.history.state, "", window.location.pathname);
    if (day !== null && /^\d{4}-\d{2}-\d{2}$/.test(day)) setFocus(day);
    if (id !== null) {
      void import("@/lib/api").then(({ getCalendarEvent }) =>
        getCalendarEvent(id)
          .then((e) => {
            setFocus(e.start.slice(0, 10));
            setOpen(e);
          })
          .catch(() => undefined),
      );
    }
  }, []);

  const byUri = new Map(calendars.map((c) => [c.uri, c]));
  const shown = (events ?? []).filter((e) => !hidden.has(e.calendar) && byUri.has(e.calendar));
  const own = calendars.filter((c) => !c.readOnly);
  const subs = calendars.filter((c) => c.readOnly);

  const toggle = (uri: string) => {
    const next = new Set(hidden);
    if (next.has(uri)) next.delete(uri);
    else next.add(uri);
    setHidden(next);
    writeHidden(next);
  };

  const step = (n: number) => {
    if (view === "week") setFocus(addDays(focus, 7 * n));
    else if (view === "month") setFocus(addMonths(focus, n));
    else setFocus(addDays(focus, 28 * n));
  };

  const title = (() => {
    const f = new Intl.DateTimeFormat(locale, { month: "long", year: "numeric" });
    if (view === "month") return f.format(dayDate(monthStart(focus)));
    if (view === "week") {
      const a = dayDate(from);
      const b = dayDate(addDays(from, 6));
      return new Intl.DateTimeFormat(locale, { day: "numeric", month: "long", year: "numeric" }).formatRange(a, b);
    }
    return f.format(dayDate(focus));
  })();

  const newAt = (start: string, allDay = false) =>
    setEditing({ kind: "new", start, allDay, calendar: own.find((c) => !hidden.has(c.uri))?.uri ?? own[0]?.uri ?? "" });

  const views: { id: View; label: string }[] = [
    { id: "week", label: t("calendar.view.week") },
    { id: "month", label: t("calendar.view.month") },
    { id: "agenda", label: t("calendar.view.agenda") },
  ];

  const calendarMenu =
    (c: CalendarInfo): MenuBuilder =>
    () => ({
      label: c.name,
      sections: [
        [
          {
            id: "show",
            label: hidden.has(c.uri) ? t("calendar.act.show") : t("calendar.act.hide"),
            icon: hidden.has(c.uri) ? ViewIcon : ViewOffIcon,
            onSelect: () => toggle(c.uri),
          },
          { id: "settings", label: t("calendar.act.settings"), icon: MoreHorizontalIcon, onSelect: () => setSettingsOf(c) },
        ],
      ],
    });

  const calendarRow = (c: CalendarInfo) => {
    const off = hidden.has(c.uri);
    const failing = c.feed !== undefined && c.feed.error !== "";
    return (
      <li key={c.uri} ref={contextMenu(calendarMenu(c))} className="group flex items-center gap-1 pr-1">
        <button
          type="button"
          onClick={() => toggle(c.uri)}
          aria-pressed={!off}
          aria-label={off ? t("calendar.act.showNamed", { name: c.name }) : t("calendar.act.hideNamed", { name: c.name })}
          className="flex min-w-0 flex-1 items-center gap-2.5 rounded-control px-3 py-1.5 text-left text-sm transition-[background-color,scale] hover:bg-sunk active:scale-[0.97]"
        >
          <span
            {...colorProps(c.color)}
            aria-hidden="true"
            className={cn("size-3 shrink-0 rounded-full border-2 border-person transition-colors", off ? "bg-transparent" : "bg-person")}
          />
          <span className={cn("min-w-0 flex-1 truncate", off && "text-muted")}>{c.name}</span>
          {failing && <span className="size-1.5 shrink-0 rounded-full bg-crit" title={c.feed?.error} />}
          {c.published !== undefined && <HugeiconsIcon icon={RssIcon} size={13} strokeWidth={1.5} className="shrink-0 text-muted" aria-label={t("calendar.published")} />}
        </button>
        <Button
          variant="ghost"
          size="icon-sm"
          aria-label={t("calendar.act.settingsNamed", { name: c.name })}
          title={t("calendar.act.settings")}
          className="opacity-60 group-hover:opacity-100 focus-visible:opacity-100"
          onClick={() => setSettingsOf(c)}
        >
          <HugeiconsIcon icon={MoreHorizontalIcon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
        </Button>
      </li>
    );
  };

  return (
    <>
      <div className="flex flex-col">
        <div className="flex flex-wrap items-start gap-3">
          <div className="min-w-0 flex-1">
            <PaneHeader title={t("calendar.title")} summary={t("calendar.summary")} />
          </div>
          <div className="flex shrink-0 flex-wrap gap-2">
            <Button variant="secondary" onClick={() => setDialog("subscribe")}>
              <HugeiconsIcon icon={Link01Icon} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
              {t("calendar.subscribe")}
            </Button>
            <Button variant="secondary" onClick={() => setDialog("import")}>
              <HugeiconsIcon icon={Upload04Icon} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
              {t("calendar.import")}
            </Button>
            <Button onClick={() => newAt(`${view === "week" && focus !== today ? from : today}T${nextHour()}`)} disabled={own.length === 0}>
              <HugeiconsIcon icon={CalendarAdd01Icon} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
              {t("calendar.new")}
            </Button>
          </div>
        </div>

        <div className="mb-3 flex flex-wrap items-center gap-2">
          <Button variant="secondary" size="sm" onClick={() => setFocus(today)}>
            {t("calendar.today")}
          </Button>
          <div className="flex">
            <Button variant="ghost" size="icon-sm" aria-label={t("calendar.previous")} title={t("calendar.previous")} onClick={() => step(-1)}>
              <HugeiconsIcon icon={ArrowLeft01Icon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
            </Button>
            <Button variant="ghost" size="icon-sm" aria-label={t("calendar.next")} title={t("calendar.next")} onClick={() => step(1)}>
              <HugeiconsIcon icon={ArrowRight01Icon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
            </Button>
          </div>
          <h2 className="min-w-0 text-[15px] font-semibold tracking-tight" aria-live="polite">
            {title}
          </h2>
          <Button variant="ghost" size="sm" className="md:hidden" onClick={() => setColumn(column === "list" ? "view" : "list")}>
            {t("calendar.calendars")}
          </Button>
          <nav aria-label={t("calendar.views")} className="ml-auto">
            <ul className="flex gap-1">
              {views.map((v) => (
                <li key={v.id}>
                  <Button
                    variant={v.id === view ? "secondary" : "ghost"}
                    size="sm"
                    aria-current={v.id === view ? "page" : undefined}
                    onClick={() => setView(v.id)}
                    className={cn(v.id === view && "text-ink")}
                  >
                    {v.label}
                  </Button>
                </li>
              ))}
            </ul>
          </nav>
        </div>

        <Card className="flex h-[min(48rem,calc(100dvh-15rem))] min-h-[28rem] overflow-hidden">
          <section
            aria-label={t("calendar.calendars")}
            className={cn(
              "min-w-0 flex-1 flex-col overflow-y-auto py-2 md:flex md:w-60 md:flex-none md:border-r md:border-line",
              column === "list" ? "flex" : "hidden",
            )}
          >
            <h3 className="px-4 pt-1 pb-1 text-[12px] font-semibold text-muted">{t("calendar.mine")}</h3>
            <ul className="px-1">{own.map(calendarRow)}</ul>
            <div className="px-3 pb-2">
              <Button variant="ghost" size="sm" className="text-muted" onClick={() => setDialog("new")}>
                <HugeiconsIcon icon={Add01Icon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
                {t("calendar.newCalendar")}
              </Button>
            </div>
            <h3 className="px-4 pt-2 pb-1 text-[12px] font-semibold text-muted">{t("calendar.subscriptions")}</h3>
            {subs.length === 0 ? (
              <p className="px-4 pb-2 text-[12.5px] text-muted">{t("calendar.noSubscriptions")}</p>
            ) : (
              <ul className="px-1">{subs.map(calendarRow)}</ul>
            )}
            <div className="px-3">
              <Button variant="ghost" size="sm" className="text-muted" onClick={() => setDialog("subscribe")}>
                <HugeiconsIcon icon={Link01Icon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
                {t("calendar.subscribe")}
              </Button>
            </div>
          </section>

          <section
            aria-label={title}
            className={cn("min-w-0 flex-1 flex-col md:flex", column === "view" ? "flex" : "hidden")}
          >
            {problem !== null ? (
              <Empty className="m-4 border-0">
                <EmptyHeader>
                  <EmptyMedia variant="icon">
                    <HugeiconsIcon icon={Calendar03Icon} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
                  </EmptyMedia>
                  <EmptyDescription>{t("calendar.failed", { detail: problem })}</EmptyDescription>
                </EmptyHeader>
                <Button variant="secondary" onClick={() => void reload()}>
                  {t("calendar.retry")}
                </Button>
              </Empty>
            ) : events === null ? (
              <div className="grid flex-1 place-items-center">
                <Spinner label={t("calendar.loading")} />
              </div>
            ) : view === "week" ? (
              <Week from={from} today={today} events={shown} calendars={byUri} onOpen={setOpen} onNew={newAt} />
            ) : view === "month" ? (
              <Month
                focus={focus}
                from={from}
                today={today}
                events={shown}
                calendars={byUri}
                onOpen={setOpen}
                onDay={(d) => {
                  setFocus(d);
                  setView("week");
                }}
                onNew={(d) => newAt(d, true)}
              />
            ) : (
              <Agenda from={from} to={to} today={today} events={shown} calendars={byUri} onOpen={setOpen} />
            )}
          </section>
        </Card>
      </div>

      {open !== null && (
        <EventDetails
          event={open}
          calendar={byUri.get(open.calendar)}
          onClose={() => setOpen(null)}
          onEdit={(full) => {
            setOpen(null);
            setEditing({ kind: "edit", event: full });
          }}
          onChanged={() => {
            setOpen(null);
            void reload();
          }}
        />
      )}
      {editing !== null && (
        <EventEditor
          draft={editing}
          calendars={own}
          onClose={() => setEditing(null)}
          onSaved={(saved) => {
            setEditing(null);
            setFocus(saved.start.slice(0, 10));
            void reload();
          }}
        />
      )}
      {dialog === "subscribe" && (
        <SubscribeDialog
          calendars={calendars}
          onClose={() => setDialog(null)}
          onDone={() => {
            setDialog(null);
            void reload();
          }}
        />
      )}
      {dialog === "import" && <ImportDialog calendars={own} onClose={() => setDialog(null)} onImported={() => void reload()} />}
      {dialog === "new" && (
        <NewCalendarDialog
          calendars={calendars}
          onClose={() => setDialog(null)}
          onDone={() => {
            setDialog(null);
            void reload();
          }}
        />
      )}
      {settingsOf !== null && (
        <CalendarSettings
          calendar={calendars.find((c) => c.uri === settingsOf.uri) ?? settingsOf}
          last={own.length === 1 && !settingsOf.readOnly}
          onClose={() => setSettingsOf(null)}
          onChanged={(c) => {
            if (c === null) setSettingsOf(null);
            void reload();
          }}
        />
      )}
    </>
  );
}

/* The next whole hour, for a new event's start. */
function nextHour(): string {
  const h = Math.min(new Date().getHours() + 1, 23);
  return `${String(h).padStart(2, "0")}:00`;
}

function timeText(ms: number, locale: string): string {
  return new Intl.DateTimeFormat(locale, { hour: "numeric", minute: "2-digit" }).format(new Date(ms));
}

/* One event as a coloured block or chip, in every view. */
function Chip({
  event,
  calendar,
  onOpen,
  className,
  children,
}: {
  event: CalendarEvent;
  calendar: CalendarInfo | undefined;
  onOpen: (e: CalendarEvent) => void;
  className?: string;
  children: React.ReactNode;
}) {
  const t = useT();
  const declined = event.attendees.some((a) => a.status === "DECLINED") && event.attendees.length === 1;
  return (
    <button
      type="button"
      {...colorProps(calendar?.color)}
      onClick={(ev) => {
        ev.stopPropagation();
        onOpen(event);
      }}
      title={event.summary}
      aria-label={`${event.summary || t("calendar.untitled")}${event.call !== undefined && event.call !== "" ? `, ${t("calendar.call.video")}` : ""}`}
      className={cn(
        "block w-full min-w-0 overflow-hidden rounded-control border-l-[3px] border-person bg-person-wash text-left text-[12.5px] leading-tight text-ink transition-[filter,scale] hover:brightness-95 active:scale-[0.98] focus-visible:outline-2 focus-visible:outline-offset-1 focus-visible:outline-accent",
        declined && "opacity-60",
        className,
      )}
    >
      {children}
    </button>
  );
}

function CallMark({ event }: { event: CalendarEvent }) {
  if (event.call === undefined || event.call === "") return null;
  return <HugeiconsIcon icon={Video01Icon} size={12} strokeWidth={1.75} className="mr-1 inline shrink-0 align-[-2px] text-person" aria-hidden="true" />;
}

/* Lays timed events of one day out side by side where they overlap. */
function columns(list: { e: CalendarEvent; a: number; b: number }[]): { e: CalendarEvent; a: number; b: number; col: number; cols: number }[] {
  const sorted = [...list].sort((x, y) => x.a - y.a || y.b - x.b);
  const out: { e: CalendarEvent; a: number; b: number; col: number; cols: number }[] = [];
  let group: typeof out = [];
  let end = -Infinity;
  const flush = () => {
    const cols = Math.max(1, ...group.map((g) => g.col + 1));
    for (const g of group) g.cols = cols;
    out.push(...group);
    group = [];
  };
  for (const item of sorted) {
    if (item.a >= end && group.length > 0) {
      flush();
      end = -Infinity;
    }
    const taken = new Set(group.filter((g) => g.b > item.a).map((g) => g.col));
    let col = 0;
    while (taken.has(col)) col++;
    group.push({ ...item, col, cols: 1 });
    end = Math.max(end, item.b);
  }
  if (group.length > 0) flush();
  return out;
}

const HOUR_PX = 48;

function Week({
  from,
  today,
  events,
  calendars,
  onOpen,
  onNew,
}: {
  from: string;
  today: string;
  events: CalendarEvent[];
  calendars: Map<string, CalendarInfo>;
  onOpen: (e: CalendarEvent) => void;
  onNew: (start: string, allDay?: boolean) => void;
}) {
  const t = useT();
  const locale = useLocale();
  const days = Array.from({ length: 7 }, (_, i) => addDays(from, i));
  const scroller = React.useRef<HTMLDivElement>(null);
  const [now, setNow] = React.useState(() => Date.now());
  React.useEffect(() => {
    const timer = window.setInterval(() => setNow(Date.now()), 60_000);
    return () => window.clearInterval(timer);
  }, []);
  // Morning in view first: 07:00 at the top.
  React.useEffect(() => {
    if (scroller.current !== null) scroller.current.scrollTop = HOUR_PX * 7;
  }, [from]);

  const allDay = events.filter((e) => e.allDay || span(e)[1] - span(e)[0] >= DAY_MS);
  const timed = events.filter((e) => !allDay.includes(e));
  const head = new Intl.DateTimeFormat(locale, { weekday: "short" });

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div className="grid grid-cols-[3.5rem_repeat(7,minmax(0,1fr))] border-b border-line">
        <div />
        {days.map((d) => (
          <div key={d} className={cn("px-1 py-2 text-center", d === today && "text-accent")}>
            <p className="text-[12px] text-muted">{head.format(dayDate(d))}</p>
            <p
              className={cn(
                "mx-auto mt-0.5 grid size-7 place-items-center rounded-full text-[15px] font-semibold tabular-nums",
                d === today && "bg-accent text-surface",
              )}
            >
              {dayDate(d).getDate()}
            </p>
          </div>
        ))}
      </div>
      {allDay.length > 0 && (
        <div className="grid grid-cols-[3.5rem_repeat(7,minmax(0,1fr))] border-b border-line py-1">
          <p className="self-center pr-2 text-right text-[11px] text-muted">{t("calendar.allDay")}</p>
          {days.map((d) => (
            <ul key={d} className="flex min-w-0 flex-col gap-0.5 px-0.5">
              {allDay
                .filter((e) => daysCovered(e).includes(d))
                .map((e) => (
                  <li key={`${e.id}-${e.occurrence ?? ""}`}>
                    <Chip event={e} calendar={calendars.get(e.calendar)} onOpen={onOpen} className="truncate px-1.5 py-0.5">
                      <CallMark event={e} />
                      {e.summary || t("calendar.untitled")}
                    </Chip>
                  </li>
                ))}
            </ul>
          ))}
        </div>
      )}
      <div ref={scroller} className="relative min-h-0 flex-1 overflow-y-auto">
        <div className="relative grid grid-cols-[3.5rem_repeat(7,minmax(0,1fr))]" style={{ height: HOUR_PX * 24 }}>
          <div className="relative">
            {Array.from({ length: 23 }, (_, i) => i + 1).map((h) => (
              <span key={h} className="absolute right-2 -translate-y-1/2 text-[11px] text-faint tabular-nums" style={{ top: h * HOUR_PX }}>
                {timeText(new Date(2026, 0, 1, h).getTime(), locale)}
              </span>
            ))}
          </div>
          {days.map((d) => {
            const dayStart = dayDate(d).getTime();
            const dayEnd = dayDate(addDays(d, 1)).getTime();
            const placed = columns(
              timed
                .map((e) => {
                  const [a, b] = span(e);
                  return { e, a, b: Math.max(b, a + 15 * 60_000) };
                })
                .filter(({ a, b }) => a < dayEnd && b > dayStart),
            );
            const hours = (dayEnd - dayStart) / 3_600_000;
            return (
              <div
                key={d}
                className="relative border-l border-hair"
                onDoubleClick={(ev) => {
                  const box = ev.currentTarget.getBoundingClientRect();
                  const h = Math.max(0, Math.min(23, Math.floor(((ev.clientY - box.top) / box.height) * 24)));
                  onNew(`${d}T${String(h).padStart(2, "0")}:00`);
                }}
              >
                {Array.from({ length: 24 }, (_, h) => (
                  <div key={h} className="border-b border-hair" style={{ height: HOUR_PX }} />
                ))}
                {d === today && now >= dayStart && now < dayEnd && (
                  <div
                    aria-hidden="true"
                    className="pointer-events-none absolute inset-x-0 z-10 h-0.5 bg-crit"
                    style={{ top: ((now - dayStart) / 3_600_000 / hours) * HOUR_PX * 24 }}
                  />
                )}
                {placed.map(({ e, a, b, col, cols }) => {
                  const top = ((Math.max(a, dayStart) - dayStart) / 3_600_000 / hours) * HOUR_PX * 24;
                  const height = Math.max(((Math.min(b, dayEnd) - Math.max(a, dayStart)) / 3_600_000 / hours) * HOUR_PX * 24, 18);
                  return (
                    <div
                      key={`${e.id}-${e.occurrence ?? ""}`}
                      className="absolute px-0.5"
                      style={{ top, height, left: `${(col / cols) * 100}%`, width: `${100 / cols}%` }}
                    >
                      <Chip event={e} calendar={calendars.get(e.calendar)} onOpen={onOpen} className="h-full px-1.5 py-1">
                        <span className="block truncate font-medium">
                          <CallMark event={e} />
                          {e.summary || t("calendar.untitled")}
                        </span>
                        {height > 30 && <span className="block truncate text-muted">{timeText(a, locale)}</span>}
                        {height > 46 && e.location !== undefined && <span className="block truncate text-muted">{e.location}</span>}
                      </Chip>
                    </div>
                  );
                })}
              </div>
            );
          })}
        </div>
      </div>
    </div>
  );
}

function Month({
  focus,
  from,
  today,
  events,
  calendars,
  onOpen,
  onDay,
  onNew,
}: {
  focus: string;
  from: string;
  today: string;
  events: CalendarEvent[];
  calendars: Map<string, CalendarInfo>;
  onOpen: (e: CalendarEvent) => void;
  onDay: (day: string) => void;
  onNew: (day: string) => void;
}) {
  const t = useT();
  const locale = useLocale();
  const days = Array.from({ length: 42 }, (_, i) => addDays(from, i));
  const month = focus.slice(0, 7);
  const head = new Intl.DateTimeFormat(locale, { weekday: "short" });
  const byDay = new Map<string, CalendarEvent[]>();
  for (const e of events) {
    for (const d of daysCovered(e)) {
      const list = byDay.get(d) ?? [];
      list.push(e);
      byDay.set(d, list);
    }
  }
  for (const list of byDay.values()) list.sort((a, b) => Number(b.allDay) - Number(a.allDay) || span(a)[0] - span(b)[0]);
  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div className="grid grid-cols-7 border-b border-line">
        {days.slice(0, 7).map((d) => (
          <p key={d} className="px-2 py-1.5 text-[12px] text-muted">
            {head.format(dayDate(d))}
          </p>
        ))}
      </div>
      <div className="grid min-h-0 flex-1 grid-cols-7 grid-rows-6">
        {days.map((d) => {
          const list = byDay.get(d) ?? [];
          const more = list.length - 3;
          return (
            <div
              key={d}
              onDoubleClick={() => onNew(d)}
              className={cn("flex min-h-0 min-w-0 flex-col gap-0.5 overflow-hidden border-r border-b border-hair p-1", d.slice(0, 7) !== month && "bg-sunk/50")}
            >
              <button
                type="button"
                onClick={() => onDay(d)}
                aria-label={new Intl.DateTimeFormat(locale, { dateStyle: "full" }).format(dayDate(d))}
                className={cn(
                  "grid size-6 shrink-0 place-items-center rounded-full text-[12.5px] tabular-nums transition-colors hover:bg-sunk",
                  d === today && "bg-accent text-surface hover:bg-accent",
                  d.slice(0, 7) !== month && d !== today && "text-faint",
                )}
              >
                {dayDate(d).getDate()}
              </button>
              {list.slice(0, 3).map((e) => (
                <Chip key={`${e.id}-${e.occurrence ?? ""}`} event={e} calendar={calendars.get(e.calendar)} onOpen={onOpen} className="truncate px-1.5 py-0.5">
                  {!e.allDay && <span className="mr-1 text-muted tabular-nums">{timeText(span(e)[0], locale)}</span>}
                  <CallMark event={e} />
                  {e.summary || t("calendar.untitled")}
                </Chip>
              ))}
              {more > 0 && (
                <button type="button" onClick={() => onDay(d)} className="px-1.5 text-left text-[12px] text-muted hover:text-ink">
                  {t("calendar.more", { count: more })}
                </button>
              )}
            </div>
          );
        })}
      </div>
    </div>
  );
}

function Agenda({
  from,
  to,
  today,
  events,
  calendars,
  onOpen,
}: {
  from: string;
  to: string;
  today: string;
  events: CalendarEvent[];
  calendars: Map<string, CalendarInfo>;
  onOpen: (e: CalendarEvent) => void;
}) {
  const t = useT();
  const locale = useLocale();
  const groups: { day: string; list: CalendarEvent[] }[] = [];
  for (let d = from; d < to; d = addDays(d, 1)) {
    const list = events
      .filter((e) => (e.allDay ? daysCovered(e)[0] === d || (d === from && daysCovered(e).includes(d)) : dayOf(new Date(span(e)[0])) === d))
      .sort((a, b) => Number(b.allDay) - Number(a.allDay) || span(a)[0] - span(b)[0]);
    if (list.length > 0) groups.push({ day: d, list });
  }
  if (groups.length === 0) {
    return (
      <Empty className="m-4 border-0">
        <EmptyHeader>
          <EmptyMedia variant="icon">
            <HugeiconsIcon icon={Calendar03Icon} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
          </EmptyMedia>
          <EmptyDescription>{t("calendar.agendaEmpty")}</EmptyDescription>
        </EmptyHeader>
      </Empty>
    );
  }
  const dayName = new Intl.DateTimeFormat(locale, { weekday: "long", day: "numeric", month: "long" });
  return (
    <div className="min-h-0 flex-1 overflow-y-auto">
      {groups.map((g) => (
        <section key={g.day} aria-label={dayName.format(dayDate(g.day))}>
          <h3
            className={cn(
              "sticky top-0 z-10 border-b border-hair bg-surface/95 px-4 py-1.5 text-[12.5px] font-semibold backdrop-blur-sm",
              g.day === today ? "text-accent" : "text-muted",
            )}
          >
            {g.day === today ? `${t("calendar.today")} · ` : ""}
            {dayName.format(dayDate(g.day))}
          </h3>
          <ul className="flex flex-col gap-1 p-2">
            {g.list.map((e) => (
              <li key={`${e.id}-${e.occurrence ?? ""}`}>
                <Chip event={e} calendar={calendars.get(e.calendar)} onOpen={onOpen} className="flex items-center gap-3 px-3 py-2 text-sm">
                  <span className="w-24 shrink-0 text-[12.5px] text-muted tabular-nums">
                    {e.allDay ? t("calendar.allDay") : `${timeText(span(e)[0], locale)} – ${timeText(span(e)[1], locale)}`}
                  </span>
                  <span className="min-w-0 flex-1">
                    <span className="block truncate font-medium">
                      <CallMark event={e} />
                      {e.summary || t("calendar.untitled")}
                    </span>
                    {e.location !== undefined && <span className="block truncate text-[12.5px] text-muted">{e.location}</span>}
                  </span>
                  <Badge variant="secondary" className="hidden shrink-0 sm:inline-flex">
                    {calendars.get(e.calendar)?.name}
                  </Badge>
                </Chip>
              </li>
            ))}
          </ul>
        </section>
      ))}
    </div>
  );
}
