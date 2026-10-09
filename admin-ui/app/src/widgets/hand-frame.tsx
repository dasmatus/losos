/* The frame a hand-written widget runs in, from the board's side.
 *
 * `<iframe sandbox="allow-scripts" src="/widget-frame/">`, and the choice of
 * every word in that tag is in public/widget-frame/index.html's header. In
 * short: no `allow-same-origin`, so the widget is an opaque origin that
 * cannot read this page, its sessionStorage (the admin token) or the API;
 * and a real URL rather than `srcdoc`, because a srcdoc document inherits
 * this page's CSP and inline script would be refused there just the same.
 * The frame's own, permissive policy comes from nginx for that directory
 * alone (modules/containers.nix).
 *
 * This component is the bridge's parent half. It sends the widget's files,
 * the theme, the language and the box's palette; it answers `metric` requests
 * through the same sandbox the spec widgets use (widgets/sandbox.ts), so a
 * hand-written widget can read exactly what a built-in can and nothing
 * else; and it sizes the iframe to what the widget drew. Every message is
 * checked against the iframe's own window before it is believed.
 */

import * as React from "react";
import { subscribeTheme, getResolvedTheme } from "@/lib/theme";
import { getLocale, subscribeLocale } from "@/lib/i18n";
import type { WidgetFile } from "@/lib/api";
import { cn } from "@/lib/utils";
import { createSandbox } from "./sandbox";
import { isMetricName, WidgetError } from "./types";

/** The frame's address. A directory, so nginx's header map matches both
 *  the request and the index it resolves to. */
export const FRAME_URL = "/widget-frame/";

/** The colours handed to the widget, by variable name, read off <html> at
 *  send time so a theme change carries the new values. */
const PALETTE_VARS: readonly string[] = [
  "--ground",
  "--surface",
  "--sunk",
  "--ink",
  "--muted",
  "--faint",
  "--line",
  "--hair",
  "--accent",
  "--accent-wash",
  "--ok",
  "--warn",
  "--crit",
  "--font-ui",
  "--font-code",
] as const;

function readPalette(): Record<string, string> {
  const computed = getComputedStyle(document.documentElement);
  const palette: Record<string, string> = {};
  for (const name of PALETTE_VARS) {
    const value = computed.getPropertyValue(name).trim();
    if (value.length > 0) palette[name] = value;
  }
  return palette;
}

/** Heights the tile will take. Below the floor a widget that drew nothing
 *  yet is still a tile; above the ceiling it scrolls inside itself. */
export const MIN_FRAME_HEIGHT = 72;
export const MAX_FRAME_HEIGHT = 560;

export interface HandFrameProps {
  /** The widget's files. Different files reload the frame. */
  files: readonly WidgetFile[];
  /** For the iframe's accessible name. */
  name: string;
  /** Called with the widget's own error text, when it throws. */
  onError?: (message: string) => void;
  className?: string;
}

export function HandFrame({ files, name, onError, className }: HandFrameProps) {
  const ref = React.useRef<HTMLIFrameElement>(null);
  const onErrorRef = React.useRef(onError);
  onErrorRef.current = onError;

  /* One listener for the life of the frame; the files it hands over are
   * whatever the latest render passed. A change to any file is a `key`
   * change below, so a new frame, so there is never an old document
   * showing new files or the other way round. */
  const key = React.useMemo(() => JSON.stringify(files), [files]);
  React.useEffect(() => {
    const frame = ref.current;
    if (frame === null) return;

    const post = (message: unknown): void => {
      // "*" is the only target an opaque origin can be addressed by, and the
      // payload is the owner's own source and the box's public readings.
      frame.contentWindow?.postMessage(message, "*");
    };

    const load = (): void => {
      post({
        losos: "load",
        files,
        theme: getResolvedTheme(),
        lang: getLocale(),
        palette: readPalette(),
      });
    };

    const theme = (): void => {
      post({
        losos: "theme",
        theme: getResolvedTheme(),
        lang: getLocale(),
        palette: readPalette(),
      });
    };

    const onMessage = (event: MessageEvent): void => {
      if (event.source !== frame.contentWindow) return;
      const message = event.data as Record<string, unknown> | null;
      if (typeof message !== "object" || message === null) return;
      switch (message["losos"]) {
        case "hello":
          load();
          break;
        case "size": {
          const height = message["height"];
          if (typeof height !== "number" || !Number.isFinite(height)) return;
          const clamped = Math.min(MAX_FRAME_HEIGHT, Math.max(MIN_FRAME_HEIGHT, Math.ceil(height)));
          // CSSOM, which the admin CSP allows; a style attribute would not be.
          frame.style.height = `${clamped}px`;
          break;
        }
        case "error": {
          const text = message["message"];
          onErrorRef.current?.(typeof text === "string" ? text : "error");
          break;
        }
        case "metric": {
          const id = message["id"];
          const metric = message["name"];
          const opts = message["opts"];
          if (!isMetricName(metric)) {
            post({ losos: "metric:error", id, message: `no reading called ${String(metric)}` });
            return;
          }
          const options = typeof opts === "object" && opts !== null ? opts : {};
          void createSandbox()
            .metric(metric, options)
            .then((value) => post({ losos: "metric:result", id, value }))
            .catch((error: unknown) => {
              const text =
                error instanceof WidgetError || error instanceof Error
                  ? error.message
                  : "the reading could not be made";
              post({ losos: "metric:error", id, message: text });
            });
          break;
        }
        default:
          break;
      }
    };

    window.addEventListener("message", onMessage);
    const unsubscribeTheme = subscribeTheme(theme);
    const unsubscribeLocale = subscribeLocale(theme);
    // The frame may already be listening (a cached document): offer the load
    // now as well as on hello. The frame ignores nothing and renders twice at
    // worst, which is the same picture.
    load();

    return () => {
      window.removeEventListener("message", onMessage);
      unsubscribeTheme();
      unsubscribeLocale();
    };
    // `key` stands for the files' content, which is what matters here.
  }, [key]);

  return (
    <iframe
      key={key}
      ref={ref}
      src={FRAME_URL}
      sandbox="allow-scripts"
      referrerPolicy="no-referrer"
      title={name}
      loading="lazy"
      className={cn("block w-full border-0 bg-transparent", className)}
      // `height` as an attribute is presentational, not a style attribute,
      // and gives the frame its floor before the widget reports a size.
      height={MIN_FRAME_HEIGHT}
    />
  );
}
