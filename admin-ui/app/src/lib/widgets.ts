/* The widget board: what is on it, and where that is remembered.
 *
 * Widgets are a property of THIS BROWSER, not of the appliance. Adding one
 * writes localStorage and nothing else — no /api/apply, no overrides.nix, no
 * rebuild. That is the whole design constraint: a rebuild takes minutes,
 * restarts lososd underneath itself and is the one operation on this box that
 * can fail in a way nobody can log in to fix. Arranging tiles on a dashboard
 * must never be able to reach it.
 *
 * localStorage rather than sessionStorage because a board that empties itself
 * when the tab closes is not a board. It is per-origin, and every appliance is
 * its own origin, so two boxes cannot inherit each other's layout.
 *
 * The board starts EMPTY. Nothing here is on by default; the owner opens the
 * gallery and picks. A fresh box shows an invitation, not a wall of tiles.
 */

import { parseSpec, type WidgetSpec } from "@/widgets/spec";

// ── Built-ins ─────────────────────────────────────────────────────────────

/* The ids only. What each one draws lives in widgets/catalogue.tsx, which
 * imports this — never the other way round, so that persistence does not drag
 * React and the five widget implementations into anything that only wants to
 * know what is on the board. */
export type BuiltinId = "uptime" | "disk" | "mesh" | "apps" | "rebuilds";

export const BUILTIN_IDS: readonly BuiltinId[] = [
  "uptime",
  "disk",
  "mesh",
  "apps",
  "rebuilds",
] as const;

export function isBuiltinId(value: unknown): value is BuiltinId {
  return typeof value === "string" && (BUILTIN_IDS as readonly string[]).includes(value);
}

// ── What sits on the board ────────────────────────────────────────────────

/** Where a tile's behaviour comes from: compiled in, or written by the owner. */
export type WidgetSource =
  | { readonly kind: "builtin"; readonly id: BuiltinId }
  | { readonly kind: "custom"; readonly spec: WidgetSpec };

export interface WidgetInstance {
  /** Stable for the life of the tile. Not derived from the source: the same
   *  built-in can legitimately appear twice, showing different windows. */
  readonly id: string;
  readonly source: WidgetSource;
  /** Overrides the title the widget returns. Empty means "use the widget's". */
  readonly title?: string;
}

export interface BoardState {
  readonly version: 1;
  readonly widgets: readonly WidgetInstance[];
}

const EMPTY_BOARD: BoardState = { version: 1, widgets: [] };

// ── Ids ───────────────────────────────────────────────────────────────────

/* crypto.randomUUID() is deliberately NOT used.
 *
 * It is gated on a secure context, and this page is reached at
 * http://<hostName>.local — which is not one. The call is simply `undefined`
 * there, so the first "Add widget" would throw a TypeError on a real box
 * while working perfectly on localhost, which is a secure context and is
 * where it would have been tested. A counter plus the clock is enough: these
 * ids are array keys in one browser, not identifiers anything trusts. */
let counter = 0;

function makeId(): string {
  counter += 1;
  const random = Math.floor(Math.random() * 0xfffff).toString(36);
  return `w${Date.now().toString(36)}${counter.toString(36)}${random}`;
}

// ── Persistence ───────────────────────────────────────────────────────────

export const STORAGE_KEY = "losos-widgets";

/* Reading is total: a board that fails to parse is an empty board, never an
 * exception. The dashboard is the first thing rendered after sign-in, and a
 * corrupt value in localStorage — a half-written record, a key an older
 * version wrote — must not be able to blank the page. Unknown entries are
 * dropped one by one, so one bad custom widget costs its own tile and not the
 * other five. */
function readBoard(): BoardState {
  let raw: string | null = null;
  try {
    raw = window.localStorage.getItem(STORAGE_KEY);
  } catch {
    return EMPTY_BOARD;
  }
  if (raw === null) return EMPTY_BOARD;

  let parsed: unknown;
  try {
    parsed = JSON.parse(raw);
  } catch {
    return EMPTY_BOARD;
  }
  if (typeof parsed !== "object" || parsed === null) return EMPTY_BOARD;

  const list = (parsed as { widgets?: unknown }).widgets;
  if (!Array.isArray(list)) return EMPTY_BOARD;

  const widgets: WidgetInstance[] = [];
  for (const entry of list) {
    const instance = reviveInstance(entry);
    if (instance !== null) widgets.push(instance);
  }
  return { version: 1, widgets };
}

