/* The app catalogue, and which of it actually exists on this box.
 *
 * Photos, Calendar, Contacts, Notes, Tasks, Mail and Music are not separate
 * programs: every one of them is served by the Files app, and none of them
 * exists until first-run setup has finished. A tile for an app that is not
 * there yet is a lie the owner discovers by clicking it and landing on an
 * error page, so the catalogue below is inert data and `deriveApps` is the
 * only thing that decides what gets rendered.
 *
 * What the client can actually know today
 * ---------------------------------------
 * There is no `GET /api/setup`. modules/setup.nix reserves that route and
 * says so in its header comment, but backend/schema.json lists eight routes
 * and none of them reports whether setup finished. So the honest states are
 * three, not two: complete, incomplete, and *not known yet* — and the last
 * one is what a browser sees before the admin key is pasted in.
 *
 * The homepage cannot sit blank waiting for a key, so the answer is memory:
 * the moment the box answers the admin API with a real configuration this
 * browser records that setup is done (`rememberSetup`), and every later
 * visit paints the full grid on the first frame, before any request. A
 * browser that has never seen this box complete gets the fresh-box reading
 * instead — Settings, and a pointer at the wizard — which is the right guess
 * precisely because nothing has ever told it otherwise. localStorage is
 * per-origin and every appliance is its own origin, so two boxes cannot
 * inherit each other's answer.
 *
 * Vocabulary: nothing in here may name a container runtime or a scheduler.
 * It is "an app", it "runs on this box", and the user-visible strings in this
 * file are the ones that have to hold that line.
 */

import type { IconSvgElement } from "@hugeicons/react";
import {
  Calendar03Icon,
  CheckListIcon,
  Contact01Icon,
  Folder01Icon,
  GitBranchIcon,
  Image02Icon,
  Mail01Icon,
  MusicNote01Icon,
  Note03Icon,
  Settings01Icon,
} from "@hugeicons/core-free-icons";

// ── Catalogue ─────────────────────────────────────────────────────────────

export type AppId =
  | "files"
  | "photos"
  | "calendar"
  | "contacts"
  | "notes"
  | "tasks"
  | "mail"
  | "music"
  | "code"
  | "settings";

/** What has to be running for an app to exist at all. */
export type Provider =
  /** Served by the Files app. Nothing here exists before setup finishes. */
  | "files"
  /** Served by the code host. */
  | "code"
  /** This admin page itself. Always there, including on a box mid-setup. */
  | "box";

export interface AppDefinition {
  readonly id: AppId;
  /** Shown under the tile. Short enough not to wrap at 84px. */
  readonly name: string;
  readonly icon: IconSvgElement;
  /** Path under the provider's base; joined by {@link deriveApps}. */
  readonly path: string;
  readonly provider: Provider;
  /** The second line when nothing has been measured. Never a fake number. */
  readonly note: string;
}

/* Where each provider answers on this box.
 *
 * modules/containers.nix routes `/nextcloud` and `/forgejo/` on the front
 * vhost. Both are overridable because they are only the defaults: with
 * `losos.nextcloud.mode = "native"` the Files app is served by its own vhost
 * at the root of the box's name instead, and a tile pointing at /nextcloud
 * would 404. Pass `filesBase` when the caller knows better. */
export const FILES_BASE = "/nextcloud";
export const CODE_BASE = "/forgejo";

/** Where the first-run wizard lives in this SPA. */
export const SETUP_ROUTE = "/setup";

/* Display order, and it is deliberate: Files first because it is the box's
 * reason to exist, the seven apps it provides next, the code host after them,
 * and Settings last because it is the one tile that is always present and the
 * only one the owner should not need. */
export const CATALOGUE: readonly AppDefinition[] = [
  {
    id: "files",
    name: "Files",
    icon: Folder01Icon,
    path: "/apps/files/",
    provider: "files",
    note: "Everything you keep here",
  },
  {
    id: "photos",
    name: "Photos",
    icon: Image02Icon,
    path: "/apps/photos/",
    provider: "files",
    note: "Pictures and albums",
  },
  {
    id: "calendar",
    name: "Calendar",
    icon: Calendar03Icon,
    path: "/apps/calendar/",
    provider: "files",
    note: "Dates and reminders",
  },
  {
    id: "contacts",
    name: "Contacts",
    icon: Contact01Icon,
    path: "/apps/contacts/",
    provider: "files",
    note: "Names and addresses",
  },
  {
    id: "notes",
    name: "Notes",
    icon: Note03Icon,
    path: "/apps/notes/",
    provider: "files",
    note: "Written down, kept here",
  },
  {
    id: "tasks",
    name: "Tasks",
    icon: CheckListIcon,
    path: "/apps/tasks/",
    provider: "files",
    note: "Lists and due dates",
  },
  {
    id: "mail",
    name: "Mail",
    icon: Mail01Icon,
    path: "/apps/mail/",
    provider: "files",
    note: "Your mail accounts",
  },
  {
    id: "music",
    name: "Music",
    icon: MusicNote01Icon,
    path: "/apps/music/",
    provider: "files",
    note: "Your own library",
  },
  {
    id: "code",
    name: "Code",
    icon: GitBranchIcon,
    path: "/",
    provider: "code",
    note: "Your repositories",
  },
  {
    id: "settings",
    name: "Settings",
    icon: Settings01Icon,
    path: "/settings",
    provider: "box",
    note: "Run this box",
  },
];

