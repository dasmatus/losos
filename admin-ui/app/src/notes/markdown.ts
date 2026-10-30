import DOMPurify from "dompurify";
import katex from "katex";
import { parse as parseYaml } from "yaml";
import { renderNote as crateRenderNote, toNote as crateToNote } from "@/mdx/pkg/losos_mdx.js";
import { loadMdx } from "@/mdx/wasm";
import { highlight } from "./highlight";
import { escapeHtml } from "./html";
import { slug } from "./links";

/* A note to HTML, the way Obsidian and GitHub read it, by losos-mdx
 * (backend/mdx) in WebAssembly: the parser and allowlist the box itself
 * renders documents with. A `.note` is MDX, a `.md` Markdown, and both
 * carry Obsidian's syntax: properties, [[wikilinks]] and ![[embeds]],
 * #tags, callouts, ==highlights==, %%comments%%, task marks, ^block ids,
 * $maths$ and single line breaks kept. MDX components are a fixed table
 * the crate draws itself; no expression, import or export is ever run,
 * so a note carries no script.
 *
 * The crate's HTML is already through its allowlist. What it may not write
 * (a checkbox, a player, maths, coloured code) is added here from data
 * attributes, then the whole goes through DOMPurify again, and no element
 * carries a style attribute: the pages run under `style-src 'self'`. Maths
 * is MathML (KaTeX's "mathml" output); code is classes note.css colours.
 * What a link points at is the caller's business (`Resolver`). */

export interface Resolved {
  /** The note's path in the vault. */
  path: string;
}

export interface Resolver {
  /** A wikilink's note, or null when the vault has none by that name. */
  note(target: string): Resolved | null;
  /** The address of a picture or other file the note names, or null. */
  file(target: string): string | null;
  /** Where a note opens, with `#heading` or `#^block` when asked for. */
  href(path: string, sub: string): string;
}

export interface Heading {
  level: number;
  text: string;
  id: string;
  /** The heading's line in the source, from 0. */
  line: number;
}

export interface Rendered {
  html: string;
  /** The note's properties, or null without front matter. */
  properties: Record<string, unknown> | null;
  /** The front matter as written, when it is not YAML this can read. */
  propertiesError: string | null;
  headings: Heading[];
  /** The text with the front matter and comments taken out. */
  body: string;
  /** What the note asked for and did not get (a component that does not
   * exist, a prop out of range), or why a .note was read as Markdown. */
  problems: Problem[];
}

export interface Problem {
  /** From 1. */
  line: number;
  message: string;
}

/** Which way a note is read: a .note is MDX, anything else Markdown. */
export type Flavour = "mdx" | "md";

export function flavourOf(path: string): Flavour {
  return /\.note$/i.test(path) ? "mdx" : "md";
}

const FRONT = /^---[ \t]*\r?\n([\s\S]*?)\r?\n(?:---|\.\.\.)[ \t]*(?:\r?\n|$)/;

/** The front matter and the rest of a note. */
export function splitFrontMatter(source: string): { yaml: string | null; body: string; lines: number } {
  const m = FRONT.exec(source);
  if (m === null) return { yaml: null, body: source, lines: 0 };
  return { yaml: m[1] ?? "", body: source.slice(m[0].length), lines: m[0].split("\n").length - 1 };
}

export function readProperties(yaml: string | null): { properties: Record<string, unknown> | null; error: string | null } {
  if (yaml === null) return { properties: null, error: null };
  try {
    const value: unknown = parseYaml(yaml, { maxAliasCount: 50 });
    if (value === null || value === undefined) return { properties: {}, error: null };
    if (typeof value !== "object" || Array.isArray(value)) return { properties: null, error: yaml };
    return { properties: value as Record<string, unknown>, error: null };
  } catch {
    return { properties: null, error: yaml };
  }
}

/** Fenced code, line by line: which lines are inside a fence. Comments,
 * tags and links are never read there. */
