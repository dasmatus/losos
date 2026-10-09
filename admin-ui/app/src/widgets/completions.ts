/* What the widget editor offers as the owner types.
 *
 * The language packages already complete HTML tags and attributes, CSS
 * properties and values, and JavaScript keywords and local names. This adds
 * what only this page knows: the frame's `losos` object and its readings,
 * the box's colour variables, the widget's own file names where a file can
 * be named, and the browser's globals for a script. The frame's contract is
 * public/widget-frame/index.html; when it grows, this list grows with it.
 */

import {
  ifNotIn,
  startCompletion,
  type Completion,
  type CompletionContext,
  type CompletionResult,
  type CompletionSource,
} from "@codemirror/autocomplete";
import { METRIC_NAMES } from "./types";

/** A method that takes a name: write `name("")` with the cursor between
 *  the quotes, and offer the names at once. */
function callWithName(method: string): Completion["apply"] {
  return (view, _completion, from, to) => {
    view.dispatch({
      changes: { from, to, insert: `${method}("")` },
      selection: { anchor: from + method.length + 2 },
      userEvent: "input.complete",
    });
    // After this transaction settles: the list that offered the method
    // closes on it, and would take a list opened in the same tick with it.
    setTimeout(() => startCompletion(view), 0);
  };
}

/** The `losos` object, member by member. */
export const BRIDGE: readonly Completion[] = [
  {
    label: "metric",
    type: "method",
    detail: "(name, opts?) → Promise",
    info: "One of the box's readings by name. Rejects when the box does not report it.",
    apply: callWithName("metric"),
  },
  { label: "theme", type: "property", detail: '"light" | "dark"', info: "The admin page's theme." },
  {
    label: "onTheme",
    type: "method",
    detail: "(fn) → unsubscribe",
    info: "Calls fn with the new theme whenever the admin page switches.",
  },
  { label: "lang", type: "property", detail: '"en" | "sk" | "de"', info: "The admin page's language." },
  {
    label: "palette",
    type: "property",
    detail: "{ [variable]: value }",
    info: "The box's colours, also set as CSS variables on <html>.",
  },
  { label: "resize", type: "method", detail: "()", info: "Ask the board to measure the widget again." },
  { label: "files", type: "property", detail: "string[]", info: "The names of this widget's files." },
  {
    label: "file",
    type: "method",
    detail: "(name) → string | undefined",
    info: "One of this widget's files, as text.",
    apply: callWithName("file"),
  },
  {
    label: "asset",
    type: "method",
    detail: "(name) → data: URL",
    info: "One of this widget's files as a data: URL, for an image or a CSS url() set from script.",
    apply: callWithName("asset"),
  },
];

/** The colour variables the frame sets on <html> (hand-frame.tsx's list). */
export const PALETTE: readonly Completion[] = [
  ["--ground", "the page's ground colour"],
  ["--surface", "a card"],
  ["--sunk", "a well inside a card"],
  ["--ink", "body text"],
  ["--muted", "secondary text"],
  ["--faint", "the faintest text"],
  ["--line", "a border"],
  ["--hair", "a hairline"],
  ["--accent", "the accent colour"],
  ["--accent-wash", "a light wash of the accent"],
  ["--ok", "good"],
  ["--warn", "needs a look"],
  ["--crit", "wrong"],
  ["--font-ui", "the interface font"],
  ["--font-code", "the code font"],
].map(([label, info]) => ({ label: label ?? "", type: "variable", info }));

/** `losos.` and a member. */
function bridgeMembers(context: CompletionContext): CompletionResult | null {
  const match = context.matchBefore(/\blosos\.\w*$/);
  if (match === null) return null;
  return { from: match.from + "losos.".length, options: BRIDGE, validFor: /^\w*$/ };
}

