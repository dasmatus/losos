/* Times in LosOS Calendar. The box keeps an event's start as the wall clock
 * it was written in plus its zone (backend/src/ical.rs `Time`), and has no
 * zone database; the browser does. Here a wall clock in a zone becomes an
 * instant, and an instant becomes the wall clock of whoever is looking.
 *
 * Days are "YYYY-MM-DD" strings throughout; an all-day event's end day is
 * the day after its last, as in iCalendar. */

export const DAY_MS = 86_400_000;

/** The viewer's own zone, as the browser names it. */
export function viewerZone(): string {
  return Intl.DateTimeFormat().resolvedOptions().timeZone || "UTC";
}

const pad = (n: number, w = 2) => String(n).padStart(w, "0");

/** A day string from a year, month (1-12) and day. */
export function dayString(y: number, m: number, d: number): string {
  return `${pad(y, 4)}-${pad(m)}-${pad(d)}`;
}

/** The day string a Date falls on in the viewer's zone. */
export function dayOf(date: Date): string {
  return dayString(date.getFullYear(), date.getMonth() + 1, date.getDate());
}

/** A day string as local midnight. */
export function dayDate(day: string): Date {
  const [y, m, d] = day.split("-").map(Number);
  return new Date(y ?? 1970, (m ?? 1) - 1, d ?? 1);
}

/** `n` days after `day`, by the calendar, not by 24 hours. */
export function addDays(day: string, n: number): string {
  const d = dayDate(day);
  d.setDate(d.getDate() + n);
  return dayOf(d);
}

/** The Monday on or before `day`. */
export function weekStart(day: string): string {
  const d = dayDate(day);
  return addDays(day, -((d.getDay() + 6) % 7));
}

/** The first of the month `day` is in. */
export function monthStart(day: string): string {
  return `${day.slice(0, 8)}01`;
}

/** The first of the month `n` months after `day`'s. */
export function addMonths(day: string, n: number): string {
  const d = dayDate(monthStart(day));
  d.setMonth(d.getMonth() + n);
  return dayOf(d);
}

/** Two-letter iCalendar day names, Monday first. */
export const WEEKDAYS = ["MO", "TU", "WE", "TH", "FR", "SA", "SU"] as const;

/** `day`'s iCalendar name. */
export function weekdayCode(day: string): (typeof WEEKDAYS)[number] {
  return WEEKDAYS[(dayDate(day).getDay() + 6) % 7] ?? "MO";
}

/* Parts of a wall clock string: "2026-10-22" or "2026-10-22T08:00[:00]". */
function parts(local: string): [number, number, number, number, number, number] {
  const m = /^(\d{4})-(\d{2})-(\d{2})(?:T(\d{2}):(\d{2})(?::(\d{2}))?)?/.exec(local);
  if (m === null) return [1970, 1, 1, 0, 0, 0];
  return [Number(m[1]), Number(m[2]), Number(m[3]), Number(m[4] ?? 0), Number(m[5] ?? 0), Number(m[6] ?? 0)];
}

const offsetFormats = new Map<string, Intl.DateTimeFormat>();

/** How far `zone`'s wall clock is ahead of UTC at `ms`, in milliseconds. */
export function zoneOffset(zone: string, ms: number): number {
  let f = offsetFormats.get(zone);
  if (f === undefined) {
    try {
      f = new Intl.DateTimeFormat("en-US", {
        timeZone: zone,
        hourCycle: "h23",
        year: "numeric",
        month: "2-digit",
        day: "2-digit",
        hour: "2-digit",
        minute: "2-digit",
        second: "2-digit",
      });
    } catch {
      return 0;
    }
    offsetFormats.set(zone, f);
  }
  const p: Record<string, number> = {};
  for (const part of f.formatToParts(new Date(ms))) {
    if (part.type !== "literal") p[part.type] = Number(part.value);
  }
  const wall = Date.UTC(p.year ?? 1970, (p.month ?? 1) - 1, p.day ?? 1, p.hour ?? 0, p.minute ?? 0, p.second ?? 0);
  return wall - (ms - (ms % 1000));
}

/** The instant a wall clock names: in `zone`, in UTC, or (no zone) in the
 *  viewer's own, which is what a floating time and an all-day day mean. */
export function instant(local: string, zone?: string): number {
  const [y, mo, d, h, mi, s] = parts(local);
  if (zone === undefined || zone === "") return new Date(y, mo - 1, d, h, mi, s).getTime();
  const wall = Date.UTC(y, mo - 1, d, h, mi, s);
  if (zone === "UTC") return wall;
  // Twice: the offset at the guess can differ from the one at the answer
  // around a change of clocks.
  let at = wall - zoneOffset(zone, wall);
  at = wall - zoneOffset(zone, at);
  return at;
}

/** The viewer's wall clock at `ms`, as the editor writes it: "2026-10-22T08:00". */
export function localInput(ms: number): string {
  const d = new Date(ms);
  return `${dayOf(d)}T${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

/** Minutes east of UTC in the viewer's zone on `day`. */
export function offsetOn(day: string): number {
  return -dayDate(day).getTimezoneOffset();
}

/** An event's span as instants, for the grids. An all-day event runs from
 *  the viewer's midnight on its first day to midnight after its last. */
export function span(e: { start: string; end: string; zone?: string; allDay: boolean }): [number, number] {
  if (e.allDay) return [instant(e.start), instant(e.end)];
  const start = instant(e.start, e.zone);
  return [start, Math.max(instant(e.end, e.zone), start)];
}

/** The days, as strings, an event covers in the viewer's calendar. */
export function daysCovered(e: { start: string; end: string; zone?: string; allDay: boolean }): string[] {
  const [a, b] = span(e);
  const first = dayOf(new Date(a));
  // An event ending at midnight does not reach into the next day.
  const last = dayOf(new Date(Math.max(a, b - 1)));
  const out: string[] = [];
  for (let d = first; d <= last && out.length < 400; d = addDays(d, 1)) out.push(d);
  return out;
}
