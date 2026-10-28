/* The document LosOS Office edits, as ProseMirror nodes and marks: what
 * losos-mdx reads (CommonMark, GFM, front matter, maths) and each of its
 * components (backend/mdx/src/components.rs), one node or mark apiece.
 * The file is the MDX text (from-mdast.ts reads it, to-mdx.ts writes it);
 * this is only its shape while it is on the screen.
 *
 * Nothing here writes a `style` attribute: the admin page's CSP refuses
 * them, so every look is a class in paper.css. What the editor cannot
 * draw is kept as `raw`, its source untouched, never dropped. */

import { Schema, type DOMOutputSpec, type NodeSpec, type MarkSpec } from "prosemirror-model";

export const CALLOUT_TYPES = ["note", "tip", "important", "warning", "caution"] as const;
export const SLIDE_LAYOUTS = ["title", "content", "two-columns", "picture"] as const;
export const CHART_TYPES = ["bar", "line", "area", "pie"] as const;
export const ALIGNS = ["left", "center", "right"] as const;
export const DATE_FORMATS = ["short", "long"] as const;

const block = (tag: string, cls: string): DOMOutputSpec => [tag, { class: cls }, 0];

const cellAttrs = {
  colspan: { default: 1 },
  rowspan: { default: 1 },
  colwidth: { default: null },
  align: { default: null as string | null },
};

const cellDOM = (tag: string) => (n: { attrs: Record<string, unknown> }): DOMOutputSpec => {
  const a: Record<string, string> = {};
  if (n.attrs["colspan"] !== 1) a["colspan"] = String(n.attrs["colspan"]);
  if (n.attrs["rowspan"] !== 1) a["rowspan"] = String(n.attrs["rowspan"]);
  if (n.attrs["align"]) a["class"] = `is-align-${String(n.attrs["align"])}`;
  return [tag, a, 0];
};

