/* The Markdown a card holds, as Obsidian writes it: headings, paragraphs,
 * lists with task boxes, quotes and callouts (`> [!note] Title`), code,
 * rules, and inline bold, italic, strike, ==highlight==, `code`, links,
 * bare addresses and [[wiki links]]. Parsed to a small tree that two
 * readers share: the card on the canvas (markdown.tsx) and the export,
 * which sets the same tree as SVG text.
 *
 * Nothing here makes HTML from the card's words: the tree becomes React
 * elements or SVG text, so a card cannot carry markup or script. */

export type Inline =
  | { k: "t"; text: string }
  | { k: "b" | "i" | "s" | "mark"; kids: Inline[] }
  | { k: "code"; text: string }
  | { k: "a"; href: string; kids: Inline[] }
  | { k: "wiki"; target: string; label: string };

export interface Item {
  kids: Inline[];
  /** null: no box; true or false: a task, done or not. */
  task: boolean | null;
  depth: number;
}

export type Block =
  | { k: "h"; level: number; kids: Inline[] }
  | { k: "p"; kids: Inline[] }
  | { k: "ul" | "ol"; items: Item[]; start: number }
  | { k: "quote"; callout: string | null; title: Inline[] | null; blocks: Block[] }
  | { k: "code"; text: string }
  | { k: "hr" };

/** An address a card may link to: the web and mail, nothing that runs. */
export function safeHref(href: string): string | null {
  const h = href.trim();
  return /^(https?:\/\/|mailto:)/i.test(h) ? h : null;
}

