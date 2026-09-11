/* The pre-paint theme stamp.
 *
 * vite.config.ts compiles this file on its own into a standalone IIFE at
 * /theme-boot.js, which index.html loads as a blocking classic script in
 * <head>. It runs before the body is parsed, so the palette is already
 * decided by the time anything is painted.
 *
 * Keep it importing nothing but lib/theme.ts, and keep lib/theme.ts free of
 * React: whatever this file reaches ends up inlined into that script, and a
 * blocking script in <head> is the one place in the app where every byte is
 * paid for before the user sees anything.
 */
import { applyStoredTheme } from "./lib/theme";

applyStoredTheme();
