/* losos-mdx in the page (backend/mdx/README.md): the same parser and
 * allowlist the box renders documents with, so the editor, a preview and
 * an export cannot disagree. The module is ./pkg (gitignored; `bun run
 * mdx` writes it in a checkout, the Nix build copies it in), loaded once
 * on first use.
 *
 * Positions in the tree markdown-rs gives are UTF-8 byte offsets; the
 * page's strings count UTF-16 units, so `tree` turns every offset into one
 * of those before anything here reads it. */

import init, { page as pageJson, render as renderJson, tree as treeJson } from "./pkg/losos_mdx.js";
import wasmUrl from "./pkg/losos_mdx_bg.wasm?url";

export type Flavour = "mdx" | "md";

export interface Place {
  line: number;
  column: number;
  offset: number;
}

export interface Problem {
  place: Place | null;
  message: string;
}

export interface Heading {
  level: number;
  text: string;
  id: string;
  line: number;
}

export interface Rendered {
  html: string;
  properties: Record<string, unknown> | null;
  propertiesError: string | null;
  headings: Heading[];
  links: { target: string; embed: boolean; external: boolean }[];
  problems: Problem[];
}

/** One mdast node, as markdown-rs writes it, with `start` and `end` in
 * UTF-16 units of the source. */
export interface MdNode {
  type: string;
  start: number;
  end: number;
  children?: MdNode[];
  value?: string;
  depth?: number;
  ordered?: boolean;
  start_?: number;
  spread?: boolean;
  checked?: boolean | null;
  lang?: string | null;
  meta?: string | null;
  url?: string;
  title?: string | null;
  alt?: string;
  align?: (string | null)[];
  name?: string | null;
  identifier?: string;
  label?: string | null;
  attributes?: { type: string; name?: string; value?: unknown }[];
}

/** A resolver as the crate's `Resolver` trait reads it from the page. */
export interface Resolver {
  note?: (target: string) => string | null;
  file?: (target: string) => string | null;
  href?: (path: string, sub: string) => string;
}

let ready: Promise<unknown> | null = null;

/** Loads the module; every other call here needs it done first. */
export function loadMdx(): Promise<unknown> {
  ready ??= init({ module_or_path: wasmUrl });
  return ready;
}

export function renderMdx(source: string, flavour: Flavour, resolver?: Resolver): Rendered {
  return JSON.parse(renderJson(source, flavour, resolver ?? null)) as Rendered;
}

/** A page of the admin UI (src/pages/README.md) as the box reads it:
 *  `registry` is blocks.json as text. Throws on a page id it does not
 *  list. */
export function renderPage(source: string, registry: string, id: string): { html: string; problems: Problem[] } {
  return JSON.parse(pageJson(source, registry, id)) as { html: string; problems: Problem[] };
}

/** Byte offset to UTF-16 index, for every byte offset of `source`. */
function offsets(source: string): Uint32Array {
  const bytes = new TextEncoder().encode(source).length;
  const map = new Uint32Array(bytes + 1);
  let b = 0;
  for (let i = 0; i < source.length; i++) {
    const c = source.charCodeAt(i);
    const width = c < 0x80 ? 1 : c < 0x800 ? 2 : c >= 0xd800 && c < 0xdc00 ? 4 : 3;
    for (let k = 0; k < width; k++) map[b + k] = i;
    b += width;
    // The low half of a pair was counted with the high one.
    if (width === 4) i++;
  }
  map[b] = source.length;
  return map;
}

interface RawNode {
  type: string;
  position?: { start: { offset: number }; end: { offset: number } };
  children?: RawNode[];
  start?: number | null;
  [key: string]: unknown;
}

function convert(node: RawNode, map: Uint32Array): MdNode {
  const { position, children, start, ...rest } = node;
  const out = {
    ...rest,
    start: position === undefined ? 0 : (map[position.start.offset] ?? 0),
    end: position === undefined ? 0 : (map[position.end.offset] ?? 0),
  } as MdNode;
  // An ordered list's first number shares the name `start`.
  if (typeof start === "number") out.start_ = start;
  if (children !== undefined) out.children = children.map((c) => convert(c, map));
  return out;
}

/** The parse of `source`: the tree, and what was refused. Throws, with the
 * line and column in the message, on a document that does not parse. */
export function parseMdx(source: string, flavour: Flavour): { tree: MdNode; problems: Problem[] } {
  const raw = JSON.parse(treeJson(source, flavour)) as { tree: RawNode; problems: Problem[] };
  return { tree: convert(raw.tree, offsets(source)), problems: raw.problems };
}
