import MarkdownIt from "markdown-it";
import type { PluginSimple, StateBlock, StateCore, StateInline, Token } from "markdown-it";
import footnote from "markdown-it-footnote";
import DOMPurify from "dompurify";
import katex from "katex";
import { parse as parseYaml } from "yaml";
import { highlight } from "./highlight";
import { escapeHtml } from "./html";
import { parseLink, slug } from "./links";

/* A note's Markdown to HTML, the way Obsidian and GitHub read it.
 *
 * GitHub's flavour: tables, task lists, strikethrough, autolinks, footnotes,
 * fenced code with highlighting and alerts (`> [!NOTE]`). Obsidian's on
 * top: properties (YAML front matter), [[wikilinks]] with headings, blocks
 * and aliases, ![[embeds]], #tags, callouts (the same syntax as the alerts,
 * with a title and folding), ==highlights==, %%comments%%, block ids (^id)
 * and $maths$. Single line breaks are kept, as Obsidian keeps them.
 *
 * The HTML goes through DOMPurify before it reaches a page, and no element
 * carries a style attribute: the pages run under `style-src 'self'`, which
 * refuses them. Maths is MathML (KaTeX's "mathml" output), which needs no
 * stylesheet; code is classes the note stylesheet colours (note.css). What
 * a link points at is the caller's business (`Resolver`): LosOS Cloud
 * opens notes in its own view, a knowledge base in its own pages. */

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
  /** Lines the front matter takes: a line in `body` is this many further
   * down the file. */
  frontLines: number;
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

// ── Inline rules ────────────────────────────────────────────────────────

/** [[target#sub|alias]] and ![[target#sub|size]]. */
function wikilinkRule(state: StateInline, silent: boolean): boolean {
  const src = state.src;
  let pos = state.pos;
  const embed = src.charCodeAt(pos) === 0x21; /* ! */
  if (embed) pos++;
  if (src.charCodeAt(pos) !== 0x5b || src.charCodeAt(pos + 1) !== 0x5b) return false;
  const end = src.indexOf("]]", pos + 2);
  if (end < 0) return false;
  const inner = src.slice(pos + 2, end);
  if (inner.includes("\n") || inner.trim() === "" || inner.includes("[[")) return false;
  if (!silent) {
    const token = state.push(embed ? "obsidian_embed" : "wikilink", "", 0);
    token.content = inner;
  }
  state.pos = end + 2;
  return true;
}

/** ==highlight== */
function highlightRule(state: StateInline, silent: boolean): boolean {
  const src = state.src;
  const pos = state.pos;
  if (src.charCodeAt(pos) !== 0x3d || src.charCodeAt(pos + 1) !== 0x3d) return false;
  const end = src.indexOf("==", pos + 2);
  if (end < 0 || end === pos + 2) return false;
  const inner = src.slice(pos + 2, end);
  if (/^\s|\s$/.test(inner)) return false;
  if (!silent) {
    state.push("mark_open", "mark", 1);
    const oldMax = state.posMax;
    state.pos = pos + 2;
    state.posMax = end;
    state.md.inline.tokenize(state);
    state.posMax = oldMax;
    state.push("mark_close", "mark", -1);
  }
  state.pos = end + 2;
  return true;
}

/** A #tag: letters, digits, _, -, / and not only digits, after a space or
 * at the start of the text. */
export const TAG = /^#([\p{L}\p{N}_\-/]*[\p{L}_\-/][\p{L}\p{N}_\-/]*)/u;

