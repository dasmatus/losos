import { cn } from "@/lib/utils";
import { describeWindow, hourCells } from "./window";

/* Twenty-four cells, midnight to midnight, in the owner's own local hours.
 *
 * The hours are tinted with the same accent-and-texture pairing the rest of
 * the app uses for "this is the mesh's": a solid hour is one the mesh gets
 * in full, a hatched one is an hour the window only partly covers (a window
 * like 23:30→06:45 has a ragged cell at each end, and rounding it to a whole
 * hour would paint half an hour this box never actually lends).
 *
 * One accessible name for the whole strip, not twenty-four. A screen reader
 * reading out "hour 0 shared, hour 1 shared, hour 2 shared" twenty-four times
 * conveys less than the sentence describeWindow() already has to produce for
 * the caption, so the cells are aria-hidden and the strip is a labelled
 * role="img".
 */

export interface HourStripProps {
  start: string;
  end: string;
  /** Grey the whole strip out: sharing is switched off, or the box is busy. */
  muted?: boolean;
}

/** A tick under the strip every six hours — enough to orient by, few enough
 *  that the labels do not collide at a phone's width. */
const TICK_EVERY = 6;

export function HourStrip({ start, end, muted = false }: HourStripProps) {
  const cells = hourCells(start, end);

  return (
    <div className={cn("flex flex-col gap-1.5", muted && "opacity-50")}>
      <div
        role="img"
        aria-label={describeWindow(start, end)}
        className="flex h-9 w-full gap-px overflow-hidden rounded-control border border-line p-px"
      >
        {cells.map((cell) => (
          <div
            key={cell.hour}
            aria-hidden="true"
            className={cn(
              "h-full flex-1 rounded-[2px]",
              "transition-colors duration-200 ease-out",
              /* An unshared hour is --sunk, not --surface. The strip sits on a
               * card that is already --surface, and a cell painted the colour
               * of the thing behind it is not a cell — the owner's own hours
               * have to read as kept, not as absent. */
              cell.partial ? "hatched" : cell.shared ? "hatched-solid" : "bg-sunk",
            )}
          />
        ))}
      </div>

      {/* The ticks are their own 24-column grid rather than children of the
          cells: a label wider than a cell would otherwise force that cell
          open and the strip would stop being twenty-four equal hours. */}
      <div
        aria-hidden="true"
        className="grid grid-cols-[repeat(24,minmax(0,1fr))] text-[11px] text-faint"
      >
        {cells.map((cell) => (
          <span key={cell.hour} className="numeric text-center">
            {cell.hour % TICK_EVERY === 0 ? String(cell.hour).padStart(2, "0") : ""}
          </span>
        ))}
      </div>
    </div>
  );
}
