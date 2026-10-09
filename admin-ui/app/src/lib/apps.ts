/* The app catalogue, and which of it actually exists on this box.
 *
 * Every tile but Code and Settings is an app inside the Files app (LosOS
 * cloud), not a separate program, and none of them exists until that app is
 * up and set up. The catalogue lists the apps LosOS cloud enables on every
 * start (`apps` in modules/nextcloud-stack.nix) plus the four it always has
 * (Files, Dashboard, Photos, Activity), in the order its own app menu shows
 * them, so the two menus agree. An app added there wants a tile here. A tile for an app that is not there
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
  Activity01Icon,
  Album02Icon,
  Bookmark02Icon,
  Calendar03Icon,
  CheckListIcon,
  Contact01Icon,
  DashboardSquare02Icon,
  Folder01Icon,
  FormIcon,
  GitBranchIcon,
  GridTableIcon,
  Image02Icon,
  KanbanIcon,
  LibraryIcon,
  Mail01Icon,
  MapsLocation01Icon,
  MusicNote01Icon,
  News01Icon,
  Note03Icon,
  Settings01Icon,
  VoteIcon,
} from "@hugeicons/core-free-icons";
import type { ServiceMode } from "@/lib/api";
import { formatNumber, t, type MessageKey } from "@/lib/i18n";

// ── Catalogue ─────────────────────────────────────────────────────────────

export type AppId =
  | "files"
  | "dashboard"
  | "photos"
  | "activity"
  | "contacts"
  | "calendar"
  | "notes"
  | "bookmarks"
  | "deck"
  | "music"
  | "collectives"
  | "polls"
  | "forms"
  | "tables"
  | "memories"
  | "news"
  | "tasks"
  | "maps"
  | "mail"
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
  /** Shown under the tile. Short enough not to wrap at 84px. A message key,
   *  translated at render — this catalogue is evaluated once, at import. */
  readonly name: MessageKey;
  readonly icon: IconSvgElement;
  /** Path under the provider's base; joined by {@link deriveApps}. */
  readonly path: string;
  readonly provider: Provider;
  /* What the app is, in a few words. Not the second line — a tile is 78px
   * wide and a sentence under it is an ellipsis with a word in front of it.
   * It is the tile's tooltip, and the second line is kept for things that
   * were actually measured. A message key, like `name`. */
  readonly note: MessageKey;
  /* Not shipped: the owner may have installed it from the App Store, or not.
   * The tile is drawn only once {@link probeOptional} found it, because
   * without it the link is Nextcloud's "page not found". */
  readonly optional?: true;
}

/** Where the Files app answers on this box's front door, when it does.
 *
 * Every app path below goes through `index.php`. The workload image serves
 * Nextcloud without its pretty-URL rewrites (flake/images.nix transcribes
 * upstream's .htaccess minus that block, and never sets
 * `front_controller_active`), so `/nextcloud/apps/tasks/` is an Apache 404
 * (the short form is what the tiles used to link) while
 * `/nextcloud/index.php/apps/tasks/` is the URL Nextcloud itself generates
 * on this box. */
export const FILES_BASE = "/nextcloud";
/** Where the code host answers on this box's front door, when it does. */
export const CODE_BASE = "/forgejo";

/** Where the first-run wizard lives in this SPA. */
export const SETUP_ROUTE = "/setup";

/** Where this admin page keeps its own settings. */
export const SETTINGS_ROUTE = "/settings";

/* Display order, and it is deliberate: Files first because it is the box's
 * reason to exist, the apps it provides next in the order of LosOS cloud's
 * own menu, Mail after them because it is not shipped, the code host after
 * that, and Settings last because it is the one tile that is always present
 * and the only one the owner should never need. */
