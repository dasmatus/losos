/* MDX text to the editor's document: losos-mdx parses (wasm.ts), and the
 * tree it gives becomes nodes of schema.ts here.
 *
 * Each top-level node remembers the text it came from (`sourceOf`). A
 * ProseMirror node is never changed in place, only replaced, so a block
 * the person did not touch is the same object when the document is written
 * back, and to-mdx.ts writes its text exactly as it was: an edit in one
 * paragraph changes that paragraph of the file and nothing else.
 *
 * Whatever the schema has no node for, or an element whose props are not
 * the ones its component takes, becomes `raw` with its source, so a
 * document always opens and nothing in it is lost. */

import type { Mark, Node as PMNode } from "prosemirror-model";
import { ALIGNS, CALLOUT_TYPES, CHART_TYPES, DATE_FORMATS, SLIDE_LAYOUTS, schema } from "./schema";
import type { MdNode } from "./wasm";

const sources = new WeakMap<PMNode, string>();

/** The text a top-level block was read from, with the blank lines after
 * it; undefined for a block made or changed in the editor. */
export function sourceOf(node: PMNode): string | undefined {
  return sources.get(node);
}

/** Gives the top-level blocks of `doc` the texts of the same blocks in
 * `read`, a fresh read of the file: after a change from elsewhere is
 * merged in, the blocks it did not touch keep being written as the file
 * has them now. */
export function adoptSources(doc: PMNode, read: PMNode): void {
  if (doc.childCount !== read.childCount) return;
  doc.forEach((node, _, i) => {
    const other = read.child(i);
    const text = sources.get(other);
    if (text !== undefined && node.eq(other)) sources.set(node, text);
  });
}

type Attrs = Record<string, string | null>;

const INLINE = new Set(["text", "emphasis", "strong", "delete", "inlineCode", "link", "image", "break", "inlineMath", "footnoteReference", "mdxJsxTextElement", "mdxTextExpression", "linkReference", "imageReference"]);

/** Components that are blocks even when written on one line inside a
 * paragraph, as losos-mdx draws them (render.rs, `only_blocks`). */
const BLOCKS = new Set(["Callout", "Details", "Columns", "Tabs", "Slide", "Notes", "Header", "Footer", "Table", "Figure", "Chart", "PageBreak", "TableOfContents"]);

/** The element's props, or null when one is an expression or a spread. */
function propsOf(node: MdNode): Attrs | null {
  const out: Attrs = {};
  for (const a of node.attributes ?? []) {
    if (a.type !== "mdxJsxAttribute" || a.name === undefined) return null;
    if (a.value !== null && a.value !== undefined && typeof a.value !== "string") return null;
    out[a.name] = (a.value as string | null | undefined) ?? null;
  }
  return out;
}

/** Whether `props` holds only names from `allowed`, each with a value
 * (or none, for a flag) and the choices in range. */
function fits(props: Attrs, allowed: Record<string, readonly string[] | "text" | "flag" | "number" | "date">, required: string[] = []): boolean {
  for (const [name, value] of Object.entries(props)) {
    const kind = allowed[name];
    if (kind === undefined) return false;
    if (kind === "flag") {
      if (value !== null) return false;
      continue;
    }
    if (value === null) return false;
    if (kind === "number" && !/^\d{1,4}$/.test(value)) return false;
    if (kind === "date" && !/^\d{4}-\d\d-\d\d$/.test(value)) return false;
    if (Array.isArray(kind) && !kind.includes(value)) return false;
  }
  return required.every((r) => props[r] !== undefined && props[r] !== null);
}

class Reader {
  constructor(private readonly source: string) {}

  slice(node: MdNode): string {
    return this.source.slice(node.start, node.end);
  }

  raw(node: MdNode): PMNode {
    return schema.nodes["raw"]!.create({ source: this.slice(node) });
  }

  rawInline(node: MdNode): PMNode {
    return schema.nodes["raw_inline"]!.create({ source: this.slice(node) });
  }

  blocks(children: MdNode[] | undefined): PMNode[] {
    const out: PMNode[] = [];
    for (const c of children ?? []) out.push(...this.block(c));
    return out;
  }

  /** Children for a node whose content is `block+`. */
  content(children: MdNode[] | undefined): PMNode[] {
    const kids = children ?? [];
    if (kids.length > 0 && kids.every((k) => INLINE.has(k.type))) {
      return [schema.nodes["paragraph"]!.create(null, this.inline(kids, []))];
    }
    const out = this.blocks(children);
    return out.length > 0 ? out : [schema.nodes["paragraph"]!.create()];
  }

