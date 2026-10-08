/* Which logo the pages show today.
 *
 * The logo is a live coho salmon (losos is Slovak for salmon), cut out of a
 * NOAA Fisheries photo. On 31 October, by the viewer's own calendar, it is
 * the earlier plate of salmon instead. A fish on a plate is a dead fish, and
 * Halloween is the one day a year that suits it. Both are bundled PNGs, so
 * img-src 'self' holds either way. Both sources and their licences are in
 * admin-ui/themes/brand/CREDITS.md. */
import salmonUrl from "@/assets/losos.png";
import plateUrl from "@/assets/losos-halloween.png";
import type { MessageKey } from "@/lib/i18n";

export interface Logo {
  url: string;
  /** The sentence that says what the picture is: tooltip and accessible name. */
  tip: MessageKey;
}

export function isHalloween(now: Date = new Date()): boolean {
  return now.getMonth() === 9 && now.getDate() === 31;
}

export function logoFor(now: Date = new Date()): Logo {
  return isHalloween(now)
    ? { url: plateUrl, tip: "shell.logoTipHalloween" }
    : { url: salmonUrl, tip: "shell.logoTip" };
}

/** Point the tab's icon at today's logo. index.html names the salmon, which
 * is right on every other day, so this only changes anything on Halloween. */
export function applyFavicon(now: Date = new Date()): void {
  const link = document.querySelector<HTMLLinkElement>('link[rel="icon"]');
  if (link !== null) link.href = logoFor(now).url;
}
