/* The pins on the sidebar: what they are, where each one opens, and the
 * store the sidebar, the tiles and the picker share.
 *
 * The list lives on the box (backend/src/pins.rs, GET/PUT /api/pins), so a
 * phone and a laptop show the same pins. A pin is a reference and this file
 * is the only place that turns one into an address, always on this box:
 *
 *   tile   an app tile of the Overview      its tile's address (lib/apps.ts)
 *   app    an app from the Apps search      the port the box opened for it
 *   file   a file or folder in LosOS Cloud  /cloud/files?fileid=<id>
 *   repo   a repository on LosOS Git        /git/<owner>/<name>
 *   issue  an issue of LosOS Git's tracker  /git/issues/<key>
 *   project a project of the tracker        /git/issues/p/<key>
 *
 * An issue's title and a project's name are not stored with the pin: the
 * store asks LosOS Git for them as the person who opened the page, so a
 * renamed issue shows its new title and an issue they may not read shows
 * only its key.
 *
 * The store has the shape of lib/look.ts: loaded once a tab holds a token,
 * dropped with it, and every change written to the box first and shown
 * second, so the sidebar never shows a pin the box refused. */

import type { IconSvgElement } from "@hugeicons/react";
import { File01Icon, Folder01Icon, GitBranchIcon, PackageIcon, TaskDaily01Icon, TaskDone01Icon } from "@hugeicons/core-free-icons";
import {
  getApps,
  getPins,
  hasToken,
  isUnauthorized,
  putPins,
  subscribeAuth,
  type InstalledApp,
  type Pin,
} from "./api";
import { CATALOGUE, CODE_BASE, FILES_BASE, type AppDefinition } from "./apps";
import type { MessageKey } from "./i18n";

export type { Pin };

/** What the box keeps at most, until it says otherwise. */
export const DEFAULT_LIMIT = 24;

/** What makes two pins the same pin; matches `Pin::key` in pins.rs. */
export function pinKey(pin: Pin): string {
  switch (pin.kind) {
    case "tile":
      return `tile:${pin.app}`;
    case "app":
      return `app:${pin.release}`;
    case "file":
      return `file:${String(pin.id)}`;
    case "repo":
      return `repo:${pin.owner}/${pin.name}`;
    case "issue":
      return `issue:${pin.key}`;
    case "project":
      return `project:${pin.key}`;
  }
}

// ── Where a pin opens ─────────────────────────────────────────────────────

export interface PinView {
  key: string;
  /** Translated by the caller when `labelKey` is set; otherwise shown as is
   *  (a file name, a repository name, a release name). */
  label: string;
  labelKey: MessageKey | null;
  /** A second line for the picker and the tooltip: the folder a file is
   *  in, the owner of a repository. */
  detail: string | null;
  icon: IconSvgElement;
  href: string;
  /** A page of this app (a router link) rather than another app. */
  inApp: boolean;
  /** Opens in a new tab: an installed app on its own port. */
  newTab: boolean;
}

function tileDefinition(app: string): AppDefinition | undefined {
  return CATALOGUE.find((entry) => entry.id === app);
}

function tileHref(definition: AppDefinition): string {
  const base =
    definition.provider === "files" ? FILES_BASE : definition.provider === "code" ? CODE_BASE : "";
  if (base === "") return definition.path;
  return `${base}${definition.path.startsWith("/") ? "" : "/"}${definition.path}`;
}

function parentOf(path: string): string {
  const cut = path.replace(/\/+$/, "").lastIndexOf("/");
  return cut <= 0 ? "/" : path.slice(0, cut);
}

function baseName(path: string): string {
  const trimmed = path.replace(/\/+$/, "");
  return trimmed.slice(trimmed.lastIndexOf("/") + 1) || trimmed;
}

/** An installed app's address on this box, once it runs with a page. */
export function appHref(record: InstalledApp | undefined, hostname = window.location.hostname): string | null {
  if (record === undefined || record.phase !== "running" || record.appPort === null) return null;
  return `http://${hostname}:${String(record.frontPort)}/`;
}

/** Where a pin opens and how it is drawn, or null for a pin that names
 *  nothing this page knows (a tile id from a newer catalogue). `names` are
 *  the titles of issue and project pins, by {@link pinKey}. */
