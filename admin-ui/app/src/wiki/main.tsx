/* A space of LosOS Wiki as its own site: wiki/index.html loads this, and
 * nginx serves it at /wiki/ (modules/containers.nix). Its data is LosOS
 * Git's API at /git/api/v1/ (backend/src/forge/wiki.rs). The stylesheets in
 * the same order as src/main.tsx; see src/lab/main.tsx for why the order
 * matters to the shared chunk. */

import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { applyFavicon } from "@/lib/logo";
import { applyStoredTheme } from "@/lib/theme";
import "sonner/dist/styles.css";
import "@/styles/index.css";
import { WikiSite } from "./site";

applyStoredTheme();
applyFavicon();

const container = document.getElementById("wiki");
if (container === null) throw new Error("#wiki is missing from wiki/index.html");

createRoot(container).render(
  <StrictMode>
    <WikiSite />
  </StrictMode>,
);
