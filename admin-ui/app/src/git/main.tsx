/* LosOS Git's web app: git/index.html loads this, and nginx serves it at
 * /git/ behind LosOS Login (modules/containers.nix). Its data is LosOS
 * Git's own API at /git/api/v1/ (backend/src/forge/). The stylesheets in
 * the same order as src/main.tsx; see src/lab/main.tsx for why the order
 * matters to the shared chunk. */

import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { applyFavicon } from "@/lib/logo";
import { applyStoredTheme } from "@/lib/theme";
import "sonner/dist/styles.css";
import "@/styles/index.css";
import { GitRoot } from "./app";

applyStoredTheme();
applyFavicon();

const container = document.getElementById("git");
if (container === null) throw new Error("#git is missing from git/index.html");

createRoot(container).render(
  <StrictMode>
    <GitRoot />
  </StrictMode>,
);