/** A reading's name, inside `losos.metric("`. */
function metricNames(context: CompletionContext): CompletionResult | null {
  const match = context.matchBefore(/\blosos\.metric\(\s*["'`][\w.]*$/);
  if (match === null) return null;
  const quote = match.text.search(/["'`]/);
  return {
    from: match.from + quote + 1,
    options: METRIC_NAMES.map((name) => ({ label: name, type: "constant", detail: "reading" })),
    validFor: /^[\w.]*$/,
  };
}

/** A file name where script names one: `fetch("`, `losos.file("`,
 *  `losos.asset("`. */
function scriptFileNames(files: () => readonly string[]): CompletionSource {
  return (context) => {
    const match = context.matchBefore(/\b(?:fetch|losos\.file|losos\.asset)\(\s*["'`][\w.-]*$/);
    if (match === null) return null;
    const quote = match.text.search(/["'`]/);
    return {
      from: match.from + quote + 1,
      options: files().map((name) => ({ label: name, type: "text", detail: "file" })),
      validFor: /^[\w.-]*$/,
    };
  };
}

/** A file name in an HTML `src=` or `href=`. */
export function htmlFileNames(files: () => readonly string[]): CompletionSource {
  return (context) => {
    const match = context.matchBefore(/\b(?:src|href)=["']?[\w.-]*$/);
    if (match === null) return null;
    const value = match.text.replace(/^\w+=["']?/, "");
    return {
      from: context.pos - value.length,
      options: files()
        .filter((name) => name !== "index.html")
        .map((name) => ({ label: name, type: "text", detail: "file" })),
      validFor: /^[\w.-]*$/,
    };
  };
}

/** `var(--` and a colour, or `url(` and a file, in a stylesheet. */
export function cssCompletions(files: () => readonly string[]): CompletionSource {
  return (context) => {
    const variable = context.matchBefore(/var\(\s*--[\w-]*$/);
    if (variable !== null) {
      return {
        from: context.pos - (variable.text.length - variable.text.indexOf("--")),
        options: PALETTE,
        validFor: /^--[\w-]*$/,
      };
    }
    const url = context.matchBefore(/url\(\s*["']?[\w.-]*$/);
    if (url !== null) {
      const value = url.text.replace(/^url\(\s*["']?/, "");
      return {
        from: context.pos - value.length,
        options: files()
          .filter((name) => /\.(svg|json|txt)$/.test(name))
          .map((name) => ({ label: name, type: "text", detail: "file" })),
        validFor: /^[\w.-]*$/,
      };
    }
    return null;
  };
}

/** What a browser gives a widget's script, beyond the language itself. The
 *  admin window's own globals are not offered: the frame is another
 *  window, and half of this one's would not exist there. */
const BROWSER: readonly Completion[] = [
  ["document", "variable"],
  ["window", "variable"],
  ["fetch", "function"],
  ["setTimeout", "function"],
  ["setInterval", "function"],
  ["clearTimeout", "function"],
  ["clearInterval", "function"],
  ["requestAnimationFrame", "function"],
  ["console", "variable"],
  ["Intl", "namespace"],
  ["Date", "class"],
  ["Math", "namespace"],
  ["JSON", "namespace"],
  ["Number", "class"],
  ["String", "class"],
  ["Array", "class"],
  ["Object", "class"],
  ["Promise", "class"],
  ["URL", "class"],
  ["URLSearchParams", "class"],
  ["AbortController", "class"],
  ["ResizeObserver", "class"],
  ["navigator", "variable"],
  ["losos", "variable"],
].map(([label, type]) => ({ label: label ?? "", type: type ?? "variable", boost: label === "losos" ? 2 : 0 }));

/** A global, where a name starts: not after a dot, not in a string. */
const browserGlobals: CompletionSource = ifNotIn(
  ["String", "TemplateString", "LineComment", "BlockComment", "RegExp"],
  (context) => {
    const word = context.matchBefore(/[\w$]*$/);
    if (word === null || (word.from === word.to && !context.explicit)) return null;
    if (context.state.sliceDoc(word.from - 1, word.from) === ".") return null;
    return { from: word.from, options: BROWSER, validFor: /^[\w$]*$/ };
  },
);

/** Everything a script gets: the bridge, readings, file names, globals. */
export function scriptCompletions(files: () => readonly string[]): CompletionSource[] {
  return [bridgeMembers, metricNames, scriptFileNames(files), browserGlobals];
}
