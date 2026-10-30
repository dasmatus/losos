/* What the bar above the page and the keys do to the document: marks,
 * paragraph styles, lists, and putting in each component. Every command
 * is a plain ProseMirror command, so a button, a key and a test call the
 * same thing. */

import { chainCommands, exitCode, lift, setBlockType, toggleMark, wrapIn } from "prosemirror-commands";
import { InputRule, inputRules, textblockTypeInputRule, wrappingInputRule } from "prosemirror-inputrules";
import type { Mark, MarkType, Node as PMNode, NodeType } from "prosemirror-model";
import { liftListItem, sinkListItem, splitListItem, wrapInList } from "prosemirror-schema-list";
import { NodeSelection, TextSelection, type Command, type EditorState } from "prosemirror-state";
import { schema } from "@/mdx/schema";

const N = schema.nodes;
const M = schema.marks;
const node = (name: string): NodeType => N[name]!;
const mark = (name: string): MarkType => M[name]!;

export type MarkName = "strong" | "em" | "underline" | "strike" | "sub" | "sup" | "highlight" | "code" | "kbd" | "small";

export function markActive(state: EditorState, name: MarkName | "link" | "comment"): boolean {
  const type = mark(name);
  const { from, $from, to, empty } = state.selection;
  if (empty) return type.isInSet(state.storedMarks ?? $from.marks()) !== undefined;
  return state.doc.rangeHasMark(from, to, type);
}

export const toggle = (name: MarkName): Command => toggleMark(mark(name));

/** 0 for body text, else the heading level the cursor's paragraph has. */
export function styleAt(state: EditorState): number {
  const parent = state.selection.$from.parent;
  return parent.type === node("heading") ? Number(parent.attrs["level"]) : 0;
}

export function setStyle(level: number): Command {
  return level === 0 ? setBlockType(node("paragraph")) : setBlockType(node("heading"), { level });
}

function ancestor(state: EditorState, test: (n: PMNode) => boolean): { node: PMNode; pos: number; depth: number } | null {
  const $from = state.selection.$from;
  for (let d = $from.depth; d > 0; d--) {
    const n = $from.node(d);
    if (test(n)) return { node: n, pos: $from.before(d), depth: d };
  }
  return null;
}

export type ListKind = "bullets" | "numbers" | "tasks";

export function listAt(state: EditorState): ListKind | null {
  const item = ancestor(state, (n) => n.type === node("list_item"));
  if (item === null) return null;
  const list = state.selection.$from.node(item.depth - 1);
  if (item.node.attrs["checked"] !== null) return "tasks";
  return list.type === node("ordered_list") ? "numbers" : "bullets";
}

/** Each item of the list at `listPos` in `tr`'s document made a task, or
 * not one. */
function markTasks(tr: import("prosemirror-state").Transaction, listPos: number, on: boolean) {
  const list = tr.doc.nodeAt(listPos);
  list?.forEach((child, offset) => {
    tr.setNodeMarkup(listPos + 1 + offset, undefined, { ...child.attrs, checked: on ? (child.attrs["checked"] ?? false) : null });
  });
}

export function toggleList(kind: ListKind): Command {
  return (state, dispatch, view) => {
    const now = listAt(state);
    if (now === kind) return liftListItem(node("list_item"))(state, dispatch, view);
    const type = kind === "numbers" ? node("ordered_list") : node("bullet_list");
    if (now !== null) {
      // Another kind of list: change it in place.
      if (dispatch) {
        const item = ancestor(state, (n) => n.type === node("list_item"))!;
        const listPos = state.selection.$from.before(item.depth - 1);
        const list = state.doc.nodeAt(listPos)!;
        const tr = state.tr.setNodeMarkup(listPos, type, kind === "numbers" ? { tight: list.attrs["tight"], order: 1 } : { tight: list.attrs["tight"] });
        markTasks(tr, listPos, kind === "tasks");
        dispatch(tr);
      }
      return true;
    }
    if (!dispatch) return wrapInList(type)(state);
    let done = false;
    wrapInList(type)(state, (tr) => {
      if (kind === "tasks") {
        const after = state.apply(tr);
        const item = ancestor(after, (n) => n.type === node("list_item"));
        if (item !== null) markTasks(tr, after.selection.$from.before(item.depth - 1), true);
      }
      dispatch(tr);
      done = true;
    });
    return done;
  };
}

export const indent: Command = sinkListItem(node("list_item"));
export const outdent: Command = liftListItem(node("list_item"));

export function quoteActive(state: EditorState): boolean {
  return ancestor(state, (n) => n.type === node("blockquote")) !== null;
}

export const toggleQuote: Command = (state, dispatch, view) =>
  quoteActive(state) ? lift(state, dispatch, view) : wrapIn(node("blockquote"))(state, dispatch, view);

