/* LosOS chat's entry: chat/index.html loads this. Chat is its own page, not
 * a route of the admin SPA, because it runs under a policy of its own
 * (script-src adds 'wasm-unsafe-eval' for the end-to-end crypto, img-src and
 * media-src add blob: for decrypted pictures and calls; modules/chat.nix)
 * and carries the Matrix SDK, which the admin pages have no use for. nginx
 * serves it only while the box shares its storage, as the shared user's
 * app. It shares the SPA's components, palette, theme and language. The
 * stylesheets in the same order as src/main.tsx; see src/lab/main.tsx for
 * why the order matters to the shared chunk. */

import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { applyFavicon } from "@/lib/logo";
import { applyStoredTheme } from "@/lib/theme";
import "sonner/dist/styles.css";
import "@/styles/index.css";
import { ChatRoot } from "./app";

applyStoredTheme();
applyFavicon();

const container = document.getElementById("chat");
if (container === null) throw new Error("#chat is missing from chat/index.html");

createRoot(container).render(
  <StrictMode>
    <ChatRoot />
  </StrictMode>,
);