function tagRule(state: StateInline, silent: boolean): boolean {
  const pos = state.pos;
  if (state.src.charCodeAt(pos) !== 0x23) return false;
  if (pos > 0 && !/[\s([{,;]/.test(state.src[pos - 1] ?? "")) return false;
  const m = TAG.exec(state.src.slice(pos));
  if (m === null || m[1] === undefined) return false;
  if (!silent) {
    const token = state.push("obsidian_tag", "", 0);
    token.content = m[1];
  }
  state.pos += m[0].length;
  return true;
}

/** $inline maths$: not `$ ` at the start, not ` $` at the end, and not a
 * price (a digit straight after the closing dollar). */
function mathInlineRule(state: StateInline, silent: boolean): boolean {
  const src = state.src;
  const pos = state.pos;
  if (src.charCodeAt(pos) !== 0x24 || src.charCodeAt(pos + 1) === 0x24) return false;
  if (/\s/.test(src[pos + 1] ?? " ")) return false;
  let end = pos + 1;
  for (;;) {
    end = src.indexOf("$", end);
    if (end < 0) return false;
    if (src[end - 1] === "\\") {
      end++;
      continue;
    }
    break;
  }
  if (/\s/.test(src[end - 1] ?? " ") || /\d/.test(src[end + 1] ?? "")) return false;
  if (!silent) {
    const token = state.push("math_inline", "", 0);
    token.content = src.slice(pos + 1, end);
  }
  state.pos = end + 1;
  return true;
}

/** $$ block maths $$, on lines of their own or on one line. */
function mathBlockRule(state: StateBlock, startLine: number, endLine: number, silent: boolean): boolean {
  const start = (state.bMarks[startLine] ?? 0) + (state.tShift[startLine] ?? 0);
  const max = state.eMarks[startLine] ?? 0;
  const first = state.src.slice(start, max);
  if (!first.startsWith("$$")) return false;
  if (silent) return true;
  let content: string;
  let next = startLine;
  const rest = first.slice(2);
  if (rest.trimEnd().endsWith("$$") && rest.trim() !== "") {
    content = rest.trimEnd().slice(0, -2);
  } else {
    const parts = [rest];
    let closed = false;
    while (++next < endLine) {
      const s = (state.bMarks[next] ?? 0) + (state.tShift[next] ?? 0);
      const line = state.src.slice(s, state.eMarks[next] ?? 0);
      if (line.trimEnd().endsWith("$$")) {
        parts.push(line.trimEnd().slice(0, -2));
        closed = true;
        break;
      }
      parts.push(line);
    }
    if (!closed) return false;
    content = parts.join("\n");
  }
  const token = state.push("math_block", "", 0);
  token.content = content.trim();
  token.map = [startLine, next + 1];
  token.block = true;
  state.line = next + 1;
  return true;
}

function renderMath(tex: string, display: boolean): string {
  try {
    return katex.renderToString(tex, { displayMode: display, output: "mathml", throwOnError: false, strict: "ignore", trust: false });
  } catch {
    return `<code class="math-error">${escapeHtml(tex)}</code>`;
  }
}

// ── Core rules ──────────────────────────────────────────────────────────

const TASK = /^\[([ xX/\-><!?*"lbiSIpcfkwud])\][ \t]/;

/** - [ ] and - [x], and Obsidian's other marks (- [/], - [-], …). */
function taskLists(state: StateCore): void {
  const tokens = state.tokens;
  for (let i = 2; i < tokens.length; i++) {
    const inline = tokens[i];
    if (inline?.type !== "inline" || tokens[i - 1]?.type !== "paragraph_open" || tokens[i - 2]?.type !== "list_item_open") continue;
    const m = TASK.exec(inline.content);
    if (m === null) continue;
    const mark = m[1] ?? " ";
    const item = tokens[i - 2] as Token;
    item.attrJoin("class", "task-list-item");
    item.attrSet("data-task", mark);
    const first = inline.children?.[0];
    if (first?.type === "text") first.content = first.content.replace(TASK, "");
    const box = new state.Token("task_box", "", 0);
    box.meta = { mark, line: item.map?.[0] ?? -1 };
    inline.children?.unshift(box);
    // Mark the list too, which drops its bullets.
    for (let j = i - 3; j >= 0; j--) {
      const t = tokens[j] as Token;
      if ((t.type === "bullet_list_open" || t.type === "ordered_list_open") && t.level === item.level - 1) {
        t.attrJoin("class", "contains-task-list");
        break;
      }
    }
  }
}

const CALLOUT = /^\[!([\w-]+)\]([+-]?)[ \t]*(.*)$/;

/** > [!type]± Title: Obsidian's callouts and GitHub's alerts. */
function callouts(state: StateCore): void {
  const tokens = state.tokens;
  for (let i = 0; i < tokens.length; i++) {
    const open = tokens[i] as Token;
    if (open.type !== "blockquote_open") continue;
    const p = tokens[i + 1];
    const inline = tokens[i + 2];
    if (p?.type !== "paragraph_open" || inline?.type !== "inline") continue;
    const firstLine = inline.content.split("\n")[0] ?? "";
    const m = CALLOUT.exec(firstLine);
    if (m === null) continue;
    const kind = (m[1] ?? "note").toLowerCase();
    const fold = m[2] ?? "";
    const title = m[3]?.trim() ?? "";
    // The rest of the first paragraph, after the marker line.
    const rest = inline.content.split("\n").slice(1).join("\n");
    open.type = "callout_open";
    open.meta = { kind, fold, title: title === "" ? null : title };
    // The matching close, at the same level.
    let depth = 0;
    for (let j = i; j < tokens.length; j++) {
      const t = tokens[j] as Token;
      if (t.type === "blockquote_open" || t.type === "callout_open") depth++;
      if (t.type === "blockquote_close") {
        depth--;
        if (depth === 0) {
          t.type = "callout_close";
          t.meta = { fold };
          break;
        }
      }
    }
    // The title is parsed inline, like any other text.
    const titleTokens: Token[] = [];
    if (title !== "") state.md.inline.parse(title, state.md, state.env, titleTokens);
    (open.meta as { titleTokens?: Token[] }).titleTokens = titleTokens;
    if (rest.trim() === "") {
      // Only the title: drop the paragraph.
      tokens.splice(i + 1, 3);
    } else {
      inline.content = rest;
      inline.children = [];
      state.md.inline.parse(rest, state.md, state.env, inline.children);
    }
  }
}

const BLOCK_ID = /[ \t]\^([A-Za-z0-9-]+)[ \t]*$/;
const BLOCK_ID_ALONE = /^\^([A-Za-z0-9-]+)[ \t]*$/;

/** A block's ^id at the end of its last line becomes the block's id, so
 * [[note#^id]] lands on it. On a line of its own it names the block above. */
function blockIds(state: StateCore): void {
  const tokens = state.tokens;
  for (let i = 0; i < tokens.length; i++) {
    const inline = tokens[i] as Token;
    if (inline.type !== "inline") continue;
    const lines = inline.content.split("\n");
    const last = lines[lines.length - 1] ?? "";
    const m = BLOCK_ID.exec(last) ?? (lines.length > 1 ? null : BLOCK_ID_ALONE.exec(last));
    if (m === null) continue;
    const id = m[1] ?? "";
    // The block that holds it: the nearest opening token before.
    let target: Token | null = null;
    for (let j = i - 1; j >= 0; j--) {
      const t = tokens[j] as Token;
      if (t.nesting === 1) {
        target = t;
        // A list item's paragraph: the id goes on the item.
        const above = tokens[j - 1];
        if (above?.type === "list_item_open" && t.type === "paragraph_open") target = above;
        break;
      }
    }
    if (target === null) continue;
    if (BLOCK_ID_ALONE.test(last) && lines.length === 1) {
      // A paragraph that is only the id: give it to the block before it.
      const before = findBlockBefore(tokens, i - 1);
      if (before !== null) before.attrSet("id", `^${id}`);
      const p = tokens[i - 1] as Token;
      p.hidden = true;
      inline.children = [];
      (tokens[i + 1] as Token).hidden = true;
      continue;
    }
    target.attrSet("id", `^${id}`);
    const children = inline.children ?? [];
    const tail = children[children.length - 1];
    if (tail?.type === "text") tail.content = tail.content.replace(BLOCK_ID, "");
  }
}

function findBlockBefore(tokens: Token[], from: number): Token | null {
  // The opening token of the block that closed just before `from`.
  const close = tokens[from - 1];
  if (close === undefined || close.nesting !== -1) return null;
  let depth = 0;
  for (let j = from - 1; j >= 0; j--) {
    const t = tokens[j] as Token;
    depth += t.nesting;
    if (depth === 0) return t;
  }
  return null;
}

// ── The renderer ────────────────────────────────────────────────────────

interface Env {
  resolver: Resolver;
  headings: Heading[];
  slugs: Map<string, number>;
  lineOffset: number;
}

const EMBED_SIZE = /^(\d+)(?:x(\d+))?$/;
const IMAGE = /\.(png|jpe?g|gif|webp|avif|bmp|svg)$/i;
const AUDIO = /\.(mp3|wav|m4a|ogg|flac|3gp|webm)$/i;
const VIDEO = /\.(mp4|mov|mkv|ogv)$/i;

function attrs(pairs: Record<string, string | null | undefined>): string {
  return Object.entries(pairs)
    .filter((p): p is [string, string] => p[1] !== null && p[1] !== undefined)
    .map(([k, v]) => ` ${k}="${escapeHtml(v)}"`)
    .join("");
}

function createMd(): MarkdownIt {
  const md = new MarkdownIt({
    html: true,
    linkify: true,
    breaks: true,
    typographer: false,
    highlight: (code, lang) => highlight(code, lang),
  });
  // Its types are written against an older markdown-it; the plugin is not.
  md.use(footnote as unknown as PluginSimple);
  md.inline.ruler.before("link", "wikilink", wikilinkRule);
  md.inline.ruler.before("emphasis", "highlight", highlightRule);
  md.inline.ruler.after("wikilink", "obsidian_tag", tagRule);
  md.inline.ruler.after("escape", "math_inline", mathInlineRule);
  md.block.ruler.before("fence", "math_block", mathBlockRule, { alt: ["paragraph", "reference", "blockquote", "list"] });
  md.core.ruler.after("inline", "callouts", callouts);
  md.core.ruler.after("callouts", "task_lists", taskLists);
  md.core.ruler.after("task_lists", "block_ids", blockIds);

  const r = md.renderer.rules;
  r["math_inline"] = (tokens, idx) => renderMath(tokens[idx]?.content ?? "", false);
  r["math_block"] = (tokens, idx) => `<div class="math-block">${renderMath(tokens[idx]?.content ?? "", true)}</div>\n`;
  r["mark_open"] = () => "<mark>";
  r["mark_close"] = () => "</mark>";
  r["task_box"] = (tokens, idx) => {
    const { mark, line } = tokens[idx]?.meta as { mark: string; line: number };
    const done = mark !== " ";
    return `<input type="checkbox" class="task-box"${done ? " checked" : ""}${attrs({ "data-line": String(line), "aria-label": done ? "done" : "to do" })}> `;
  };
  r["obsidian_tag"] = (tokens, idx) => {
    const tag = tokens[idx]?.content ?? "";
    return `<a class="tag"${attrs({ href: `#tag/${tag}`, "data-tag": tag })}>#${escapeHtml(tag)}</a>`;
  };
  r["wikilink"] = (tokens, idx, _o, env: Env) => {
    const link = parseLink(tokens[idx]?.content ?? "");
    const text = link.alias ?? (link.target === "" ? link.sub.replace(/^\^/, "") : link.sub === "" ? link.target : `${link.target} > ${link.sub.replace(/^\^/, "")}`);
    if (link.target === "") {
      // [[#Heading]]: a place in this note.
      return `<a class="wikilink internal"${attrs({ href: `#${anchorFor(link.sub)}`, "data-sub": link.sub })}>${escapeHtml(text)}</a>`;
    }
    const found = env.resolver.note(link.target);
    if (found === null) {
      const file = env.resolver.file(link.target);
      if (file !== null) return `<a class="wikilink file"${attrs({ href: file, target: "_blank", rel: "noopener" })}>${escapeHtml(text)}</a>`;
      return `<a class="wikilink is-unresolved"${attrs({ href: "#", "data-note-target": link.target })}>${escapeHtml(text)}</a>`;
    }
    return `<a class="wikilink"${attrs({ href: env.resolver.href(found.path, link.sub), "data-note-path": found.path, "data-sub": link.sub })}>${escapeHtml(text)}</a>`;
  };
  r["obsidian_embed"] = (tokens, idx, _o, env: Env) => {
    const link = parseLink(tokens[idx]?.content ?? "");
    const size = link.alias !== null ? EMBED_SIZE.exec(link.alias) : null;
    const file = IMAGE.test(link.target) || AUDIO.test(link.target) || VIDEO.test(link.target) || /\.pdf$/i.test(link.target) ? env.resolver.file(link.target) : null;
    if (file !== null) {
      if (IMAGE.test(link.target)) {
        const alt = size === null ? (link.alias ?? link.target) : link.target;
        return `<img class="embed-image"${attrs({ src: file, alt, loading: "lazy", width: size?.[1] ?? null, height: size?.[2] ?? null })}>`;
      }
      if (AUDIO.test(link.target)) return `<audio class="embed-audio" controls${attrs({ src: file })}></audio>`;
      if (VIDEO.test(link.target)) return `<video class="embed-video" controls${attrs({ src: file })}></video>`;
      return `<a class="wikilink file"${attrs({ href: file, target: "_blank", rel: "noopener" })}>${escapeHtml(link.alias ?? link.target)}</a>`;
    }
    const found = env.resolver.note(link.target);
    if (found === null) {
      return `<span class="embed-missing"${attrs({ "data-note-target": link.target })}>${escapeHtml(link.target)}</span>`;
    }
    // Filled in by the page (NoteView), which has the note's text.
    return `<span class="note-embed"${attrs({ "data-embed-path": found.path, "data-sub": link.sub })}></span>`;
  };
  r["callout_open"] = (tokens, idx, options, env: Env, self) => {
    const { kind, fold, title, titleTokens } = tokens[idx]?.meta as { kind: string; fold: string; title: string | null; titleTokens: Token[] };
    const titleHtml = title === null ? escapeHtml(calloutTitle(kind)) : self.renderInline(titleTokens, options, env);
    const head = `<span class="callout-icon" aria-hidden="true"></span><span class="callout-title-inner">${titleHtml}</span>`;
    if (fold !== "") {
      return `<details class="callout"${attrs({ "data-callout": kind })}${fold === "+" ? " open" : ""}><summary class="callout-title">${head}</summary><div class="callout-content">\n`;
    }
    return `<div class="callout"${attrs({ "data-callout": kind })}><div class="callout-title">${head}</div><div class="callout-content">\n`;
  };
  r["callout_close"] = (tokens, idx) => ((tokens[idx]?.meta as { fold: string }).fold !== "" ? "</div></details>\n" : "</div></div>\n");

  // Headings carry ids, and the page learns the outline.
  const headingOpen = r["heading_open"];
  r["heading_open"] = (tokens, idx, options, env: Env, self) => {
    const token = tokens[idx] as Token;
    const text = inlineText(tokens[idx + 1]);
    const id = anchorFor(text, env.slugs);
    if (token.attrGet("id") === null) token.attrSet("id", id);
    env.headings.push({ level: Number(token.tag.slice(1)), text, id, line: (token.map?.[0] ?? 0) + env.lineOffset });
    return headingOpen ? headingOpen(tokens, idx, options, env, self) : self.renderToken(tokens, idx, options);
  };

  // Links: outside ones open in a new tab; a .md file in the vault is a
  // note; a picture off the box is a link, since the page loads none.
  const linkOpen = r["link_open"];
  r["link_open"] = (tokens, idx, options, env: Env, self) => {
    const token = tokens[idx] as Token;
    const href = token.attrGet("href") ?? "";
    if (/^[a-z][a-z0-9+.-]*:/i.test(href)) {
      token.attrSet("target", "_blank");
      token.attrSet("rel", "noopener noreferrer");
      token.attrJoin("class", "external");
    } else if (!href.startsWith("#")) {
      const [target, sub = ""] = safeDecode(href).split("#", 2);
      const found = target !== undefined && target !== "" ? env.resolver.note(target) : null;
      if (found !== null) {
        token.attrSet("href", env.resolver.href(found.path, sub));
        token.attrSet("data-note-path", found.path);
        token.attrSet("data-sub", sub);
        token.attrJoin("class", "wikilink");
      } else if (target !== undefined && target !== "") {
        const file = env.resolver.file(target);
        if (file !== null) token.attrSet("href", file);
      }
    }
    return linkOpen ? linkOpen(tokens, idx, options, env, self) : self.renderToken(tokens, idx, options);
  };
  r["image"] = (tokens, idx, options, env: Env, self) => {
    const token = tokens[idx] as Token;
    const src = token.attrGet("src") ?? "";
    const alt = self.renderInlineAsText(token.children ?? [], options, env);
    if (/^(https?:)?\/\//i.test(src)) {
      return `<a class="external image-link"${attrs({ href: src, target: "_blank", rel: "noopener noreferrer" })}>${escapeHtml(alt || src)}</a>`;
    }
    if (/^data:/i.test(src)) return escapeHtml(alt);
    const file = env.resolver.file(safeDecode(src));
    if (file === null) return `<span class="embed-missing">${escapeHtml(alt || src)}</span>`;
    const size = EMBED_SIZE.exec(alt.split("|").pop() ?? "");
    return `<img class="embed-image"${attrs({ src: file, alt, loading: "lazy", width: size?.[1] ?? null, height: size?.[2] ?? null })}>`;
  };
  return md;
}

function safeDecode(text: string): string {
  try {
    return decodeURIComponent(text);
  } catch {
    return text;
  }
}

function inlineText(token: Token | undefined): string {
  if (token === undefined) return "";
  return (token.children ?? [])
    .map((c) => (c.type === "text" || c.type === "code_inline" ? c.content : c.type === "wikilink" ? (parseLink(c.content).alias ?? parseLink(c.content).target) : c.type === "obsidian_tag" ? `#${c.content}` : ""))
    .join("")
    .trim();
}

/** The id a heading gets, and the one a link to it looks for. */
export function anchorFor(text: string, used?: Map<string, number>): string {
  if (text.startsWith("^")) return text;
  const base = `h-${slug(text)}`;
  if (used === undefined) return base;
  const n = used.get(base) ?? 0;
  used.set(base, n + 1);
  return n === 0 ? base : `${base}-${n}`;
}

const CALLOUT_TITLES: Record<string, string> = {
  note: "Note",
  abstract: "Abstract",
  summary: "Summary",
  tldr: "TL;DR",
  info: "Info",
  todo: "To do",
  tip: "Tip",
  hint: "Hint",
  important: "Important",
  success: "Success",
  check: "Check",
  done: "Done",
  question: "Question",
  help: "Help",
  faq: "FAQ",
  warning: "Warning",
  caution: "Caution",
  attention: "Attention",
  failure: "Failure",
  fail: "Fail",
  missing: "Missing",
  danger: "Danger",
  error: "Error",
  bug: "Bug",
  example: "Example",
  quote: "Quote",
  cite: "Cite",
};

function calloutTitle(kind: string): string {
  return CALLOUT_TITLES[kind] ?? kind.charAt(0).toUpperCase() + kind.slice(1);
}

let shared: MarkdownIt | null = null;

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

/** A note, rendered. */
export function renderNote(source: string, resolver: Resolver): Rendered {
  const { yaml, body, lines } = splitFrontMatter(source);
  const { properties, error } = readProperties(yaml);
  const text = stripComments(body);
  shared ??= createMd();
  const env: Env = { resolver, headings: [], slugs: new Map(), lineOffset: lines };
  const raw = shared.render(text, env);
  return { html: sanitize(raw), properties, propertiesError: error, headings: env.headings, body: text, frontLines: lines };
}

/** One part of a note: under a heading, or the block with an id. */
export function section(source: string, sub: string): string {
  const { body } = splitFrontMatter(source);
  if (sub === "") return body;
  const lines = body.split("\n");
  if (sub.startsWith("^")) {
    const id = sub.slice(1);
    const at = lines.findIndex((l) => new RegExp(`(^|\\s)\\^${id.replace(/[-]/g, "\\-")}\\s*$`).test(l));
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