export const codeBlock: Command = (state, dispatch, view) =>
  state.selection.$from.parent.type === node("code_block")
    ? setBlockType(node("paragraph"))(state, dispatch, view)
    : setBlockType(node("code_block"))(state, dispatch, view);

/** Puts `made` where the selection is and selects it, so the component's
 * fields show under the bar. */
export function insertNode(made: PMNode): Command {
  return (state, dispatch) => {
    const tr = state.tr.replaceSelectionWith(made);
    if (dispatch) {
      // The node just put in ends where the mapped selection now is.
      const end = tr.selection.from;
      const start = end - made.nodeSize;
      if (made.isAtom && start >= 0 && tr.doc.nodeAt(start) === made) tr.setSelection(NodeSelection.create(tr.doc, start));
      dispatch(tr.scrollIntoView());
    }
    return true;
  };
}

/** A container with the selected blocks inside it, or an empty one. */
export function insertContainer(name: "callout" | "details" | "notes" | "slide", attrs: Record<string, unknown> = {}): Command {
  return (state, dispatch, view) => {
    const type = node(name);
    if (!state.selection.empty && wrapIn(type, attrs)(state)) return wrapIn(type, attrs)(state, dispatch, view);
    return insertBlock(type.create(attrs, node("paragraph").create()), true)(state, dispatch, view);
  };
}

/** A block after the one the cursor is in, with the cursor inside it
 * when it has room for text. */
export function insertBlock(made: PMNode, cursorInside = false): Command {
  return (state, dispatch) => {
    const $from = state.selection.$from;
    // After the top-level block, or after the innermost block that may
    // hold it.
    let depth = $from.depth;
    while (depth > 0 && !$from.node(depth - 1).type.contentMatch.matchType(made.type)) depth--;
    const at = depth === 0 ? $from.pos : $from.after(depth);
    if (dispatch) {
      const tr = state.tr.insert(at, made);
      if (cursorInside && made.isTextblock) tr.setSelection(TextSelection.create(tr.doc, at + 1));
      else if (cursorInside && !made.isLeaf) tr.setSelection(TextSelection.near(tr.doc.resolve(at + 1)));
      else if (made.isAtom) tr.setSelection(NodeSelection.create(tr.doc, at));
      dispatch(tr.scrollIntoView());
    }
    return true;
  };
}

export function insertTable(rows = 3, cols = 3): Command {
  const cell = (header: boolean) => node(header ? "table_header" : "table_cell").create(null, node("paragraph").create());
  const made = node("table").create(
    null,
    Array.from({ length: rows }, (_, r) => node("table_row").create(null, Array.from({ length: cols }, () => cell(r === 0)))),
  );
  return insertBlock(made, true);
}

export function insertColumns(): Command {
  const col = () => node("column").create(null, node("paragraph").create());
  return insertBlock(node("columns").create(null, [col(), col()]), true);
}

export function insertTabs(first: string, second: string): Command {
  const tab = (label: string) => node("tab").create({ label }, node("paragraph").create());
  return insertBlock(node("tabs").create(null, [tab(first), tab(second)]), true);
}

/** The header goes first on the page, after the properties; the footer
 * last. One of each. */
export function insertPageEdge(which: "page_header" | "page_footer", content: PMNode[] = []): Command {
  return (state, dispatch) => {
    let found = -1;
    state.doc.forEach((child, offset) => {
      if (child.type === node(which)) found = offset;
    });
    if (dispatch) {
      const tr = state.tr;
      if (found >= 0) {
        tr.setSelection(TextSelection.create(tr.doc, found + 1));
      } else {
        const first = state.doc.firstChild;
        const at = which === "page_footer" ? state.doc.content.size : first?.type === node("front_matter") ? first.nodeSize : 0;
        tr.insert(at, node(which).create(null, content));
        tr.setSelection(TextSelection.create(tr.doc, at + 1));
      }
      dispatch(tr.scrollIntoView());
    }
    return true;
  };
}

/** A footnote: its mark where the cursor is, its text at the end, and the
 * cursor in that text. */
export const insertFootnote: Command = (state, dispatch) => {
  const used = new Set<string>();
  state.doc.descendants((n) => {
    if (n.type === node("footnote_ref") || n.type === node("footnote_definition")) used.add(String(n.attrs["label"]));
  });
  let k = 1;
  while (used.has(String(k))) k++;
  const label = String(k);
  if (dispatch) {
    const tr = state.tr.replaceSelectionWith(node("footnote_ref").create({ label }), false);
    const end = tr.doc.content.size;
    tr.insert(end, node("footnote_definition").create({ label }, node("paragraph").create()));
    tr.setSelection(TextSelection.create(tr.doc, end + 2));
    dispatch(tr.scrollIntoView());
  }
  return true;
};

