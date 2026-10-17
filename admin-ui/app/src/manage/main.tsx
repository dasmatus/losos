/* The company edge's manage page: manage/index.html loads this. The edge
 * serves it at /manage/ (edge/registrar/src/server/manage.rs), from the same
 * build as the admin page, so a box's parts look and behave the same on both.
 * The stylesheets in the same order as src/main.tsx; see src/lab/main.tsx
 * for why the order matters to the shared chunk. */

import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { linkHandbookAt } from "@/lib/handbook";
import { applyFavicon } from "@/lib/logo";
import { applyStoredTheme } from "@/lib/theme";
import "sonner/dist/styles.css";
import "@/styles/index.css";
import { ManageRoot } from "./app";

applyStoredTheme();
applyFavicon();
linkHandbookAt("https://losos.dasmat.us/");

const container = document.getElementById("manage");
if (container === null) throw new Error("#manage is missing from manage/index.html");

createRoot(container).render(
  <StrictMode>
    <ManageRoot />
  </StrictMode>,
);
