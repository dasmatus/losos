/* The contribution-graph heatmap: 53 columns, 7 rows, a year of days.
 *
 * THREE cell states, not two. That is the whole design.
 *
 *   answering   — the accent, at one of five opacities
 *   quiet       — --sunk, a filled square in the background colour, so an
 *                 outage reads as a hole punched in the year
 *   not watched — a hairline outline and nothing inside
 *
 * The third one exists because this journal is kept by the browser, not by
 * the box (see widgets/metrics.ts). A day nobody had this page open is a day
 * with no reading, and drawing it the same as a day the box was down would
 * turn "I was on holiday" into "my box was off for a fortnight". Distinct
 * shapes; the legend names both.
 *
 * Opacity carries intensity and nothing else carries colour: there is one
 * accent in this app, and a green-to-red ramp would be inventing a second
 * meaning for hue on the one tile most likely to be looked at.
 */

import * as React from "react";
import { ScrollArea } from "@/components/ui/scroll-area";
import { cn, setCssVar } from "@/lib/utils";
import { DASH, formatLongDate, parseDayKey } from "../format";
import type { HeatmapData, HeatmapDay } from "../types";
import "../widgets.css";

/** 53 weeks is the widest the grid goes — a year plus the ragged ends. */
const MAX_COLUMNS = 53;
const ROWS = 7;

/* Monday first. The box ships Europe/Berlin and the owner is reading a
 * calendar, not a GitHub profile; a week that starts on Sunday looks wrong to
 * most of the people this appliance is for. */
const WEEKDAYS = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"] as const;
/** Only three labels fit beside 10px rows without crowding. */
const SHOWN_WEEKDAYS = new Set([0, 2, 4]);

const MONTHS = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

// ── Intensity ─────────────────────────────────────────────────────────────

/* Five steps, and they are NOT evenly spaced.
 *
 * Uptime lives in the top percent. Linear buckets would put every real day in
 * the darkest bin and the graph would be one flat block — which is a graph
 * that has stopped saying anything. The thresholds below spend four of the
 * five steps between 90% and 100%, where the differences an owner cares
 * about actually are.
 *
 * Spelled out as whole class names because Tailwind only emits what it can
 * read in the source; a computed `bg-accent/${n}` produces no CSS at all. */
const LEVELS = [
  "bg-accent/25",
  "bg-accent/45",
  "bg-accent/65",
  "bg-accent/85",
  "bg-accent",
] as const;

function levelOf(value: number): number {
  if (value >= 0.9999) return 4;
  if (value >= 0.999) return 3;
  if (value >= 0.99) return 2;
  if (value >= 0.9) return 1;
  return 0;
}

type CellKind = "none" | "quiet" | "up" | "future";

interface Cell {
  key: string;
  date: string;
  kind: CellKind;
  level: number;
  label: string;
}

function cellClass(cell: Cell): string {
  switch (cell.kind) {
    case "future":
      // Present in the grid so the columns line up, invisible because a day
      // that has not happened is not a reading.
      return "bg-transparent";
    case "none":
      return "bg-transparent border border-hair";
    case "quiet":
      return "bg-sunk";
    case "up":
      return LEVELS[cell.level] ?? "bg-accent";
  }
}

// ── Grid construction ─────────────────────────────────────────────────────

function startOfWeek(date: Date): Date {
  // getDay() is Sunday-0; shift so Monday is 0.
  const offset = (date.getDay() + 6) % 7;
  const out = new Date(date.getFullYear(), date.getMonth(), date.getDate());
  out.setDate(out.getDate() - offset);
  return out;
}

function addDays(date: Date, count: number): Date {
  const out = new Date(date.getFullYear(), date.getMonth(), date.getDate());
  out.setDate(out.getDate() + count);
  return out;
}

function key(date: Date): string {
  const month = `${date.getMonth() + 1}`.padStart(2, "0");
  const day = `${date.getDate()}`.padStart(2, "0");
  return `${date.getFullYear()}-${month}-${day}`;
}

interface Grid {
  columns: Cell[][];
  /** Column index -> month label, for the columns where a month begins. */
  months: (string | null)[];
}

/* Build the grid from the DATES in the data, not from a fixed window.
 *
 * A custom widget may hand over thirty days; laying those out on a 53-column
 * frame would be fifty columns of nothing with a smear at the end. The grid
 * spans what it was given, rounded out to whole weeks and capped at 53
 * columns. */