/** The slide the cursor is in, or the last one before it. */
export function insertSlide(layout = "content"): Command {
  return (state, dispatch) => {
    const slide = ancestor(state, (n) => n.type === node("slide"));
    const made = node("slide").create({ layout }, node("heading").create({ level: 2 }));
    const at = slide !== null ? slide.pos + slide.node.nodeSize : state.doc.content.size;
    if (dispatch) {
      const tr = state.tr.insert(at, made);
      tr.setSelection(TextSelection.create(tr.doc, at + 2));
      dispatch(tr.scrollIntoView());
    }
    return true;
  };
}

/** A mark over the selection, or over `text` put in where the cursor is. */
export function applyMark(made: Mark, text: string): Command {
  return (state, dispatch) => {
    const { from, to, empty } = state.selection;
    if (dispatch) {
      const tr = state.tr;
      if (empty) {
        tr.insertText(text, from);
        tr.addMark(from, from + text.length, made);
        tr.setSelection(TextSelection.create(tr.doc, from, from + text.length));
      } else {
        tr.addMark(from, to, made);
      }
      dispatch(tr);
    }
    return true;
  };
}

/** The range a mark of `type` covers around the cursor. */
export function markRange(state: EditorState, type: MarkType): { from: number; to: number; mark: Mark } | null {
  const { $from, from, to } = state.selection;
  const found = type.isInSet($from.marks()) ?? (from < to ? type.isInSet(state.doc.resolve(from + 1).marks()) : undefined);
  if (found === undefined) return null;
  const parent = $from.parent;
  const start = $from.start();
  let a = -1;
  let b = -1;
  parent.forEach((child, offset) => {
    const pos = start + offset;
    const has = found.isInSet(child.marks);
    if (has && a < 0 && pos + child.nodeSize >= from) a = pos;
    if (has && a >= 0 && (b < 0 || b === pos)) b = pos + child.nodeSize;
  });
  return a < 0 ? null : { from: a, to: Math.max(b, to), mark: found };
}

export function removeMark(type: MarkType): Command {
  return (state, dispatch) => {
    const range = markRange(state, type);
    if (range === null) return false;
    dispatch?.(state.tr.removeMark(range.from, range.to, type));
    return true;
  };
}

export const hardBreak: Command = chainCommands(exitCode, (state, dispatch) => {
  dispatch?.(state.tr.replaceSelectionWith(node("hard_break").create()).scrollIntoView());
  return true;
});

export const splitItem: Command = splitListItem(node("list_item"));

/** Markdown typed as Markdown: `# `, `- `, `1. `, `[ ] `, `> `, three
 * backticks, `---`, and `**`, `*`, `` ` `` around words. */
export function markdownRules() {
  const markRule = (re: RegExp, type: MarkType) =>
    new InputRule(re, (state, match, start, end) => {
      const text = match[1] ?? "";
      const tr = state.tr.replaceWith(start, end, schema.text(text, [type.create()]));
      return tr.removeStoredMark(type);
    });
  return inputRules({
    rules: [
      textblockTypeInputRule(/^(#{1,6})\s$/, node("heading"), (m) => ({ level: m[1]!.length })),
      wrappingInputRule(/^\s*([-+*])\s$/, node("bullet_list")),
      wrappingInputRule(/^(\d+)\.\s$/, node("ordered_list"), (m) => ({ order: Number(m[1]) }), (m, n) => n.childCount + Number(n.attrs["order"]) === Number(m[1])),
      wrappingInputRule(/^\s*>\s$/, node("blockquote")),
      textblockTypeInputRule(/^```([\w-]*)\s$/, node("code_block"), (m) => ({ language: m[1] ?? "" })),
      new InputRule(/^\[( |x)\]\s$/, (state, match, start, end) => {
        const item = ancestor(state, (n) => n.type === node("list_item"));
        if (item === null) return null;
        return state.tr.delete(start, end).setNodeMarkup(item.pos, undefined, { ...item.node.attrs, checked: match[1] === "x" });
      }),
      new InputRule(/^---$/, (state, _m, start) => {
        const $start = state.doc.resolve(start);
        if ($start.parent.type !== node("paragraph")) return null;
        return state.tr.replaceRangeWith($start.before(), $start.after(), node("horizontal_rule").create());
      }),
      markRule(/\*\*([^*]+)\*\*$/, mark("strong")),
      markRule(/(?<![*\w])\*([^*\s][^*]*)\*$/, mark("em")),
      markRule(/`([^`]+)`$/, mark("code")),
      markRule(/~~([^~]+)~~$/, mark("strike")),
    ],
  });
}

/** Today as YYYY-MM-DD, in the reader's own day. */
export function today(): string {
  const d = new Date();
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
}

export { N as nodes, M as marks };
