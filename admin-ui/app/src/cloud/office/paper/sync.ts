/* The page and the file kept in step: ProseMirror on one side, the shared
 * Yjs text of LosOS Cloud's editor on the other (../../collab/, the same
 * room and the same text a person in Notes or the text editor types in).
 *
 * The text is what counts. A change made on the page is written as MDX
 * (to-mdx.ts) and only the characters that differ go into the shared
 * text. A change from anyone else is read again (losos-mdx in the page)
 * and only the part of the page that differs is replaced, so the caret and
 * the blocks nobody touched stay put. Undo is the shared text's own, so it
 * takes back this person's typing and never someone else's.
 *
 * Text someone else left half-written, which does not read as MDX, stops
 * the page from writing: `broken` says where, and the page shows it, until
 * the text reads again. Writing the old page back over it would undo what
 * they typed. */

import { Plugin, PluginKey, type EditorState, type Transaction } from "prosemirror-state";
import { Decoration, DecorationSet, type EditorView } from "prosemirror-view";
import type { Node as PMNode } from "prosemirror-model";
import * as Y from "yjs";
import type { Awareness } from "y-protocols/awareness";
import { adoptSources, fromMdast } from "@/mdx/from-mdast";
import { toMdxParts } from "@/mdx/to-mdx";
import { parseMdx, type Problem } from "@/mdx/wasm";
import { ACCENTS, accentFor, nameOf, type Accent } from "../../collab/people";
import type { PresenceUser } from "../../collab/cursors";

export interface SyncState {
  /** Why the page is not writing: the text does not read. */
  broken: string | null;
  /** What losos-mdx refused in the text, with places. */
  problems: Problem[];
}

export const syncKey = new PluginKey<SyncState>("paper-sync");
const cursorKey = new PluginKey<DecorationSet>("paper-cursors");

/** Reads `text` as the editor's document; throws on text that does not read. */
export function readDoc(text: string): { doc: PMNode; problems: Problem[] } {
  const { tree, problems } = parseMdx(text, "mdx");
  return { doc: fromMdast(tree, text), problems };
}

/** The part of the page that differs from `next`, replaced by it. */
function replaceChanged(state: EditorState, next: PMNode): Transaction {
  const tr = state.tr;
  const start = state.doc.content.findDiffStart(next.content);
  if (start !== null) {
    const end = state.doc.content.findDiffEnd(next.content);
    if (end !== null) {
      let { a, b } = end;
      const overlap = start - Math.min(a, b);
      if (overlap > 0) {
        a += overlap;
        b += overlap;
      }
      tr.replace(start, a, next.slice(start, b));
    }
  }
  adoptSources(tr.doc, next);
  return tr;
}

function common(a: string, b: string): { from: number; toA: number; toB: number } {
  let from = 0;
  const max = Math.min(a.length, b.length);
  while (from < max && a.charCodeAt(from) === b.charCodeAt(from)) from++;
  let toA = a.length;
  let toB = b.length;
  while (toA > from && toB > from && a.charCodeAt(toA - 1) === b.charCodeAt(toB - 1)) {
    toA--;
    toB--;
  }
  return { from, toA, toB };
}

/** Where a page position falls in the text, and back. Each top-level
 * block's place in the text is known exactly; inside one, the block's
 * visible characters are matched in order against its text, which is
 * right for the caret in anything but a raw element. */
export class TextMap {
  constructor(
    readonly doc: PMNode,
    readonly text: string,
    readonly starts: number[],
  ) {}

  private blockAt(pos: number): { index: number; offset: number } {
    let offset = 0;
    for (let i = 0; i < this.doc.childCount; i++) {
      const size = this.doc.child(i).nodeSize;
      if (pos < offset + size || i === this.doc.childCount - 1) return { index: i, offset };
      offset += size;
    }
    return { index: 0, offset: 0 };
  }

  private blockText(index: number): string {
    const end = this.starts[index + 1] ?? this.text.length;
    return this.text.slice(this.starts[index] ?? 0, end);
  }

