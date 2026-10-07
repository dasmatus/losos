/* The look: the box's background picture and the widgets written by hand.
 *
 * One module-level store read through useSyncExternalStore, the shape the
 * auth state in lib/api.ts and the board in lib/widgets.ts use. It is loaded
 * once a tab holds a token, dropped when the token goes, and every mutation
 * below writes the box first and the snapshot second — so what the page
 * shows is what the box has, never what it hoped to have.
 *
 * The picture reaches the screen as two CSS custom properties on <html>,
 * set through CSSOM (`style.setProperty`), which the admin CSP allows where
 * a `style` attribute or an injected <style> element would be refused (see
 * the house rules in App.tsx). styles/index.css paints `--losos-bg-image`
 * under a veil of the theme's own ground colour at `--losos-bg-veil`
 * opacity, so the same picture reads in light and in dark, and every card on
 * top of it keeps its opaque surface.
 *
 * `img-src 'self'` is why the picture is a URL and not bytes: an uploaded
 * picture is fetched from /api/look/background?v=N without a token (look.rs
 * says why that is the accepted shape), a shipped one from /backgrounds/,
 * and both are this origin.
 */

import {
  deleteBackground,
  deleteHandWidget,
  getLook,
  getToken,
  isUnauthorized,
  postHandWidget,
  postLook,
  putBackground,
  subscribeAuth,
  type HandSpan,
  type HandWidget,
  type HandWidgetDraft,
  type LookBackground,
  type LookBackgroundChoice,
  type LookLimits,
  type LookResponse,
} from "./api";

export type { HandSpan, HandWidget, HandWidgetDraft, LookBackground, LookResponse };

/** The pictures the page knows how to show a thumbnail for. The box sends
 *  its own list; a name in neither is drawn as a plain swatch. */
export const SHIPPED_BACKGROUNDS: readonly string[] = ["tide", "grid", "dusk"] as const;

export function shippedUrl(name: string): string {
  return `/backgrounds/${encodeURIComponent(name)}.svg`;
}

/** Where a background's picture is fetched from, or null for none. */
export function backgroundUrl(background: LookBackground): string | null {
  switch (background.kind) {
    case "none":
      return null;
    case "shipped":
      return shippedUrl(background.name);
    case "upload":
      return background.url;
  }
}

// ── Limits the page enforces before asking ────────────────────────────────

/* The box's numbers win when it has answered; these are what the page uses
 * before then, and they match backend/src/look.rs. */
export const DEFAULT_LIMITS: LookLimits = {
  widgets: 24,
  nameChars: 60,
  sourceBytes: 64 * 1024,
  imageBytes: 8 * 1024 * 1024,
  veil: { min: 20, max: 90 },
};

export const DEFAULT_VEIL = 60;

/** The picture types lososd accepts, by magic number; the file picker's
 *  `accept` and the pre-flight check here agree with it. */
export const IMAGE_TYPES: readonly string[] = [
  "image/png",
  "image/jpeg",
  "image/gif",
  "image/webp",
] as const;

// ── The store ─────────────────────────────────────────────────────────────

export type LookStatus = "idle" | "loading" | "ready" | "failed";

export interface LookState {
  readonly status: LookStatus;
  /** The box's document once it has answered; null before, and after a
   *  sign-out. */
  readonly look: LookResponse | null;
  readonly error: string | null;
}

const EMPTY: LookState = { status: "idle", look: null, error: null };

let state: LookState = EMPTY;
const listeners = new Set<() => void>();
let inFlight: Promise<void> | null = null;

function set(next: LookState): void {
  state = next;
  applyBackground(next.look);
  for (const fn of listeners) fn();
}

export function getLookState(): LookState {
  return state;
}

export function getServerLookState(): LookState {
  return EMPTY;
}

export function subscribeLook(onChange: () => void): () => void {
  listeners.add(onChange);
  return () => {
    listeners.delete(onChange);
  };
}

/* A document from an older lososd, or from a test stub answering `{}`, is
 * read leniently: every field has a default, and a widget that is not the
 * shape is dropped on its own. The page must never blank on this. */
