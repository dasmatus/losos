/* A knowledge base: a folder of notes someone published from LosOS Cloud,
 * read at /kb/<name>/. kb/index.html loads this; nginx serves it to anyone
 * (or to people signed in to the box, as the site's owner chose), and its
 * data is the cloud service's /kb/api/<name>/ (backend/src/cloud/notes/kb.rs).
 * The stylesheets in the same order as src/main.tsx; see src/lab/main.tsx
 * for why the order matters to the shared chunk. */

import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { applyFavicon } from "@/lib/logo";
import { applyStoredTheme } from "@/lib/theme";
import "sonner/dist/styles.css";
import "@/styles/index.css";
import { KbRoot } from "./app";

applyStoredTheme();
applyFavicon();

const container = document.getElementById("kb");
if (container === null) throw new Error("#kb is missing from kb/index.html");

createRoot(container).render(
  <StrictMode>
    <KbRoot />
  </StrictMode>,
);
