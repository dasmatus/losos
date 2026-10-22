/* The frame one of the owner's apps runs in, from the admin page's side.
 *
 * `<iframe sandbox="allow-scripts allow-forms" src="/app-frame/">`, chosen
 * word by word for the reasons public/app-frame/index.html gives: the app
 * is an opaque origin that cannot read this page, its token or the API, and
 * the frame's own policy comes from nginx for that directory alone.
 *
 * This is the bridge's parent half. It sends the compiled app, its data,
 * the theme, the language and who is using it; it makes the app's writes
 * on the box (/api/user-apps/{id}/data) and answers `metric` through the
 * same sandbox the widgets use. An app that is not saved yet (the builder's
 * preview) keeps its data here, in memory, and loses it on reload: trying
 * an app out never writes to the box. Every message is checked against the
 * iframe's own window before it is believed.
 *
 * A preview can also be tried for the builder (`trial`): the frame fills
 * every field, presses the first button and says what it showed, kept and
 * threw (app-frame/runtime.tsx); the page hands that to the box and loads
 * the app afresh, so the owner never sees the try's entries.
 */

import * as React from "react";
import { subscribeTheme, getResolvedTheme } from "@/lib/theme";
import { getLocale } from "@/lib/i18n";
import {
  getUserAppData,
  useMe,
  writeUserAppData,
  type AppData,
  type AppDataOp,
  type AppItem,
} from "@/lib/api";
import { cn } from "@/lib/utils";
import { createSandbox } from "@/widgets/sandbox";
import { isMetricName, WidgetError } from "@/widgets/types";

export const APP_FRAME_URL = "/app-frame/";

/** A copy of the box's write rules for a preview, which writes nowhere. */
function applyLocally(data: AppData, op: AppDataOp, next: () => string): Record<string, unknown> {
  switch (op.op) {
    case "add": {
      const item = { ...op.value, id: next(), createdAt: Date.now() } as AppItem;
      data.collections[op.collection] = [...(data.collections[op.collection] ?? []), item];
      return { item };
    }
    case "update": {
      const list = data.collections[op.collection] ?? [];
      const found = list.find((i) => i.id === op.id);
      if (found === undefined) throw new Error("there is no such entry; it may have been deleted");
      const item = { ...found, ...op.value, id: found.id };
      data.collections[op.collection] = list.map((i) => (i.id === op.id ? item : i));
      return { item };
    }
    case "remove": {
      const list = data.collections[op.collection] ?? [];
      data.collections[op.collection] = list.filter((i) => i.id !== op.id);
      return { removed: list.length !== data.collections[op.collection]?.length };
    }
    case "replace": {
      const items = op.items.map((i) => ({ ...i, id: typeof i["id"] === "string" && i["id"] !== "" ? i["id"] : next() }) as AppItem);
      data.collections[op.collection] = items;
      return { items };
    }
    case "set":
      data.stored[op.key] = op.value;
      return { value: op.value };
  }
}

export interface AppFrameProps {
  /** The compiled app. A new script reloads the frame. */
  code: string;
  /** The saved app whose data this is; none for a preview. */
  appId?: string;
  /** For the iframe's accessible name. */
  name: string;
  /** The app threw, or a write was refused. */
  onError?: (message: string) => void;
  onSaveError?: (message: string) => void;
  /** Try the preview once for each new value. */
  trial?: string | null;
  onTried?: (trial: string, result: TryResult) => void;
  className?: string;
}

/** What trying an app found: what it shows, the button pressed, what it
 *  kept and what it threw. */
export interface TryResult {
  shown: string[];
  pressed: string;
  kept: string[];
  /** When nothing was kept: the choices changed before a second press, and
   *  what that kept. */
  chose: string[];
  keptAfterChoosing: string[];
  errors: string[];
  threwOnPress: string[];
}

function tryResult(value: unknown): TryResult {
  const v = (typeof value === "object" && value !== null ? value : {}) as Record<string, unknown>;
  const strings = (x: unknown) => (Array.isArray(x) ? x.filter((s): s is string => typeof s === "string") : []);
  return {
    shown: strings(v["shown"]),
    pressed: typeof v["pressed"] === "string" ? v["pressed"] : "",
    kept: strings(v["kept"]),
    chose: strings(v["chose"]),
    keptAfterChoosing: strings(v["keptAfterChoosing"]),
    errors: strings(v["errors"]),
    threwOnPress: strings(v["threwOnPress"]),
  };
}

