/* The Office page on its own: office/index.html loads this. It opens one
 * document full screen, named in the address's fragment, either
 *
 *   /office/#file=<id>                       a file in the box's Office
 *                                            folder (losos.office.folder)
 *   /office/#url=…&token=…&ttl=…&name=…      a session another box opened
 *          &writable=0|1&back=https://…      for a document it shares, with
 *                                            the way back to that box
 *
 * and with nothing after the # it lists the documents in the Office folder
 * (src/office/files.tsx). LosOS cloud shows the same editor inside its own
 * app instead. The stylesheets in the same order as src/main.tsx; see
 * src/lab/main.tsx for why. */

import { StrictMode, useSyncExternalStore } from "react";
import { createRoot } from "react-dom/client";
import { applyFavicon } from "@/lib/logo";
import { applyStoredTheme } from "@/lib/theme";
import "sonner/dist/styles.css";
import "@/styles/index.css";
import { OfficePage } from "./page";

applyStoredTheme();
applyFavicon();

const container = document.getElementById("office");
if (container === null) throw new Error("#office is missing from office/index.html");

function subscribeHash(onChange: () => void): () => void {
  window.addEventListener("hashchange", onChange);
  return () => window.removeEventListener("hashchange", onChange);
}

function Office() {
  const fragment = useSyncExternalStore(subscribeHash, () => window.location.hash.slice(1));
  // A new document is a new editor, not the old one with another file.
  return <OfficePage key={fragment} fragment={fragment} />;
}

createRoot(container).render(
  <StrictMode>
    <Office />
  </StrictMode>,
);
