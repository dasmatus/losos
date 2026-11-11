/* Two-state wide mode: the shell keeps its margins and column ceilings, or
 * the bar and the page take the whole window — no gaps.
 *
 * The choice is an attribute on <html> and nothing else:
 *
 *   off -> no data-wide attribute. The shell lays out as it always has.
 *   on  -> data-wide="on". styles/index.css drops the column ceilings and
 *          the padding and the gaps of the bar and of [data-shell-body]
 *          under that attribute.
 *
 * The shape of lib/theme.ts, minus the middle state: a layout is on or off.
 * Deliberately framework-agnostic for the same reason that file is —
 * src/theme-boot.ts stamps it before the first paint too (see build.ts), so
 * a wide page does not start narrow and snap wide once React is up. The
 * React binding lives in components/ui/wide-toggle.tsx.
 *
 * localStorage, not sessionStorage: like the theme, a layout the owner
 * picked once should still be there when they come back.
 */

export const WIDE_KEY = "losos-wide";

/* Every storage access is guarded, as in lib/theme.ts: localStorage throws
 * outright in some privacy modes, and a layout is not worth the page. */
function readStored(): boolean {
  try {
    return window.localStorage.getItem(WIDE_KEY) === "on";
  } catch {
    return false;
  }
}

function writeStored(on: boolean): void {
  try {
    if (on) window.localStorage.setItem(WIDE_KEY, "on");
    else window.localStorage.removeItem(WIDE_KEY);
  } catch {
    /* not fatal; the choice just will not survive the tab */
  }
}

/** Stamp the choice on <html>. Off removes the attribute rather than setting it. */
export function applyWide(on: boolean): void {
  const root = document.documentElement;
  if (on) root.setAttribute("data-wide", "on");
  else root.removeAttribute("data-wide");
}

/** What theme-boot.ts runs, and what main.tsx re-runs as a belt-and-braces. */
export function applyStoredWide(): void {
  applyWide(readStored());
}

const listeners = new Set<() => void>();
let current: boolean | null = null;

function notify(): void {
  for (const fn of listeners) fn();
}

/** The stored choice. Cached so useSyncExternalStore sees a stable snapshot. */
export function getWide(): boolean {
  current ??= readStored();
  return current;
}

/** Store the choice, stamp it on <html>, and wake every subscriber. */
export function setWide(on: boolean): void {
  if (getWide() === on) return;
  current = on;
  writeStored(on);
  applyWide(on);
  notify();
}

/* Fires on a choice made in this tab, and on another tab writing the key —
 * a box is often open twice, and the second tab should not keep margins the
 * first has just dropped. */
export function subscribeWide(onChange: () => void): () => void {
  listeners.add(onChange);

  const onStorage = (event: StorageEvent): void => {
    if (event.key !== null && event.key !== WIDE_KEY) return;
    current = readStored();
    applyWide(current);
    notify();
  };
  window.addEventListener("storage", onStorage);

  return () => {
    listeners.delete(onChange);
    window.removeEventListener("storage", onStorage);
  };
}