export function AppFrame({ code, appId, name, onError, onSaveError, trial, onTried, className }: AppFrameProps) {
  const ref = React.useRef<HTMLIFrameElement>(null);
  const me = useMe();
  const user = me?.displayName || me?.name || "";
  const handlers = React.useRef({ onError, onSaveError, onTried });
  handlers.current = { onError, onSaveError, onTried };
  // A fresh frame after each try, so its entries go with it.
  const [generation, setGeneration] = React.useState(0);
  const loaded = React.useRef(false);
  const trying = React.useRef<string | null>(null);
  const tried = React.useRef<string | null>(null);
  const startTry = React.useCallback(() => {
    if (!loaded.current || trial == null || trial === tried.current || trying.current !== null) return;
    trying.current = trial;
    ref.current?.contentWindow?.postMessage({ losos: "try" }, "*");
  }, [trial]);
  const startTryRef = React.useRef(startTry);
  startTryRef.current = startTry;

  React.useEffect(() => {
    const frame = ref.current;
    if (frame === null) return;
    let stopped = false;
    let sent = false;
    // A new frame: a try asked of the one before it is gone with it.
    loaded.current = false;
    trying.current = null;
    let seq = 0;
    const preview: AppData = { collections: {}, stored: {} };
    const post = (message: unknown): void => {
      frame.contentWindow?.postMessage(message, "*");
    };
    const data = (): Promise<AppData> =>
      appId === undefined ? Promise.resolve(structuredClone(preview)) : getUserAppData(appId);

    const load = (): void => {
      if (sent) return;
      sent = true;
      void data()
        .catch(() => ({ collections: {}, stored: {} }) as AppData)
        .then((snapshot) => {
          if (stopped) return;
          post({ losos: "load", code, data: snapshot, theme: getResolvedTheme(), lang: getLocale(), user });
          loaded.current = true;
          startTryRef.current();
        });
    };

    const answer = (id: unknown, value: unknown): void => post({ losos: "answer", id, value });
    const refuse = (id: unknown, error: string): void => post({ losos: "answer", id, error });

    const onMessage = (event: MessageEvent): void => {
      if (event.source !== frame.contentWindow) return;
      const message = event.data as Record<string, unknown> | null;
      if (typeof message !== "object" || message === null) return;
      switch (message["losos"]) {
        case "hello":
          load();
          break;
        case "tried": {
          const which = trying.current;
          if (which === null) break;
          trying.current = null;
          tried.current = which;
          handlers.current.onTried?.(which, tryResult(message["value"]));
          if (appId === undefined) setGeneration((g) => g + 1);
          break;
        }
        case "error": {
          if (trying.current !== null) break;
          const text = message["message"];
          handlers.current.onError?.(typeof text === "string" ? text : "error");
          break;
        }
        case "data": {
          const id = message["id"];
          const op = message["op"] as AppDataOp;
          const write =
            appId === undefined
              ? Promise.resolve().then(() =>
                  applyLocally(preview, op, () => {
                    seq += 1;
                    return `i${seq}`;
                  }),
                )
              : writeUserAppData(appId, op);
          void write
            .then((value) => answer(id, value))
            .catch((error: unknown) => {
              const text = error instanceof Error ? error.message : "the box did not keep that";
              refuse(id, text);
              handlers.current.onSaveError?.(text);
              // Put the box's copy back, so the frame shows what is kept.
              void data().then((snapshot) => post({ losos: "snapshot", data: snapshot }));
            });
          break;
        }
        case "metric": {
          const id = message["id"];
          const metric = message["name"];
          if (!isMetricName(metric)) {
            refuse(id, `no reading called ${String(metric)}`);
            return;
          }
          void createSandbox()
            .metric(metric, {})
            .then((value) => answer(id, { value }))
            .catch((error: unknown) => {
              refuse(id, error instanceof WidgetError || error instanceof Error ? error.message : "the reading could not be made");
            });
          break;
        }
        default:
          break;
      }
    };

    window.addEventListener("message", onMessage);
    const unsubscribeTheme = subscribeTheme(() => post({ losos: "theme", theme: getResolvedTheme() }));
    return () => {
      stopped = true;
      window.removeEventListener("message", onMessage);
      unsubscribeTheme();
    };
  }, [code, appId, user, generation]);
  // After the frame's own effect, so a new trial that comes with new code
  // waits for the new frame to load instead of asking the old one.
  React.useEffect(startTry, [startTry]);

  return (
    <iframe
      key={`${appId ?? "preview"}:${generation}:${code.length}:${code.slice(-64)}`}
      ref={ref}
      src={APP_FRAME_URL}
      sandbox="allow-scripts allow-forms"
      referrerPolicy="no-referrer"
      title={name}
      className={cn("block h-full w-full border-0 bg-ground", className)}
    />
  );
}
