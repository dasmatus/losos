/* The app frame's runtime: React, the LosOS components, the data hooks and
 * the bridge to the admin page, as one classic script.
 *
 * public/app-frame/index.html loads this as /app-frame/runtime.js (built by
 * vite.config.ts's appFrame plugin, an IIFE like theme-boot.js). It is a
 * classic script on purpose: the frame is an opaque origin (the admin page
 * embeds it with `sandbox="allow-scripts"` and no `allow-same-origin`), and
 * a module script would be fetched in CORS mode, which nginx does not answer
 * for the box's own files.
 *
 * The page sends `load` with the app's compiled script (backend/src/jsx.rs
 * wraps it to read React and these names off `window`), its data, the theme,
 * the language and who is using it. The script goes into a <script> element:
 * the frame's own policy (the `~^/(widget|app)-frame/` arm in
 * modules/containers.nix) allows inline script here and nowhere else.
 */

import * as React from "react";
import { createRoot, type Root } from "react-dom/client";
import "../styles/tokens.css";
import "./ui.css";
import { UI } from "./ui";
import { setSnapshot, setWriter, tally, useCollection, useStored, type Op, type Snapshot } from "./data";

type Message = Record<string, unknown>;

const parentWindow = window.parent;
const send = (message: Message): void => parentWindow.postMessage(message, "*");

let seq = 0;
const pending = new Map<number, { resolve: (v: Record<string, unknown>) => void; reject: (e: Error) => void }>();
function ask(message: Message): Promise<Record<string, unknown>> {
  return new Promise((resolve, reject) => {
    seq += 1;
    pending.set(seq, { resolve, reject });
    send({ ...message, id: seq });
  });
}

const current = { lang: "en", user: "" };

/** A reading made of one list ("apps.list" is `{ apps }`, "uptime.days" is
 *  `{ days, upRatio }`) is also that list: an array carrying the reading's
 *  fields. `reading.apps.map` and `reading.map` both work, so an app that
 *  takes the reading for the list itself, as small models often write it,
 *  still runs. */
function listLike(value: unknown): unknown {
  if (typeof value !== "object" || value === null || Array.isArray(value)) return value;
  const lists = Object.values(value).filter(Array.isArray);
  return lists.length === 1 ? Object.assign([...(lists[0] as unknown[])], value) : value;
}

/** The `losos` object: the box's readings, the page's language, the person. */
const losos = Object.freeze({
  metric(name: string): Promise<unknown> {
    return ask({ losos: "metric", name: String(name) }).then((answer) => listLike(answer["value"]));
  },
  get lang(): string {
    return current.lang;
  },
  get user(): string {
    return current.user;
  },
});

setWriter((op: Op) =>
  ask({ losos: "data", op }).catch((error: Error) => {
    throw new Error(`save: ${error.message}`);
  }),
);

const errors: string[] = [];
function report(message: string): void {
  errors.push(message.slice(0, 200));
  send({ losos: "error", message: message.slice(0, 500) });
}

const wait = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));
const words = (el: Element) => ((el as HTMLElement).innerText || el.textContent || "").trim().replace(/\s+/g, " ").slice(0, 80);

/** What the app shows, a line per heading, field, button and entry, in
 *  the order they are on the page: what the builder's model reads back
 *  when it tries the app. */
function outline(): string[] {
  const lines: string[] = [];
  const inside = new Set<Element>();
  for (const el of document.querySelectorAll("#root h1, #root h2, #root h3, #root button, #root input, #root select, #root textarea, #root li, #root tr, #root p, #root [role=alert]")) {
    if (inside.has(el)) continue;
    const tag = el.tagName.toLowerCase();
    if (el instanceof HTMLInputElement || el instanceof HTMLSelectElement || el instanceof HTMLTextAreaElement) {
      const label = el.labels?.[0] ? words(el.labels[0]) : (el.getAttribute("placeholder") ?? "");
      const kind = el instanceof HTMLInputElement ? `${tag}[${el.type}]` : tag;
      const options = el instanceof HTMLSelectElement ? ` options: ${[...el.options].map((o) => o.text).join(", ")}` : "";
      lines.push(`${kind} "${label}"${options}`);
    } else {
      const text = words(el);
      if (text === "") continue;
      lines.push(`${tag}: ${text}`);
      for (const child of el.querySelectorAll("*")) inside.add(child);
    }
    if (lines.length >= 40) break;
  }
  return lines;
}

/** Fill every field, press the first button, and say what the app kept and
 *  what it threw. When it kept nothing and has a choice to make, pick
 *  another option in each and press again: an app whose choice starts empty
 *  while showing its first option keeps nothing until the owner changes it,
 *  a mistake small models make often. Only ever asked of a preview, whose
 *  data is thrown away. */
