/* How each component looks on the page while it is edited: a callout with
 * its title, a figure with its picture, an equation drawn, a table of
 * contents that follows the headings, a date in words. Equations and dates
 * come from losos-mdx, so the page shows what an export and the box's
 * HTML (which needs no script) show.
 * The text inside a container stays editable in place; a component with
 * no text of its own (a figure, a chart, a page break) is one piece the
 * caret steps over, and its fields are under the bar when it is selected
 * (fields.tsx).
 *
 * Nothing here sets a `style` attribute (the page's CSP refuses them):
 * every look is a class in paper.css. A link is never followed from the
 * page, and a picture is only ever a file beside the document or an
 * https address. */

import type { Node as PMNode } from "prosemirror-model";
import { Plugin } from "prosemirror-state";
import type { EditorView, NodeView } from "prosemirror-view";
import { formatDate, mathml } from "@/mdx/wasm";

export interface ViewContext {
  /** The address of a picture the document names, or null for one that
   * cannot be shown. */
  picture: (src: string) => string | null;
  locale: string;
  label: (key: ViewLabel) => string;
}

export type ViewLabel =
  | "callout.note"
  | "callout.tip"
  | "callout.important"
  | "callout.warning"
  | "callout.caution"
  | "details"
  | "notes"
  | "pageBreak"
  | "contents"
  | "noHeadings"
  | "header"
  | "footer"
  | "chart"
  | "equation"
  | "raw"
  | "slide"
  | "layout.title"
  | "layout.content"
  | "layout.two-columns"
  | "layout.picture"
  | "noPicture";

function el<K extends keyof HTMLElementTagNameMap>(tag: K, cls?: string, text?: string): HTMLElementTagNameMap[K] {
  const e = document.createElement(tag);
  if (cls !== undefined) e.className = cls;
  if (text !== undefined) e.textContent = text;
  return e;
}

/** A strip on a container that is not part of its text. */
function chrome(cls: string, text: string): HTMLElement {
  const e = el("div", cls, text);
  e.contentEditable = "false";
  return e;
}

