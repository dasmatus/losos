import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import App from "@/App";
import { applyStoredTheme } from "@/lib/theme";
import "@/styles/index.css";

/* The theme is already on <html> by now — /theme-boot.js stamped it before
 * the first paint (see vite.config.ts). This second call is the belt to that
 * pair of braces: if the boot script ever fails to load, the app still comes
 * up in the stored theme, one frame late instead of not at all. */
applyStoredTheme();

const container = document.getElementById("root");
if (container === null) throw new Error("#root is missing from index.html");

createRoot(container).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
