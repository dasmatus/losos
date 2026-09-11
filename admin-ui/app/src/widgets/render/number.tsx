/* One figure, set large.
 *
 * Monospace with tabular numerals, because two number tiles side by side
 * whose digits do not line up look like a table that has come apart. `numeric`
 * is the app-wide utility for that; it is not restated here.
 *
 * `null` draws an em-dash. That is a deliberate, visible state: nothing on
 * this board invents a figure it was not given, and a tile reading "—" with a
 * caption saying why is the honest rendering of a measurement this box does
 * not publish.
 */

import { Badge } from "@/components/ui/badge";
import { cn } from "@/lib/utils";
import { applyFormat, DASH } from "../format";
import type { NumberData, Tone } from "../types";

const TONE_VARIANT: Record<Tone, "neutral" | "ok" | "warn" | "crit"> = {
  neutral: "neutral",
  ok: "ok",
  warn: "warn",
  crit: "crit",
};

export function NumberView({ data }: { data: NumberData }) {
  const text = data.value === null ? DASH : applyFormat(data.value, data.format);

  return (
    <div className="flex flex-col gap-2">
      <div className="flex flex-wrap items-baseline gap-x-3 gap-y-1">
        <span
          className={cn(
            "numeric text-[34px] leading-none font-semibold tracking-tight",
            data.value === null ? "text-faint" : "text-ink",
          )}
        >
          {text}
        </span>
        {data.chip !== undefined && (
          <Badge variant={TONE_VARIANT[data.chip.tone ?? "neutral"]}>{data.chip.text}</Badge>
        )}
      </div>

      {data.caption !== undefined && (
        <p className="text-[12.5px] leading-snug text-muted">{data.caption}</p>
      )}

      {data.trend !== undefined && data.trend.length > 1 && <Sparkline values={data.trend} />}
    </div>
  );
}

/* A sparkline is not a fifth widget type: a number may carry one, and that is
 * the only place a line chart appears on this board.
 *
 * Inline SVG with the geometry in the `points` attribute — an attribute, not
 * a style, so nothing here goes near the CSP's `style-src`. The viewBox is a
 * fixed 100x24 grid and `preserveAspectRatio="none"` stretches it to whatever
 * width the tile is; the stroke is vector-effect'd so it does not stretch with
 * it and end up three pixels thick on a wide card. */
function Sparkline({ values }: { values: readonly number[] }) {
  const span = values.length - 1;
  const points = values
    .map((value, index) => {
      const x = (index / span) * 100;
      const clamped = Math.min(1, Math.max(0, Number.isFinite(value) ? value : 0));
      const y = 24 - clamped * 22 - 1;
      return `${x.toFixed(2)},${y.toFixed(2)}`;
    })
    .join(" ");

  return (
    <svg
      viewBox="0 0 100 24"
      preserveAspectRatio="none"
      className="h-6 w-full text-accent"
      aria-hidden="true"
      focusable="false"
    >
      <polyline
        points={points}
        fill="none"
        stroke="currentColor"
        strokeWidth={1.5}
        strokeLinecap="round"
        strokeLinejoin="round"
        vectorEffect="non-scaling-stroke"
      />
    </svg>
  );
}
