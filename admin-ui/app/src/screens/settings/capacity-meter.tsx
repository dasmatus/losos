import * as React from "react";
import { cn, setCssVar } from "@/lib/utils";
import { formatBytes } from "./format";
import type { StorageFacts, StorageSource } from "./use-storage";

/* The capacity meter: filled is yours, hatched is lent out, empty is free.
 *
 * One hue in the whole app, so "yours" and "lent to the mesh" cannot be two
 * colours. They are the same accent with a texture between them — the same
 * pairing the local/mesh badges use, so a reader who has learned it once on
 * a badge reads the bar without a legend. The legend is there anyway,
 * because a meter is the one place the distinction carries a number.
 *
 * Two widths are genuinely dynamic and no set of classes covers 0–100, so
 * they go on as custom properties through the CSSOM (setCssVar) and the
 * stylesheet reads them back with `w-[var(--losos-used,0%)]`. Not a style
 * attribute: `style-src 'self'` refuses those, silently, on a box with no
 * console anyone will ever read.
 *
 * When the box has not reported a capacity the bar does not guess. An empty
 * track with a sentence under it is worth more than a plausible-looking bar
 * that is decoration — this is the screen where a wrong number gets someone
 * to run a resize against a mounted filesystem.
 */

export interface CapacityMeterProps {
  facts: StorageFacts;
  source: StorageSource;
}

function percent(part: number, whole: number): number {
  if (whole <= 0) return 0;
  return Math.min(100, Math.max(0, (part / whole) * 100));
}

export function CapacityMeter({ facts, source }: CapacityMeterProps) {
  const usedBar = React.useRef<HTMLDivElement>(null);
  const lentBar = React.useRef<HTMLDivElement>(null);

  const total = facts.totalBytes;
  const measured = total !== null && total > 0;

  /* `usedBytes` is this box's own data and `lentBytes` is the mesh's copies
   * sitting alongside it. They stack: the bar is used, then lent, then the
   * remainder. A box reporting one and not the other still draws the half it
   * knows rather than nothing. */
  const used = measured ? (facts.usedBytes ?? 0) : 0;
  const lent = measured ? (facts.lentBytes ?? 0) : 0;
  const usedPct = measured ? percent(used, total) : 0;
  const lentPct = measured ? percent(lent, total) : 0;
  const freeBytes = measured ? Math.max(0, total - used - lent) : null;

  React.useEffect(() => {
    setCssVar(usedBar.current, "--losos-used", `${usedPct.toFixed(2)}%`);
    setCssVar(lentBar.current, "--losos-lent", `${lentPct.toFixed(2)}%`);
  }, [usedPct, lentPct]);

  const label = measured
    ? `${formatBytes(used)} in use by this box, ${formatBytes(lent)} holding copies for the mesh, ${formatBytes(freeBytes)} free of ${formatBytes(total)}.`
    : "This box has not reported how full its disk is.";

  return (
    <div className="flex flex-col gap-2.5">
      <div
        role="img"
        aria-label={label}
        className={cn(
          "flex h-3 w-full overflow-hidden rounded-full border border-line bg-sunk",
          !measured && "opacity-60",
        )}
      >
        <div
          ref={usedBar}
          className={cn(
            "h-full w-[var(--losos-used,0%)] bg-accent",
            "transition-[width] duration-500 ease-out",
          )}
        />
        <div
          ref={lentBar}
          className={cn(
            "hatched-solid h-full w-[var(--losos-lent,0%)]",
            "transition-[width] duration-500 ease-out",
          )}
        />
      </div>

      {measured ? (
        <ul className="flex flex-wrap gap-x-5 gap-y-1.5">
          <Key swatch="bg-accent" name="Yours" value={formatBytes(used)} />
          <Key swatch="hatched-solid" name="Lent to the mesh" value={formatBytes(lent)} />
          <Key swatch="bg-sunk border border-line" name="Free" value={formatBytes(freeBytes)} />
          <li className="ml-auto text-[12.5px] text-muted">
            <span className="numeric">{formatBytes(total)}</span> in total
          </li>
        </ul>
      ) : (
        <p className="text-[12.5px] leading-snug text-muted">{unmeasuredReason(source)}</p>
      )}
    </div>
  );
}

/* Why the bar is empty, in the reader's terms. Exhaustive on purpose: a new
 * StorageSource variant should stop the build here rather than fall through
 * to a sentence that happens to be wrong about it. */
function unmeasuredReason(source: StorageSource): string {
  switch (source) {
    case "loading":
      return "Asking this box how full its disk is…";
    case "error":
      return "This box did not answer when asked how full its disk is.";
    case "absent":
      return "This box cannot yet report how full its disk is, so there is nothing to draw here.";
    case "box":
      // Answered, but with no total. Nothing to divide by.
      return "This box answered without a size for its disk.";
    default: {
      const exhaustive: never = source;
      return exhaustive;
    }
  }
}

function Key({ swatch, name, value }: { swatch: string; name: string; value: string }) {
  return (
    <li className="flex items-center gap-2 text-[12.5px] text-muted">
      <span aria-hidden="true" className={cn("size-2.5 shrink-0 rounded-[3px]", swatch)} />
      {name}
      <span className="numeric text-ink">{value}</span>
    </li>
  );
}
