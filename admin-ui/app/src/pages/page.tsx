/* A page drawn from its MDX (src/pages/README.md).
 *
 * A default page's HTML is in the bundle, made at build time by the same
 * losos-mdx (pages.ts). A page an administrator changed arrives as its
 * MDX source (GET /api/pages) and is rendered here, by losos-mdx in the
 * page (src/mdx/wasm.ts) — the same read the editor and the box's save
 * make of it. The HTML has been through the crate's allowlist either
 * way. It is read with DOMParser, which runs nothing, and turned into
 * React elements: each `<losos-block>` placeholder becomes the page's own
 * component for that block, and everything else becomes the same element
 * with the same few attributes. No markup reaches the page as markup, so
 * nothing in it can run, and no attribute the table below does not name
 * is carried over.
 *
 * A default page is only placeholders, so it draws exactly the blocks, in
 * order, with nothing around them: the screen looks as it did before it
 * was MDX. */

import * as React from "react";
import { Link } from "react-router-dom";
import { DEFAULTS } from "losos:pages";
import { buttonVariants } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import blocks from "./blocks.json";
import { getPagesState, subscribePages } from "./store";

/** The registry a page is rendered against: blocks.json as text, the same
 *  read the editor makes of a change and the box makes of a save. */
export const REGISTRY = JSON.stringify(blocks);

export type PageId = keyof typeof blocks.pages;
type BlocksOf<P extends PageId> = (typeof blocks.pages)[P]["blocks"];
export type BlockName<P extends PageId> = Extract<keyof BlocksOf<P>, string>;

/** What a block gets: its checked props (a flag is `true`) and the MDX an
 *  administrator wrote inside it, for a block that takes any. */
export interface BlockProps {
  props: Readonly<Record<string, string | true>>;
  children?: React.ReactNode;
}

/** Every block of a page, by name. The type asks for exactly the names
 *  blocks.json lists for the page, so the two cannot drift. */
export type BlockMap<P extends PageId> = { readonly [N in BlockName<P>]: React.ComponentType<BlockProps> };

export const PAGE_IDS = Object.keys(blocks.pages) as PageId[];

/** The HTML a page draws now: a draft, the administrator's page rendered
 *  here, or the default. Null while a first visit waits for the box. */
export function usePageHtml(id: PageId): string | null {
  const state = React.useSyncExternalStore(subscribePages, getPagesState, getPagesState);
  // The parser is a lazy chunk: a changed page draws the default until it
  // has arrived, then what the administrator saved.
  const [parser, setParser] = React.useState(() => parserReady());
  React.useEffect(() => {
    if (parser) return;
    let alive = true;
    void loadParser().then(() => {
      if (alive) setParser(true);
    });
    return () => {
      alive = false;
    };
  }, [parser]);
  const source = state.drafts[id] ?? state.custom[id];
  if (source === undefined) return state.ready ? (DEFAULTS[id] ?? "") : null;
  if (!parser) return DEFAULTS[id] ?? "";
  return renderCached(id, source) ?? (DEFAULTS[id] ?? "");
}

/** The page renderer, loaded once: src/mdx/wasm.ts, the same losos-mdx
 *  the editor reads with. Unavailable when it cannot load — the defaults
 *  draw. */
let mdx: typeof import("@/mdx/wasm") | null = null;
let parserPromise: Promise<boolean> | null = null;
let parserLoaded = false;

function parserReady(): boolean {
  return parserLoaded;
}

function loadParser(): Promise<boolean> {
  parserPromise ??= import("@/mdx/wasm")
    .then(async (m) => {
      await m.loadMdx();
      mdx = m;
      parserLoaded = true;
    })
    .catch(() => {})
    .then(() => parserLoaded);
  return parserPromise;
}

/** A rendered page, kept per id until its source changes. A source that
 *  does not read keeps the last good render, as the editor's preview
 *  does; a first one that does not read draws the default. */
const renderedBy = new Map<PageId, { source: string; html: string }>();

function renderCached(id: PageId, source: string): string | null {
  const hit = renderedBy.get(id);
  if (hit?.source === source) return hit.html;
  if (mdx === null) return hit?.html ?? null;
  try {
    const out = mdx.renderPage(source, REGISTRY, id);
    const entry = { source, html: out.html };
    renderedBy.set(id, entry);
    return entry.html;
  } catch {
    return hit?.html ?? null;
  }
}

/** Names the page on the root element, `html[data-page="overview"]`, for
 *  the administrators' stylesheet. */
function useNamedRoot(id: PageId): void {
  React.useEffect(() => {
    const root = document.documentElement;
    root.dataset["page"] = id;
    return () => {
      if (root.dataset["page"] === id) delete root.dataset["page"];
    };
  }, [id]);
}

export function Page<P extends PageId>({ id, blocks: map }: { id: P; blocks: BlockMap<P> }) {
  useNamedRoot(id);
  const html = usePageHtml(id);
  const nodes = React.useMemo(() => (html === null ? [] : parse(html)), [html]);
  if (html === null) return null;
  return <>{draw(nodes, map as Readonly<Record<string, React.ComponentType<BlockProps>>>, true)}</>;
}