function buildGrid(days: readonly HeatmapDay[]): Grid {
  const byDate = new Map<string, HeatmapDay>();
  let min: Date | null = null;
  let max: Date | null = null;

  for (const day of days) {
    const parsed = parseDayKey(day.date);
    if (parsed === null) continue;
    byDate.set(day.date, day);
    if (min === null || parsed < min) min = parsed;
    if (max === null || parsed > max) max = parsed;
  }

  const today = new Date();
  const last = max ?? today;
  const first = min ?? addDays(last, -(MAX_COLUMNS * ROWS - 1));

  const gridEnd = addDays(startOfWeek(last), ROWS - 1);
  let columns = Math.ceil((gridEnd.getTime() - startOfWeek(first).getTime()) / 86_400_000 / ROWS);
  columns = Math.min(MAX_COLUMNS, Math.max(1, columns));
  const gridStart = addDays(gridEnd, -(columns * ROWS - 1));

  const todayKey = key(today);
  const out: Cell[][] = [];
  const months: (string | null)[] = [];
  let lastMonth = -1;

  for (let column = 0; column < columns; column += 1) {
    const cells: Cell[] = [];
    for (let row = 0; row < ROWS; row += 1) {
      const date = addDays(gridStart, column * ROWS + row);
      const stamp = key(date);
      const record = byDate.get(stamp);

      let kind: CellKind = "none";
      let level = 0;
      let label: string;

      if (stamp > todayKey) {
        kind = "future";
        label = "";
      } else if (record === undefined || record.value === null) {
        kind = "none";
        label = `${formatLongDate(stamp)}: not watched`;
      } else if (record.value <= 0) {
        kind = "quiet";
        label = `${formatLongDate(stamp)}: did not answer`;
      } else {
        kind = "up";
        level = levelOf(record.value);
        label = `${formatLongDate(stamp)}: ${(record.value * 100).toFixed(1)}% answered`;
      }

      if (record?.note !== undefined) label = `${formatLongDate(stamp)}: ${record.note}`;

      cells.push({ key: stamp, date: stamp, kind, level, label });
    }

    // The month label belongs on the column whose FIRST row starts a new
    // month; anything else drifts a week either side of the boundary.
    const columnStart = addDays(gridStart, column * ROWS);
    const month = columnStart.getMonth();
    if (month !== lastMonth && columns > 6) {
      months.push(MONTHS[month] ?? null);
      lastMonth = month;
    } else {
      months.push(null);
    }

    out.push(cells);
  }

  return { columns: out, months };
}

// ── The view ──────────────────────────────────────────────────────────────

export interface HeatmapViewProps {
  data: HeatmapData;
  /** The tile's summary line. Also the graphic's accessible name — a screen
   *  reader gets one sentence rather than 371 unlabelled squares. */
  summary?: string;
}

export function HeatmapView({ data, summary }: HeatmapViewProps) {
  const grid = React.useMemo(() => buildGrid(data.days), [data.days]);

  const counts = React.useMemo(() => {
    let watched = 0;
    let quiet = 0;
    for (const day of data.days) {
      if (day.value === null) continue;
      watched += 1;
      if (day.value <= 0) quiet += 1;
    }
    return { watched, quiet };
  }, [data.days]);

  return (
    <div className="flex flex-col gap-3">
      <ScrollArea orientation="horizontal" className="-mx-1 px-1 pb-1">
        <div
          role="img"
          aria-label={summary ?? "Daily record"}
          className="inline-flex w-max gap-2"
        >
          <WeekdayRail />

          <div className="flex flex-col gap-1">
            <MonthRail months={grid.months} />
            <div className="flex gap-[3px]">
              {grid.columns.map((cells, index) => (
                <Column key={cells[0]?.key ?? index} cells={cells} index={index} />
              ))}
            </div>
          </div>
        </div>
      </ScrollArea>

      <Legend
        low={data.lowLabel ?? "Quiet"}
        high={data.highLabel ?? "Answering"}
        watched={counts.watched}
        quiet={counts.quiet}
      />
    </div>
  );
}

function Column({ cells, index }: { cells: Cell[]; index: number }) {
  return (
    <div
      ref={(element) => {
        // The stagger. A number, not a duration: the stylesheet multiplies it
        // by the per-column step, so the pacing stays in CSS.
        setCssVar(element, "--heat-i", String(index));
      }}
      className="heat-col flex flex-col gap-[3px]"
    >
      {cells.map((cell) => (
        <div
          key={cell.key}
          // aria-hidden: the grid above carries one label. 371 announcements
          // is not an accessible graph, it is a denial of service on a reader.
          aria-hidden="true"
          title={cell.label.length > 0 ? cell.label : undefined}
          className={cn("heat-cell size-2.5 rounded-[2px]", cellClass(cell))}
        />
      ))}
    </div>
  );
}

function WeekdayRail() {
  return (
    <div className="flex flex-col gap-[3px] pt-[22px]" aria-hidden="true">
      {WEEKDAYS.map((day, index) => (
        <div key={day} className="flex h-2.5 items-center text-[9px] leading-none text-faint">
          {SHOWN_WEEKDAYS.has(index) ? day : ""}
        </div>
      ))}
    </div>
  );
}

function MonthRail({ months }: { months: (string | null)[] }) {
  return (
    <div className="flex h-3.5 gap-[3px]" aria-hidden="true">
      {months.map((month, index) => (
        <div
          key={index}
          // relative + absolute so a three-letter label may overhang its
          // 10px column without widening the grid under it.
          className="relative w-2.5"
        >
          {month !== null && (
            <span className="absolute top-0 left-0 text-[9px] leading-none whitespace-nowrap text-faint">
              {month}
            </span>
          )}
        </div>
      ))}
    </div>
  );
}

function Legend({
  low,
  high,
  watched,
  quiet,
}: {
  low: string;
  high: string;
  watched: number;
  quiet: number;
}) {
  return (
    <div className="flex flex-wrap items-center gap-x-4 gap-y-2 text-[11px] text-faint">
      <div className="flex items-center gap-1.5">
        <span>{low}</span>
        <span className="size-2.5 rounded-[2px] bg-sunk" aria-hidden="true" />
        {LEVELS.map((level) => (
          <span key={level} className={cn("size-2.5 rounded-[2px]", level)} aria-hidden="true" />
        ))}
        <span>{high}</span>
      </div>

      <div className="flex items-center gap-1.5">
        <span className="size-2.5 rounded-[2px] border border-hair" aria-hidden="true" />
        <span>Not watched</span>
      </div>

      <div className="numeric ml-auto">
        {watched === 0 ? DASH : `${watched} watched · ${quiet} quiet`}
      </div>
    </div>
  );
}
