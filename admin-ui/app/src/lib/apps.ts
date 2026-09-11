/* The app catalogue, and which of it actually exists on this box.
 *
 * Photos, Calendar, Contacts, Notes, Tasks, Mail and Music are not separate
 * programs: every one of them is served by the Files app, and none of them
 * exists until that app is up and set up. A tile for an app that is not there
 * is a lie the owner discovers by clicking it and landing on an error page, so
 * the catalogue below is inert data and `deriveApps` is the only thing that
 * decides what gets rendered.
 *
 * How the client can know, without a route that tells it
 * -----------------------------------------------------
 * There is no `GET /api/setup`. modules/setup.nix reserves it and says so in
 * its header; backend/schema.json lists eight routes and none of them reports
 * whether first-run setup finished. So the answer is measured instead of
 * asked: `probeApps` fetches the one public, same-origin, token-free URL each
 * app already serves and reads the result.
 *
 *   Files -> GET /nextcloud/status.php   JSON, `installed: true` when real
 *   Code  -> GET /forgejo/               200 when the git host is answering
 *
 * Both are same-origin, so `connect-src 'self'` allows them and no token is
 * needed — which is the point: the homepage paints before the admin key is
 * pasted in. `credentials: "omit"` keeps the probe from carrying whatever
 * session cookie the Files app left behind.
 *
 * A probe is still a request, so the last positive answer is remembered in
 * localStorage (`recallAvailability`) and the grid paints from memory on the
 * first frame. localStorage is per-origin and every appliance is its own
 * origin, so two boxes cannot inherit each other's answer.
 *
 * Served here, or served elsewhere
 * -------------------------------
 * modules/containers.nix routes `/nextcloud` and `/forgejo/` on the front
 * vhost only while the service is in the deployment mode that puts it behind
 * that vhost. In the other mode the Files app answers at the root of a name
 * this page cannot derive — `losos.nextcloud.hostName` is not in the settings
 * response — so there is no href worth writing. `filesServedHere: false` then
 * drops those tiles and sets a notice, because a tile is a promise that
 * clicking it works.
 *
 * Vocabulary: nothing in here may name a container runtime or a scheduler. It
 * is "an app", it "runs on this box", and every user-visible string in this
 * file has to hold that line.
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
import type { ServiceMode } from "@/lib/api";

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

/** What has to be answering for an app to exist at all. */
export type Provider =
  /** Served by the Files app. Nothing here exists before it is set up. */
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
  /* What the app is, in a few words. Not the second line — a tile is 78px
   * wide and a sentence under it is an ellipsis with a word in front of it.
   * It is the tile's tooltip, and the second line is kept for things that
   * were actually measured. */
  readonly note: string;
}

/** Where the Files app answers on this box's front door, when it does. */
export const FILES_BASE = "/nextcloud";
/** Where the code host answers on this box's front door, when it does. */
export const CODE_BASE = "/forgejo";

/** Where the first-run wizard lives in this SPA. */
export const SETUP_ROUTE = "/setup";

/** Where this admin page keeps its own settings. */
export const SETTINGS_ROUTE = "/settings";

/* Display order, and it is deliberate: Files first because it is the box's
 * reason to exist, the seven apps it provides next, the code host after them,
 * and Settings last because it is the one tile that is always present and the
 * only one the owner should never need. */
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
    path: SETTINGS_ROUTE,
    provider: "box",
    note: "Run this box",
  },
];

// ── What is known about this box ──────────────────────────────────────────

/* Three answers, not two. "unknown" is the state a browser is in before
 * anything has replied, and the grid paints differently in it — a skeleton
 * rather than an empty state, because "we have not looked yet" and "it is not
 * there" are different things to tell someone. */
export type Availability = "unknown" | "absent" | "present";

export interface Availabilities {
  readonly files: Availability;
  readonly code: Availability;
}

/* A tile corner marker. Semantic colours only: amber for pressure, red for
 * something broken, a breathing grey for work in progress. Never the accent —
 * the accent means "this box", and a tile that needs attention is not a
 * different kind of box. */