function parse(html: string): ChildNode[] {
  const doc = new DOMParser().parseFromString(`<body>${html}</body>`, "text/html");
  return [...doc.body.childNodes];
}

/** The elements the crate writes (backend/mdx/src/sanitize.rs). Anything
 *  else is drawn as its children. */
const TAGS = new Set([
  "a", "abbr", "aside", "blockquote", "br", "caption", "code", "del", "details", "div", "em",
  "figcaption", "figure", "footer", "h1", "h2", "h3", "h4", "h5", "h6", "header", "hr", "img",
  "kbd", "li", "mark", "nav", "ol", "p", "pre", "s", "section", "small", "span", "strong", "sub",
  "summary", "sup", "table", "tbody", "td", "th", "thead", "time", "tr", "u", "ul",
]);
const VOID = new Set(["br", "hr", "img"]);

/** HTML attribute to React prop, for the attributes the crate writes. */
const ATTRIBUTES: Record<string, string> = {
  class: "className",
  id: "id",
  href: "href",
  title: "title",
  src: "src",
  alt: "alt",
  start: "start",
  align: "align",
  colspan: "colSpan",
  rowspan: "rowSpan",
  datetime: "dateTime",
  rel: "rel",
};

/* The other documents on this origin (the admin SPA is one of several,
 * build.ts): a link into one of them loads that page, a link
 * elsewhere on / is the router's. */
const OTHER_DOCUMENTS = ["/api/", "/auth/", "/basic/", "/chat/", "/cloud/", "/git/", "/handbook/", "/kb/", "/manage/", "/wiki/"];

function inApp(href: string): boolean {
  return href.startsWith("/") && !href.startsWith("//") && !OTHER_DOCUMENTS.some((p) => href === p.slice(0, -1) || href.startsWith(p));
}

function draw(
  nodes: readonly ChildNode[],
  map: Readonly<Record<string, React.ComponentType<BlockProps>>>,
  top: boolean,
): React.ReactNode[] {
  const out: React.ReactNode[] = [];
  nodes.forEach((node, key) => {
    if (node.nodeType === Node.TEXT_NODE) {
      const text = node.textContent ?? "";
      // Between blocks at the top, the crate's line breaks are not text.
      if (!(top && text.trim() === "")) out.push(text);
      return;
    }
    if (node.nodeType !== Node.ELEMENT_NODE) return;
    const el = node as Element;
    const tag = el.localName;
    const children = (): React.ReactNode[] => draw([...el.childNodes], map, false);
    if (tag === "losos-block") {
      const Block = map[el.getAttribute("data-block") ?? ""];
      if (Block === undefined) return;
      const inner = children();
      out.push(
        <Block key={key} props={blockProps(el.getAttribute("data-props"))}>
          {inner.length > 0 ? inner : undefined}
        </Block>,
      );
      return;
    }
    if (!TAGS.has(tag)) {
      out.push(<React.Fragment key={key}>{children()}</React.Fragment>);
      return;
    }
    const props: Record<string, unknown> = { key };
    for (const attr of [...el.attributes]) {
      const name = ATTRIBUTES[attr.name];
      if (name !== undefined) props[name] = attr.value;
      else if (attr.name.startsWith("data-")) props[attr.name] = attr.value;
      else if (attr.name === "open" && tag === "details") props["open"] = true;
    }
    if (tag === "a") {
      const href = typeof props["href"] === "string" ? props["href"] : "";
      const button = buttonClass(el);
      if (button !== null) props["className"] = button;
      if (inApp(href)) {
        const { key: _key, href: _href, rel: _rel, ...rest } = props;
        out.push(
          <Link key={key} to={href} {...rest}>
            {children()}
          </Link>,
        );
        return;
      }
    }
    out.push(VOID.has(tag) ? React.createElement(tag, props) : React.createElement(tag, props, ...children()));
  });
  return out;
}

/** A `<Button>` the crate wrote (`a.mdx-button.mdx-button-<variant>
 *  .mdx-button-<size>`), drawn with the page's own button classes. */
function buttonClass(el: Element): string | null {
  if (!el.classList.contains("mdx-button")) return null;
  const variants = ["primary", "secondary", "outline", "ghost", "link"] as const;
  const sizes = ["sm", "md", "lg"] as const;
  const variant = variants.find((v) => el.classList.contains(`mdx-button-${v}`)) ?? "primary";
  const size = sizes.find((s) => el.classList.contains(`mdx-button-${s}`)) ?? "md";
  return cn("mdx-button no-underline", buttonVariants({ variant, size }));
}

function blockProps(json: string | null): Record<string, string | true> {
  const out: Record<string, string | true> = {};
  try {
    const value: unknown = JSON.parse(json ?? "{}");
    if (typeof value === "object" && value !== null && !Array.isArray(value)) {
      for (const [k, v] of Object.entries(value)) {
        if (typeof v === "string" || v === true) out[k] = v;
      }
    }
  } catch {
    /* a placeholder the crate wrote always reads; anything else has none */
  }
  return out;
}
