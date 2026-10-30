/* The pages an administrator changed, for every screen that draws one.
 *
 * GET /api/pages answers once per sign-in with the HTML of each changed
 * page and whether there is a stylesheet of theirs; a page nobody changed
 * is not in it, and draws the default the bundle carries. The answer is
 * also kept in localStorage, so a reload draws the changed page at once
 * instead of the default first, and only a first visit waits for the box
 * (a short while, then the default).
 *
 * An administrator's editor (./editor.tsx) puts a draft here while they
 * type, and the page under it draws the draft. */

import { getPages, hasToken, subscribeAuth, type PagesResponse } from "@/lib/api";

export interface PagesState {
  /** False until the box has answered, or the wait for it ran out. */
  readonly ready: boolean;
  /** Page id to its HTML, for the pages an administrator changed. */
  readonly custom: Readonly<Record<string, string>>;
  /** The stylesheet's version, or null for none. */
  readonly style: string | null;
  /** Page id to the HTML of a draft being edited. */
  readonly drafts: Readonly<Record<string, string>>;
  /** The stylesheet being edited, drawn in place of the saved one. */
  readonly styleDraft: string | null;
  /** What the editor has open, or null when it is closed. */
  readonly editor: EditorTarget | null;
}

/** A page by id, or the stylesheet every page shares. */
export type EditorTarget = { readonly kind: "page"; readonly id: string } | { readonly kind: "style" };

const CACHE = "losos.pages";
/** How long a first visit waits for the box before drawing the defaults. */
const WAIT_MS = 800;

let state: PagesState = { ready: false, custom: {}, style: null, drafts: {}, styleDraft: null, editor: null };
const listeners = new Set<() => void>();

function set(next: Partial<PagesState>): void {
  state = { ...state, ...next };
  applyStyle(state.style, state.styleDraft);
  for (const fn of listeners) fn();
}

export function getPagesState(): PagesState {
  return state;
}

export function subscribePages(onChange: () => void): () => void {
  listeners.add(onChange);
  return () => {
    listeners.delete(onChange);
  };
}

/* What the box sent, kept to the shape the pages read. An older lososd, a
 * test page answering with its fallback, or anything else is no changes. */
function revive(doc: unknown): { custom: Record<string, string>; style: string | null } {
  const custom: Record<string, string> = {};
  const record = typeof doc === "object" && doc !== null ? (doc as Partial<PagesResponse>) : {};
  const pages = typeof record.pages === "object" && record.pages !== null ? record.pages : {};
  for (const [id, page] of Object.entries(pages)) {
    if (typeof page === "object" && page !== null && typeof page.html === "string") custom[id] = page.html;
  }
  const style = typeof record.style === "string" && record.style.length > 0 ? record.style : null;
  return { custom, style };
}

function readCache(): { custom: Record<string, string>; style: string | null } | null {
  try {
    const raw = localStorage.getItem(CACHE);
    return raw === null ? null : revive(JSON.parse(raw));
  } catch {
    return null;
  }
}

function writeCache(doc: { custom: Record<string, string>; style: string | null }): void {
  try {
    const pages: Record<string, { html: string }> = {};
    for (const [id, html] of Object.entries(doc.custom)) pages[id] = { html };
    localStorage.setItem(CACHE, JSON.stringify({ pages, style: doc.style }));
  } catch {
    /* private window or full storage: the next visit waits again */
  }
}

/** Takes an answer the box gave (a load, or a save in the editor). */
export function takePages(doc: unknown): void {
  const next = revive(doc);
  writeCache(next);
  set({ ...next, ready: true });
}

let loading: Promise<void> | null = null;

async function load(): Promise<void> {
  if (!hasToken()) {
    set({ ready: true, custom: {}, style: null, drafts: {}, styleDraft: null, editor: null });
    return;
  }
  const cached = readCache();
  if (cached !== null) set({ ...cached, ready: true });
  const timer = setTimeout(() => set({ ready: true }), WAIT_MS);
  try {
    takePages(await getPages());
  } catch {
    // No pages plugin (an older box) or no answer: the defaults, or what
    // the last visit saw.
    set({ ready: true });
  } finally {
    clearTimeout(timer);
  }
}

function reload(): void {
  loading = load().finally(() => {
    loading = null;
  });
}

let following = false;

/** Loads the pages on sign-in and forgets them on sign-out. Once. */
export function followPages(): void {
  if (following) return;
  following = true;
  subscribeAuth(() => {
    if (loading === null) reload();
  });
  reload();
}

/** A draft for `id` from the editor, or null to drop it. */
export function setDraft(id: string, html: string | null): void {
  const drafts = { ...state.drafts };
  if (html === null) delete drafts[id];
  else drafts[id] = html;
  set({ drafts });
}

/** The stylesheet being edited, or null to draw the saved one again. */
export function setStyleDraft(css: string | null): void {
  set({ styleDraft: css });
}

/** Opens the editor on a page or the stylesheet; one at a time. */
export function openPageEditor(target: EditorTarget): void {
  set({ editor: target, drafts: {}, styleDraft: null });
}

/** Closes the editor and drops whatever it had not saved. */
export function closePageEditor(): void {
  set({ editor: null, drafts: {}, styleDraft: null });
}

// ── The administrators' stylesheet ────────────────────────────────────────

/* A <link> at the end of <body>, after every stylesheet Vite puts in
 * <head> (the lazy screens' included), so an equal rule of the
 * administrator's wins. Served from the box, which the admin page's
 * `style-src 'self'` allows. */
const LINK_ID = "losos-pages-style";

/* A draft is a constructed stylesheet adopted by the document, a CSSOM
 * write the policy allows, with the saved one switched off under it. */
let draftSheet: CSSStyleSheet | null = null;

function applyDraft(css: string | null): void {
  if (css === null) {
    if (draftSheet !== null) {
      const sheet = draftSheet;
      document.adoptedStyleSheets = document.adoptedStyleSheets.filter((s) => s !== sheet);
      draftSheet = null;
    }
    return;
  }
  if (draftSheet === null) {
    draftSheet = new CSSStyleSheet();
    document.adoptedStyleSheets = [...document.adoptedStyleSheets, draftSheet];
  }
  draftSheet.replaceSync(css);
}

function applyStyle(version: string | null, css: string | null): void {
  if (typeof document === "undefined") return;
  applyDraft(css);
  const existing = document.getElementById(LINK_ID) as HTMLLinkElement | null;
  if (existing !== null) existing.disabled = css !== null;
  if (version === null) {
    existing?.remove();
    return;
  }
  const href = `/api/pages/style.css?v=${encodeURIComponent(version)}`;
  if (existing?.getAttribute("href") === href) return;
  const link = existing ?? document.createElement("link");
  link.id = LINK_ID;
  link.rel = "stylesheet";
  link.href = href;
  link.disabled = css !== null;
  if (existing === null) document.body.append(link);
}
