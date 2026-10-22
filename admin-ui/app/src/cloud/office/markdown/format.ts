import { syntaxTree } from "@codemirror/language";
import { EditorSelection, Prec, type ChangeSpec, type EditorState, type Extension, type SelectionRange } from "@codemirror/state";
import { keymap, type EditorView, type KeyBinding } from "@codemirror/view";
import { yUndoManagerKeymap } from "y-codemirror.next";

/* What the Markdown bar does to the text: one function per button, each a
 * single change to the CodeMirror view, so one undo takes it back. Every
 * mark is the one Obsidian and GitHub both read, and the renderer in
 * src/notes/markdown.ts draws: ** for bold, == for a highlight, %% for a
 * comment, [[ ]] for a link to a note, > [!note] for a callout and so on.
 *
 * A mark already around the selection or the cursor is taken off again,
 * the way a word processor's Bold button turns bold off; a line format
 * already on every selected line comes off them all. What a button would
 * put in an empty selection ("text", "Note") is passed in, in the reader's
 * language, and left selected to type over. */

export type Action =
  | "bold"
  | "italic"
  | "strike"
  | "highlight"
  | "code"
  | "mathInline"
  | "sub"
  | "sup"
  | "kbd"
  | "comment"
  | "bullets"
  | "numbers"
  | "tasks"
  | "quote"
  | "codeBlock"
  | "mathBlock"
  | "details"
  | "rule"
  | "link"
  | "image"
  | "wikilink"
  | "embed"
  | "tag"
  | "blockId"
  | "footnote"
  | "table"
  | "properties"
  | "slide";

/** The words a button leaves selected in an empty selection. */
export interface Words {
  text: string;
  code: string;
  formula: string;
  key: string;
  url: string;
  picture: string;
  note: string;
  tag: string;
  title: string;
  summary: string;
  column: string;
  footnote: string;
  callout: string;
}

/** The callouts on the bar's list: GitHub's five alerts, then the rest of
 * Obsidian's own. Both read `> [!kind]`. */
export const CALLOUTS = [
  "note",
  "tip",
  "important",
  "warning",
  "caution",
  "abstract",
  "info",
  "todo",
  "success",
  "question",
  "failure",
  "danger",
  "bug",
  "example",
  "quote",
] as const;
export type Callout = (typeof CALLOUTS)[number];

const INLINE: Partial<Record<Action, readonly [string, string, keyof Words]>> = {
  bold: ["**", "**", "text"],
  italic: ["*", "*", "text"],
  strike: ["~~", "~~", "text"],
  highlight: ["==", "==", "text"],
  code: ["`", "`", "code"],
  mathInline: ["$", "$", "formula"],
  sub: ["<sub>", "</sub>", "text"],
  sup: ["<sup>", "</sup>", "text"],
  kbd: ["<kbd>", "</kbd>", "key"],
  comment: ["%%", "%%", "text"],
};

const LIST_ANY = /^(\s*)(?:[-*+]\s\[.\]\s|[-*+]\s|\d+[.)]\s)/;
const LINES: Partial<Record<Action, { test: RegExp; make: (i: number) => string; list: boolean }>> = {
  bullets: { test: /^(\s*)[-*+]\s(?!\[.\]\s)/, make: () => "- ", list: true },
  numbers: { test: /^(\s*)\d+[.)]\s/, make: (i) => `${i + 1}. `, list: true },
  tasks: { test: /^(\s*)[-*+]\s\[.\]\s/, make: () => "- [ ] ", list: true },
  quote: { test: /^(\s*)>\s?/, make: () => "> ", list: false },
};

