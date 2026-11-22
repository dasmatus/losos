/* The pre-paint theme stamp.
 *
 * build.ts compiles this file on its own into a standalone IIFE at
 * /theme-boot.js, which each page loads as a blocking classic script in
 * <head>. It runs before the body is parsed, so the palette is already
 * decided by the time anything is painted.
 *
 * It stamps the last signed-in person's colour too (lib/person-accent.ts),
 * so their accent does not arrive a moment after the page, the wide
 * choice (lib/wide.ts), so a wide page does not start narrow and snap wide,
 * and the television (lib/tv.ts), so a set does not paint the desk and
 * then jump to the 10-foot view.
 *
 * Keep it importing nothing but lib/theme.ts, lib/person-accent.ts,
 * lib/wide.ts and lib/tv.ts, and keep all four free of React: whatever this file reaches
 * ends up inlined into that script, and a
 * blocking script in <head> is the one place in the app where every byte is
 * paid for before the user sees anything.
 */
import { applyStoredPersonAccent } from "./lib/person-accent";
import { applyStoredTheme } from "./lib/theme";
import { applyStoredTvLayout } from "./lib/tv";
import { applyStoredWide } from "./lib/wide";

applyStoredTheme();
applyStoredPersonAccent();
applyStoredWide();
applyStoredTvLayout();
