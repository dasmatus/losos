/* A figure and the bar under it.
 *
 * The bar is TWO segments of one accent. Filled is what is used, or what is
 * this box's own; hatched is what is held back, or what is lent to the mesh.
 * That pairing is the app's whole visual grammar for "here" versus "shared",
 * and it is why there is no second hue on this board: a colour would have to
 * mean something, and ok/warn/crit already mean state.
 *
 * Both widths are genuinely continuous, so they arrive as percentages on
 * `--losos-fill` and `--losos-reserve` through the CSSOM (setCssVar) and the
 * stylesheet reads them back. Not `style={{ width }}` — this app does not
 * write style attributes, because the appliance CSP does not accept them.
 */

import * as React from "react";
import { Badge } from "@/components/ui/badge";
import { cn, setCssVar } from "@/lib/utils";
import { applyFormat, DASH } from "../format";
import type { BarData, Tone } from "../types";
import "../widgets.css";

const TONE_VARIANT: Record<Tone, "neutral" | "ok" | "warn" | "crit"> = {
  neutral: "neutral",
  ok: "ok",
  warn: "warn",
  crit: "crit",
};

function percent(part: number, whole: number): string {
  if (!Number.isFinite(part) || !Number.isFinite(whole) || whole <= 0) return "0%";
  return `${Math.min(100, Math.max(0, (part / whole) * 100)).toFixed(2)}%`;
}

export function BarView({ data }: { data: BarData }) {
  const fill = React.useRef<HTMLDivElement>(null);
  const reserve = React.useRef<HTMLDivElement>(null);

  const value = data.value ?? 0;
  const held = data.reserve ?? 0;
  const max = data.max > 0 ? data.max : value + held;

  const fillWidth = percent(value, max);
  const reserveWidth = percent(held, max);

  React.useEffect(() => {
    setCssVar(fill.current, "--losos-fill", data.value === null ? "0%" : fillWidth);
    setCssVar(reserve.current, "--losos-reserve", reserveWidth);
  }, [fillWidth, reserveWidth, data.value]);

  const headline = data.value === null ? DASH : applyFormat(data.value, data.format);

  return (
    <div className="flex flex-col gap-2.5">
      <div className="flex flex-wrap items-baseline gap-x-3 gap-y-1">
        <span
          className={cn(
            "numeric text-[34px] leading-none font-semibold tracking-tight",
            data.value === null ? "text-faint" : "text-ink",
          )}
        >
          {headline}
        </span>
        {data.max > 0 && (
          <span className="numeric text-[13px] text-faint">
            of {applyFormat(max, data.format)}
          </span>
        )}
        {data.chip !== undefined && (
          <Badge variant={TONE_VARIANT[data.chip.tone ?? "neutral"]}>{data.chip.text}</Badge>
        )}
      </div>

      <div
        role="img"
        aria-label={barLabel(data, max)}
        className="flex h-2.5 w-full overflow-hidden rounded-full bg-sunk"
      >
        <div ref={fill} className="bar-seg bar-fill h-full bg-accent" />
        {/* Same accent, textured. The hatch is the whole distinction. */}
        <div ref={reserve} className="bar-seg bar-reserve hatched-solid h-full" />
      </div>

      {(data.fillLabel !== undefined || data.reserveLabel !== undefined) && (
        <div className="flex flex-wrap items-center gap-x-4 gap-y-1 text-[11.5px] text-muted">
          {data.fillLabel !== undefined && (
            <span className="inline-flex items-center gap-1.5">
              <span className="size-2.5 rounded-[2px] bg-accent" aria-hidden="true" />
              {data.fillLabel}
            </span>
          )}
          {data.reserveLabel !== undefined && (
            <span className="inline-flex items-center gap-1.5">
              <span className="hatched-solid size-2.5 rounded-[2px]" aria-hidden="true" />
              {data.reserveLabel}
            </span>
          )}
        </div>
      )}

      {data.caption !== undefined && (
        <p className="text-[12.5px] leading-snug text-muted">{data.caption}</p>
      )}
    </div>
  );
}

/* The bar's only channel for a screen reader. Spelled as a sentence rather
 * than a percentage, because "68%" without saying of what is not a reading. */
function barLabel(data: BarData, max: number): string {
  const parts: string[] = [];
  const fillName = data.fillLabel ?? "Used";
  parts.push(
    data.value === null
      ? `${fillName}: not measured`
      : `${fillName}: ${applyFormat(data.value, data.format)} of ${applyFormat(max, data.format)}`,
  );
  if (data.reserve !== undefined && data.reserve > 0) {
    parts.push(`${data.reserveLabel ?? "Held back"}: ${applyFormat(data.reserve, data.format)}`);
  }
  return parts.join(". ");
}
