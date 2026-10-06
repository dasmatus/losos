import type { IconSvgElement } from "@hugeicons/react";
import {
  ArrowReloadHorizontalIcon,
  CpuIcon,
  Globe02Icon,
  HardDriveIcon,
  InformationCircleIcon,
  LayoutGridIcon,
  Share08Icon,
  ShoppingCart01Icon,
  Shield01Icon,
} from "@hugeicons/core-free-icons";
import { t, template, type MessageKey } from "@/lib/i18n";

/* The sidebar's list of panes, in order.
 *
 * Ordered the way the box is likely to be used rather than alphabetically:
 * the two things an owner came here to change (their disk, and what they
 * lend to the mesh) sit at the top, the irreversible one sits alone at the
 * bottom, and About is the second-to-last stop because it is a read.
 *
 * Security sits after Hardware rather than near the top on purpose: every
 * switch on it is off by default and costs something, so it is a place an
 * owner goes deliberately, not one they should meet while looking for their
 * disk. The protections that cost nothing are always on and appear nowhere.
 *
 * `keywords` is what the search field matches on beyond the label (English
 * here; each language adds its own under `settings.panes.<id>.keywords`). It exists
 * because the word an owner types is rarely the word on the row — "GPU" and
 * "graphics" both have to find Hardware, "hostname" has to find Network —
 * and because a settings search that only matches seven visible labels is
 * decoration.
 */

export type SettingsPaneId =
  | "storage"
  | "mesh"
  | "market"
  | "apps"
  | "network"
  | "hardware"
  | "security"
  | "about"
  | "reset";

export interface SettingsPane {
  id: SettingsPaneId;
  /** Message key of the pane's name. */
  labelKey: MessageKey;
  /** Message key of the one line under the pane's title. */
  summaryKey: MessageKey;
  /** Message key of extra search words in the current language. */
  keywordsKey: MessageKey;
  /** The pane's name in the current language. Read during render. */
  readonly label: string;
  icon: IconSvgElement;
  /** One line under the pane's title, in the current language. Says what the
   *  pane is for. Read during render. */
  readonly summary: string;
  /** English search words. Always searched, whatever the language. */
  keywords: readonly string[];
  /** Drawn in the sidebar but not open yet: greyed, not focusable, and its
   *  address is not an address. The pane's code stays in the tree so it can
   *  be opened by flipping this flag, and nothing else. */
  planned: boolean;
}

/* The text lives in the catalogue under keys, never as a string here: this
 * list is built at import time, and a string would freeze in whichever
 * language was on screen then. `label` and `summary` are getters so a caller
 * reading `pane.label` while it renders gets the current language. */
function pane(
  id: SettingsPaneId,
  icon: IconSvgElement,
  keywords: readonly string[],
  { planned = false }: { planned?: boolean } = {},
): SettingsPane {
  const labelKey = `settings.panes.${id}.label` as const;
  const summaryKey = `settings.panes.${id}.summary` as const;
  return {
    id,
    labelKey,
    summaryKey,
    keywordsKey: `settings.panes.${id}.keywords`,
    icon,
    keywords,
    planned,
    get label() {
      return t(labelKey);
    },
    get summary() {
      return t(summaryKey);
    },
  };
}

/* Typed as a non-empty tuple, not `readonly SettingsPane[]`. That is what
 * lets `paneById` fall back to SETTINGS_PANES[0] without a non-null
 * assertion under `noUncheckedIndexedAccess` — the shape carries the
 * guarantee instead of a `!` asserting it. */
export const SETTINGS_PANES: readonly [SettingsPane, ...SettingsPane[]] = [
  pane("storage", HardDriveIcon, ["disk", "space", "capacity", "room", "grow", "reserve", "full"]),
  pane("mesh", Share08Icon, [
    "share",
    "join",
    "compute",
    "window",
    "hours",
    "sleep",
    "night",
    "lend",
    "others",
  ]),
  /* The market is planned, not open: it needs a registered business behind
   * the Stripe platform account before a single cent can move, and there is
   * none yet. The row stays on the sidebar, greyed and labelled "soon(TM)",
   * because it is one of the things the box can say it has planned; the pane
   * behind it (pane-market.tsx, lososd's /api/market relay, the registrar's
   * /market routes) is finished and untouched. The disk-sharing switch sits
   * on that pane too, so sharing opens with the market and not before.
   * Opening it is `planned: false` here and the matching switch in
   * tests/app.browser.mjs. */
  pane(
    "market",
    ShoppingCart01Icon,
    [
      "sell",
      "buy",
      "pay",
      "payment",
      "stripe",
      "money",
      "price",
      "earn",
      "storage",
      "compute",
      // The disk-sharing switch lives on this pane (see pane-market.tsx).
      "share",
      "lend",
      "disk",
      "mesh",
      "backup",
      "copies",
    ],
    { planned: true },
  ),
  pane("apps", LayoutGridIcon, [
    "files",
    "code",
    "nextcloud",
    "forgejo",
    "install",
    "search",
    "add",
    "catalogue",
  ]),
  pane("network", Globe02Icon, [
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
  ]),
  pane("hardware", CpuIcon, ["gpu", "graphics", "card", "video", "processor", "acceleration"]),
  pane("security", Shield01Icon, [
    "hardening",
    "harden",
    "protection",
    "apparmor",
    "usb",
    "usbguard",
    "memory",
    "malloc",
    "smt",
    "hyperthreading",
    "processor",
    "lock down",
    "safe",
  ]),
  pane("about", InformationCircleIcon, ["version", "info", "identity", "key", "admin", "updates"]),
  pane("reset", ArrowReloadHorizontalIcon, [
    "factory",
    "defaults",
    "wipe",
    "start over",
    "erase",
    "undo",
  ]),
];

export const DEFAULT_PANE: SettingsPaneId = "storage";

/** Parse a URL segment into the id of a pane that can be opened. For an
 *  integrator wiring `/settings/:pane`; an unknown segment falls back rather
 *  than 404s. A planned pane's segment is unknown here on purpose: a deep
 *  link to it lands on the default pane, the same as typing it would. */
export function isSettingsPaneId(value: unknown): value is SettingsPaneId {
  return SETTINGS_PANES.some((pane) => pane.id === value && !pane.planned);
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
 * widens, which is what a person means by adding a word.
 *
 * The haystack is the pane's name, summary and keywords in the language on
 * screen AND in English, so an owner who reads Slovak but types "disk" or
 * "GPU" still lands. Accents are folded on both sides, so "zabezpecenie"
 * finds "Zabezpečenie" from a keyboard without them. Reads the language at
 * call time: call it while a component subscribed with useT() renders. */
export function paneMatches(pane: SettingsPane, search: string): boolean {
  const terms = fold(search)
    .split(/\s+/)
    .filter((term) => term.length > 0);
  if (terms.length === 0) return true;
  const haystack = fold(
    [
      t(pane.labelKey),
      t(pane.summaryKey),
      t(pane.keywordsKey),
      template(pane.labelKey, undefined, "en"),
      template(pane.summaryKey, undefined, "en"),
      pane.keywords.join(" "),
    ].join(" "),
  );
  return terms.every((term) => haystack.includes(term));
}

/** Lower-case and strip diacritics: "Úložisko" -> "ulozisko". */
function fold(text: string): string {
  return text
    .toLowerCase()
    .normalize("NFD")
    .replace(/[\u0300-\u036f]/g, "");
}