const nodes: Record<string, NodeSpec> = {
  doc: { content: "front_matter? block+" },

  /** The YAML between the `---` lines, kept as written; page setup and
   * the properties read and write it. Not drawn in the page. */
  front_matter: {
    attrs: { yaml: { default: "" } },
    atom: true,
    selectable: false,
    toDOM: () => ["div", { class: "paper-front-matter", hidden: "" }],
  },

  paragraph: { group: "block", content: "inline*", toDOM: () => ["p", 0], parseDOM: [{ tag: "p" }] },

  heading: {
    group: "block",
    content: "inline*",
    attrs: { level: { default: 1 } },
    defining: true,
    toDOM: (n) => [`h${String(n.attrs["level"])}`, 0],
    parseDOM: [1, 2, 3, 4, 5, 6].map((level) => ({ tag: `h${level}`, attrs: { level } })),
  },

  blockquote: { group: "block", content: "block+", defining: true, toDOM: () => ["blockquote", 0], parseDOM: [{ tag: "blockquote" }] },

  bullet_list: {
    group: "block",
    content: "list_item+",
    attrs: { tight: { default: true } },
    toDOM: () => ["ul", 0],
    parseDOM: [{ tag: "ul" }],
  },

  ordered_list: {
    group: "block",
    content: "list_item+",
    attrs: { tight: { default: true }, order: { default: 1 } },
    toDOM: (n) => ["ol", n.attrs["order"] === 1 ? {} : { start: String(n.attrs["order"]) }, 0],
    parseDOM: [{ tag: "ol", getAttrs: (d) => ({ order: Number((d as HTMLElement).getAttribute("start") ?? 1) || 1 }) }],
  },

  /** `checked` is null for a plain item, true or false for a task. */
  list_item: {
    content: "paragraph block*",
    attrs: { checked: { default: null as boolean | null } },
    defining: true,
    toDOM: (n) =>
      n.attrs["checked"] === null
        ? ["li", 0]
        : ["li", { class: "paper-task", "data-checked": String(n.attrs["checked"]) }, 0],
    parseDOM: [{ tag: "li" }],
  },

  code_block: {
    group: "block",
    content: "text*",
    marks: "",
    code: true,
    defining: true,
    attrs: { language: { default: "" } },
    toDOM: (n) => ["pre", { "data-language": String(n.attrs["language"]) }, ["code", 0]],
    parseDOM: [{ tag: "pre", preserveWhitespace: "full" }],
  },

  horizontal_rule: { group: "block", toDOM: () => ["hr"], parseDOM: [{ tag: "hr" }] },

  /** Display maths, `$$` or `<Equation>`; `form` remembers which. */
  equation: {
    group: "block",
    atom: true,
    attrs: { tex: { default: "" }, form: { default: "dollar" } },
    toDOM: () => ["div", { class: "paper-equation" }],
  },

  table: {
    group: "block",
    content: "table_row+",
    tableRole: "table",
    isolating: true,
    attrs: { caption: { default: "" } },
    toDOM: () => ["table", { class: "paper-table" }, ["tbody", 0]],
    parseDOM: [{ tag: "table" }],
  },
  table_row: { content: "(table_cell | table_header)*", tableRole: "row", toDOM: () => ["tr", 0], parseDOM: [{ tag: "tr" }] },
  table_cell: {
    content: "paragraph",
    attrs: cellAttrs,
    tableRole: "cell",
    isolating: true,
    toDOM: cellDOM("td"),
    parseDOM: [{ tag: "td" }],
  },
  table_header: {
    content: "paragraph",
    attrs: cellAttrs,
    tableRole: "header_cell",
    isolating: true,
    toDOM: cellDOM("th"),
    parseDOM: [{ tag: "th" }],
  },

  footnote_definition: {
    group: "block",
    content: "block+",
    attrs: { label: { default: "1" } },
    defining: true,
    toDOM: (n) => ["div", { class: "paper-footnote", "data-label": String(n.attrs["label"]) }, 0],
  },

  // The components. Containers keep their children editable in place.
  callout: {
    group: "block",
    content: "block+",
    defining: true,
    attrs: { type: { default: "note" }, title: { default: "" } },
    toDOM: (n) => ["aside", { class: `callout callout-${String(n.attrs["type"])}` }, 0],
  },
  details: {
    group: "block",
    content: "block+",
    defining: true,
    attrs: { summary: { default: "" }, open: { default: false } },
    toDOM: () => block("div", "paper-details"),
  },
  columns: { group: "block", content: "column{2,4}", defining: true, toDOM: () => block("div", "mdx-columns") },
  column: { content: "block+", isolating: true, toDOM: () => block("div", "mdx-column") },
  tabs: { group: "block", content: "tab+", defining: true, toDOM: () => block("div", "paper-tabs") },
  tab: { content: "block+", isolating: true, attrs: { label: { default: "" } }, toDOM: () => block("section", "paper-tab") },
  slide: {
    group: "block",
    content: "block+",
    defining: true,
    isolating: true,
    attrs: { layout: { default: "content" } },
    toDOM: (n) => ["section", { class: "paper-slide", "data-layout": String(n.attrs["layout"]) }, 0],
  },
  notes: { group: "block", content: "block+", defining: true, toDOM: () => block("aside", "mdx-speaker-notes") },
  page_header: { group: "block", content: "inline*", defining: true, toDOM: () => block("header", "mdx-page-header") },
  page_footer: { group: "block", content: "inline*", defining: true, toDOM: () => block("footer", "mdx-page-footer") },

  figure: {
    group: "block",
    atom: true,
    draggable: true,
    attrs: { src: { default: "" }, alt: { default: "" }, caption: { default: "" } },
    toDOM: () => ["figure", { class: "mdx-figure" }],
  },
  chart: {
    group: "block",
    atom: true,
    attrs: { src: { default: "" }, range: { default: "" }, type: { default: "bar" }, title: { default: "" } },
    toDOM: () => ["figure", { class: "mdx-chart" }],
  },
  page_break: { group: "block", atom: true, toDOM: () => ["div", { class: "page-break" }] },
  toc: { group: "block", atom: true, attrs: { title: { default: "" } }, toDOM: () => ["nav", { class: "mdx-toc" }] },

  /** Anything the editor does not draw: an element it does not know, an
   * expression, HTML in a `.md`, a link definition. Written back as it
   * came. */
  raw: {
    group: "block",
    atom: true,
    attrs: { source: { default: "" } },
    toDOM: (n) => ["pre", { class: "paper-raw" }, String(n.attrs["source"])],
  },

  text: { group: "inline" },
  hard_break: { group: "inline", inline: true, selectable: false, toDOM: () => ["br"], parseDOM: [{ tag: "br" }] },
  image: {
    group: "inline",
    inline: true,
    draggable: true,
    attrs: { src: { default: "" }, alt: { default: "" }, title: { default: null as string | null } },
    toDOM: () => ["img", { class: "paper-image" }],
  },
  math: { group: "inline", inline: true, atom: true, attrs: { tex: { default: "" }, form: { default: "dollar" } }, toDOM: () => ["span", { class: "paper-math" }] },
  mention: { group: "inline", inline: true, atom: true, attrs: { login: { default: "" } }, toDOM: (n) => ["span", { class: "mdx-mention" }, `@${String(n.attrs["login"])}`] },
  date: {
    group: "inline",
    inline: true,
    atom: true,
    attrs: { value: { default: "" }, format: { default: null as string | null } },
    toDOM: () => ["time", { class: "mdx-date" }],
  },
  page_number: { group: "inline", inline: true, atom: true, toDOM: () => ["span", { class: "mdx-page-number" }, "#"] },
  footnote_ref: { group: "inline", inline: true, atom: true, attrs: { label: { default: "1" } }, toDOM: (n) => ["sup", { class: "paper-footnote-ref" }, String(n.attrs["label"])] },
  embed: {
    group: "inline",
    inline: true,
    atom: true,
    attrs: { href: { default: "" }, title: { default: "" } },
    toDOM: () => ["span", { class: "mdx-embed" }],
  },
  raw_inline: { group: "inline", inline: true, atom: true, attrs: { source: { default: "" } }, toDOM: (n) => ["code", { class: "paper-raw" }, String(n.attrs["source"])] },
};