const HEADING = /^(#{1,6})\s+/;

function dispatch(view: EditorView, spec: Parameters<EditorView["dispatch"]>[0]): void {
  view.dispatch(spec);
  view.focus();
}

/** Whether the text right before `from` and after `to` is the mark, and
 * not a longer run of the same character (`*` inside `**`). */
function marked(state: EditorState, from: number, to: number, open: string, close: string): boolean {
  if (state.sliceDoc(from - open.length, from) !== open || state.sliceDoc(to, to + close.length) !== close) return false;
  if (open.length === 1 && state.sliceDoc(from - 2, from - 1) === open) return false;
  if (close.length === 1 && state.sliceDoc(to + 1, to + 2) === close) return false;
  return true;
}

/** Where on its line the cursor's span between `open` and `close` is, or
 * null: `==this|==` with the cursor at the bar. */
function spanAround(state: EditorState, pos: number, open: string, close: string): { from: number; to: number } | null {
  const line = state.doc.lineAt(pos);
  const text = line.text;
  const at = pos - line.from;
  const start = text.lastIndexOf(open, at - open.length);
  if (start < 0) return null;
  // The mark before the cursor must open a span, not close one.
  if (open === close) {
    let count = 0;
    for (let i = text.indexOf(open); i >= 0 && i < start; i = text.indexOf(open, i + open.length)) count++;
    if (count % 2 === 1) return null;
  }
  const end = text.indexOf(close, Math.max(at, start + open.length));
  if (end < 0) return null;
  if (open.length === 1 && (text[start - 1] === open || text[start + 1] === open)) return null;
  return { from: line.from + start + open.length, to: line.from + end };
}

function toggleInline(view: EditorView, open: string, close: string, placeholder: string): void {
  const { state } = view;
  dispatch(
    view,
    state.update(
      state.changeByRange((range) => {
        // The selection sits inside the marks: take them off.
        if (marked(state, range.from, range.to, open, close)) {
          return {
            changes: [
              { from: range.from - open.length, to: range.from },
              { from: range.to, to: range.to + close.length },
            ],
            range: EditorSelection.range(range.from - open.length, range.to - open.length),
          };
        }
        const text = state.sliceDoc(range.from, range.to);
        // The selection takes the marks in: take them off.
        if (text.length >= open.length + close.length && text.startsWith(open) && text.endsWith(close)) {
          const inner = text.slice(open.length, text.length - close.length);
          return {
            changes: { from: range.from, to: range.to, insert: inner },
            range: EditorSelection.range(range.from, range.from + inner.length),
          };
        }
        // A bare cursor inside a marked span: unmark the span.
        if (range.empty) {
          const span = spanAround(state, range.head, open, close);
          if (span !== null) {
            return {
              changes: [
                { from: span.from - open.length, to: span.from },
                { from: span.to, to: span.to + close.length },
              ],
              range: EditorSelection.cursor(range.head - open.length),
            };
          }
        }
        const inner = range.empty ? placeholder : text;
        return {
          changes: { from: range.from, to: range.to, insert: open + inner + close },
          range: EditorSelection.range(range.from + open.length, range.from + open.length + inner.length),
        };
      }),
      { scrollIntoView: true, userEvent: "input.format" },
    ),
  );
}

/** The numbers of the lines the selection touches, each once. */
function selectedLines(state: EditorState): number[] {
  const seen = new Set<number>();
  for (const r of state.selection.ranges) {
    const first = state.doc.lineAt(r.from).number;
    // A selection that ends at the start of a line leaves that line out.
    let last = state.doc.lineAt(r.to).number;
    if (last > first && state.doc.line(last).from === r.to) last--;
    for (let n = first; n <= last; n++) seen.add(n);
  }
  return [...seen].sort((a, b) => a - b);
}

function toggleLines(view: EditorView, kind: { test: RegExp; make: (i: number) => string; list: boolean }): void {
  const { state } = view;
  const lines = selectedLines(state).map((n) => state.doc.line(n));
  const filled = lines.filter((l) => l.text.trim() !== "");
  const on = filled.length > 0 && filled.every((l) => kind.test.test(l.text));
  const changes: ChangeSpec[] = [];
  let i = 0;
  for (const line of lines) {
    if (line.text.trim() === "" && lines.length > 1) continue;
    const m = kind.test.exec(line.text);
    if (on && m !== null) {
      const indent = m[1]?.length ?? 0;
      changes.push({ from: line.from + indent, to: line.from + m[0].length });
      continue;
    }
    const other = kind.list ? LIST_ANY.exec(line.text) : null;
    if (other !== null) {
      const indent = other[1]?.length ?? 0;
      changes.push({ from: line.from + indent, to: line.from + other[0].length, insert: kind.make(i) });
    } else {
      const indent = /^\s*/.exec(line.text)?.[0].length ?? 0;
      changes.push({ from: line.from + indent, insert: kind.make(i) });
    }
    i++;
  }
  dispatch(view, { changes, scrollIntoView: true, userEvent: "input.format" });
}

/** A heading of `level` on the selected lines, 0 for plain text; the same
 * level again makes them plain. */
export function setHeading(view: EditorView, level: number): void {
  const { state } = view;
  const lines = selectedLines(state).map((n) => state.doc.line(n));
  const same = level > 0 && lines.every((l) => HEADING.exec(l.text)?.[1]?.length === level);
  const changes: ChangeSpec[] = lines.map((line) => {
    const m = HEADING.exec(line.text);
    const insert = same || level === 0 ? "" : `${"#".repeat(level)} `;
    return { from: line.from, to: line.from + (m?.[0].length ?? 0), insert };
  });
  dispatch(view, { changes, scrollIntoView: true, userEvent: "input.format" });
}

/** The heading level of the cursor's line, 0 for plain text. */
export function headingAt(state: EditorState): number {
  return HEADING.exec(state.doc.lineAt(state.selection.main.head).text)?.[1]?.length ?? 0;
}

/** `text` on lines of its own at the cursor: a blank line before it and
 * after it when the neighbours are not blank. `select` is the part of
 * `text` to leave selected, as [from, to] in it. */
function insertBlock(view: EditorView, text: string, select: readonly [number, number], range?: SelectionRange): void {
  const { state } = view;
  const main = range ?? state.selection.main;
  const startLine = state.doc.lineAt(main.from);
  const endLine = state.doc.lineAt(main.to);
  const replacing = !main.empty;
  const from = replacing ? startLine.from : main.from === startLine.from && startLine.text.trim() === "" ? startLine.from : startLine.to;
  const to = replacing ? endLine.to : from === startLine.from && startLine.text.trim() === "" ? startLine.to : from;
  const before = state.sliceDoc(0, from);
  const after = state.sliceDoc(to);
  const lead = before === "" || before.endsWith("\n\n") ? "" : before.endsWith("\n") ? "\n" : "\n\n";
  const tail = after === "" || after.startsWith("\n\n") ? "" : after.startsWith("\n") ? "\n" : "\n\n";
  const at = from + lead.length;
  dispatch(view, {
    changes: { from, to, insert: lead + text + tail },
    selection: EditorSelection.range(at + select[0], at + select[1]),
    scrollIntoView: true,
    userEvent: "input.format",
  });
}

/** `open` and `close` on lines of their own around the selected lines, or
 * around `placeholder`. */
function fence(view: EditorView, open: string, close: string, placeholder: string): void {
  const { state } = view;
  const main = state.selection.main;
  const inner = main.empty ? placeholder : state.sliceDoc(state.doc.lineAt(main.from).from, state.doc.lineAt(main.to).to);
  const text = `${open}\n${inner}\n${close}`;
  insertBlock(view, text, [open.length + 1, open.length + 1 + inner.length]);
}

/** A callout of `kind` around the selected lines, or with a title and a
 * line to fill in. */
export function insertCallout(view: EditorView, kind: Callout, words: Words): void {
  const { state } = view;
  const main = state.selection.main;
  if (main.empty) {
    const head = `> [!${kind}] `;
    insertBlock(view, `${head}${words.title}\n> ${words.callout}`, [head.length, head.length + words.title.length]);
    return;
  }
  const from = state.doc.lineAt(main.from).from;
  const lines = state.sliceDoc(from, state.doc.lineAt(main.to).to).split("\n");
  const text = [`> [!${kind}]`, ...lines.map((l) => (l === "" ? ">" : `> ${l}`))].join("\n");
  insertBlock(view, text, [text.length, text.length]);
}

function frontMatterEnd(state: EditorState): number | null {
  if (!/^---[ \t]*\r?\n/.test(state.sliceDoc(0, 5))) return null;
  for (let n = 2; n <= state.doc.lines; n++) {
    if (/^(---|\.\.\.)[ \t]*$/.test(state.doc.line(n).text)) return state.doc.line(n).from;
  }
  return null;
}

function insertProperties(view: EditorView, words: Words): void {
  const { state } = view;
  const end = frontMatterEnd(state);
  if (end !== null) {
    // Properties already: a new line at their end.
    dispatch(view, {
      changes: { from: end, insert: `${words.tag}: \n` },
      selection: EditorSelection.range(end, end + words.tag.length),
      scrollIntoView: true,
      userEvent: "input.format",
    });
    return;
  }
  const head = "---\ntags: ";
  const text = `${head}\n---\n\n`;
  dispatch(view, {
    changes: { from: 0, insert: text },
    selection: EditorSelection.cursor(head.length),
    scrollIntoView: true,
    userEvent: "input.format",
  });
}

function insertFootnote(view: EditorView, words: Words): void {
  const { state } = view;
  const text = state.doc.toString();
  let n = 0;
  for (const m of text.matchAll(/\[\^(\d+)\]/g)) n = Math.max(n, Number(m[1]));
  const label = `[^${n + 1}]`;
  const at = state.selection.main.head;
  const end = state.doc.length;
  const gap = text.endsWith("\n\n") ? "" : text.endsWith("\n") ? "\n" : "\n\n";
  const note = `${gap}${label}: ${words.footnote}`;
  dispatch(view, {
    changes: [
      { from: at, insert: label },
      { from: end, insert: note },
    ],
    selection: EditorSelection.range(end + label.length + note.length - words.footnote.length, end + label.length + note.length),
    scrollIntoView: true,
    userEvent: "input.format",
  });
}

function insertBlockId(view: EditorView): void {
  const { state } = view;
  const line = state.doc.lineAt(state.selection.main.head);
  const id = Array.from(crypto.getRandomValues(new Uint8Array(3)), (b) => b.toString(16).padStart(2, "0")).join("");
  const existing = /\s\^([A-Za-z0-9-]+)\s*$/.exec(line.text);
  if (existing !== null) {
    dispatch(view, { selection: EditorSelection.cursor(line.to), scrollIntoView: true });
    return;
  }
  const insert = `${line.text.trim() === "" ? "" : " "}^${id}`;
  dispatch(view, {
    changes: { from: line.to, insert },
    selection: EditorSelection.range(line.to + insert.length - id.length, line.to + insert.length),
    scrollIntoView: true,
    userEvent: "input.format",
  });
}

/** Text in front of and behind the selection: `[`…`](url)`. */
function surround(view: EditorView, open: string, middle: string, placeholder: string, selectTail: string | null): void {
  const { state } = view;
  dispatch(
    view,
    state.update(
      state.changeByRange((range) => {
        const text = range.empty ? placeholder : state.sliceDoc(range.from, range.to);
        const insert = `${open}${text}${middle}`;
        if (selectTail !== null && !range.empty) {
          // The text is the person's; the address is what is left to type.
          const tail = range.from + open.length + text.length + middle.indexOf(selectTail);
          return { changes: { from: range.from, to: range.to, insert }, range: EditorSelection.range(tail, tail + selectTail.length) };
        }
        return {
          changes: { from: range.from, to: range.to, insert },
          range: EditorSelection.range(range.from + open.length, range.from + open.length + text.length),
        };
      }),
      { scrollIntoView: true, userEvent: "input.format" },
    ),
  );
}

/** Runs one of the bar's actions on the view. `deck` says the text is a
 * presentation, where `---` starts a new slide and a rule is `***`. */
export function run(view: EditorView, action: Action, words: Words, deck = false): void {
  const inline = INLINE[action];
  if (inline !== undefined) {
    toggleInline(view, inline[0], inline[1], words[inline[2]]);
    return;
  }
  const lines = LINES[action];
  if (lines !== undefined) {
    toggleLines(view, lines);
    return;
  }
  switch (action) {
    case "codeBlock":
      fence(view, "```", "```", words.code);
      return;
    case "mathBlock":
      fence(view, "$$", "$$", words.formula);
      return;
    case "details": {
      const head = `<details>\n<summary>${words.summary}</summary>\n\n`;
      const { state } = view;
      const main = state.selection.main;
      const inner = main.empty ? words.text : state.sliceDoc(state.doc.lineAt(main.from).from, state.doc.lineAt(main.to).to);
      insertBlock(view, `${head}${inner}\n\n</details>`, [19, 19 + words.summary.length]);
      return;
    }
    case "rule":
      insertBlock(view, deck ? "***" : "---", [3, 3]);
      return;
    case "slide": {
      const { state } = view;
      const head = "---\n\n## ";
      const line = state.doc.lineAt(state.selection.main.head);
      insertBlock(view, `${head}${words.title}`, [head.length, head.length + words.title.length], EditorSelection.cursor(line.to));
      return;
    }
    case "table": {
      const c = words.column;
      const text = `| ${c} 1 | ${c} 2 |\n| --- | --- |\n|  |  |`;
      insertBlock(view, text, [2, 2 + c.length + 2]);
      return;
    }
    case "link":
      surround(view, "[", `](${words.url})`, words.text, words.url);
      return;
    case "image":
      surround(view, "![", `](${words.picture})`, words.text, words.picture);
      return;
    case "wikilink":
      surround(view, "[[", "]]", words.note, null);
      return;
    case "embed":
      surround(view, "![[", "]]", words.note, null);
      return;
    case "tag":
      surround(view, "#", "", words.tag, null);
      return;
    case "blockId":
      insertBlockId(view);
      return;
    case "footnote":
      insertFootnote(view, words);
      return;
    case "properties":
      insertProperties(view, words);
      return;
    default:
      return;
  }
}

/** What is on at the cursor, for the bar's pressed buttons. */
export function activeAt(state: EditorState): Set<Action> {
  const on = new Set<Action>();
  const head = state.selection.main.head;
  for (let node: ReturnType<ReturnType<typeof syntaxTree>["resolveInner"]> | null = syntaxTree(state).resolveInner(head, -1); node !== null; node = node.parent) {
    switch (node.name) {
      case "StrongEmphasis":
        on.add("bold");
        break;
      case "Emphasis":
        on.add("italic");
        break;
      case "InlineCode":
        on.add("code");
        break;
      case "FencedCode":
        on.add("codeBlock");
        break;
    }
  }
  const line = state.doc.lineAt(head).text;
  for (const [action, kind] of Object.entries(LINES) as [Action, { test: RegExp }][]) {
    if (kind.test.test(line)) on.add(action);
  }
  for (const [action, [open, close]] of Object.entries(INLINE) as [Action, readonly [string, string, keyof Words]][]) {
    if (on.has(action)) continue;
    if (action === "bold" || action === "italic" || action === "code") continue;
    const range = state.selection.main;
    if (marked(state, range.from, range.to, open, close) || (range.empty && spanAround(state, head, open, close) !== null)) on.add(action);
  }
  return on;
}

function binding(key: string): KeyBinding["run"] {
  return yUndoManagerKeymap.find((k) => k.key === key)?.run ?? (() => false);
}

/** Undo and redo through the shared text's own history (Yjs), the same as
 * Mod-z and Mod-y. */
export const undo = (view: EditorView): boolean => (binding("Mod-z") as (v: EditorView) => boolean)(view);
export const redo = (view: EditorView): boolean => (binding("Mod-y") as (v: EditorView) => boolean)(view);

/** The usual shortcuts for the commonest buttons. */
export function formatKeys(words: () => Words): Extension {
  const k = (action: Action) => (view: EditorView) => {
    run(view, action, words());
    return true;
  };
  // Above the editor's own keymap, which takes Mod-i for something else.
  return Prec.high(keymap.of([
    { key: "Mod-b", run: k("bold") },
    { key: "Mod-i", run: k("italic") },
    { key: "Mod-k", run: k("link") },
    { key: "Mod-e", run: k("code") },
    { key: "Mod-Shift-x", run: k("strike") },
    { key: "Mod-Shift-h", run: k("highlight") },
  ]));
}