  toText(pos: number): number {
    if (this.doc.childCount === 0) return 0;
    const { index, offset } = this.blockAt(pos);
    const visible = this.doc.textBetween(offset, Math.min(pos, offset + this.doc.child(index).nodeSize), "\n", "￼");
    const text = this.blockText(index);
    let at = 0;
    for (const ch of visible) {
      const found = text.indexOf(ch, at);
      if (found < 0) break;
      at = found + 1;
    }
    return (this.starts[index] ?? 0) + at;
  }

  toPos(index: number): number {
    let i = 0;
    while (i + 1 < this.starts.length && (this.starts[i + 1] ?? 0) <= index) i++;
    let offset = 0;
    for (let k = 0; k < i; k++) offset += this.doc.child(k).nodeSize;
    const node = this.doc.child(i);
    const text = this.blockText(i).slice(0, index - (this.starts[i] ?? 0));
    // How many of the block's characters come before `index`.
    let want = 0;
    let at = 0;
    const visible = this.doc.textBetween(offset, offset + node.nodeSize, "\n", "￼");
    for (const ch of visible) {
      const found = text.indexOf(ch, at);
      if (found < 0) break;
      at = found + 1;
      want++;
    }
    // The page position after that many characters.
    let pos = offset + 1;
    let seen = 0;
    node.descendants((child, p) => {
      if (seen >= want) return false;
      if (child.isText) {
        const len = child.text?.length ?? 0;
        if (seen + len >= want) {
          pos = offset + 1 + p + (want - seen);
          seen = want;
          return false;
        }
        seen += len;
        pos = offset + 1 + p + len;
      } else if (child.isLeaf) {
        seen++;
        pos = offset + 1 + p + child.nodeSize;
      }
      return true;
    });
    return Math.min(pos, offset + node.nodeSize - 1);
  }
}

function accentOf(user: PresenceUser | undefined): Accent {
  const a = user?.accent;
  return a !== undefined && (ACCENTS as readonly string[]).includes(a) ? a : accentFor(nameOf(user?.address ?? ""));
}

interface Remote {
  user?: PresenceUser;
  cursor?: { anchor: unknown; head: unknown } | null;
}

function caret(accent: Accent, name: string): HTMLElement {
  const el = document.createElement("span");
  el.className = `paper-who-caret paper-who-${accent}`;
  el.setAttribute("aria-hidden", "true");
  const label = document.createElement("span");
  label.className = "paper-who-name";
  label.textContent = name;
  el.append("⁠", label, "⁠");
  return el;
}

/** The plugins that tie `view` to `ytext`: sync, undo, and the others'
 * cursors from `awareness`. */
/** Meta on a transaction from the bar or the fields (one press, one
 * change): it is its own undo step, however soon after the last one. */
export const STEP = "paperStep";

