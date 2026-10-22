/* The guest page's entry: guest.html loads this. It is its own build
 * (vite.guest.config.ts) under /calls/guest/, because nginx serves it to
 * anyone holding a link, from the internet too, while every other page and
 * the shared /assets/ stay on the LAN behind LosOS Login. */

import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { applyFavicon } from "@/lib/logo";
import { applyStoredTheme } from "@/lib/theme";
import "sonner/dist/styles.css";
import "@/styles/index.css";
import { GuestPage, readLink } from "./guest-page";

applyStoredTheme();
applyFavicon();

const container = document.getElementById("guest");
if (container === null) throw new Error("#guest is missing from guest.html");

createRoot(container).render(
  <StrictMode>
    <GuestPage link={readLink(location.href)} />
  </StrictMode>,
);