function revive(doc: unknown): LookResponse {
  const record = typeof doc === "object" && doc !== null ? (doc as Record<string, unknown>) : {};
  const background = reviveBackground(record["background"]);
  const veil =
    typeof record["veil"] === "number" && Number.isFinite(record["veil"])
      ? record["veil"]
      : DEFAULT_VEIL;
  const widgets: HandWidget[] = [];
  if (Array.isArray(record["widgets"])) {
    for (const entry of record["widgets"]) {
      const widget = reviveWidget(entry);
      if (widget !== null) widgets.push(widget);
    }
  }
  const shipped = Array.isArray(record["shipped"])
    ? record["shipped"].filter((name): name is string => typeof name === "string")
    : [...SHIPPED_BACKGROUNDS];
  const limits = reviveLimits(record["limits"]);
  return { version: 1, background, veil, widgets, shipped, limits };
}

function reviveBackground(value: unknown): LookBackground {
  if (typeof value !== "object" || value === null) return { kind: "none" };
  const record = value as Record<string, unknown>;
  if (record["kind"] === "shipped" && typeof record["name"] === "string") {
    return { kind: "shipped", name: record["name"] };
  }
  if (
    record["kind"] === "upload" &&
    typeof record["version"] === "number" &&
    typeof record["url"] === "string"
  ) {
    return {
      kind: "upload",
      version: record["version"],
      contentType: typeof record["contentType"] === "string" ? record["contentType"] : "",
      url: record["url"],
    };
  }
  return { kind: "none" };
}

function reviveWidget(value: unknown): HandWidget | null {
  if (typeof value !== "object" || value === null) return null;
  const record = value as Record<string, unknown>;
  const id = record["id"];
  const name = record["name"];
  const source = record["source"];
  if (typeof id !== "string" || typeof name !== "string" || typeof source !== "string") return null;
  const span: HandSpan = record["span"] === "full" ? "full" : "half";
  return { id, name, span, source };
}

function reviveLimits(value: unknown): LookLimits {
  if (typeof value !== "object" || value === null) return DEFAULT_LIMITS;
  const record = value as Record<string, unknown>;
  const number = (key: keyof LookLimits, fallback: number): number => {
    const candidate = record[key];
    return typeof candidate === "number" && candidate > 0 ? candidate : fallback;
  };
  const veil = record["veil"];
  const veilRecord =
    typeof veil === "object" && veil !== null ? (veil as Record<string, unknown>) : {};
  return {
    widgets: number("widgets", DEFAULT_LIMITS.widgets),
    nameChars: number("nameChars", DEFAULT_LIMITS.nameChars),
    sourceBytes: number("sourceBytes", DEFAULT_LIMITS.sourceBytes),
    imageBytes: number("imageBytes", DEFAULT_LIMITS.imageBytes),
    veil: {
      min: typeof veilRecord["min"] === "number" ? veilRecord["min"] : DEFAULT_LIMITS.veil.min,
      max: typeof veilRecord["max"] === "number" ? veilRecord["max"] : DEFAULT_LIMITS.veil.max,
    },
  };
}

/** Fetch the look. One request at a time; a second call while one is out
 *  joins it. Nothing happens without a token. */
export function loadLook(): Promise<void> {
  if (inFlight !== null) return inFlight;
  if (getToken() === null) {
    if (state !== EMPTY) set(EMPTY);
    return Promise.resolve();
  }
  set({ status: "loading", look: state.look, error: null });
  inFlight = (async () => {
    try {
      const doc = await getLook();
      set({ status: "ready", look: revive(doc), error: null });
    } catch (error) {
      if (isUnauthorized(error)) {
        set(EMPTY);
        return;
      }
      set({
        status: "failed",
        look: state.look,
        error: error instanceof Error ? error.message : "no answer",
      });
    } finally {
      inFlight = null;
    }
  })();
  return inFlight;
}

/* Follow the token: a sign-in loads the look, a sign-out drops it along
 * with the wallpaper, so the unlock dialog of the next owner — or the same
 * one, later — sits on the plain ground. Started once, from the shell. */
let following = false;

export function followAuth(): void {
  if (following) return;
  following = true;
  subscribeAuth(() => {
    void loadLook();
  });
  void loadLook();
}

// ── Painting the background ───────────────────────────────────────────────

export const BG_IMAGE_VAR = "--losos-bg-image";
export const BG_VEIL_VAR = "--losos-bg-veil";
export const BG_CLASS = "has-bg";