function fencedLines(lines: readonly string[]): boolean[] {
  const inside: boolean[] = [];
  let fence: string | null = null;
  for (const line of lines) {
    const m = /^ {0,3}(`{3,}|~{3,})/.exec(line);
    if (fence === null) {
      if (m !== null) {
        fence = m[1] ?? null;
        inside.push(true);
        continue;
      }
      inside.push(false);
    } else {
      inside.push(true);
      if (m !== null && m[1]?.[0] === fence[0] && (m[1]?.length ?? 0) >= fence.length && line.trim() === m[1]) fence = null;
    }
  }
  return inside;
}

/** Take %%comments%% out, inline or over several lines, outside code.
 * Each removed line stays as an empty one so line numbers still match. */
export function stripComments(text: string): string {
  if (!text.includes("%%")) return text;
  const lines = text.split("\n");
  const fenced = fencedLines(lines);
  let open = false;
  return lines
    .map((line, i) => {
      if (fenced[i] === true && !open) return line;
      let out = "";
      let rest = line;
      for (;;) {
        const at = rest.indexOf("%%");
        if (at < 0) {
          if (!open) out += rest;
          break;
        }
        if (!open) out += rest.slice(0, at);
        open = !open;
        rest = rest.slice(at + 2);
      }
      return out;
    })
    .join("\n");
}

// ── The renderer ────────────────────────────────────────────────────────

let ready = false;
let loading: Promise<void> | null = null;

/** Load the renderer once; every page that draws a note awaits it first. */
export function loadNotes(): Promise<void> {
  loading ??= loadMdx().then(() => {
    ready = true;
  });
  return loading;
}

/** Whether renderNote can run yet. */
export function notesReady(): boolean {
  return ready;
}

interface CrateRendered {
  html: string;
  headings: Heading[];
  problems: { place: { line: number; column: number }; message: string }[];
}

const SIZE = /^(\d{1,5})(?:x(\d{1,5}))?$/;

function renderMath(tex: string, display: boolean): string {
  try {
    return katex.renderToString(tex, { displayMode: display, output: "mathml", throwOnError: false, strict: "ignore", trust: false });
  } catch {
    return `<code class="math-error">${escapeHtml(tex)}</code>`;
  }
}

/** What the crate leaves to the page, from its data attributes. */
function finish(html: string, links: Map<string, { path: string; sub: string }>): string {
  // Already through the crate's allowlist; a parsed document runs nothing.
  const doc = new DOMParser().parseFromString(`<body>${html}</body>`, "text/html");
  const body = doc.body;
  for (const box of body.querySelectorAll<HTMLElement>("span.task-box")) {
    const input = doc.createElement("input");
    input.type = "checkbox";
    input.className = "task-box";
    if ((box.dataset["task"] ?? " ") !== " ") input.setAttribute("checked", "");
    input.dataset["line"] = box.dataset["line"] ?? "-1";
    input.setAttribute("aria-label", input.hasAttribute("checked") ? "done" : "to do");
    box.replaceWith(input);
  }
  for (const spot of body.querySelectorAll<HTMLElement>("span.embed-media")) {
    const kind = spot.dataset["kind"] === "audio" ? "audio" : "video";
    const media = doc.createElement(kind);
    media.className = `embed-${kind}`;
    media.setAttribute("controls", "");
    media.setAttribute("src", spot.dataset["src"] ?? "");
    spot.replaceWith(media);
  }
  for (const img of body.querySelectorAll<HTMLImageElement>("img")) {
    const src = img.getAttribute("src") ?? "";
    const alt = img.getAttribute("alt") ?? "";
    if (/^(https?:)?\/\//i.test(src)) {
      // The page loads no picture from off the box: a link to it instead.
      const a = doc.createElement("a");
      a.className = "external image-link";
      a.setAttribute("href", src);
      a.textContent = alt || src;
      img.replaceWith(a);
      continue;
    }
    img.classList.add("embed-image");
    img.setAttribute("loading", "lazy");
    const width = img.dataset["width"];
    const size = SIZE.exec(width !== undefined ? `${width}${img.dataset["height"] !== undefined ? `x${img.dataset["height"]}` : ""}` : (alt.split("|").pop() ?? ""));
    if (size !== null) {
      img.setAttribute("width", size[1] ?? "");
      if (size[2] !== undefined) img.setAttribute("height", size[2]);
    }
  }
  for (const a of body.querySelectorAll<HTMLAnchorElement>("a[href]")) {
    let href = a.getAttribute("href") ?? "";
    if (href.startsWith("http://www.") && a.textContent === href.slice("http://".length)) {
      // A bare www. address: GitHub's flavour says http, the web is https.
      href = `https://${a.textContent}`;
      a.setAttribute("href", href);
    }
    const note = links.get(href);
    if (a.dataset["notePath"] === undefined && note !== undefined && !a.classList.contains("tag")) {
      // A Markdown link to a note: opened in the page, like a wikilink.
      a.dataset["notePath"] = note.path;
      a.dataset["sub"] = note.sub;
      a.classList.add("wikilink");
    }
    if (/^[a-z][a-z0-9+.-]*:/i.test(href) || a.classList.contains("is-file")) {
      a.setAttribute("target", "_blank");
      a.setAttribute("rel", "noopener noreferrer");
      if (!a.classList.contains("is-file")) a.classList.add("external");
    }
  }
  for (const code of body.querySelectorAll<HTMLElement>("code.language-math")) {
    const display = code.classList.contains("math-display");
    const pre = code.parentElement?.tagName === "PRE" ? code.parentElement : null;
    const box = doc.createElement(display ? "div" : "span");
    if (display) box.className = "math-block";
    box.innerHTML = renderMath(code.textContent ?? "", display);
    (display && pre !== null ? pre : code).replaceWith(box);
  }
  // losos-mdx writes equations as MathML; a display one gets the same box.
  for (const math of body.querySelectorAll<MathMLElement>('math[display="block"]')) {
    if (math.parentElement?.classList.contains("math-block") === true) continue;
    const box = doc.createElement("div");
    box.className = "math-block";
    math.replaceWith(box);
    box.append(math);
  }
  for (const code of body.querySelectorAll<HTMLElement>("pre > code")) {
    const lang = [...code.classList].find((c) => c.startsWith("language-"))?.slice("language-".length) ?? "";
    if (lang === "") continue;
    code.classList.add("hljs");
    code.innerHTML = highlight(code.textContent ?? "", lang);
  }
  return sanitize(body.innerHTML);
}