  /** Inline content from children that may be paragraphs or inline. */
  flatInline(children: MdNode[] | undefined): PMNode[] {
    const out: PMNode[] = [];
    for (const c of children ?? []) {
      if (c.type === "paragraph") {
        if (out.length > 0) out.push(schema.nodes["hard_break"]!.create());
        out.push(...this.inline(c.children, []));
      } else {
        out.push(...this.inline([c], []));
      }
    }
    return out;
  }

  block(node: MdNode): PMNode[] {
    const n = schema.nodes;
    switch (node.type) {
      case "paragraph": {
        const only = (node.children ?? []).filter((k) => !(k.type === "text" && (k.value ?? "").trim() === ""));
        const one = only[0];
        if (only.length === 1 && one?.type === "mdxJsxTextElement" && BLOCKS.has(one.name ?? "")) return this.flowElement(one);
        const inline = this.inline(node.children, []);
        return [n["paragraph"]!.create(null, inline)];
      }
      case "heading":
        return [n["heading"]!.create({ level: node.depth ?? 1 }, this.inline(node.children, []))];
      case "thematicBreak":
        return [n["horizontal_rule"]!.create()];
      case "blockquote":
        return [n["blockquote"]!.create(null, this.content(node.children))];
      case "list": {
        const items = (node.children ?? []).map((item) => {
          const kids = this.blocks(item.children);
          if (kids[0]?.type.name !== "paragraph") kids.unshift(n["paragraph"]!.create());
          return n["list_item"]!.create({ checked: item.checked ?? null }, kids);
        });
        if (items.length === 0) return [this.raw(node)];
        return node.ordered
          ? [n["ordered_list"]!.create({ tight: !node.spread, order: node.start_ ?? 1 }, items)]
          : [n["bullet_list"]!.create({ tight: !node.spread }, items)];
      }
      case "code":
        return [n["code_block"]!.create({ language: node.lang ?? "" }, node.value ? schema.text(node.value) : null)];
      case "math":
        return [n["equation"]!.create({ tex: node.value ?? "", form: "dollar" })];
      case "table":
        return [this.gfmTable(node)];
      case "footnoteDefinition":
        return [n["footnote_definition"]!.create({ label: node.label ?? node.identifier ?? "1" }, this.content(node.children))];
      case "mdxJsxFlowElement":
        return this.flowElement(node);
      default:
        // html, definition, expressions, esm, and anything newer.
        return [this.raw(node)];
    }
  }

  gfmTable(node: MdNode): PMNode {
    const n = schema.nodes;
    const rows = (node.children ?? []).map((row, r) =>
      n["table_row"]!.create(
        null,
        (row.children ?? []).map((cell, c) =>
          n[r === 0 ? "table_header" : "table_cell"]!.create(
            { align: node.align?.[c] ?? null },
            n["paragraph"]!.create(null, this.inline(cell.children, [])),
          ),
        ),
      ),
    );
    return n["table"]!.create(null, rows);
  }

  /** `<Table>` with `<Row>` and `<Cell>`, whether the rows were read as
   * flow or as text inside a paragraph. */
  componentTable(node: MdNode, caption: string): PMNode | null {
    const n = schema.nodes;
    const rows: MdNode[] = [];
    for (const c of node.children ?? []) {
      if (c.type === "paragraph") {
        for (const k of c.children ?? []) {
          if (k.type === "mdxJsxTextElement" && k.name === "Row") rows.push(k);
          else if (k.type !== "text" || (k.value ?? "").trim() !== "") return null;
        }
      } else if (c.type === "mdxJsxFlowElement" && c.name === "Row") {
        rows.push(c);
      } else {
        return null;
      }
    }
    if (rows.length === 0) return null;
    const out: PMNode[] = [];
    for (const row of rows) {
      const rp = propsOf(row);
      if (rp === null || !fits(rp, { header: "flag" })) return null;
      const header = "header" in rp;
      const cells: PMNode[] = [];
      const kids = (row.children ?? []).flatMap((k) => (k.type === "paragraph" ? (k.children ?? []) : [k]));
      for (const cell of kids) {
        if (cell.type === "text" && (cell.value ?? "").trim() === "") continue;
        if (!(cell.type === "mdxJsxTextElement" || cell.type === "mdxJsxFlowElement") || cell.name !== "Cell") return null;
        const cp = propsOf(cell);
        if (cp === null || !fits(cp, { colSpan: "number", rowSpan: "number", align: ALIGNS })) return null;
        cells.push(
          n[header ? "table_header" : "table_cell"]!.create(
            { colspan: Number(cp["colSpan"] ?? 1), rowspan: Number(cp["rowSpan"] ?? 1), align: cp["align"] ?? null },
            n["paragraph"]!.create(null, this.flatInline(cell.children)),
          ),
        );
      }
      out.push(n["table_row"]!.create(null, cells));
    }
    return n["table"]!.create({ caption }, out);
  }