/* CSSOM writes only: `style.setProperty` and `classList` are allowed under
 * `style-src 'self'`; `setAttribute("style", …)` is not. Idempotent, and
 * cheap enough to run on every snapshot. */
function applyBackground(look: LookResponse | null): void {
  if (typeof document === "undefined") return;
  const root = document.documentElement;
  const url = look === null ? null : backgroundUrl(look.background);
  if (url === null) {
    root.style.removeProperty(BG_IMAGE_VAR);
    root.style.removeProperty(BG_VEIL_VAR);
    root.classList.remove(BG_CLASS);
    return;
  }
  root.style.setProperty(BG_IMAGE_VAR, `url("${url.replace(/["\\]/g, "")}")`);
  root.style.setProperty(BG_VEIL_VAR, String(clampVeil(look?.veil ?? DEFAULT_VEIL) / 100));
  root.classList.add(BG_CLASS);
}

function clampVeil(value: number): number {
  const limits = state.look?.limits.veil ?? DEFAULT_LIMITS.veil;
  return Math.min(limits.max, Math.max(limits.min, Math.round(value)));
}

// ── Mutations ─────────────────────────────────────────────────────────────

function ready(doc: LookResponse): void {
  set({ status: "ready", look: doc, error: null });
}

/** Pick a shipped picture, or none. Resolves once the box has it. */
export async function chooseBackground(choice: LookBackgroundChoice): Promise<void> {
  ready(revive(await postLook({ background: choice })));
}

/** The pre-flight a picker runs before uploading: the type lososd takes and
 *  the size it allows, in a sentence key the caller translates. */
export function checkPicture(file: Blob): "type" | "size" | null {
  if (!IMAGE_TYPES.includes(file.type)) return "type";
  const limit = state.look?.limits.imageBytes ?? DEFAULT_LIMITS.imageBytes;
  if (file.size > limit) return "size";
  return null;
}

/** Upload a picture and make it the background. */
export async function uploadBackground(file: Blob): Promise<void> {
  ready(revive(await putBackground(file)));
}

export async function removeBackground(): Promise<void> {
  ready(revive(await deleteBackground()));
}

/* The veil is dragged, so it is painted at once and saved a moment after
 * the last movement: the box sees one write per adjustment, the screen
 * follows the thumb. */
let veilTimer: ReturnType<typeof setTimeout> | null = null;

export function previewVeil(veil: number): void {
  if (state.look === null) return;
  set({ ...state, look: { ...state.look, veil: clampVeil(veil) } });
}

export function saveVeil(veil: number, delayMs = 400): Promise<void> {
  previewVeil(veil);
  if (veilTimer !== null) clearTimeout(veilTimer);
  return new Promise((resolve, reject) => {
    veilTimer = setTimeout(() => {
      veilTimer = null;
      postLook({ veil: clampVeil(veil) })
        .then((doc) => {
          ready(revive(doc));
          resolve();
        })
        .catch(reject);
    }, delayMs);
  });
}

/** Add a widget, or replace the one its id names. Returns it as the box
 *  keeps it, id included. */
export async function saveHandWidget(draft: HandWidgetDraft): Promise<HandWidget> {
  const { widget } = await postHandWidget(draft);
  const kept = reviveWidget(widget);
  if (kept === null) throw new Error("the box answered with no widget");
  if (state.look !== null) {
    const widgets = state.look.widgets.some((w) => w.id === kept.id)
      ? state.look.widgets.map((w) => (w.id === kept.id ? kept : w))
      : [...state.look.widgets, kept];
    ready({ ...state.look, widgets });
  }
  return kept;
}

export async function removeHandWidget(id: string): Promise<void> {
  await deleteHandWidget(id);
  if (state.look !== null) {
    ready({ ...state.look, widgets: state.look.widgets.filter((w) => w.id !== id) });
  }
}

/** The widget `id` names, if the box has it. */
export function handWidget(id: string): HandWidget | undefined {
  return state.look?.widgets.find((w) => w.id === id);
}

export function lookLimits(): LookLimits {
  return state.look?.limits ?? DEFAULT_LIMITS;
}

/* Testing seam: forget everything, so a test can start from a cold store. */
export function resetLookStore(): void {
  state = EMPTY;
  inFlight = null;
  applyBackground(null);
}
