/* Rows: a label, an optional detail, an optional badge.
 *
 * A real <ul>, so a screen reader announces how many rows there are before
 * reading any of them — which on a tile called "Apps" is the first thing
 * anyone wants to know.
 *
 * The badge is the local/mesh pair again: filled accent for what runs on this
 * box, the same accent hatched for what runs on the mesh. Tone is separate
 * and only ever state — a row that is not answering gets a red dot, never a
 * red badge, because the badge slot means WHERE and the dot means HOW.
 */

import { StatusDot, Badge } from "@/components/ui/badge";
import { cn } from "@/lib/utils";
import type { ListData, Texture, Tone } from "../types";

const DOT_STATE: Record<Tone, "ok" | "warn" | "crit" | "idle"> = {
  neutral: "idle",
  ok: "ok",
  warn: "warn",
  crit: "crit",
};

const BADGE_VARIANT: Record<Texture, "local" | "mesh" | "neutral"> = {
  local: "local",
  mesh: "mesh",
  none: "neutral",
};

/** Rows beyond this are not drawn. A tile is a glance, not a table. */
const VISIBLE_ROWS = 8;

export function ListView({ data }: { data: ListData }) {
  if (data.items.length === 0) {
    return (
      <p className="text-[12.5px] leading-snug text-faint">
        {data.empty ?? "Nothing to show yet."}
      </p>
    );
  }

  const shown = data.items.slice(0, VISIBLE_ROWS);
  const hidden = data.items.length - shown.length;

  return (
    <div className="flex flex-col gap-2">
      <ul className="flex flex-col">
        {shown.map((item, index) => (
          <li
            key={`${item.label}-${index}`}
            className={cn(
              "flex items-center gap-2.5 border-b border-hair py-2 last:border-b-0",
              "animate-fade-in",
            )}
          >
            {item.tone !== undefined && item.tone !== "neutral" && (
              <StatusDot state={DOT_STATE[item.tone]} />
            )}

            <div className="min-w-0 flex-1">
              <p className="truncate text-[13.5px] leading-snug text-ink">{item.label}</p>
              {item.detail !== undefined && (
                <p className="truncate text-[12px] leading-snug text-muted">{item.detail}</p>
              )}
            </div>

            {item.badge !== undefined && (
              <Badge variant={BADGE_VARIANT[item.texture ?? "none"]}>{item.badge}</Badge>
            )}
          </li>
        ))}
      </ul>

      {hidden > 0 && (
        <p className="text-[11.5px] text-faint">
          {hidden} more not shown.
        </p>
      )}
    </div>
  );
}