let hooked = false;
function hookPurify(): void {
  if (hooked) return;
  hooked = true;
  // Links that leave the page never carry the page along.
  DOMPurify.addHook("afterSanitizeAttributes", (node) => {
    if (node.tagName === "A" && node.getAttribute("target") === "_blank") node.setAttribute("rel", "noopener noreferrer");
  });
}

/** Clean HTML: no script, no handlers, no style attribute or element. */
export function sanitize(html: string): string {
  hookPurify();
  return DOMPurify.sanitize(html, {
    FORBID_TAGS: ["style", "form", "iframe", "object", "embed", "base", "link", "meta"],
    FORBID_ATTR: ["style", "srcset", "formaction"],
    ADD_ATTR: ["target"],
    ALLOW_DATA_ATTR: true,
  });
}

const EMPTY: Rendered = { html: "", properties: null, propertiesError: null, headings: [], body: "", problems: [] };

/** A note, rendered. Until loadNotes() has finished it is empty: a page
 * that draws notes waits for it (useNotesReady). A .note that is not valid
 * MDX is drawn as Markdown, with the reason among its problems. */
export function renderNote(source: string, resolver: Resolver, flavour: Flavour = "md"): Rendered {
  if (!ready) return EMPTY;
  const links = new Map<string, { path: string; sub: string }>();
  const js = {
    note: (target: string) => resolver.note(target)?.path ?? null,
    file: (target: string) => resolver.file(target),
    href: (path: string, sub: string) => {
      const href = resolver.href(path, sub);
      links.set(href, { path, sub });
      return href;
    },
  };
  const problems: Problem[] = [];
  const { yaml, body } = splitFrontMatter(source);
  // Read here rather than taken from the crate, whose JSON object sorts
  // the keys: properties show in the order the note gives them.
  const { properties, error } = readProperties(yaml);
  const attempt = (f: Flavour): CrateRendered | null => {
    try {
      return JSON.parse(crateRenderNote(source, f, js)) as CrateRendered;
    } catch (e) {
      const text = e instanceof Error ? e.message : String(e);
      const m = /^line (\d+), column \d+: ([\s\S]*)$/.exec(text);
      problems.push({ line: Number(m?.[1] ?? 1), message: m?.[2] ?? text });
      return null;
    }
  };
  // A .note that is not MDX is still the owner's text: read as Markdown.
  const out = attempt(flavour) ?? (flavour === "mdx" ? attempt("md") : null);
  if (out === null) {
    // Too large or too deep for either: the text as it is.
    return { ...EMPTY, html: `<pre class="note-plain">${escapeHtml(source)}</pre>`, properties, propertiesError: error, body, problems };
  }
  for (const p of out.problems) problems.push({ line: p.place.line, message: p.message });
  return {
    html: finish(out.html, links),
    properties,
    propertiesError: error,
    headings: out.headings,
    body: stripComments(body),
    problems,
  };
}

