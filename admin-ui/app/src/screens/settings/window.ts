/* The compute window, as arithmetic. Pure — no React, no DOM.
 *
 * This is a transcription of `in_window()` in modules/edge.nix, and the two
 * have to agree or the strip in the Mesh pane paints hours the edge will not
 * actually hand out:
 *
 *     if [ "$s" -le "$e" ]; then
 *       [ "$now" -ge "$s" ] && [ "$now" -lt "$e" ]
 *     else
 *       [ "$now" -ge "$s" ] || [ "$now" -lt "$e" ]
 *     fi
 *
 * Three consequences the picture has to carry:
 *
 *   1. The end is EXCLUSIVE. A 23:00→07:00 window stops at 07:00 sharp, so
 *      the cell labelled 07 is the owner's again.
 *   2. end < start wraps midnight, and that is the normal case: the feature
 *      is "share my box while I sleep".
 *   3. start == end shares NOTHING. `now >= s && now < e` cannot both hold,
 *      so an owner who sets both bounds the same has turned sharing off
 *      without meaning to — which is why `describeWindow` says so in words
 *      instead of leaving an evenly grey strip to be read as "all day".
 *
 * And the zone: these bounds are the OWNER's local time. The edge evaluates
 * them under `TZ="$tz" date` with the zone that travelled alongside them,
 * precisely because the edge's own clock is not the owner's. Nothing here
 * converts anything — the strip is drawn in the same local hours the fields
 * hold, which is the only reading that stays true after a DST change.
 */

/** Minutes since local midnight, or null if the value is not HH:MM. */
export function minutesOf(hhmm: string): number | null {
  const match = /^([01][0-9]|2[0-3]):([0-5][0-9])$/.exec(hhmm);
  if (match === null) return null;
  const hours = Number(match[1]);
  const minutes = Number(match[2]);
  return hours * 60 + minutes;
}

/** Half-open [from, to) in minutes since midnight. */
interface Span {
  from: number;
  to: number;
}

const DAY = 24 * 60;

/* The window as one or two half-open spans on a single day's timeline.
 * Two when it wraps midnight, none when the bounds coincide. */
function spansOf(start: number, end: number): readonly Span[] {
  if (start === end) return [];
  if (start < end) return [{ from: start, to: end }];
  return [
    { from: start, to: DAY },
    { from: 0, to: end },
  ];
}

/** Minutes of overlap between [from, to) and the window's spans. */
function overlapMinutes(spans: readonly Span[], from: number, to: number): number {
  let total = 0;
  for (const span of spans) {
    const lo = Math.max(span.from, from);
    const hi = Math.min(span.to, to);
    if (hi > lo) total += hi - lo;
  }
  return total;
}

/** One cell of the 24-hour strip: the hour, and how much of it is lent out. */
export interface HourCell {
  /** 0–23, local. */
  hour: number;
  /** The hour is lent out, in whole or in part. */
  shared: boolean;
  /** Lent out for some of the hour but not all of it — the ragged ends of a
   *  window like 23:30→06:45. Painted lighter so the edge is visible. */
  partial: boolean;
}

/** Minutes a day this box lends, 0–1440. Wraps midnight the edge's way. */
export function sharedMinutes(start: number, end: number): number {
  let total = 0;
  for (const span of spansOf(start, end)) total += span.to - span.from;
  return total;
}

/* The 24 cells, always 24 and always in local order 00…23.
 *
 * A malformed bound gives an all-unshared strip rather than an exception:
 * the fields are validated separately and say so themselves, and a picture
 * that throws takes the pane with it. */
export function hourCells(startHhmm: string, endHhmm: string): readonly HourCell[] {
  const start = minutesOf(startHhmm);
  const end = minutesOf(endHhmm);
  const spans = start === null || end === null ? [] : spansOf(start, end);

  const cells: HourCell[] = [];
  for (let hour = 0; hour < 24; hour += 1) {
    const covered = overlapMinutes(spans, hour * 60, hour * 60 + 60);
    cells.push({ hour, shared: covered > 0, partial: covered > 0 && covered < 60 });
  }
  return cells;
}

/** What the window does, in a sentence — also the strip's accessible name. */
export function describeWindow(startHhmm: string, endHhmm: string): string {
  const start = minutesOf(startHhmm);
  const end = minutesOf(endHhmm);
  if (start === null || end === null) return "The hours are not set.";
  if (start === end) {
    return "Nothing is shared: the window starts and ends at the same minute.";
  }

  const total = sharedMinutes(start, end);
  const hours = total / 60;
  const length =
    Number.isInteger(hours) && hours >= 1
      ? `${hours} ${hours === 1 ? "hour" : "hours"}`
      : `${Math.round(total)} minutes`;
  const wraps = start > end ? ", across midnight" : "";
  return `Shared from ${startHhmm} until ${endHhmm}${wraps}. That is ${length} a day, your local time.`;
}