export interface AppAttention {
  readonly level: "warn" | "crit" | "pending";
  /** One sentence. The dot itself is aria-hidden, so this is the only channel
   *  a screen reader gets, and the tile renders it as hidden text. */
  readonly note: string;
}

export interface AppFacts {
  /** Whether the Files app answered. See {@link probeApps}. */
  readonly files: Availability;
  /** Whether the code host answered. */
  readonly code: Availability;
  /* Has a probe finished, whatever it concluded?
   *
   * This is what separates "we have not looked yet" from "we looked and could
   * not tell". The first draws placeholders; the second draws the tiles,
   * because a box that did not answer one extra request is far more likely to
   * have its apps than not, and hiding them would be a worse guess than the
   * one this page made before it started measuring at all. */
  readonly measured?: boolean;
  /** False when this origin has no route to the app at all, whatever its
   *  state. Derive it with {@link servedHere}; default is true. */
  readonly filesServedHere?: boolean;
  readonly codeServedHere?: boolean;
  /** Override where the apps answer. Defaults to {@link FILES_BASE}/{@link CODE_BASE}. */
  readonly filesBase?: string;
  readonly codeBase?: string;
  /** Measured second lines, e.g. `{ files: "847 GB", code: "19 repos" }`.
   *  Absent means the static note is used; never invent a number here. */
  readonly details?: Partial<Record<AppId, string>>;
  readonly attention?: Partial<Record<AppId, AppAttention>>;
}

export interface AppTileModel {
  readonly app: AppDefinition;
  /** Same-origin and absolute. */
  readonly href: string;
  /** True when following it leaves this admin page for another app, which is
   *  a real navigation rather than a route change. */
  readonly leavesAdmin: boolean;
  /* The second line: a figure something measured, or nothing.
   *
   * Null is the common case and it is the right one. There is no route that
   * reports how much room the Files app is using or how many repositories the
   * code host holds, so on most boxes there is nothing true to put here — and
   * a line of filler under every tile is a row of ellipses that says less than
   * blank space would. */
  readonly detail: string | null;
  readonly attention: AppAttention | null;
}

export interface AppGridModel {
  readonly tiles: readonly AppTileModel[];
  /** How many tiles are still being measured. Draw that many placeholders. */
  readonly pendingCount: number;
  /** The box looks unfinished: offer the wizard rather than an empty grid. */
  readonly offerSetup: boolean;
  /** Why the grid is short, when the reason is configuration rather than an
   *  unfinished box. Null when there is nothing to explain. */
  readonly notice: string | null;
}

function joinPath(base: string, path: string): string {
  const trimmed = base.endsWith("/") ? base.slice(0, -1) : base;
  const joined = `${trimmed}${path}`;
  return joined.startsWith("/") ? joined : `/${joined}`;
}

function baseFor(provider: Provider, facts: AppFacts): string {
  if (provider === "files") return facts.filesBase ?? FILES_BASE;
  if (provider === "code") return facts.codeBase ?? CODE_BASE;
  return "";
}

/** Does this origin route to an app running in `mode`?
 *
 *  Only one of the two deployment modes puts the app behind this page's front
 *  door (modules/containers.nix). In the other it answers under a name the
 *  settings response does not carry, so this page cannot link to it. */
export function servedHere(mode: ServiceMode): boolean {
  return mode === "container";
}

/* Whether an app gets a tile, which is the whole point of this module.
 *
 * Three outcomes, because there are three states to be in. "pending" is the
 * one worth having: on the very first visit to a box, nothing is known yet,
 * and drawing nine tiles that might collapse to one is as wrong as drawing
 * one that might expand to nine. It draws placeholders instead, for the one
 * round trip it takes to find out, and after that the answer is remembered
 * and the first frame of every later visit is the real grid. */
type Presence = "show" | "hide" | "pending";