/** A .md note's text as a .note's. Throws, naming the line, when MDX
 * cannot say the same thing. */
export async function toNote(source: string): Promise<string> {
  await loadNotes();
  return crateToNote(source);
}

/** The id a heading gets, the crate's own slug (render::slug), and the one
 * a link to it looks for. */
export function anchorFor(text: string, used?: Map<string, number>): string {
  if (text.startsWith("^")) return text;
  let base = "";
  for (const c of text.trim()) {
    if (/[\p{L}\p{N}_-]/u.test(c)) base += c.toLowerCase();
    else if (/\s/.test(c)) base += "-";
  }
  if (used === undefined) return base;
  const n = used.get(base) ?? 0;
  used.set(base, n + 1);
  return n === 0 ? base : `${base}-${n}`;
}

const BLOCK_ID_ALONE = /^\^([A-Za-z0-9-]+)[ \t]*$/;

/** One part of a note: under a heading, or the block with an id. */
export function section(source: string, sub: string): string {
  const { body } = splitFrontMatter(source);
  if (sub === "") return body;
  const lines = body.split("\n");
  if (sub.startsWith("^")) {
    // Compared as text, not built into a RegExp: the id comes from whatever
    // link the note's author typed, and a `(` or `\` in it would throw or match
    // something else.
    const at = lines.findIndex((l) => {
      const t = l.trimEnd();
      return t.endsWith(sub) && (t.length === sub.length || /\s/.test(t[t.length - sub.length - 1] ?? ""));
    });
    if (at < 0) return "";
    // The paragraph or list item the id ends.
    let from = at;
    if (BLOCK_ID_ALONE.test(lines[at] ?? "")) from = at - 1;
    let start = from;
    while (start > 0 && (lines[start - 1] ?? "").trim() !== "" && !/^\s*([-*+]|\d+\.)\s/.test(lines[start] ?? "")) start--;
    return lines.slice(start, at + 1).join("\n");
  }
  const want = slug(sub);
  const fenced = fencedLines(lines);
  let start = -1;
  let level = 0;
  for (let i = 0; i < lines.length; i++) {
    if (fenced[i] === true) continue;
    const m = /^(#{1,6})\s+(.*?)\s*#*\s*$/.exec(lines[i] ?? "");
    if (m === null) continue;
    const l = m[1]?.length ?? 1;
    if (start < 0) {
      if (slug(m[2] ?? "") === want) {
        start = i;
        level = l;
      }
    } else if (l <= level) {
      return lines.slice(start, i).join("\n");
    }
  }
  return start < 0 ? "" : lines.slice(start).join("\n");
}