// ── What is known about this box ──────────────────────────────────────────

/** Has first-run setup finished? "unknown" is a real answer, not a loading flag. */
export type SetupState = "unknown" | "incomplete" | "complete";

/* A tile corner marker. Semantic colours only: amber for pressure, red for
 * something broken, a breathing grey for work in progress. Never the accent —
 * the accent means "this box", and a tile that needs attention is not a
 * different kind of box. */
export interface AppAttention {
  readonly level: "warn" | "crit" | "pending";
  /** One sentence. Shown to the eye as the tile's second line and to a screen
   *  reader as the only channel the dot itself has (the dot is aria-hidden). */
  readonly note: string;
}

/* The second line under a tile's name.
 *
 * Split by kind because a measured figure is set in tabular mono so columns of
 * digits line up, and a sentence very much is not. */
export type AppDetail =
  | { readonly kind: "figure"; readonly text: string }
  | { readonly kind: "note"; readonly text: string };

export interface AppFacts {
  readonly setup: SetupState;
  /* settingsResponse carries `forgejoMode` but no `forgejo.enable`, and the
   * appliance default (modules/defaults.nix) is on. Undefined therefore means
   * "the API does not say", which is not the same as "off". */
  readonly codeHosting?: boolean;
  /** Measured second lines, e.g. `{ files: "847 GB", code: "19 repos" }`. */
  readonly details?: Partial<Record<AppId, string>>;
  readonly attention?: Partial<Record<AppId, AppAttention>>;
  /** Override where the Files app answers. See {@link FILES_BASE}. */
  readonly filesBase?: string;
  readonly codeBase?: string;
}

export interface AppTileModel {
  readonly app: AppDefinition;
  /** Same-origin, absolute. */
  readonly href: string;
  /** True when following it leaves this admin page for another app. */
  readonly leavesAdmin: boolean;
  readonly detail: AppDetail;
  readonly attention: AppAttention | null;
}

export interface AppGridModel {
  readonly setup: SetupState;
  readonly tiles: readonly AppTileModel[];
}

function joinPath(base: string, path: string): string {
  const trimmed = base.endsWith("/") ? base.slice(0, -1) : base;
  return `${trimmed}${path}`;
}

function baseFor(provider: Provider, facts: AppFacts): string {
  if (provider === "files") return facts.filesBase ?? FILES_BASE;
  if (provider === "code") return facts.codeBase ?? CODE_BASE;
  return "";
}

/* Whether an app exists, which is the whole point of this module.
 *
 * The `files` arm is the one that matters: before setup there is no admin
 * account, no data directory and no route worth following, so those seven
 * tiles are not dimmed or disabled — they are absent. */
function exists(provider: Provider, facts: AppFacts): boolean {
  switch (provider) {
    case "box":
      return true;
    case "files":
      return facts.setup === "complete";
    case "code":
      return facts.setup === "complete" && facts.codeHosting !== false;
    default: {
      const unreachable: never = provider;
      return unreachable;
    }
  }
}

/** The grid, derived. The only sanctioned way to decide what gets a tile. */
export function deriveApps(facts: AppFacts): AppGridModel {
  const tiles = CATALOGUE.filter((app) => exists(app.provider, facts)).map((app) => {
    const figure = facts.details?.[app.id];
    return {
      app,
      href: joinPath(baseFor(app.provider, facts), app.path),
      leavesAdmin: app.provider !== "box",
      detail:
        figure === undefined
          ? ({ kind: "note", text: app.note } as const)
          : ({ kind: "figure", text: figure } as const),
      attention: facts.attention?.[app.id] ?? null,
    };
  });

  return { setup: facts.setup, tiles };
}

// ── Remembering that this box is set up ───────────────────────────────────

/* Why a cache and not a request: see the header. Both accessors are wrapped
 * because localStorage throws outright in some privacy modes, and a homepage
 * is not worth taking down over a grid that would have rendered one state
 * less confidently. */
export const SETUP_MEMORY_KEY = "losos-setup-complete";

/** What this browser last saw. "unknown" when it has never seen this box. */
export function recallSetup(): SetupState {
  try {
    const raw = window.localStorage.getItem(SETUP_MEMORY_KEY);
    if (raw === "yes") return "complete";
    if (raw === "no") return "incomplete";
    return "unknown";
  } catch {
    return "unknown";
  }
}

/** Record what the box just said, so the next first paint does not have to ask. */
export function rememberSetup(state: SetupState): void {
  try {
    if (state === "unknown") window.localStorage.removeItem(SETUP_MEMORY_KEY);
    else window.localStorage.setItem(SETUP_MEMORY_KEY, state === "complete" ? "yes" : "no");
  } catch {
    /* the grid just asks again next time */
  }
}

// ── Figures ───────────────────────────────────────────────────────────────

const UNITS = ["bytes", "kB", "MB", "GB", "TB", "PB"] as const;

/* Base 1000, because that is what a disk is sold as and what the owner will
 * compare this against. Returns "unknown" rather than a plausible zero for a
 * value nothing measured — a fabricated figure on a homepage is indistinguish-
 * able from a real one. */
export function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes < 0) return "unknown";
  let value = bytes;
  let unit = 0;
  while (value >= 1000 && unit < UNITS.length - 1) {
    value /= 1000;
    unit += 1;
  }
  const digits = unit > 0 && value < 10 ? 1 : 0;
  return `${value.toFixed(digits)} ${UNITS[unit] ?? "bytes"}`;
}