function reviveInstance(entry: unknown): WidgetInstance | null {
  if (typeof entry !== "object" || entry === null) return null;
  const record = entry as Record<string, unknown>;

  const id = typeof record["id"] === "string" && record["id"].length > 0 ? record["id"] : makeId();
  const title = typeof record["title"] === "string" ? record["title"] : undefined;

  const source = record["source"];
  if (typeof source !== "object" || source === null) return null;
  const kind = (source as Record<string, unknown>)["kind"];

  if (kind === "builtin") {
    const builtin = (source as Record<string, unknown>)["id"];
    if (!isBuiltinId(builtin)) return null;
    return title === undefined
      ? { id, source: { kind: "builtin", id: builtin } }
      : { id, title, source: { kind: "builtin", id: builtin } };
  }

  if (kind === "custom") {
    try {
      const spec = parseSpec((source as Record<string, unknown>)["spec"]);
      return title === undefined
        ? { id, source: { kind: "custom", spec } }
        : { id, title, source: { kind: "custom", spec } };
    } catch {
      // A spec this build no longer understands. Dropping the tile is better
      // than keeping one that can only ever render its own error.
      return null;
    }
  }

  return null;
}

function writeBoard(next: BoardState): void {
  try {
    window.localStorage.setItem(STORAGE_KEY, JSON.stringify(next));
  } catch {
    /* Private mode, or the quota. The board still works for this session. */
  }
}

// ── The store ─────────────────────────────────────────────────────────────

/* A module-level store read through useSyncExternalStore, the same shape the
 * auth state in lib/api.ts uses. The snapshot is a stable reference that only
 * changes when the board changes — returning a fresh object each call makes
 * React re-render forever. */
let board: BoardState | null = null;
const listeners = new Set<() => void>();

function current(): BoardState {
  if (board === null) board = readBoard();
  return board;
}

function commit(widgets: readonly WidgetInstance[]): void {
  board = { version: 1, widgets };
  writeBoard(board);
  for (const fn of listeners) fn();
}

/** The board as it stands. Stable between changes; safe as a store snapshot. */
export function getBoard(): BoardState {
  return current();
}

/** Server-render / first-paint fallback. There is no server here, but
 *  useSyncExternalStore wants one and an empty board is the honest answer. */
export function getServerBoard(): BoardState {
  return EMPTY_BOARD;
}

export function subscribeBoard(onChange: () => void): () => void {
  listeners.add(onChange);
  return () => {
    listeners.delete(onChange);
  };
}

// ── Mutations ─────────────────────────────────────────────────────────────

/** How many tiles one board may hold. Every tile polls; a hundred of them
 *  would turn a dashboard into a load generator pointed at a mini-PC. */
export const MAX_WIDGETS = 12;

export function isBoardFull(): boolean {
  return current().widgets.length >= MAX_WIDGETS;
}

/** Add a built-in. Returns the new tile's id, or null if the board is full. */
export function addBuiltin(id: BuiltinId): string | null {
  const widgets = current().widgets;
  if (widgets.length >= MAX_WIDGETS) return null;
  const instance: WidgetInstance = { id: makeId(), source: { kind: "builtin", id } };
  commit([...widgets, instance]);
  return instance.id;
}

/** Add a widget the owner wrote. The spec is re-validated here even though
 *  the editor already validated it: this is the only door into the board. */
export function addCustom(spec: WidgetSpec): string | null {
  const widgets = current().widgets;
  if (widgets.length >= MAX_WIDGETS) return null;
  const instance: WidgetInstance = { id: makeId(), source: { kind: "custom", spec: parseSpec(spec) } };
  commit([...widgets, instance]);
  return instance.id;
}

export function removeWidget(id: string): void {
  const widgets = current().widgets;
  const next = widgets.filter((widget) => widget.id !== id);
  if (next.length !== widgets.length) commit(next);
}

/** Move a tile one place earlier or later. No drag-and-drop: two buttons work
 *  with a keyboard, a touchscreen and a screen reader, and drag does not. */
export function moveWidget(id: string, direction: "up" | "down"): void {
  const widgets = [...current().widgets];
  const index = widgets.findIndex((widget) => widget.id === id);
  if (index < 0) return;
  const target = direction === "up" ? index - 1 : index + 1;
  if (target < 0 || target >= widgets.length) return;
  const moved = widgets[index];
  const displaced = widgets[target];
  if (moved === undefined || displaced === undefined) return;
  widgets[index] = displaced;
  widgets[target] = moved;
  commit(widgets);
}

/** Rename a tile. An empty or blank name restores the widget's own title. */
export function renameWidget(id: string, title: string): void {
  const trimmed = title.trim();
  const widgets = current().widgets.map((widget) => {
    if (widget.id !== id) return widget;
    if (trimmed.length === 0) {
      return { id: widget.id, source: widget.source } satisfies WidgetInstance;
    }
    return { id: widget.id, source: widget.source, title: trimmed } satisfies WidgetInstance;
  });
  commit(widgets);
}

export function clearBoard(): void {
  if (current().widgets.length > 0) commit([]);
}

/** Whether this built-in is already on the board — the gallery dims the row. */
export function hasBuiltin(id: BuiltinId): boolean {
  return current().widgets.some(
    (widget) => widget.source.kind === "builtin" && widget.source.id === id,
  );
}

/* Testing seam: drop the cached snapshot so the next read comes off storage
 * again. Nothing in the app calls this; it exists so a future test can put a
 * board into localStorage and see it. */
export function resetBoardCache(): void {
  board = null;
}