const INLINE: [RegExp, (m: RegExpExecArray) => Inline | null][] = [
  [/`([^`\n]+)`/y, (m) => ({ k: "code", text: m[1]! })],
  [/\[\[([^\]|\n]+)(?:\|([^\]\n]+))?\]\]/y, (m) => ({ k: "wiki", target: m[1]!.trim(), label: (m[2] ?? m[1]!).trim() })],
  [
    /\[([^\]\n]+)\]\(([^)\s]+)\)/y,
    (m) => {
      const href = safeHref(m[2]!);
      return href ? { k: "a", href, kids: inline(m[1]!) } : { k: "t", text: m[1]! };
    },
  ],
  [/(https?:\/\/[^\s<>()]+[^\s<>().,;:!?'"])/y, (m) => ({ k: "a", href: m[1]!, kids: [{ k: "t", text: m[1]! }] })],
  [/\*\*(?=\S)([^\n]*?\S)\*\*/y, (m) => ({ k: "b", kids: inline(m[1]!) })],
  [/__(?=\S)([^\n]*?\S)__/y, (m) => ({ k: "b", kids: inline(m[1]!) })],
  [/~~(?=\S)([^\n]*?\S)~~/y, (m) => ({ k: "s", kids: inline(m[1]!) })],
  [/==(?=\S)([^\n]*?\S)==/y, (m) => ({ k: "mark", kids: inline(m[1]!) })],
  [/\*(?=[^\s*])([^*\n]*?[^\s*])\*/y, (m) => ({ k: "i", kids: inline(m[1]!) })],
  [/(?<![\w])_(?=[^\s_])([^_\n]*?[^\s_])_(?![\w])/y, (m) => ({ k: "i", kids: inline(m[1]!) })],
];

/** The characters an inline mark can start with. */
const STARTS = new Set(["`", "[", "h", "*", "_", "~", "="]);

export function inline(src: string): Inline[] {
  const out: Inline[] = [];
  let text = "";
  let i = 0;
  const flush = () => {
    if (text) out.push({ k: "t", text });
    text = "";
  };
  outer: while (i < src.length) {
    const ch = src[i]!;
    if (ch === "\\" && i + 1 < src.length && /[\\`*_~=[\]()#>-]/.test(src[i + 1]!)) {
      text += src[i + 1];
      i += 2;
      continue;
    }
    if (STARTS.has(ch)) {
      for (const [re, make] of INLINE) {
        re.lastIndex = i;
        const m = re.exec(src);
        if (m && m.index === i) {
          const node = make(m);
          if (node) {
            flush();
            out.push(node);
            i += m[0].length;
            continue outer;
          }
        }
      }
    }
    text += ch;
    i += 1;
  }
  flush();
  return out;
}

const LIST = /^(\s*)([-*+]|\d{1,9}[.)])\s+(?:\[([ xX])\]\s+)?(.*)$/;

export function parse(src: string): Block[] {
  const lines = src.replace(/\r\n?/g, "\n").split("\n");
  const blocks: Block[] = [];
  let para: string[] = [];
  const endPara = () => {
    if (para.length) blocks.push({ k: "p", kids: inline(para.join("\n")) });
    para = [];
  };
  for (let i = 0; i < lines.length; i++) {
    const line = lines[i]!;
    const fence = /^\s*(```|~~~)/.exec(line);
    if (fence) {
      endPara();
      const body: string[] = [];
      i += 1;
      while (i < lines.length && !lines[i]!.trimStart().startsWith(fence[1]!)) body.push(lines[i++]!);
      blocks.push({ k: "code", text: body.join("\n") });
      continue;
    }
    if (!line.trim()) {
      endPara();
      continue;
    }
    const h = /^(#{1,6})\s+(.*?)\s*#*\s*$/.exec(line);
    if (h) {
      endPara();
      blocks.push({ k: "h", level: h[1]!.length, kids: inline(h[2]!) });
      continue;
    }
    if (/^\s*([-*_])(\s*\1){2,}\s*$/.test(line)) {
      endPara();
      blocks.push({ k: "hr" });
      continue;
    }
    if (/^\s*>/.test(line)) {
      endPara();
      const body: string[] = [];
      while (i < lines.length && /^\s*>/.test(lines[i]!)) body.push(lines[i++]!.replace(/^\s*>\s?/, ""));
      i -= 1;
      const c = /^\[!(\w+)\][+-]?\s*(.*)$/.exec(body[0] ?? "");
      if (c) {
        blocks.push({
          k: "quote",
          callout: c[1]!.toLowerCase(),
          title: inline(c[2] || c[1]!.charAt(0).toUpperCase() + c[1]!.slice(1).toLowerCase()),
          blocks: parse(body.slice(1).join("\n")),
        });
      } else blocks.push({ k: "quote", callout: null, title: null, blocks: parse(body.join("\n")) });
      continue;
    }
    const li = LIST.exec(line);
    if (li) {
      endPara();
      const ordered = /\d/.test(li[2]!);
      const items: Item[] = [];
      const start = ordered ? parseInt(li[2]!, 10) : 1;
      while (i < lines.length) {
        const m = LIST.exec(lines[i]!);
        if (!m || /\d/.test(m[2]!) !== ordered) break;
        items.push({
          kids: inline(m[4]!),
          task: m[3] === undefined ? null : m[3] !== " ",
          depth: Math.min(3, Math.floor(m[1]!.replace(/\t/g, "  ").length / 2)),
        });
        i += 1;
      }
      i -= 1;
      blocks.push({ k: ordered ? "ol" : "ul", items, start });
      continue;
    }
    para.push(line);
  }
  endPara();
  return blocks;
}

/** The words of an inline run, marks dropped. */
export function plain(kids: Inline[]): string {
  return kids
    .map((n) => (n.k === "t" || n.k === "code" ? n.text : n.k === "wiki" ? n.label : plain(n.kids)))
    .join("");
}

/** A card's first line of words: what a screen reader and a small card say. */
export function firstLine(src: string): string {
  for (const b of parse(src)) {
    if (b.k === "h" || b.k === "p") return plain(b.kids).split("\n")[0] ?? "";
    if (b.k === "ul" || b.k === "ol") return plain(b.items[0]?.kids ?? []);
  }
  return "";
}
