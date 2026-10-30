/* The signed-in person's colour as the page's accent.
 *
 * A colour picked under Your account › Profile is that person's: their ring
 * and avatar everywhere, and on their own pages the accent too. It is an
 * attribute on <html>, `data-person-accent="<name>"`, which styles/index.css
 * turns into `--accent` and `--accent-wash` from the matching person tokens
 * of styles/tokens.css, in light and in dark alike. No attribute is the
 * box's own teal: the spare key, nobody signed in, and a person who never
 * picked a colour (the one their name gives them is for telling people
 * apart, not for their pages).
 *
 * The pages without JavaScript put the same attribute on <html> themselves
 * (backend/plugins/page, `shell::document`).
 *
 * The last person's colour is kept in localStorage so that src/theme-boot.ts
 * can stamp it before the first paint, the way it stamps the theme; the bar
 * corrects it as soon as it knows who is signed in, and signing out drops
 * it. Like lib/theme.ts this file stays free of React, because theme-boot
 * inlines whatever it imports into a blocking script.
 */

export const PERSON_ACCENT_KEY = "losos-accent";

/* The names styles/index.css has a rule for: lib/api.ts's ACCENTS, written
 * out so that theme-boot does not pull in the whole API module. Teal is
 * the box's own accent, so it needs no attribute. */
const NAMES: readonly string[] = ["blue", "violet", "rose", "amber", "green"];

function wanted(accent: string | null): string | null {
  return accent !== null && NAMES.includes(accent) ? accent : null;
}

function stamp(accent: string | null): void {
  const root = document.documentElement;
  if (accent === null) root.removeAttribute("data-person-accent");
  else root.setAttribute("data-person-accent", accent);
}

/** Show `accent` (a picked colour, or null for the box's teal) and keep it
 *  for the next page load. */
export function applyPersonAccent(accent: string | null): void {
  const shown = wanted(accent);
  stamp(shown);
  try {
    if (shown === null) window.localStorage.removeItem(PERSON_ACCENT_KEY);
    else window.localStorage.setItem(PERSON_ACCENT_KEY, shown);
  } catch {
    /* not fatal; the colour is just found again on the next load */
  }
}

/** For theme-boot: the last signed-in person's colour, before anything paints. */
export function applyStoredPersonAccent(): void {
  try {
    stamp(wanted(window.localStorage.getItem(PERSON_ACCENT_KEY)));
  } catch {
    /* the box's teal, then */
  }
}