export function viewPin(
  pin: Pin,
  installed: readonly InstalledApp[] = [],
  names: ReadonlyMap<string, string> = NO_NAMES,
): PinView | null {
  const key = pinKey(pin);
  switch (pin.kind) {
    case "tile": {
      const definition = tileDefinition(pin.app);
      if (definition === undefined) return null;
      return {
        key,
        label: definition.name,
        labelKey: definition.name,
        detail: null,
        icon: definition.icon,
        href: tileHref(definition),
        inApp: definition.provider === "box",
        newTab: false,
      };
    }
    case "app": {
      const href = appHref(installed.find((record) => record.release === pin.release));
      return {
        key,
        label: pin.release,
        labelKey: null,
        detail: null,
        icon: PackageIcon,
        // Not running yet, or gone: the Apps pane says which.
        href: href ?? "/apps",
        inApp: href === null,
        newTab: href !== null,
      };
    }
    case "file":
      return {
        key,
        label: baseName(pin.path),
        labelKey: null,
        detail: parentOf(pin.path),
        icon: pin.folder ? Folder01Icon : File01Icon,
        // LosOS Cloud's link by id: it opens the file's folder with the file
        // marked, or the folder, wherever it has been moved to since.
        href: `${FILES_BASE}/files?fileid=${String(pin.id)}`,
        inApp: false,
        newTab: false,
      };
    case "repo":
      return {
        key,
        label: pin.name,
        labelKey: null,
        detail: pin.owner,
        icon: GitBranchIcon,
        href: `${CODE_BASE}/${encodeURIComponent(pin.owner)}/${encodeURIComponent(pin.name)}`,
        inApp: false,
        newTab: false,
      };
    case "issue":
    case "project": {
      const name = names.get(key);
      return {
        key,
        label: name ?? pin.key,
        labelKey: null,
        detail: name === undefined ? null : pin.key,
        icon: pin.kind === "issue" ? TaskDone01Icon : TaskDaily01Icon,
        href: `${CODE_BASE}/issues/${pin.kind === "project" ? "p/" : ""}${encodeURIComponent(pin.key)}`,
        inApp: false,
        newTab: false,
      };
    }
  }
}

const NO_NAMES: ReadonlyMap<string, string> = new Map();

// ── The store ─────────────────────────────────────────────────────────────

export type PinsStatus = "idle" | "loading" | "ready" | "failed";

export interface PinsState {
  readonly status: PinsStatus;
  readonly pins: readonly Pin[];
  readonly limit: number;
  /** The installed apps, for the address of an app pin. */
  readonly installed: readonly InstalledApp[];
  /** Issue titles and project names, by {@link pinKey}. */
  readonly names: ReadonlyMap<string, string>;
}

const EMPTY: PinsState = { status: "idle", pins: [], limit: DEFAULT_LIMIT, installed: [], names: NO_NAMES };

let state: PinsState = EMPTY;
const listeners = new Set<() => void>();
let inFlight: Promise<void> | null = null;

function set(next: PinsState): void {
  state = next;
  for (const fn of listeners) fn();
}

export function getPinsState(): PinsState {
  return state;
}

export function getServerPinsState(): PinsState {
  return EMPTY;
}

export function subscribePins(fn: () => void): () => void {
  listeners.add(fn);
  return () => listeners.delete(fn);
}

/* An app pin opens on the port the box gave the app, which only the app
 * list knows. Asked only when there is such a pin. */
async function installedFor(pins: readonly Pin[]): Promise<readonly InstalledApp[]> {
  if (!pins.some((pin) => pin.kind === "app")) return [];
  try {
    const reply = await getApps();
    return reply.available ? reply.apps : [];
  } catch {
    return [];
  }
}

/* An issue's title and a project's name, from LosOS Git's tracker as the
 * person on the page. Asked only when there is such a pin; a box without
 * LosOS Git, or an issue they cannot read, leaves the key alone. */
async function namesFor(pins: readonly Pin[]): Promise<ReadonlyMap<string, string>> {
  const issues = pins.flatMap((pin) => (pin.kind === "issue" ? [pin.key] : []));
  const projects = pins.some((pin) => pin.kind === "project");
  if (issues.length === 0 && !projects) return NO_NAMES;
  const names = new Map<string, string>();
  const ask = async (path: string): Promise<Record<string, unknown> | null> => {
    try {
      const res = await fetch(`${CODE_BASE}/api/v1${path}`, { credentials: "same-origin", headers: { Accept: "application/json" } });
      return res.ok ? ((await res.json()) as Record<string, unknown>) : null;
    } catch {
      return null;
    }
  };
  const [found, home] = await Promise.all([
    issues.length > 0 ? ask(`/tracker/issues?key=${encodeURIComponent(issues.join(","))}`) : null,
    projects ? ask("/tracker") : null,
  ]);
  for (const row of Array.isArray(found?.["issues"]) ? (found["issues"] as unknown[]) : []) {
    const r = row as Record<string, unknown>;
    if (typeof r["key"] === "string" && typeof r["title"] === "string") names.set(`issue:${r["key"]}`, r["title"]);
  }
  for (const row of Array.isArray(home?.["projects"]) ? (home["projects"] as unknown[]) : []) {
    const r = row as Record<string, unknown>;
    if (typeof r["key"] === "string" && typeof r["name"] === "string") names.set(`project:${r["key"]}`, r["name"]);
  }
  return names;
}

