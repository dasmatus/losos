import type { IconSvgElement } from "@hugeicons/react";
import {
  ArrowReloadHorizontalIcon,
  CloudUploadIcon,
  ComputerCloudIcon,
  CpuIcon,
  Globe02Icon,
  HardDriveIcon,
  InformationCircleIcon,
  Key01Icon,
  LayoutGridIcon,
  PaintBoardIcon,
  Share08Icon,
  SlidersHorizontalIcon,
  GitCommitIcon,
  ShoppingCart01Icon,
  Shield01Icon,
} from "@hugeicons/core-free-icons";
import { t, type MessageKey } from "@/lib/i18n";

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
 * `keywords` is what the command palette (src/palette/items.ts) matches on
 * beyond the label (English here; each language adds its own under
 * `settings.panes.<id>.keywords`). It exists
 * because the word an owner types is rarely the word on the row — "GPU" and
 * "graphics" both have to find Hardware, "hostname" has to find Network —
 * and because a settings search that only matches seven visible labels is
 * decoration.
 */

export type SettingsPaneId =
  | "storage"
  | "mesh"
  | "market"
  | "machines"
  | "apps"
  | "network"
  | "look"
  | "hardware"
  | "security"
  | "access"
  | "advanced"
  | "history"
  | "backup"
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
}

/* The text lives in the catalogue under keys, never as a string here: this
 * list is built at import time, and a string would freeze in whichever
 * language was on screen then. `label` and `summary` are getters so a caller
 * reading `pane.label` while it renders gets the current language. */
function pane(
  id: SettingsPaneId,
  icon: IconSvgElement,
  keywords: readonly string[],
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
  pane("market", ShoppingCart01Icon, [
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
  ]),
  /* Virtual machines on the mesh, sold by the replica through the market.
   * The pane itself says why nothing on it works while this box does not
   * share its disk (pane-machines.tsx). */
  pane("machines", ComputerCloudIcon, [
    "vm",
    "vms",
    "virtual",
    "machine",
    "machines",
    "kubevirt",
    "qcow2",
    "image",
    "replica",
    "replicas",
    "linux",
    "ubuntu",
    "debian",
    "server",
    "host",
  ]),
  pane("apps", LayoutGridIcon, [
    "files",
    "code",
    "nextcloud",
    "forgejo",
    "install",
    "search",
    "add",
    "catalogue",
    // Each app's box-wide settings (site-settings.tsx).
    "admin",
    "administration",
    "language",
    "phone",
    "sharing",
    "link",
    "password",
    "expiry",
    "quota",
    "trash",
    "versions",
    "sign in",
    "private",
    "repository",
    "email",
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
  /* Look sits right after Network: it is the one pane that changes the
   * page itself, and the first thing an owner reaches for once the box is
   * up. Nothing on it rebuilds — a picture and a widget take effect the
   * moment they are saved, which is why it has no Apply bar. */
  pane("look", PaintBoardIcon, [
    "appearance",
    "background",
    "wallpaper",
    "picture",
    "image",
    "theme",
    "widget",
    "widgets",
    "custom",
    "html",
    "code",
    "personalise",
    "personalize",
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
  /* Access tokens sit right after Security: they are the other way into
   * the box, and an owner who is thinking about one is thinking about the
   * other. */
  pane("access", Key01Icon, [
    "token",
    "tokens",
    "pat",
    "personal",
    "access",
    "gpg",
    "pgp",
    "key",
    "terminal",
    "tui",
    "cli",
    "ssh",
    "losos",
  ]),
  /* Advanced and History sit between Security and About: every option,
   * then the record of every change. Advanced carries the long tail an
   * owner reaches for rarely and deliberately — like Security, below the
   * everyday panes — and History is the audit of all of them. */
  pane("advanced", SlidersHorizontalIcon, [
    "all",
    "every",
    "option",
    "options",
    "expert",
    "nix",
    "config",
    "configuration",
    "overrides",
    "raw",
  ]),
  pane("history", GitCommitIcon, [
    "git",
    "commit",
    "commits",
    "log",
    "repository",
    "repo",
    "clone",
    "sync",
    "forgejo",
    "changes",
    "audit",
  ]),
  /* Backup sits just above the two panes that end a box's life: it is
   * where the way back is set up, so it should be met before Reset. */
  pane("backup", CloudUploadIcon, [
    "backup",
    "back up",
    "restore",
    "bucket",
    "s3",
    "minio",
    "cloud",
    "copy",
    "recover",
    "recovery",
    "encrypt",
  ]),
  pane("about", InformationCircleIcon, ["version", "info", "identity", "key", "admin", "updates", "tour", "welcome", "guide"]),
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

/* The section of backend/sections.json each pane works in, which decides
 * who sees it: an administrator sees every pane, anyone else the panes of
 * the sections their groups open. A pane that writes the box's
 * configuration (Mesh, Hardware, Security, Advanced, ...) is the
 * administrators'. */
const PANE_SECTION: Record<SettingsPaneId, string> = {
  storage: "storage",
  mesh: "admin",
  market: "market",
  machines: "market",
  apps: "apps",
  network: "network",
  look: "look",
  hardware: "admin",
  security: "admin",
  access: "access",
  advanced: "admin",
  history: "admin",
  backup: "backup",
  about: "admin",
  reset: "admin",
};

/** The section a pane belongs to, for `canUse`. */
export function paneSection(id: SettingsPaneId): string {
  return PANE_SECTION[id];
}

/** Parse a URL segment into the id of a pane that can be opened. For an
 *  integrator wiring `/settings/:pane`; an unknown segment falls back rather
 *  than 404s. */
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
