/* What the gallery offers, and how often each tile asks again.
 *
 * The split from lib/widgets.ts is deliberate: that module persists a board
 * and knows only ids, this one knows what an id draws. Persistence therefore
 * does not drag five widget implementations, the metric loader and the icon
 * set into anything that just wants to read the board back out of storage.
 */

import type { IconSvgElement } from "@hugeicons/react";
import {
  Calendar03Icon,
  HardDriveIcon,
  LayoutGridIcon,
  Share08Icon,
  Timer02Icon,
} from "@hugeicons/core-free-icons";
import type { BuiltinId } from "@/lib/widgets";
import { appsWidget, diskWidget, meshWidget, rebuildsWidget, uptimeWidget } from "./builtin";
import type { WidgetFn, WidgetKind } from "./types";

/** How wide a tile sits on the board. A heatmap is 53 columns of squares and
 *  is unreadable in half a card; everything else pairs up. */
export type WidgetSpan = "full" | "half";

export interface CatalogueEntry {
  readonly id: BuiltinId;
  readonly name: string;
  readonly blurb: string;
  readonly icon: IconSvgElement;
  readonly run: WidgetFn;
  readonly span: WidgetSpan;
  /** Milliseconds between runs. A tile only refreshes while it can be seen. */
  readonly refreshMs: number;
}

/* Refresh periods, and why each one:
 *
 *   uptime   — a journal of whole days. Nothing it draws can change within
 *              five minutes, and it reads localStorage rather than the box.
 *   disk     — reads a recorded measurement; cheap, but no point spinning.
 *   mesh     — /api/settings, cached for 30 s in metrics.ts anyway.
 *   apps     — sends two HEAD requests at the apps themselves. A minute.
 *   rebuilds — the one tile that tracks something live. Fifteen seconds is
 *              slow enough not to matter and fast enough to notice a change
 *              starting. The Progress bar on the settings screens is where a
 *              rebuild is actually watched.
 */
export const CATALOGUE: readonly CatalogueEntry[] = [
  {
    id: "uptime",
    name: "Answering",
    blurb: "A year of days, one square each, shaded by how much of the day this box answered.",
    icon: Calendar03Icon,
    run: uptimeWidget,
    span: "full",
    refreshMs: 300_000,
  },
  {
    id: "disk",
    name: "Room",
    blurb: "How much of the disk is in use, and how much is held back for the box to grow into.",
    icon: HardDriveIcon,
    run: diskWidget,
    span: "half",
    refreshMs: 60_000,
  },
  {
    id: "mesh",
    name: "Sharing",
    blurb: "The hours a day this box lends its spare time to other boxes, against the hours it keeps.",
    icon: Share08Icon,
    run: meshWidget,
    span: "half",
    refreshMs: 60_000,
  },
  {
    id: "apps",
    name: "Apps",
    blurb: "Which apps this box serves, and whether each one answered just now.",
    icon: LayoutGridIcon,
    run: appsWidget,
    span: "half",
    refreshMs: 60_000,
  },
  {
    id: "rebuilds",
    name: "Changes",
    blurb: "Settings changes this box has applied, newest first, and whether each one took.",
    icon: Timer02Icon,
    run: rebuildsWidget,
    span: "half",
    refreshMs: 15_000,
  },
];

const BY_ID = new Map<BuiltinId, CatalogueEntry>(CATALOGUE.map((entry) => [entry.id, entry]));

export function catalogueEntry(id: BuiltinId): CatalogueEntry | undefined {
  return BY_ID.get(id);
}

/** Where a custom widget of each shape sits. Same rule as the built-ins. */
export function spanForKind(kind: WidgetKind): WidgetSpan {
  return kind === "heatmap" ? "full" : "half";
}

/** How often a custom widget runs. One period for all of them: a spec cannot
 *  say how expensive it is, and a minute is cheap against every metric. */
export const CUSTOM_REFRESH_MS = 60_000;
