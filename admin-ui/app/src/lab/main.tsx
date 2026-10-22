/* The hosted copy's entry: diagrams/index.html loads this, and only the
 * hosted build (VITE_LAB_HOSTED=1, admin-ui/lab/engine/build.sh) has that
 * page. On a box LosOS Diagrams is a screen of the admin UI at /diagrams
 * (App.tsx). This page is the same screen with no box behind it. It shares
 * the SPA's components, palette, theme and language. */

import { createRoot } from "react-dom/client";
import { applyFavicon } from "@/lib/logo";
import { applyStoredTheme } from "@/lib/theme";
/* The same two stylesheets in the same order as src/main.tsx. Both pages
 * import them, so the bundler moves them into one shared CSS chunk, and it
 * keeps them in the order the entries agree on. With only index.css here,
 * index.css alone went to the shared chunk, which loads first, and Sonner's
 * own rules (left in the admin page's chunk) then beat the house overrides
 * there: tests/toasts.browser.mjs caught the 8px radius. The Lab draws no
 * Sonner toast; this import costs it a few kB of CSS and nothing else. */
import "sonner/dist/styles.css";
import "@/styles/index.css";
import { LabRoot } from "./app";

applyStoredTheme();
applyFavicon();

const container = document.getElementById("lab");
if (container === null) throw new Error("#lab is missing from lab/index.html");

createRoot(container).render(<LabRoot />);
