import type { IconSvgElement } from "@hugeicons/react";
import {
  ArrowReloadHorizontalIcon,
  CpuIcon,
  Globe02Icon,
  HardDriveIcon,
  InformationCircleIcon,
  LayoutGridIcon,
  Share08Icon,
} from "@hugeicons/core-free-icons";

/* The sidebar's list of panes, in order.
 *
 * Ordered the way the box is likely to be used rather than alphabetically:
 * the two things an owner came here to change (their disk, and what they
 * lend to the mesh) sit at the top, the irreversible one sits alone at the
 * bottom, and About is the second-to-last stop because it is a read.
 *
 * `keywords` is what the search field matches on beyond the label. It exists
 * because the word an owner types is rarely the word on the row — "GPU" and
 * "graphics" both have to find Hardware, "hostname" has to find Network —
 * and because a settings search that only matches seven visible labels is
 * decoration.
 */

export type SettingsPaneId =
  | "storage"
  | "mesh"
  | "apps"
  | "network"
  | "hardware"
  | "about"
  | "reset";

export interface SettingsPane {
  id: SettingsPaneId;
  label: string;
  icon: IconSvgElement;
  /** One line under the pane's title. Says what the pane is for. */
  summary: string;
  keywords: readonly string[];
}

/* Typed as a non-empty tuple, not `readonly SettingsPane[]`. That is what
 * lets `paneById` fall back to SETTINGS_PANES[0] without a non-null
 * assertion under `noUncheckedIndexedAccess` — the shape carries the
 * guarantee instead of a `!` asserting it. */
export const SETTINGS_PANES: readonly [SettingsPane, ...SettingsPane[]] = [
  {
    id: "storage",
    label: "Storage",
    icon: HardDriveIcon,
    summary:
      "How much room this box has, what it is holding for other people, and the space held back when it was set up.",
    keywords: ["disk", "space", "capacity", "room", "grow", "reserve", "backup", "copies", "full"],
  },
  {
    id: "mesh",
    label: "Mesh",
    icon: Share08Icon,
    summary: "Joining other boxes, and the hours this one lends its spare capacity to them.",
    keywords: ["share", "join", "compute", "window", "hours", "sleep", "night", "lend", "others"],
  },
  {
    id: "apps",
    label: "Apps",
    icon: LayoutGridIcon,
    summary: "The apps this box runs, how each one runs, and where to look for more.",
    keywords: ["files", "code", "nextcloud", "forgejo", "install", "search", "add", "catalogue"],
  },
  {
    id: "network",
    label: "Network",
    icon: Globe02Icon,
    summary: "The name this box answers to, and how it is reached from outside your home.",
    keywords: [
      "name",
      "hostname",
      "address",
      "https",
      "tls",
      "certificate",
      "port",
      "remote",
      "internet",
      "outside",
    ],
  },
  {
    id: "hardware",
    label: "Hardware",
    icon: CpuIcon,
    summary: "What this box is allowed to use inside itself.",
    keywords: ["gpu", "graphics", "card", "video", "processor", "acceleration"],
  },
  {
    id: "about",
    label: "About",
    icon: InformationCircleIcon,
    summary: "What this box is, right now.",
    keywords: ["version", "info", "identity", "key", "admin", "updates"],
  },
  {
    id: "reset",
    label: "Reset",
    icon: ArrowReloadHorizontalIcon,
    summary: "Putting every setting back the way it came.",
    keywords: ["factory", "defaults", "wipe", "start over", "erase", "undo"],
  },
];

export const DEFAULT_PANE: SettingsPaneId = "storage";

/** Parse a URL segment into a pane id. For an integrator wiring
 *  `/settings/:pane`; an unknown segment falls back rather than 404s. */
export function isSettingsPaneId(value: unknown): value is SettingsPaneId {
  return SETTINGS_PANES.some((pane) => pane.id === value);
}

export function paneById(id: SettingsPaneId): SettingsPane {
  /* The list is a constant and `id` is a union of its members, so the find
   * cannot miss today. The fallback is for the day someone deletes a row
   * without touching the union — a pane that is gone lands on the first one
   * rather than blanking the screen. */
  return SETTINGS_PANES.find((pane) => pane.id === id) ?? SETTINGS_PANES[0];
}

/* Does this pane match what was typed into the sidebar's search field?
 *
 * Every term has to match something — typing two words narrows rather than
 * widens, which is what a person means by adding a word. */
export function paneMatches(pane: SettingsPane, search: string): boolean {
  const terms = search.toLowerCase().split(/\s+/).filter((term) => term.length > 0);
  if (terms.length === 0) return true;
  const haystack = `${pane.label} ${pane.summary} ${pane.keywords.join(" ")}`.toLowerCase();
  return terms.every((term) => haystack.includes(term));
}
