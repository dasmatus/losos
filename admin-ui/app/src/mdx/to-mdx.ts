/* The editor's document back to MDX text, the file LosOS Office keeps.
 *
 * A top-level block read from the file and not touched since is written
 * as it was read (from-mdast.ts, `sourceOf`); only blocks made or changed
 * in the editor are written here, in the plainest form losos-mdx reads:
 * Markdown where Markdown says it (`**bold**`, `- item`, a pipe table) and
 * a component where it does not (`<Callout>`, `<Table>` with merged
 * cells). tests/mdx-editor.unit.mjs reads every form back. */

import type { Mark, Node as PMNode } from "prosemirror-model";
import { escapeText } from "./escape";
import { sourceOf } from "./from-mdast";

export { escapeText };

/** `"` and `&` as the JSX reader decodes them; nothing else needs it. */
function attrValue(value: string): string {
  return value.replace(/&/g, "&amp;").replace(/"/g, "&quot;");
}

/** ` name="value"` for each prop with a value, ` name` for a flag that is
 * on, nothing for an empty or off one. */
export function props(list: [string, string | boolean | number | null | undefined][]): string {
  let out = "";
  for (const [name, value] of list) {
    if (value === true) out += ` ${name}`;
    else if (typeof value === "number") out += ` ${name}="${value}"`;
    else if (typeof value === "string" && value !== "") out += ` ${name}="${attrValue(value)}"`;
  }
  return out;
}

function codeSpan(text: string): string {
  const longest = Math.max(0, ...[...text.matchAll(/`+/g)].map((m) => m[0].length));
  const fence = "`".repeat(longest + 1);
  const pad = text.startsWith("`") || text.endsWith("`") || (text.startsWith(" ") && text.endsWith(" ") && text.trim() !== "") ? " " : "";
  return fence + pad + text + pad + fence;
}

function destination(url: string): string {
  return /[\s()<>]/.test(url) ? `<${url.replace(/[<>]/g, encodeURIComponent)}>` : url;
}

function linkTitle(title: string | null): string {
  return title ? ` "${title.replace(/["\\]/g, "\\$&")}"` : "";
}

interface InlineContext {
  /** In a pipe table: a line break is `<br />` and a newline is refused. */
  cell: boolean;
}

function wrap(mark: Mark, inner: string, _ctx: InlineContext): string {
  if (inner === "") return "";
  // Markdown's delimiters must touch the text, so spaces at either end go
  // outside them.
  const lead = /^\s*/.exec(inner)![0];
  const trail = inner.slice(lead.length).match(/\s*$/)![0];
  const core = inner.slice(lead.length, inner.length - trail.length);
  const around = (open: string, close: string) => (core === "" ? inner : lead + open + core + close + trail);
  const a = mark.attrs as Record<string, string | null>;
  switch (mark.type.name) {
    case "em":
      return around("*", "*");
    case "strong":
      return around("**", "**");
    case "strike":
      return around("~~", "~~");
    case "link":
      return `[${inner}](${destination(a["href"] ?? "")}${linkTitle(a["title"] ?? null)})`;
    case "underline":
      return `<u>${inner}</u>`;
    case "sub":
      return `<sub>${inner}</sub>`;
    case "sup":
      return `<sup>${inner}</sup>`;
    case "highlight":
      return `<mark>${inner}</mark>`;
    case "small":
      return `<small>${inner}</small>`;
    case "kbd":
      return `<kbd>${inner}</kbd>`;
    case "abbr":
      return `<abbr${props([["title", a["title"]]])}>${inner}</abbr>`;
    case "comment":
      return `<Comment${props([
        ["text", a["text"]],
        ["author", a["author"]],
        ["date", a["date"]],
      ])}>${inner}</Comment>`;
    default:
      return inner;
  }
}

function leaf(node: PMNode, lineStart: boolean, ctx: InlineContext): string {
  const a = node.attrs as Record<string, string | null>;
  switch (node.type.name) {
    case "text":
      if (node.marks.some((m) => m.type.name === "code")) return codeSpan(node.text ?? "");
      return escapeText(node.text ?? "", lineStart);
    case "hard_break":
      return ctx.cell ? "<br />" : "\\\n";
    case "image":
      return `![${escapeText(a["alt"] ?? "", false)}](${destination(a["src"] ?? "")}${linkTitle(a["title"] ?? null)})`;
    case "math": {
      const tex = a["tex"] ?? "";
      return a["form"] === "dollar" && tex !== "" && !tex.includes("$") && !/^\s|\s$/.test(tex)
        ? `$${tex}$`
        : `<Equation${props([["tex", tex], ["inline", true]])} />`;
    }
    case "mention":
      return `<Mention${props([["login", a["login"]]])} />`;
    case "date":
      return `<Date${props([
        ["value", a["value"]],
        ["format", a["format"]],
      ])} />`;
    case "page_number":
      return "<PageNumber />";
    case "footnote_ref":
      return `[^${a["label"] ?? "1"}]`;
    case "embed":
      return `<Embed${props([
        ["href", a["href"]],
        ["title", a["title"]],
      ])} />`;
    case "raw_inline":
      return a["source"] ?? "";
    default:
      return node.textContent;
  }
}

/** Inline content, its marks nested outermost first: consecutive nodes
 * that share a mark are written inside one pair of its delimiters. */
function inline(nodes: readonly PMNode[], open: readonly Mark[], lineStart: boolean, ctx: InlineContext): string {
  let out = "";
  let i = 0;
  while (i < nodes.length) {
    const node = nodes[i]!;
    const start = lineStart && (out === "" || out.endsWith("\n"));
    const mark = node.marks.find((m) => m.type.name !== "code" && !m.isInSet(open));
    if (mark === undefined) {
      out += leaf(node, start, ctx);
      i++;
      continue;
    }
    let j = i;
    while (j < nodes.length && mark.isInSet(nodes[j]!.marks)) j++;
    out += wrap(mark, inline(nodes.slice(i, j), [...open, mark], start && mark.type.name !== "link", ctx), ctx);
    i = j;
  }
  return out;
}

function inlineOf(node: PMNode, ctx: InlineContext = { cell: false }): string {
  const kids: PMNode[] = [];
  node.forEach((k) => kids.push(k));
  return inline(kids, [], true, ctx);
}

function indent(text: string, first: string, rest: string): string {
  return text
    .split("\n")
    .map((line, i) => (i === 0 ? first + line : line === "" ? "" : rest + line))
    .join("\n");
}

function children(node: PMNode, sep = "\n\n"): string {
  const out: string[] = [];
  node.forEach((k) => out.push(block(k)));
  return out.join(sep);
}

function element(name: string, attrs: string, body: string): string {
  return `<${name}${attrs}>\n${body}\n</${name}>`;
}

function fence(text: string, info: string): string {
  const longest = Math.max(2, ...[...text.matchAll(/^`{3,}/gm)].map((m) => m[0].length));
  const f = "`".repeat(longest + 1);
  return `${f}${info}\n${text}${text === "" || text.endsWith("\n") ? "" : "\n"}${f}`;
}

/** Whether a table can be a pipe table: one header row on top, no merged
 * cells, no caption. */
function plainTable(table: PMNode): boolean {
  if (table.attrs["caption"]) return false;
  let ok = true;
  table.forEach((row, _, r) => {
    row.forEach((cell) => {
      if (cell.attrs["colspan"] !== 1 || cell.attrs["rowspan"] !== 1) ok = false;
      if ((cell.type.name === "table_header") !== (r === 0)) ok = false;
    });
  });
  const widths = new Set<number>();
  table.forEach((row) => widths.add(row.childCount));
  return ok && widths.size === 1 && table.firstChild !== null && table.firstChild.childCount > 0;
}

function pipeTable(table: PMNode): string {
  const rows: string[][] = [];
  table.forEach((row) => {
    const cells: string[] = [];
    row.forEach((cell) => cells.push(inlineOf(cell.firstChild!, { cell: true }).replace(/\n/g, " ").trim()));
    rows.push(cells);
  });
  const header = table.firstChild!;
  const aligns: string[] = [];
  header.forEach((cell) => {
    const a = cell.attrs["align"] as string | null;
    aligns.push(a === "left" ? ":--" : a === "right" ? "--:" : a === "center" ? ":-:" : "---");
  });
  const line = (cells: string[]) => `| ${cells.join(" | ")} |`;
  return [line(rows[0]!), line(aligns), ...rows.slice(1).map(line)].join("\n");
}

function componentTable(table: PMNode): string {
  const rows: string[] = [];
  table.forEach((row) => {
    let header = true;
    let cells = "";
    row.forEach((cell) => {
      if (cell.type.name !== "table_header") header = false;
      cells += `<Cell${props([
        ["colSpan", cell.attrs["colspan"] === 1 ? null : (cell.attrs["colspan"] as number)],
        ["rowSpan", cell.attrs["rowspan"] === 1 ? null : (cell.attrs["rowspan"] as number)],
        ["align", cell.attrs["align"] as string | null],
      ])}>${inlineOf(cell.firstChild!, { cell: true }).replace(/\n/g, " ")}</Cell>`;
    });
    rows.push(`<Row${props([["header", header && row.childCount > 0]])}>${cells}</Row>`);
  });
  return element("Table", props([["caption", table.attrs["caption"] as string]]), rows.join("\n"));
}

/** One block as MDX, with no blank line after it. */
export function block(node: PMNode): string {
  const a = node.attrs as Record<string, unknown>;
  const s = (k: string) => (typeof a[k] === "string" ? (a[k] as string) : null);
  switch (node.type.name) {
    case "front_matter":
      return `---\n${s("yaml") ?? ""}\n---`;
    case "paragraph":
      return inlineOf(node);
    case "heading": {
      const text = inlineOf(node).replace(/\n/g, " ");
      return `${"#".repeat(Number(a["level"]) || 1)}${text === "" ? "" : " " + text}`;
    }
    case "blockquote":
      return children(node)
        .split("\n")
        .map((l) => (l === "" ? ">" : "> " + l))
        .join("\n");
    case "bullet_list":
    case "ordered_list": {
      const ordered = node.type.name === "ordered_list";
      const tight = a["tight"] !== false;
      const items: string[] = [];
      let n = Number(a["order"]) || 1;
      node.forEach((item) => {
        const marker = ordered ? `${n++}. ` : "- ";
        const checked = item.attrs["checked"] as boolean | null;
        const task = checked === null ? "" : checked ? "[x] " : "[ ] ";
        const body = children(item, tight ? "\n" : "\n\n");
        items.push(indent(task + body, marker, " ".repeat(marker.length)));
      });
      return items.join(tight ? "\n" : "\n\n");
    }
    case "list_item":
      return children(node);
    case "code_block":
      return fence(node.textContent, s("language") ?? "");
    case "horizontal_rule":
      return "---";
    case "equation": {
      const tex = s("tex") ?? "";
      return a["form"] === "dollar" && !/^\s*\$\$/m.test(tex) ? `$$\n${tex}\n$$` : `<Equation${props([["tex", tex]])} />`;
    }
    case "table":
      return plainTable(node) ? pipeTable(node) : componentTable(node);
    case "footnote_definition":
      return indent(children(node), `[^${s("label") ?? "1"}]: `, "    ");
    case "callout":
      return element("Callout", props([["type", s("type")], ["title", s("title")]]), children(node));
    case "details":
      return element("Details", props([["summary", s("summary")], ["open", a["open"] === true]]), children(node));
    case "columns":
      return element("Columns", "", children(node, "\n"));
    case "column":
      return element("Column", "", children(node));
    case "tabs":
      return element("Tabs", "", children(node, "\n"));
    case "tab":
      return element("Tab", props([["label", s("label")]]), children(node));
    case "slide":
      return element("Slide", props([["layout", s("layout")]]), children(node));
    case "notes":
      return element("Notes", "", children(node));
    case "page_header":
      return `<Header>${inlineOf(node).replace(/\n/g, " ")}</Header>`;
    case "page_footer":
      return `<Footer>${inlineOf(node).replace(/\n/g, " ")}</Footer>`;
    case "figure":
      return `<Figure${props([["src", s("src")], ["alt", s("alt")], ["caption", s("caption")]])} />`;
    case "chart":
      return `<Chart${props([["src", s("src")], ["range", s("range")], ["type", s("type")], ["title", s("title")]])} />`;
    case "page_break":
      return "<PageBreak />";
    case "toc":
      return `<TableOfContents${props([["title", s("title")]])} />`;
    case "raw":
      return (s("source") ?? "").replace(/\s+$/, "");
    default:
      return node.textContent;
  }
}

/** The whole document as the file's text, with where each top-level
 * block starts in it. */
export function toMdxParts(doc: PMNode): { text: string; starts: number[] } {
  let out = "";
  const starts: number[] = [];
  const last = doc.childCount - 1;
  doc.forEach((node, _, i) => {
    if (out !== "" && !out.endsWith("\n\n")) out += out.endsWith("\n") ? "\n" : "\n\n";
    starts.push(out.length);
    const kept = sourceOf(node);
    out += kept ?? block(node) + (i === last ? "\n" : "\n\n");
  });
  return { text: out, starts };
}

/** The whole document as the file's text. */
export function toMdx(doc: PMNode): string {
  return toMdxParts(doc).text;
}