export function paperSync(ytext: Y.Text, awareness: Awareness | null) {
  const origin = { paper: true };
  const undo = new Y.UndoManager(ytext, { trackedOrigins: new Set([origin]) });
  let map: TextMap | null = null;
  let view: EditorView | null = null;
  let step = false;

  /** The page's text and where its blocks are, for `doc`. */
  const mapFor = (doc: PMNode): TextMap => {
    if (map === null || map.doc !== doc) {
      const { text, starts } = toMdxParts(doc);
      map = new TextMap(doc, text, starts);
    }
    return map;
  };

  const write = (doc: PMNode) => {
    const { text } = mapFor(doc);
    const now = ytext.toString();
    if (text === now) return;
    const { from, toA, toB } = common(now, text);
    ytext.doc?.transact(() => {
      if (toA > from) ytext.delete(from, toA - from);
      if (toB > from) ytext.insert(from, text.slice(from, toB));
    }, origin);
  };

  const publishCursor = (state: EditorState) => {
    if (awareness === null || view === null || !view.hasFocus()) return;
    const m = mapFor(state.doc);
    const anchor = Y.createRelativePositionFromTypeIndex(ytext, m.toText(state.selection.anchor));
    const head = Y.createRelativePositionFromTypeIndex(ytext, m.toText(state.selection.head));
    const was = (awareness.getLocalState() as Remote | null)?.cursor;
    if (
      was == null ||
      !Y.compareRelativePositions(Y.createRelativePositionFromJSON(was.anchor), anchor) ||
      !Y.compareRelativePositions(Y.createRelativePositionFromJSON(was.head), head)
    ) {
      awareness.setLocalStateField("cursor", { anchor, head });
    }
  };

  const drawCursors = (doc: PMNode): DecorationSet => {
    const ydoc = ytext.doc;
    if (awareness === null || ydoc === null) return DecorationSet.empty;
    const m = mapFor(doc);
    const out: Decoration[] = [];
    awareness.getStates().forEach((raw, client) => {
      if (client === awareness.clientID) return;
      const state = raw as Remote;
      const c = state.cursor;
      if (c == null || c.anchor == null || c.head == null) return;
      const a = Y.createAbsolutePositionFromRelativePosition(Y.createRelativePositionFromJSON(c.anchor), ydoc);
      const h = Y.createAbsolutePositionFromRelativePosition(Y.createRelativePositionFromJSON(c.head), ydoc);
      if (a === null || h === null || a.type !== ytext || h.type !== ytext) return;
      const accent = accentOf(state.user);
      const pa = m.toPos(a.index);
      const ph = m.toPos(h.index);
      if (pa !== ph) {
        out.push(Decoration.inline(Math.min(pa, ph), Math.max(pa, ph), { class: `paper-who-sel paper-who-${accent}` }));
      }
      out.push(Decoration.widget(ph, () => caret(accent, state.user?.name ?? "?"), { side: 1, key: `${client}-${accent}` }));
    });
    return DecorationSet.create(doc, out);
  };

  const sync = new Plugin<SyncState>({
    key: syncKey,
    state: {
      init: () => ({ broken: null, problems: [] }),
      apply: (tr, value) => (tr.getMeta(syncKey) as SyncState | undefined) ?? value,
    },
    // Nothing is typed into text that does not read.
    filterTransaction: (tr, state) => {
      if (tr.getMeta(STEP) === true) step = true;
      return !tr.docChanged || tr.getMeta(syncKey) !== undefined || syncKey.getState(state)?.broken === null;
    },
    view: (v) => {
      view = v;
      let frame = 0;
      // True while a change from the text is going onto the page, which
      // must not be written back.
      let pulling = false;
      const pull = () => {
        frame = 0;
        const text = ytext.toString();
        let read: ReturnType<typeof readDoc>;
        try {
          read = readDoc(text);
        } catch (e) {
          v.dispatch(v.state.tr.setMeta(syncKey, { broken: e instanceof Error ? e.message : String(e), problems: [] }));
          return;
        }
        const tr = replaceChanged(v.state, read.doc);
        tr.setMeta(syncKey, { broken: null, problems: read.problems });
        tr.setMeta(cursorKey, true);
        tr.setMeta("addToHistory", false);
        pulling = true;
        try {
          v.dispatch(tr);
        } finally {
          pulling = false;
        }
      };
      const observer = (_: Y.YTextEvent, tr: Y.Transaction) => {
        if (tr.origin === origin) return;
        if (frame === 0) frame = requestAnimationFrame(pull);
      };
      ytext.observe(observer);
      pull();
      const others = () => v.dispatch(v.state.tr.setMeta(cursorKey, true));
      awareness?.on("change", others);
      return {
        update: (_, prev) => {
          const state = v.state;
          if (state.doc !== prev.doc && !pulling && syncKey.getState(state)?.broken === null) {
            if (step) undo.stopCapturing();
            write(state.doc);
            if (step) undo.stopCapturing();
          }
          step = false;
          if (state.selection !== prev.selection || state.doc !== prev.doc) publishCursor(state);
        },
        destroy: () => {
          ytext.unobserve(observer);
          awareness?.off("change", others);
          if (frame !== 0) cancelAnimationFrame(frame);
          undo.destroy();
          view = null;
        },
      };
    },
  });

  const cursors = new Plugin<DecorationSet>({
    key: cursorKey,
    state: {
      init: () => DecorationSet.empty,
      apply: (tr, set, _, state) => (tr.getMeta(cursorKey) !== undefined || tr.docChanged ? drawCursors(state.doc) : set),
    },
    props: { decorations: (state) => cursorKey.getState(state) },
  });

  return {
    plugins: [sync, cursors],
    undo: () => undo.undo() !== null,
    redo: () => undo.redo() !== null,
    undoManager: undo,
  };
}
