/* LosOS Lab's entry: lab/index.html loads this. The Lab is its own page,
 * not a route of the admin SPA, because it runs under a policy of its own
 * (script-src adds 'wasm-unsafe-eval' for the core) and carries the core's
 * WebAssembly, which the admin pages have no use for. It shares the SPA's
 * components, palette, theme and language. */

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
import "./lab.css";
import { LabRoot } from "./app";

applyStoredTheme();
applyFavicon();
document.body.classList.add("lab-body");

const container = document.getElementById("lab");
if (container === null) throw new Error("#lab is missing from lab/index.html");

createRoot(container).render(<LabRoot />);