const simple = (tag: string, cls?: string): MarkSpec => ({
  toDOM: () => (cls === undefined ? [tag, 0] : [tag, { class: cls }, 0]),
  parseDOM: [{ tag }],
});

const marks: Record<string, MarkSpec> = {
  link: {
    attrs: { href: { default: "" }, title: { default: null as string | null } },
    inclusive: false,
    // The href goes on as data: the editor never navigates, and a
    // `javascript:` in a document never becomes a live attribute.
    toDOM: (m) => ["a", { class: "paper-link", "data-href": String(m.attrs["href"]), title: String(m.attrs["href"]) }, 0],
  },
  em: simple("em"),
  strong: simple("strong"),
  strike: simple("del"),
  underline: simple("u"),
  sub: { ...simple("sub"), excludes: "sup" },
  sup: { ...simple("sup"), excludes: "sub" },
  highlight: simple("mark"),
  small: simple("small"),
  kbd: simple("kbd"),
  abbr: { attrs: { title: { default: "" } }, toDOM: (m) => ["abbr", { title: String(m.attrs["title"]) }, 0] },
  comment: {
    attrs: { text: { default: "" }, author: { default: null as string | null }, date: { default: null as string | null } },
    inclusive: false,
    excludes: "",
    toDOM: (m) => ["span", { class: "mdx-comment", "data-comment": String(m.attrs["text"]) }, 0],
  },
  code: { excludes: "_", code: true, toDOM: () => ["code", 0], parseDOM: [{ tag: "code" }] },
};

export const schema = new Schema({ nodes, marks });