export const CATALOGUE: readonly AppDefinition[] = [
  {
    id: "files",
    name: "apps.files.name",
    icon: Folder01Icon,
    path: "/index.php/apps/files/",
    provider: "files",
    note: "apps.files.note",
  },
  {
    id: "dashboard",
    name: "apps.dashboard.name",
    icon: DashboardSquare02Icon,
    path: "/index.php/apps/dashboard/",
    provider: "files",
    note: "apps.dashboard.note",
  },
  {
    id: "photos",
    name: "apps.photos.name",
    icon: Image02Icon,
    path: "/index.php/apps/photos/",
    provider: "files",
    note: "apps.photos.note",
  },
  {
    id: "activity",
    name: "apps.activity.name",
    icon: Activity01Icon,
    path: "/index.php/apps/activity/",
    provider: "files",
    note: "apps.activity.note",
  },
  {
    id: "contacts",
    name: "apps.contacts.name",
    icon: Contact01Icon,
    path: "/index.php/apps/contacts/",
    provider: "files",
    note: "apps.contacts.note",
  },
  {
    id: "calendar",
    name: "apps.calendar.name",
    icon: Calendar03Icon,
    path: "/index.php/apps/calendar/",
    provider: "files",
    note: "apps.calendar.note",
  },
  {
    id: "notes",
    name: "apps.notes.name",
    icon: Note03Icon,
    path: "/index.php/apps/notes/",
    provider: "files",
    note: "apps.notes.note",
  },
  {
    id: "bookmarks",
    name: "apps.bookmarks.name",
    icon: Bookmark02Icon,
    path: "/index.php/apps/bookmarks/",
    provider: "files",
    note: "apps.bookmarks.note",
  },
  {
    id: "deck",
    name: "apps.deck.name",
    icon: KanbanIcon,
    path: "/index.php/apps/deck/",
    provider: "files",
    note: "apps.deck.note",
  },
  {
    id: "music",
    name: "apps.music.name",
    icon: MusicNote01Icon,
    path: "/index.php/apps/music/",
    provider: "files",
    note: "apps.music.note",
  },
  {
    id: "collectives",
    name: "apps.collectives.name",
    icon: LibraryIcon,
    path: "/index.php/apps/collectives/",
    provider: "files",
    note: "apps.collectives.note",
  },
  {
    id: "polls",
    name: "apps.polls.name",
    icon: VoteIcon,
    path: "/index.php/apps/polls/",
    provider: "files",
    note: "apps.polls.note",
  },
  {
    id: "forms",
    name: "apps.forms.name",
    icon: FormIcon,
    path: "/index.php/apps/forms/",
    provider: "files",
    note: "apps.forms.note",
  },
  {
    id: "tables",
    name: "apps.tables.name",
    icon: GridTableIcon,
    path: "/index.php/apps/tables/",
    provider: "files",
    note: "apps.tables.note",
  },
  {
    id: "memories",
    name: "apps.memories.name",
    icon: Album02Icon,
    path: "/index.php/apps/memories/",
    provider: "files",
    note: "apps.memories.note",
  },
  {
    id: "news",
    name: "apps.news.name",
    icon: News01Icon,
    path: "/index.php/apps/news/",
    provider: "files",
    note: "apps.news.note",
  },
  {
    id: "tasks",
    name: "apps.tasks.name",
    icon: CheckListIcon,
    path: "/index.php/apps/tasks/",
    provider: "files",
    note: "apps.tasks.note",
  },
  {
    id: "maps",
    name: "apps.maps.name",
    icon: MapsLocation01Icon,
    path: "/index.php/apps/maps/",
    provider: "files",
    note: "apps.maps.note",
  },
  {
    id: "mail",
    name: "apps.mail.name",
    icon: Mail01Icon,
    path: "/index.php/apps/mail/",
    provider: "files",
    note: "apps.mail.note",
    optional: true,
  },
  {
    id: "code",
    name: "apps.code.name",
    icon: GitBranchIcon,
    path: "/",
    provider: "code",
    note: "apps.code.note",
  },
  {
    id: "settings",
    name: "apps.settings.name",
    icon: Settings01Icon,
    path: SETTINGS_ROUTE,
    provider: "box",
    note: "apps.settings.note",
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
  /** The one optional app, see {@link AppDefinition.optional}. */
  readonly mail: Availability;
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
  /** Whether the mail client is installed. Absent counts as not found. */
  readonly mail?: Availability;
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
 * and drawing twenty tiles that might collapse to one is as wrong as drawing
 * one that might expand to twenty. It draws placeholders instead, for the one
 * round trip it takes to find out, and after that the answer is remembered
 * and the first frame of every later visit is the real grid. */
type Presence = "show" | "hide" | "pending";

function presenceOf(app: AppDefinition, facts: AppFacts): Presence {
  const provider = app.provider;
  switch (provider) {
    case "box":
      return "show";
    case "files": {
      if (facts.filesServedHere === false) return "hide";
      const files = resolve(facts.files, facts.measured === true);
      /* An optional app is drawn only on a positive answer. "Could not tell"
       * hides it, unlike the shipped apps: those are there on every box, and
       * this one is on few. No placeholder either, for the same reason. */
      if (app.optional === true && files !== "hide") {
        return facts.mail === "present" ? files : "hide";
      }
      return files;
    }
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
  if (files && code) return t("apps.notice.both");
  if (files) return t("apps.notice.files");
  if (code) return t("apps.notice.code");
  return null;
}

/** The grid, derived. The only sanctioned way to decide what gets a tile.
 *  The notice is translated, so call this while rendering a component that
 *  used `useT()`. */
export function deriveApps(facts: AppFacts): AppGridModel {
  const tiles: AppTileModel[] = [];
  let pendingCount = 0;

  for (const app of CATALOGUE) {
    const presence = presenceOf(app, facts);
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

const UNKNOWN: Availabilities = { files: "unknown", code: "unknown", mail: "unknown" };

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
      mail: isAvailability(record["mail"]) ? record["mail"] : "unknown",
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
 * question from a browser's side: the apps it provides exist exactly
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

async function probe(
  url: string,
  signal: AbortSignal | undefined,
  redirect: RequestRedirect = "follow",
): Promise<Response | null> {
  const init: RequestInit = { method: "GET", cache: "no-store", credentials: "omit", redirect };
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
   * running but not yet installed, and in that state the apps it
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

/* Is an app that LosOS cloud does not ship installed anyway?
 *
 * Asked of the app's own page, signed out (`credentials: "omit"`). Nextcloud
 * loads the routes of enabled apps only, so a page that is not there is a
 * 404, while an installed one sends a visitor without a session to the
 * sign-in page. The redirect is not followed: `redirect: "manual"` turns it
 * into an opaque response, which is the answer, and spares the sign-in page
 * a render. Anything else (a 503 during maintenance, a refused connection)
 * is "could not tell". */
export async function probeOptional(
  app: AppDefinition,
  options: ProbeOptions = {},
): Promise<Availability> {
  const base = options.filesBase ?? FILES_BASE;
  const response = await probe(joinPath(base, app.path), options.signal, "manual");
  if (response === null) return "unknown";
  if (response.type === "opaqueredirect") return "present";
  if (response.status === 404) return "absent";
  if (response.status >= 200 && response.status < 300) return "present";
  return "unknown";
}

const MAIL = CATALOGUE.find((app) => app.id === "mail");

/** All probes, in parallel. One round trip each, no token, same origin. */
export async function probeApps(options: ProbeOptions = {}): Promise<Availabilities> {
  const [files, code, mail] = await Promise.all([
    probeFiles(options),
    probeCode(options),
    MAIL === undefined ? Promise.resolve<Availability>("unknown") : probeOptional(MAIL, options),
  ]);
  return { files, code, mail };
}

// ── Figures ───────────────────────────────────────────────────────────────

const UNITS = ["bytes", "kB", "MB", "GB", "TB", "PB"] as const;

/* Base 1000, because that is what a disk is sold as and what the owner will
 * compare this against. Returns "unknown" rather than a plausible zero for a
 * value nothing measured — a fabricated figure on a homepage is indistinguish-
 * able from a real one. */
export function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes < 0) return t("apps.format.unknown");
  let value = bytes;
  let unit = 0;
  while (value >= 1000 && unit < UNITS.length - 1) {
    value /= 1000;
    unit += 1;
  }
  const digits = unit > 0 && value < 10 ? 1 : 0;
  if (unit === 0) return t("apps.format.bytes", { count: Math.round(value) });
  /* Rounded first so formatNumber prints exactly the digits toFixed would,
   * with the locale's decimal separator. */
  const shown = formatNumber(Number(value.toFixed(digits)), undefined, {
    minimumFractionDigits: digits,
    maximumFractionDigits: digits,
  });
  return `${shown} ${UNITS[unit] ?? "bytes"}`;
}

/** Whole days and hours from a second count. "6 d 3 h", "14 h", "just now".
 *  Translated, like formatBytes: call it during a subscribed render. */
export function formatUptime(seconds: number): string {
  if (!Number.isFinite(seconds) || seconds < 0) return t("apps.format.unknown");
  const total = Math.floor(seconds);
  const days = Math.floor(total / 86_400);
  const hours = Math.floor((total % 86_400) / 3600);
  const minutes = Math.floor((total % 3600) / 60);
  if (days > 0) {
    return hours === 0 ? t("apps.uptime.days", { days }) : t("apps.uptime.daysHours", { days, hours });
  }
  if (hours > 0) {
    return minutes === 0
      ? t("apps.uptime.hours", { hours })
      : t("apps.uptime.hoursMinutes", { hours, minutes });
  }
  if (minutes > 0) return t("apps.uptime.minutes", { minutes });
  return t("apps.uptime.justNow");
}