  flowElement(node: MdNode): PMNode[] {
    const n = schema.nodes;
    const props = propsOf(node);
    if (props === null || node.name === null || node.name === undefined) return [this.raw(node)];
    const p = (k: string) => props[k] ?? "";
    const container = (type: string, attrs: Record<string, unknown> | null) => [n[type]!.create(attrs, this.content(node.children))];
    switch (node.name) {
      case "Callout":
        if (!fits(props, { type: CALLOUT_TYPES, title: "text" })) break;
        return container("callout", { type: props["type"] ?? "note", title: p("title") });
      case "Details":
        if (!fits(props, { summary: "text", open: "flag" })) break;
        return container("details", { summary: p("summary"), open: "open" in props });
      case "Notes":
        if (!fits(props, {})) break;
        return container("notes", null);
      case "Slide":
        if (!fits(props, { layout: SLIDE_LAYOUTS })) break;
        return container("slide", { layout: props["layout"] ?? "content" });
      case "Columns": {
        if (!fits(props, {})) break;
        const cols = (node.children ?? []).filter((c) => !(c.type === "text" && !(c.value ?? "").trim()));
        if (cols.length < 2 || cols.length > 4 || cols.some((c) => c.type !== "mdxJsxFlowElement" || c.name !== "Column" || (c.attributes ?? []).length > 0)) break;
        return [n["columns"]!.create(null, cols.map((c) => n["column"]!.create(null, this.content(c.children))))];
      }
      case "Tabs": {
        if (!fits(props, {})) break;
        const tabs = node.children ?? [];
        const ok = tabs.length > 0 && tabs.every((c) => c.type === "mdxJsxFlowElement" && c.name === "Tab" && propsOf(c) !== null && fits(propsOf(c)!, { label: "text" }, ["label"]));
        if (!ok) break;
        return [n["tabs"]!.create(null, tabs.map((c) => n["tab"]!.create({ label: propsOf(c)!["label"] }, this.content(c.children))))];
      }
      case "Header":
      case "Footer":
        if (!fits(props, {})) break;
        return [n[node.name === "Header" ? "page_header" : "page_footer"]!.create(null, this.flatInline(node.children))];
      case "Figure":
        if (!fits(props, { src: "text", alt: "text", caption: "text" }, ["src"]) || (node.children ?? []).length > 0) break;
        return [n["figure"]!.create({ src: p("src"), alt: p("alt"), caption: p("caption") })];
      case "Chart":
        if (!fits(props, { src: "text", range: "text", type: CHART_TYPES, title: "text" }, ["src"]) || (node.children ?? []).length > 0) break;
        return [n["chart"]!.create({ src: p("src"), range: p("range"), type: props["type"] ?? "bar", title: p("title") })];
      case "PageBreak":
        if (!fits(props, {}) || (node.children ?? []).length > 0) break;
        return [n["page_break"]!.create()];
      case "TableOfContents":
        if (!fits(props, { title: "text" }) || (node.children ?? []).length > 0) break;
        return [n["toc"]!.create({ title: p("title") })];
      case "Equation":
        if (!fits(props, { tex: "text", inline: "flag" }, ["tex"]) || "inline" in props) break;
        return [n["equation"]!.create({ tex: p("tex"), form: "component" })];
      case "Table": {
        if (!fits(props, { caption: "text" })) break;
        const table = this.componentTable(node, p("caption"));
        if (table !== null) return [table];
        break;
      }
      default: {
        // An inline component written on a line of its own.
        const inline = this.textElement(node, []);
        if (inline !== null && inline.length > 0 && inline.every((i) => i.isInline)) return [n["paragraph"]!.create(null, inline)];
      }
    }
    return [this.raw(node)];
  }