/* What the box sent, kept to the pins this page can read: an older box,
 * or a document from a newer one, is a shorter list rather than a broken
 * sidebar. */
function revive(doc: unknown): { pins: Pin[]; limit: number } {
  const record = typeof doc === "object" && doc !== null ? (doc as Record<string, unknown>) : {};
  const list = Array.isArray(record["pins"]) ? (record["pins"] as unknown[]) : [];
  const pins = list.filter((pin): pin is Pin => {
    if (typeof pin !== "object" || pin === null) return false;
    const p = pin as Record<string, unknown>;
    switch (p["kind"]) {
      case "tile":
        return typeof p["app"] === "string";
      case "app":
        return typeof p["release"] === "string";
      case "file":
        return typeof p["id"] === "number" && typeof p["path"] === "string" && typeof p["folder"] === "boolean";
      case "repo":
        return typeof p["owner"] === "string" && typeof p["name"] === "string";
      case "issue":
      case "project":
        return typeof p["key"] === "string";
      default:
        return false;
    }
  });
  const limit = typeof record["limit"] === "number" && record["limit"] > 0 ? record["limit"] : DEFAULT_LIMIT;
  return { pins, limit };
}

/** Fetch the pins. One request at a time; nothing without a token. */
export function loadPins(): Promise<void> {
  if (inFlight !== null) return inFlight;
  if (!hasToken()) {
    if (state !== EMPTY) set(EMPTY);
    return Promise.resolve();
  }
  set({ ...state, status: "loading" });
  inFlight = (async () => {
    try {
      const doc = revive(await getPins());
      const [installed, names] = await Promise.all([installedFor(doc.pins), namesFor(doc.pins)]);
      set({ status: "ready", pins: doc.pins, limit: doc.limit, installed, names });
    } catch (error) {
      set(isUnauthorized(error) ? EMPTY : { ...state, status: "failed" });
    } finally {
      inFlight = null;
    }
  })();
  return inFlight;
}

let following = false;

/** Follow the token, as the look does. Started once, from the shell. */
export function followPins(): void {
  if (following) return;
  following = true;
  subscribeAuth(() => {
    void loadPins();
  });
  void loadPins();
}

/** Replace the list on the box, then here. Throws the box's refusal.
 *  `known` names an issue or project pin the caller has the title of. */
export async function savePins(next: readonly Pin[], known?: ReadonlyMap<string, string>): Promise<void> {
  if (known !== undefined && known.size > 0) set({ ...state, names: new Map([...state.names, ...known]) });
  const doc = revive(await putPins(next));
  const installed =
    doc.pins.some((pin) => pin.kind === "app") && state.installed.length === 0
      ? await installedFor(doc.pins)
      : state.installed;
  const named = doc.pins.every((pin) => (pin.kind !== "issue" && pin.kind !== "project") || state.names.has(pinKey(pin)));
  const names = named ? state.names : await namesFor(doc.pins);
  set({ status: "ready", pins: doc.pins, limit: doc.limit, installed, names });
}

export function isPinned(pins: readonly Pin[], pin: Pin): boolean {
  const key = pinKey(pin);
  return pins.some((candidate) => pinKey(candidate) === key);
}

/** The list with `pin` added at the end, or taken out if it is there. */
export function toggled(pins: readonly Pin[], pin: Pin): Pin[] {
  const key = pinKey(pin);
  return isPinned(pins, pin) ? pins.filter((p) => pinKey(p) !== key) : [...pins, pin];
}

/** The list with the pin at `key` moved by `delta` places, held at the ends. */
export function moved(pins: readonly Pin[], key: string, delta: number): Pin[] {
  const from = pins.findIndex((pin) => pinKey(pin) === key);
  if (from < 0) return [...pins];
  const to = Math.max(0, Math.min(pins.length - 1, from + delta));
  const next = [...pins];
  const [pin] = next.splice(from, 1);
  if (pin !== undefined) next.splice(to, 0, pin);
  return next;
}

export function togglePin(pin: Pin, name?: string): Promise<void> {
  return savePins(toggled(state.pins, pin), name === undefined ? undefined : new Map([[pinKey(pin), name]]));
}
