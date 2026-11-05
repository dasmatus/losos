/* The pre-paint theme stamp.
 *
 * vite.config.ts compiles this file on its own into a standalone IIFE at
 * /theme-boot.js, which index.html loads as a blocking classic script in
 * <head>. It runs before the body is parsed, so the palette is already
 * decided by the time anything is painted.
 *
 * It stamps the last signed-in person's colour too (lib/person-accent.ts),
 * so their accent does not arrive a moment after the page.
 *
 * Keep it importing nothing but lib/theme.ts and lib/person-accent.ts, and
 * keep both free of React: whatever this file reaches ends up inlined into that script, and a
 * blocking script in <head> is the one place in the app where every byte is
 * paid for before the user sees anything.
 */
import { applyStoredPersonAccent } from "./lib/person-accent";
import { applyStoredTheme } from "./lib/theme";

applyStoredTheme();
applyStoredPersonAccent();
