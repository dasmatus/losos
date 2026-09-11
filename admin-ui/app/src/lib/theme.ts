/* Three-state theme: Auto (follow the browser), Light, Dark.
 *
 * The choice is an attribute on <html> and nothing else:
 *
 *   auto   -> no data-theme attribute. The palette in styles/index.css has a
 *             `@media (prefers-color-scheme: dark)` arm guarded by
 *             `:root:not([data-theme="light"])`, so Auto is correct with zero
 *             JavaScript — including before this module has loaded.
 *   light  -> data-theme="light". The media arm's :not() excludes it, so the
 *             base :root palette wins even on a dark-preferring browser.
 *   dark   -> data-theme="dark". A later, equally specific rule than the media
 *             arm, so it wins in both directions.
 *
 * Deliberately framework-agnostic: src/theme-boot.ts compiles this file into
 * the tiny blocking script that runs before first paint (see vite.config.ts),
 * and anything React in here would drag React into that script. The React
 * binding lives in components/ui/theme-toggle.tsx.
 *
 * localStorage, not sessionStorage: the admin token is per-tab on purpose, a
 * theme is not — a box the owner visits once a month should still come up in
 * the colours they picked.
 */

export type Theme = "auto" | "light" | "dark";
export type ResolvedTheme = "light" | "dark";

export const THEME_KEY = "losos-theme";

export const THEMES: readonly Theme[] = ["auto", "light", "dark"] as const;

export function isTheme(value: unknown): value is Theme {
  return value === "auto" || value === "light" || value === "dark";
}

/* Every storage access is guarded: localStorage throws outright in some
 * privacy modes, and a theme is not worth taking the page down for. */
function readStored(): Theme {
  try {
    const raw = window.localStorage.getItem(THEME_KEY);
    return isTheme(raw) ? raw : "auto";
  } catch {
    return "auto";
  }
}

function writeStored(theme: Theme): void {
  try {
    if (theme === "auto") window.localStorage.removeItem(THEME_KEY);
    else window.localStorage.setItem(THEME_KEY, theme);
  } catch {
    /* not fatal; the choice just will not survive the tab */
  }
}

/** Stamp `theme` onto <html>. Auto removes the attribute rather than setting it. */
export function applyTheme(theme: Theme): void {
  const root = document.documentElement;
  if (theme === "auto") root.removeAttribute("data-theme");
  else root.setAttribute("data-theme", theme);
}

/** What theme-boot.ts runs, and what main.tsx re-runs as a belt-and-braces. */
export function applyStoredTheme(): void {
  applyTheme(readStored());
}

const listeners = new Set<() => void>();
let current: Theme | null = null;

function notify(): void {
  for (const fn of listeners) fn();
}

/** The stored choice. Cached so useSyncExternalStore sees a stable snapshot. */
export function getTheme(): Theme {
  current ??= readStored();
  return current;
}

/** Store the choice, stamp it on <html>, and wake every subscriber. */
export function setTheme(theme: Theme): void {
  if (getTheme() === theme) return;
  current = theme;
  writeStored(theme);
  applyTheme(theme);
  notify();
}

function prefersDark(): boolean {
  return window.matchMedia("(prefers-color-scheme: dark)").matches;
}

/** The colour actually on screen — Auto collapsed against the browser. */
export function getResolvedTheme(): ResolvedTheme {
  const theme = getTheme();
  if (theme !== "auto") return theme;
  return prefersDark() ? "dark" : "light";
}

/* Fires on a choice made in this tab, on the browser's preference flipping
 * (which only moves the resolved theme while the choice is Auto), and on
 * another tab writing the key — a box is often open twice, and the second tab
 * should not sit in the old palette. */
export function subscribeTheme(onChange: () => void): () => void {
  listeners.add(onChange);

  const media = window.matchMedia("(prefers-color-scheme: dark)");
  const onMedia = (): void => notify();
  media.addEventListener("change", onMedia);

  const onStorage = (event: StorageEvent): void => {
    if (event.key !== null && event.key !== THEME_KEY) return;
    current = readStored();
    applyTheme(current);
    notify();
  };
  window.addEventListener("storage", onStorage);

  return () => {
    listeners.delete(onChange);
    media.removeEventListener("change", onMedia);
    window.removeEventListener("storage", onStorage);
  };
}
