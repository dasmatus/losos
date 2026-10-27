/* The last hour as sixty bars, one per minute, oldest on the left.
 *
 * Inline SVG with the geometry in attributes (x, y, width, height), never a
 * style, so the admin page's `style-src 'self'` has nothing to refuse. A
 * minute's server errors are the crit-coloured foot of its bar; a minute the
 * ring has not seen yet (Traefik started less than an hour ago) is a faint
 * stub, which is a different thing from a minute with no requests. */

import { fmt } from "@/widgets/format";
import { useLocale, useT } from "@/lib/i18n-react";
import { cn } from "@/lib/utils";
import type { StatisticsMinute } from "./types";

const SLOTS = 60;
const SLOT = 10;
const BAR = 7;

export function MinuteChart({
  minutes,
  height = 64,
  className,
}: {
  minutes: readonly StatisticsMinute[];
  /** The drawing's own height in viewBox units; the element stretches. */
  height?: number;
  className?: string;
}) {
  const t = useT();
  useLocale();
  const shown = minutes.slice(-SLOTS);
  const missing = SLOTS - shown.length;
  const peak = shown.reduce((max, m) => (m.requests > max ? m.requests : max), 0);
  const total = shown.reduce((sum, m) => sum + m.requests, 0);
  const usable = height - 2;

  return (
    <svg
      viewBox={`0 0 ${SLOTS * SLOT} ${height}`}
      preserveAspectRatio="none"
      role="img"
      aria-label={t("statistics.chart.label", { n: fmt.number(total) })}
      className={cn("block w-full", className)}
    >
      {Array.from({ length: missing }, (_, index) => (
        <rect
          key={`gap-${index}`}
          x={index * SLOT}
          y={height - 1}
          width={BAR}
          height={1}
          className="fill-hair"
        />
      ))}
      {shown.map((minute, index) => {
        const x = (missing + index) * SLOT;
        const tall = peak > 0 ? Math.max(minute.requests > 0 ? 2 : 1, (minute.requests / peak) * usable) : 1;
        const errors =
          minute.requests > 0 ? Math.min(tall, (minute.serverErrors / minute.requests) * tall) : 0;
        const label = t("statistics.chart.minute", {
          time: fmt.time(minute.at),
          requests: fmt.number(minute.requests),
          errors: fmt.number(minute.serverErrors),
        });
        return (
          <g key={minute.at}>
            <title>{label}</title>
            <rect
              x={x}
              y={height - tall}
              width={BAR}
              height={tall}
              rx={1.5}
              className={minute.requests > 0 ? "fill-accent" : "fill-line"}
            />
            {errors > 0 && (
              <rect
                x={x}
                y={height - Math.max(1.5, errors)}
                width={BAR}
                height={Math.max(1.5, errors)}
                className="fill-crit"
              />
            )}
          </g>
        );
      })}
    </svg>
  );
}