async function tryIt(): Promise<Record<string, unknown>> {
  await wait(600);
  const shown = outline();
  const thrownBefore = errors.length;
  const setValue = (el: HTMLInputElement | HTMLTextAreaElement | HTMLSelectElement, value: string, event: string) => {
    const proto =
      el instanceof HTMLTextAreaElement
        ? HTMLTextAreaElement.prototype
        : el instanceof HTMLSelectElement
          ? HTMLSelectElement.prototype
          : HTMLInputElement.prototype;
    Object.getOwnPropertyDescriptor(proto, "value")?.set?.call(el, value);
    el.dispatchEvent(new Event(event, { bubbles: true }));
  };
  const fill = () => {
    for (const el of document.querySelectorAll<HTMLInputElement | HTMLTextAreaElement>("#root input, #root textarea")) {
      const type = el instanceof HTMLInputElement ? el.type : "text";
      if (type === "checkbox" || type === "radio" || type === "date") continue;
      setValue(el, type === "number" ? "12" : "Test", "input");
    }
  };
  const press = async (): Promise<string> => {
    await wait(100);
    const button = document.querySelector<HTMLButtonElement>("#root button:not([disabled])");
    button?.click();
    await wait(600);
    return button === null ? "" : words(button);
  };
  const changed = (before: Record<string, number>) => {
    const after = tally();
    return Object.keys(after)
      .filter((k) => after[k] !== before[k])
      .map((k) => (k.startsWith("stored ") ? k : `${k}: ${before[k] ?? 0} to ${after[k]}`));
  };

  let before = tally();
  fill();
  const pressed = await press();
  const kept = changed(before);
  const chose: string[] = [];
  let keptAfterChoosing: string[] = [];
  const selects = [...document.querySelectorAll<HTMLSelectElement>("#root select")].filter((s) => s.options.length > 1);
  if (kept.length === 0 && pressed !== "" && selects.length > 0 && errors.length === thrownBefore) {
    for (const select of selects) {
      chose.push(select.labels?.[0] ? words(select.labels[0]) : "");
      setValue(select, select.options[1]?.value ?? "", "change");
    }
    before = tally();
    fill();
    await press();
    keptAfterChoosing = changed(before);
  }
  return {
    shown,
    pressed,
    kept,
    chose,
    keptAfterChoosing,
    errors: errors.slice(0, Math.min(thrownBefore, 3)),
    threwOnPress: errors.slice(thrownBefore, thrownBefore + 2),
  };
}

class Boundary extends React.Component<{ children: React.ReactNode }, { error: string | null }> {
  override state = { error: null as string | null };
  static getDerivedStateFromError(error: unknown) {
    return { error: error instanceof Error ? error.message : String(error) };
  }
  override componentDidCatch(error: unknown) {
    report(error instanceof Error ? error.message : String(error));
  }
  override render() {
    if (this.state.error !== null) {
      return (
        <div className="lx-crash" role="alert">
          <strong>The app stopped.</strong>
          <code>{this.state.error}</code>
        </div>
      );
    }
    return this.props.children;
  }
}

let root: Root | null = null;
function mount(App: React.ComponentType): void {
  if (typeof App !== "function") {
    report("the app has no App component to show");
    return;
  }
  const element = document.getElementById("root");
  if (element === null) return;
  root ??= createRoot(element);
  root.render(
    <Boundary>
      <App />
    </Boundary>,
  );
}

const w = window as unknown as Record<string, unknown>;
w["React"] = React;
w["LosOS"] = Object.freeze({ ui: UI, useCollection, useStored, losos, mount });

function setTheme(theme: unknown): void {
  document.documentElement.dataset["theme"] = theme === "dark" ? "dark" : "light";
}

let loaded = false;
function load(message: Message): void {
  if (loaded) return;
  loaded = true;
  setTheme(message["theme"]);
  current.lang = typeof message["lang"] === "string" ? message["lang"] : "en";
  current.user = typeof message["user"] === "string" ? message["user"] : "";
  document.documentElement.lang = current.lang;
  setSnapshot((message["data"] as Snapshot | undefined) ?? { collections: {}, stored: {} });
  const code = message["code"];
  if (typeof code !== "string" || code.length === 0) {
    report("the app has no code");
    return;
  }
  const script = document.createElement("script");
  script.textContent = code;
  document.body.appendChild(script);
}

window.addEventListener("message", (event: MessageEvent) => {
  if (event.source !== parentWindow) return;
  const message = event.data as Message | null;
  if (typeof message !== "object" || message === null) return;
  switch (message["losos"]) {
    case "load":
      load(message);
      break;
    case "theme":
      setTheme(message["theme"]);
      break;
    case "snapshot":
      setSnapshot(message["data"] as Snapshot);
      break;
    case "try":
      void tryIt().then((value) => send({ losos: "tried", value }));
      break;
    case "answer": {
      const entry = pending.get(Number(message["id"]));
      if (entry === undefined) return;
      pending.delete(Number(message["id"]));
      if (typeof message["error"] === "string") entry.reject(new Error(message["error"]));
      else entry.resolve((message["value"] as Record<string, unknown>) ?? {});
      break;
    }
    default:
      break;
  }
});

/* A <form> never goes anywhere: the frame's policy refuses the navigation
 * anyway (`form-action 'none'`), and an app that forgot preventDefault
 * would otherwise log a refusal on every submit. React's own onSubmit runs
 * first, on the root below the window. */
window.addEventListener("submit", (event) => event.preventDefault());

window.addEventListener("error", (event) => report(event.message || "the app stopped"));
window.addEventListener("unhandledrejection", (event) => {
  // A write the box refused is said by the page already, and its data put
  // back; the app's own promise rejecting is no crash.
  const reason = event.reason as unknown;
  if (reason instanceof Error && reason.message.startsWith("save:")) {
    event.preventDefault();
    return;
  }
  report(reason instanceof Error ? reason.message : String(reason));
});

// Tell the page the frame is listening, once the document is there.
if (document.readyState === "loading") {
  document.addEventListener("DOMContentLoaded", () => send({ losos: "hello" }));
} else {
  send({ losos: "hello" });
}
