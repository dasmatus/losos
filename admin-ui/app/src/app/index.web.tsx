/* The web "/" route: the admin SPA, unchanged.
 *
 * src/App.tsx and the pages under it keep their DOM code — react-router,
 * @base-ui, codemirror, the pages' Editors — and this route is where that
 * tree mounts, exactly as src/main.tsx did before Expo: the boot calls
 * (theme, favicon, the service worker), one createRoot per visit, no
 * hydration (the static export's HTML is the shell; the app paints on
 * boot). The .web suffix is what keeps Metro out of this file on iOS and
 * Android: the native bundle never imports react-dom or any of the DOM
 * components the pages subscribe to.
 *
 * Two things moved to build time elsewhere: the page's <head> is the
 * +html.tsx route, and the stylesheets come with the bundle through Expo
 * global styles (the same files bundler.ts shipped: index.css and
 * sonner's stylesheet — the appliance CSP allows neither an inline style
 * nor an injected one, so the import keeps them files).
 *
 * The boot runs inside the effect, not at module scope: Expo Web's static
 * export also imports this module in the SSR pass, where there is no
 * window, and nothing here runs there twice.
 */

import * as React from "react";
import { StrictMode } from "react";
import { createRoot, type Root } from "react-dom/client";
import { StyleSheet, View } from "react-native";

import "sonner/dist/styles.css";
import "@/styles/index.css";
import App from "@/App";
import { applyFavicon } from "@/lib/logo";
import { applyStoredTheme } from "@/lib/theme";

export default function AdminWebIndex(): React.JSX.Element {
  const [booted, setBooted] = React.useState(false);

  /* The SPA's own #root: the same id App's CSS (index.css's html/body
   * rules) and main.tsx's container name sat on, created here because
   * the static export's HTML carries only the router's own mount. */
  React.useEffect(() => {
    const mount = document.createElement("div");
    mount.id = "root";
    document.body.appendChild(mount);
    applyStoredTheme();
    applyFavicon();
    let swatch: Root | null = createRoot(mount);
    swatch.render(
      <StrictMode>
        <App />
      </StrictMode>,
    );
    if ("serviceWorker" in navigator) {
      window.addEventListener("load", () => {
        navigator.serviceWorker.register("/sw.js").catch(() => {});
      });
    }
    setBooted(true);
    return () => {
      mount.remove();
      swatch = null;
      setBooted(false);
    };
  }, []);

  /* The static export's HTML and the frame before the effect run: the
   * brand background so the first paint is never white (the layout story
   * index.html's <html class="has-bg"> used to tell). */
  return (
    <View style={[host, booted ? null : plateBackground]} />
  );
}

const host: React.ComponentProps<typeof View>["style"] = StyleSheet.absoluteFill;

const plateBackground = { backgroundColor: "#DCBDAA" };