  /** The nodes for an inline JSX element, or null when it is not one the
   * editor draws. */
  textElement(node: MdNode, marks: readonly Mark[]): PMNode[] | null {
    const n = schema.nodes;
    const m = schema.marks;
    const props = propsOf(node);
    if (props === null || !node.name) return null;
    const empty = (node.children ?? []).length === 0;
    const atom = (type: string, attrs: Record<string, unknown>) => [n[type]!.create(attrs, null, marks)];
    const mark = (type: string, attrs: Record<string, unknown> | null = null) =>
      this.inline(node.children, m[type]!.create(attrs).addToSet(marks));
    switch (node.name) {
      case "Mention":
        return empty && fits(props, { login: "text" }, ["login"]) ? atom("mention", { login: props["login"] }) : null;
      case "Date":
        return empty && fits(props, { value: "date", format: DATE_FORMATS }, ["value"]) ? atom("date", { value: props["value"], format: props["format"] ?? null }) : null;
      case "PageNumber":
        return empty && fits(props, {}) ? atom("page_number", {}) : null;
      case "Embed":
        return empty && fits(props, { href: "text", title: "text" }, ["href"]) ? atom("embed", { href: props["href"], title: props["title"] ?? "" }) : null;
      case "Equation":
        return empty && fits(props, { tex: "text", inline: "flag" }, ["tex"]) ? atom("math", { tex: props["tex"], form: "component" }) : null;
      case "Comment":
        return fits(props, { text: "text", author: "text", date: "date" }, ["text"])
          ? mark("comment", { text: props["text"], author: props["author"] ?? null, date: props["date"] ?? null })
          : null;
      case "br":
        return empty && fits(props, {}) ? atom("hard_break", {}) : null;
      case "abbr":
        return fits(props, { title: "text" }) ? mark("abbr", { title: props["title"] ?? "" }) : null;
    }
    const plain: Record<string, string> = { u: "underline", sub: "sub", sup: "sup", mark: "highlight", small: "small", kbd: "kbd", s: "strike" };
    const type = plain[node.name];
    return type !== undefined && fits(props, {}) ? mark(type) : null;
  }

  inline(children: MdNode[] | undefined, marks: readonly Mark[]): PMNode[] {
    const n = schema.nodes;
    const m = schema.marks;
    const out: PMNode[] = [];
    for (const c of children ?? []) {
      switch (c.type) {
        case "text":
          if (c.value) out.push(schema.text(c.value, marks));
          break;
        case "emphasis":
          out.push(...this.inline(c.children, m["em"]!.create().addToSet(marks)));
          break;
        case "strong":
          out.push(...this.inline(c.children, m["strong"]!.create().addToSet(marks)));
          break;
        case "delete":
          out.push(...this.inline(c.children, m["strike"]!.create().addToSet(marks)));
          break;
        case "inlineCode":
          if (c.value) out.push(schema.text(c.value, m["code"]!.create().addToSet(marks)));
          break;
        case "link":
          out.push(...this.inline(c.children, m["link"]!.create({ href: c.url ?? "", title: c.title ?? null }).addToSet(marks)));
          break;
        case "image":
          out.push(n["image"]!.create({ src: c.url ?? "", alt: c.alt ?? "", title: c.title ?? null }, null, marks));
          break;
        case "break":
          out.push(n["hard_break"]!.create());
          break;
        case "inlineMath":
          out.push(n["math"]!.create({ tex: c.value ?? "", form: "dollar" }, null, marks));
          break;
        case "footnoteReference":
          out.push(n["footnote_ref"]!.create({ label: c.label ?? c.identifier ?? "1" }, null, marks));
          break;
        case "mdxJsxTextElement": {
          const made = this.textElement(c, marks);
          out.push(...(made ?? [this.rawInline(c)]));
          break;
        }
        default:
          out.push(this.rawInline(c));
      }
    }
    return out;
  }
}

/** The editor's document for `source`, read through `tree` (parseMdx's
 * result for that same text). */
export function fromMdast(tree: MdNode, source: string): PMNode {
  const reader = new Reader(source);
  const kids = tree.children ?? [];
  const blocks: PMNode[] = [];
  kids.forEach((child, i) => {
    const made =
      i === 0 && child.type === "yaml"
        ? [schema.nodes["front_matter"]!.create({ yaml: child.value ?? "" })]
        : reader.block(child);
    const end = kids[i + 1]?.start ?? source.length;
    // One block per child, so its text is its own; a child that made two
    // has no one text to give back and is written afresh.
    if (made.length === 1) sources.set(made[0]!, source.slice(child.start, end));
    blocks.push(...made);
  });
  if (!blocks.some((b) => b.type.name !== "front_matter")) blocks.push(schema.nodes["paragraph"]!.create());
  return schema.nodes["doc"]!.create(null, blocks);
}