function presenceOf(provider: Provider, facts: AppFacts): Presence {
  switch (provider) {
    case "box":
      return "show";
    case "files":
      if (facts.filesServedHere === false) return "hide";
      return resolve(facts.files, facts.measured === true);
    case "code":
      if (facts.codeServedHere === false) return "hide";
      return resolve(facts.code, facts.measured === true);
    default: {
      const unreachable: never = provider;
      return unreachable;
    }
  }
}

function resolve(state: Availability, measured: boolean): Presence {
  if (state === "present") return "show";
  if (state === "absent") return "hide";
  return measured ? "show" : "pending";
}

function noticeFor(facts: AppFacts): string | null {
  const files = facts.filesServedHere === false;
  const code = facts.codeServedHere === false;
  if (files && code) {
    return "Your files and your code are served under this box's own name, not from this page.";
  }
  if (files) return "Your files are served under this box's own name, not from this page.";
  if (code) return "Your code is served under this box's own name, not from this page.";
  return null;
}

/** The grid, derived. The only sanctioned way to decide what gets a tile. */
export function deriveApps(facts: AppFacts): AppGridModel {
  const tiles: AppTileModel[] = [];
  let pendingCount = 0;

  for (const app of CATALOGUE) {
    const presence = presenceOf(app.provider, facts);
    if (presence === "hide") continue;
    if (presence === "pending") {
      pendingCount += 1;
      continue;
    }

    tiles.push({
      app,
      href: joinPath(baseFor(app.provider, facts), app.path),
      leavesAdmin: app.provider !== "box",
      detail: facts.details?.[app.id] ?? null,
      attention: facts.attention?.[app.id] ?? null,
    });
  }

  return {
    tiles,
    pendingCount,
    offerSetup: facts.filesServedHere !== false && facts.files === "absent",
    notice: noticeFor(facts),
  };
}

// ── Remembering what answered last time ───────────────────────────────────

/* Why a memory and not a request: the homepage has to paint before anything
 * has replied, and the honest thing to paint is what this browser last saw.
 * Both accessors are wrapped because localStorage throws outright in some
 * privacy modes, and a homepage is not worth taking down over a grid that
 * would have rendered one state less confidently. */
export const AVAILABILITY_KEY = "losos-apps-seen";

const UNKNOWN: Availabilities = { files: "unknown", code: "unknown" };

function isAvailability(value: unknown): value is Availability {
  return value === "unknown" || value === "absent" || value === "present";
}

/** What this browser last saw. All "unknown" when it has never seen this box. */
export function recallAvailability(): Availabilities {
  try {
    const raw = window.localStorage.getItem(AVAILABILITY_KEY);
    if (raw === null) return UNKNOWN;
    const parsed: unknown = JSON.parse(raw);
    if (typeof parsed !== "object" || parsed === null) return UNKNOWN;
    const record = parsed as Record<string, unknown>;
    return {
      files: isAvailability(record["files"]) ? record["files"] : "unknown",
      code: isAvailability(record["code"]) ? record["code"] : "unknown",
    };
  } catch {
    return UNKNOWN;
  }
}

/** Record what the box just answered, so the next first paint need not ask. */
export function rememberAvailability(seen: Availabilities): void {
  try {
    window.localStorage.setItem(AVAILABILITY_KEY, JSON.stringify(seen));
  } catch {
    /* the grid just measures again next time */
  }
}

/* The same memory, read as the one question most callers actually have.
 *
 * "Has this box been set up?" and "did the Files app answer?" are the same
 * question from a browser's side: the seven apps it provides exist exactly
 * when it does, and nothing else on this origin can tell them apart. Kept as a
 * named reading rather than a second store — two places recording the same
 * fact is how they come to disagree. */
export type SetupState = "unknown" | "incomplete" | "complete";

export function recallSetup(): SetupState {
  const { files } = recallAvailability();
  if (files === "present") return "complete";
  if (files === "absent") return "incomplete";
  return "unknown";
}