/** The document's own language, from its front matter, if it names one. */
function docLang(view: EditorView): string | null {
  const first = view.state.doc.firstChild;
  if (first?.type.name !== "front_matter") return null;
  return /^lang:\s*["']?([A-Za-z]{2,3}(?:[-_][A-Za-z0-9]+)*)/m.exec(String(first.attrs["yaml"]))?.[1] ?? null;
}

/** An equation as MathML, or its TeX as it is when it does not read. */
function equation(cls: string, tag: "div" | "span", tex: string, block: boolean, placeholder: string): HTMLElement {
  const d = el(tag, cls);
  if (tex === "") {
    d.append(el("span", "paper-placeholder", placeholder));
    return d;
  }
  const m = mathml(tex, block);
  if (m === null) d.append(el("code", "language-math", tex));
  else d.innerHTML = m;
  return d;
}

/** A heading's id as losos-mdx makes it (render.rs, `slug`). */
export function slug(text: string): string {
  let out = "";
  for (const c of text.trim()) {
    if (/[\p{L}\p{N}_-]/u.test(c)) out += c.toLowerCase();
    else if (/\s/.test(c)) out += "-";
  }
  return out;
}

type Factory = (node: PMNode, view: EditorView, getPos: () => number | undefined) => NodeView;

/** A view drawn from the node alone; a changed node is drawn afresh by
 * ProseMirror, which a false from `update` asks for. */
function drawn(type: string, make: (node: PMNode) => HTMLElement): Factory {
  return (first) => ({
    dom: make(first),
    update: (next) => next.type.name === type && next.eq(first),
    ignoreMutation: () => true,
  });
}

/** A container: a strip from `head`, then its editable children. */
function container(type: string, tag: keyof HTMLElementTagNameMap, cls: (n: PMNode) => string, head: ((n: PMNode) => HTMLElement | null) | null): Factory {
  return (first) => {
    const dom = el(tag, cls(first));
    let strip = head?.(first) ?? null;
    if (strip !== null) dom.append(strip);
    const contentDOM = el("div", "paper-body");
    dom.append(contentDOM);
    return {
      dom,
      contentDOM,
      update(next) {
        if (next.type.name !== type) return false;
        dom.className = cls(next);
        const fresh = head?.(next) ?? null;
        if (strip !== null && fresh !== null) strip.replaceWith(fresh);
        strip = fresh;
        return true;
      },
      ignoreMutation: (m) => strip !== null && strip.contains(m.target),
    };
  };
}

export function nodeViews(ctx: ViewContext): Record<string, Factory> {
  const picture = (src: string, alt: string): HTMLElement => {
    const url = ctx.picture(src);
    if (url === null) return el("span", "paper-missing", ctx.label("noPicture"));
    const img = el("img");
    img.src = url;
    img.alt = alt;
    img.draggable = false;
    return img;
  };

  return {
    callout: container(
      "callout",
      "aside",
      (n) => `callout callout-${String(n.attrs["type"])}`,
      (n) => chrome("callout-title", String(n.attrs["title"]) || ctx.label(`callout.${String(n.attrs["type"])}` as ViewLabel)),
    ),
    details: container("details", "div", () => "paper-details", (n) => {
      const s = chrome("paper-details-summary", String(n.attrs["summary"]) || ctx.label("details"));
      s.dataset["open"] = String(n.attrs["open"]);
      return s;
    }),
    tab: container("tab", "section", () => "paper-tab", (n) => chrome("paper-tab-label", String(n.attrs["label"]))),
    notes: container("notes", "aside", () => "mdx-speaker-notes", () => chrome("paper-chrome", ctx.label("notes"))),
    slide: container(
      "slide",
      "section",
      (n) => `paper-slide paper-slide-${String(n.attrs["layout"])}`,
      (n) => chrome("paper-slide-label", `${ctx.label("slide")} · ${ctx.label(`layout.${String(n.attrs["layout"])}` as ViewLabel)}`),
    ),
    page_header: container("page_header", "header", () => "mdx-page-header", () => chrome("paper-chrome", ctx.label("header"))),
    page_footer: container("page_footer", "footer", () => "mdx-page-footer", () => chrome("paper-chrome", ctx.label("footer"))),
    footnote_definition: container("footnote_definition", "div", () => "paper-footnote", (n) => chrome("paper-footnote-label", `${String(n.attrs["label"])}.`)),

    figure: drawn("figure", (n) => {
      const f = el("figure", "mdx-figure");
      f.append(picture(String(n.attrs["src"]), String(n.attrs["alt"])));
      const caption = String(n.attrs["caption"]);
      if (caption !== "") f.append(el("figcaption", undefined, caption));
      return f;
    }),
    image: drawn("image", (n) => {
      const p = picture(String(n.attrs["src"]), String(n.attrs["alt"]));
      p.classList.add("paper-image");
      return p;
    }),
    chart: drawn("chart", (n) => {
      const f = el("figure", "mdx-chart paper-chart");
      f.dataset["type"] = String(n.attrs["type"]);
      const bars = el("div", "paper-chart-bars");
      for (let i = 0; i < 5; i++) bars.append(el("span"));
      f.append(bars);
      const title = String(n.attrs["title"]);
      const range = String(n.attrs["range"]);
      f.append(el("figcaption", undefined, title !== "" ? title : ctx.label("chart")));
      f.append(el("p", "paper-chart-source", `${String(n.attrs["src"])}${range ? ` · ${range}` : ""}`));
      return f;
    }),
    equation: drawn("equation", (n) => equation("paper-equation", "div", String(n.attrs["tex"]), true, ctx.label("equation"))),
    math: drawn("math", (n) => equation("paper-math", "span", String(n.attrs["tex"]), false, ctx.label("equation"))),
    date: (first, view) => {
      const value = String(first.attrs["value"]);
      const t = el("time", "mdx-date", formatDate(value, first.attrs["format"] === "long", docLang(view) ?? ctx.locale));
      t.dateTime = value;
      return { dom: t, update: (next) => next.type.name === "date" && next.eq(first) };
    },
    embed: drawn("embed", (n) => {
      const a = el("span", "mdx-embed paper-embed");
      const href = String(n.attrs["href"]);
      let host = href;
      try {
        host = new URL(href).host;
      } catch {
        /* a path */
      }
      a.append(el("span", "paper-embed-title", String(n.attrs["title"]) || host), el("span", "paper-embed-host", host));
      return a;
    }),
    page_break: drawn("page_break", () => {
      const d = el("div", "page-break");
      d.append(el("span", undefined, ctx.label("pageBreak")));
      return d;
    }),
    raw: drawn("raw", (n) => {
      const d = el("div", "paper-raw");
      d.append(el("span", "paper-raw-label", ctx.label("raw")), el("pre", undefined, String(n.attrs["source"])));
      return d;
    }),
    raw_inline: drawn("raw_inline", (n) => el("code", "paper-raw", String(n.attrs["source"]))),
    toc: drawn("toc", (n) => {
      const nav = el("nav", "mdx-toc");
      nav.append(el("p", "mdx-toc-title", String(n.attrs["title"]) || ctx.label("contents")));
      nav.append(el("ol", "paper-toc-list"));
      return nav;
    }),
  };
}

/** Fills every table of contents on the page with the headings, each
 * time the document changes. */
export function tocPlugin(ctx: ViewContext) {
  const fill = (view: EditorView) => {
    const lists = view.dom.querySelectorAll<HTMLOListElement>(".paper-toc-list");
    if (lists.length === 0) return;
    const items: { level: number; text: string }[] = [];
    view.state.doc.descendants((n) => {
      if (n.type.name === "heading") items.push({ level: Number(n.attrs["level"]), text: n.textContent });
      return n.isBlock && !n.isTextblock;
    });
    for (const list of lists) {
      list.replaceChildren(
        ...(items.length === 0
          ? [el("li", "paper-placeholder", ctx.label("noHeadings"))]
          : items.map((h) => {
              const li = el("li", `paper-toc-${h.level}`, h.text);
              li.dataset["target"] = slug(h.text);
              return li;
            })),
      );
    }
  };
  return new Plugin({
    view: (view) => {
      fill(view);
      return {
        update: (v, prev) => {
          if (v.state.doc !== prev.doc) fill(v);
        },
      };
    },
  });
}
