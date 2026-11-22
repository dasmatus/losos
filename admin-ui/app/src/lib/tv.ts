/* Three-state TV layout: Auto (leanback when the browser says TV), On, Off.
 *
 * The resolved choice is an attribute on <html> and nothing else:
 *
 *   off  -> no data-tv attribute. The shell lays out as on a desktop browser.
 *   on   -> data-tv="on". The shell mounts the television (src/tv/view.tsx)
 *           and styles/index.css gives it the 10-foot type, the overscan
 *           margin and the focus ring.
 *   auto -> data-tv="on" only when userAgentLooksTv() is true (Android TV,
 *           Apple TV web views, Fire TV, Chromecast, many smart-TV browsers).
 *
 * Deliberately framework-agnostic for the same reason lib/theme.ts and
 * lib/wide.ts are — src/theme-boot.ts stamps it before the first paint too
 * (see build.ts), so a TV browser does not start desktop-tight and snap
 * loose once React is up. The React binding lives in components/ui/tv-toggle.tsx.
 *
 * localStorage, not sessionStorage: like the theme, a layout the owner picked
 * once should still be there when they come back.
 */

export type TvLayout = "auto" | "on" | "off";

export const TV_KEY = "losos-tv";

export const TV_LAYOUTS: readonly TvLayout[] = ["auto", "on", "off"] as const;

/** User-agents LosOS treats as a leanback / 10-foot browser. */
const TV_UA =
  /TV|AndroidTV|AppleTV|AFTT|AFTM|AFTB|AFTKA|AFTKM|CrKey|GoogleTV|SmartTV|HbbTV|NetCast|Web0S|webOS\.TV|Tizen.*SMART-TV|BRAVIA|Opera TV|Panasonic|Viera|Philips|OMI\//i;

export function isTvLayout(value: unknown): value is TvLayout {
  return value === "auto" || value === "on" || value === "off";
}

export function userAgentLooksTv(ua: string = typeof navigator !== "undefined" ? navigator.userAgent : ""): boolean {
  return TV_UA.test(ua);
}

function forcedPlatform(): string | undefined {
  let forced: string | undefined;
  try {
    forced = (globalThis as { __LOSOS_FORCE_PLATFORM__?: string }).__LOSOS_FORCE_PLATFORM__;
  } catch {
    forced = undefined;
  }
  if (!forced) {
    try {
      forced = (globalThis as { window?: { __LOSOS_FORCE_PLATFORM__?: string } }).window?.__LOSOS_FORCE_PLATFORM__;
    } catch {
      forced = undefined;
    }
  }
  if (!forced && typeof window !== "undefined") {
    try {
      const params = new URLSearchParams(window.location.search);
      forced = params.get("LOSOS_FORCE_PLATFORM") ?? undefined;
    } catch {
      forced = undefined;
    }
  }
  return forced;
}

/** Whether leanback layout is active for a stored mode and the current browser. */
export function tvLayoutActive(mode: TvLayout): boolean {
  const forced = forcedPlatform();
  if (forced === "tv") return true;
  if (mode === "on") return true;
  if (mode === "off") return false;
  return userAgentLooksTv();
}

function readStored(): TvLayout {
  try {
    const raw = window.localStorage.getItem(TV_KEY);
    return isTvLayout(raw) ? raw : "auto";
  } catch {
    return "auto";
  }
}

function writeStored(mode: TvLayout): void {
  try {
    if (mode === "auto") window.localStorage.removeItem(TV_KEY);
    else window.localStorage.setItem(TV_KEY, mode);
  } catch {
    /* not fatal; the choice just will not survive the tab */
  }
}

/** Stamp the resolved leanback choice on <html>. Off removes the attribute. */
export function applyTvLayout(mode: TvLayout): void {
  const root = document.documentElement;
  if (tvLayoutActive(mode)) root.setAttribute("data-tv", "on");
  else root.removeAttribute("data-tv");
}

/** What theme-boot.ts runs, and what main.tsx re-runs as a belt-and-braces. */
export function applyStoredTvLayout(): void {
  applyTvLayout(readStored());
}

const listeners = new Set<() => void>();
let current: TvLayout | null = null;

function notify(): void {
  for (const fn of listeners) fn();
}

/** The stored choice. Cached so useSyncExternalStore sees a stable snapshot. */
export function getTvLayout(): TvLayout {
  current ??= readStored();
  return current;
}

/** Whether leanback layout is on screen right now (mode resolved against UA). */
export function getTvLayoutActive(): boolean {
  return tvLayoutActive(getTvLayout());
}

/** Store the choice, stamp it on <html>, and wake every subscriber. */
export function setTvLayout(mode: TvLayout): void {
  if (getTvLayout() === mode) {
    applyTvLayout(mode);
    return;
  }
  current = mode;
  writeStored(mode);
  applyTvLayout(mode);
  notify();
}

export function subscribeTvLayout(onChange: () => void): () => void {
  listeners.add(onChange);

  const onStorage = (event: StorageEvent): void => {
    if (event.key !== null && event.key !== TV_KEY) return;
    current = readStored();
    applyTvLayout(current);
    notify();
  };
  window.addEventListener("storage", onStorage);

  return () => {
    listeners.delete(onChange);
    window.removeEventListener("storage", onStorage);
  };
}