/** Record a conclusion the wizard reached, so the next first paint has it.
 *  Setting a password through the Files app proves it is there; "incomplete"
 *  and "unknown" only clear the claim, they never make a negative one about
 *  the code host. */
export function rememberSetup(state: SetupState): void {
  const seen = recallAvailability();
  rememberAvailability({
    ...seen,
    files: state === "complete" ? "present" : state === "incomplete" ? "absent" : "unknown",
  });
}

// ── Measuring what is actually there ──────────────────────────────────────

/** Nextcloud's public status document, relative to wherever it is served. */
export const FILES_PROBE_PATH = "/status.php";

export interface ProbeOptions {
  signal?: AbortSignal;
  filesBase?: string;
  codeBase?: string;
}

/* One request, reduced to the three answers.
 *
 * A 404 or a bad gateway is a real "not there": Nginx has no route, or the
 * route has nothing behind it. Anything else — a refused connection, a 403
 * from a guard, the tab going away mid-flight — is "we could not tell", and
 * saying "not there" on those would delete the owner's apps off their own
 * homepage because their Wi-Fi hiccuped. */
function readStatus(status: number): Availability {
  if (status === 404 || status === 410 || status === 502 || status === 503 || status === 504) {
    return "absent";
  }
  if (status >= 200 && status < 400) return "present";
  return "unknown";
}

async function probe(url: string, signal: AbortSignal | undefined): Promise<Response | null> {
  const init: RequestInit = { method: "GET", cache: "no-store", credentials: "omit" };
  if (signal !== undefined) init.signal = signal;
  try {
    return await fetch(url, init);
  } catch {
    // A refused connection, a DNS failure, or the caller aborting. None of
    // them is evidence about the app.
    return null;
  }
}

/** Is the Files app there? Reads `installed` out of its status document. */
export async function probeFiles(options: ProbeOptions = {}): Promise<Availability> {
  const base = options.filesBase ?? FILES_BASE;
  const response = await probe(joinPath(base, FILES_PROBE_PATH), options.signal);
  if (response === null) return "unknown";

  const fromStatus = readStatus(response.status);
  if (fromStatus !== "present") return fromStatus;

  /* The document is the point. Nextcloud answers this route while it is
   * running but not yet installed, and in that state the seven apps it
   * provides do not exist — which is exactly the case these tiles must not
   * paint over. A body that is not the expected document leaves the reading
   * at what the status line said. */
  try {
    const body: unknown = await response.json();
    if (typeof body === "object" && body !== null) {
      const installed = (body as Record<string, unknown>)["installed"];
      if (installed === false) return "absent";
    }
  } catch {
    /* not JSON: something answered, and that is all we claimed */
  }
  return "present";
}

/** Is the code host there? Its front page answering is the whole test. */
export async function probeCode(options: ProbeOptions = {}): Promise<Availability> {
  const base = options.codeBase ?? CODE_BASE;
  const response = await probe(joinPath(base, "/"), options.signal);
  return response === null ? "unknown" : readStatus(response.status);
}

/** Both probes, in parallel. One round trip each, no token, same origin. */
export async function probeApps(options: ProbeOptions = {}): Promise<Availabilities> {
  const [files, code] = await Promise.all([probeFiles(options), probeCode(options)]);
  return { files, code };
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

/** Whole days and hours from a second count. "6 d 3 h", "14 h", "just now". */
export function formatUptime(seconds: number): string {
  if (!Number.isFinite(seconds) || seconds < 0) return "unknown";
  const total = Math.floor(seconds);
  const days = Math.floor(total / 86_400);
  const hours = Math.floor((total % 86_400) / 3600);
  const minutes = Math.floor((total % 3600) / 60);
  if (days > 0) return hours === 0 ? `${days} d` : `${days} d ${hours} h`;
  if (hours > 0) return minutes === 0 ? `${hours} h` : `${hours} h ${minutes} min`;
  if (minutes > 0) return `${minutes} min`;
  return "just now";
}
