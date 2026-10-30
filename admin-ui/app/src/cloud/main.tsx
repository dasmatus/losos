/* LosOS Cloud's web app: cloud/index.html loads this, and nginx serves it
 * at /cloud/ behind LosOS Login (modules/cloud.nix). Its data is the cloud
 * service's own JSON API at /cloud/api/ (backend/src/cloud/api.rs). The
 * stylesheets in the same order as src/main.tsx; see src/lab/main.tsx for
 * why the order matters to the shared chunk. */

import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { applyFavicon } from "@/lib/logo";
import { applyStoredTheme } from "@/lib/theme";
import "sonner/dist/styles.css";
import "@/styles/index.css";
import { CloudRoot } from "./app";
import { CloudTogether } from "./together";
// A .canvas file's name opens it in Diagrams, and a Markdown file's as a note.
import "./open-diagrams";
import "./office/paper/opener";
import "./notes/opener";
import "./collab/opener";
// Office once the box says which types its Collabora opens.
import "./office/opener";

applyStoredTheme();
applyFavicon();

const container = document.getElementById("cloud");
if (container === null) throw new Error("#cloud is missing from cloud/index.html");

createRoot(container).render(
  <StrictMode>
    <CloudRoot />
    <CloudTogether />
  </StrictMode>,
);
