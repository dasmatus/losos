/* Other people's cursors and selections in the editor, in their colour,
 * with their name beside the caret.
 *
 * y-codemirror.next draws these too, but with style="" attributes for the
 * colour, which the page's policy (`style-src 'self'`) refuses. Here every
 * colour is a class: `cm-who-<accent>` sets two custom properties from
 * tokens.css, and the marks read them. The rest follows that plugin: a
 * cursor travels in awareness as two Yjs relative positions, so it stays on
 * the same character while text moves around it. */

import { Annotation, RangeSet, type Range } from "@codemirror/state";
import { Decoration, EditorView, ViewPlugin, WidgetType, type DecorationSet, type ViewUpdate } from "@codemirror/view";
import * as Y from "yjs";
import type { Awareness } from "y-protocols/awareness";
import { ACCENTS, accentFor, type Accent } from "./people";

/** What each editor says about itself in awareness. */
export interface PresenceUser {
  name: string;
  /** `eva@kitchenbox.local`. */
  address: string;
  accent: Accent;
}

interface Cursor {
  anchor: unknown;
  head: unknown;
}

interface State {
  user?: PresenceUser;
  cursor?: Cursor | null;
}

const remoteChanged = Annotation.define<null>();

function accentOf(user: PresenceUser | undefined): Accent {
  const a = user?.accent;
  return a !== undefined && (ACCENTS as readonly string[]).includes(a) ? a : accentFor(user?.address ?? "");
}

class Caret extends WidgetType {
  constructor(
    readonly accent: Accent,
    readonly name: string,
  ) {
    super();
  }

  override toDOM(): HTMLElement {
    const caret = document.createElement("span");
    caret.className = `cm-who-caret cm-who-${this.accent}`;
    caret.setAttribute("aria-hidden", "true");
    // Zero-width joiners keep the caret from breaking the line around it.
    caret.append("⁠");
    const label = document.createElement("span");
    label.className = "cm-who-name";
    label.textContent = this.name;
    caret.append(label, "⁠");
    return caret;
  }

  override eq(other: Caret): boolean {
    return other.accent === this.accent && other.name === this.name;
  }

  override ignoreEvent(): boolean {
    return true;
  }
}

function plugin(ytext: Y.Text, awareness: Awareness) {
  return ViewPlugin.fromClass(
    class {
      decorations: DecorationSet = RangeSet.empty;
      private readonly listener: (changes: { added: number[]; updated: number[]; removed: number[] }) => void;

      constructor(readonly view: EditorView) {
        this.listener = ({ added, updated, removed }) => {
          const others = [...added, ...updated, ...removed].some((id) => id !== awareness.clientID);
          if (others) view.dispatch({ annotations: [remoteChanged.of(null)] });
        };
        awareness.on("change", this.listener);
      }

      destroy() {
        awareness.off("change", this.listener);
      }

      update(update: ViewUpdate) {
        this.publish(update);
        this.decorations = this.draw(update);
      }

      /** Put this editor's selection in awareness when it moved. */
      private publish(update: ViewUpdate) {
        const local = awareness.getLocalState() as State | null;
        if (local === null) return;
        const focused = update.view.hasFocus && update.view.dom.ownerDocument.hasFocus();
        if (!focused) {
          return;
        }
        const sel = update.state.selection.main;
        const anchor = Y.createRelativePositionFromTypeIndex(ytext, sel.anchor);
        const head = Y.createRelativePositionFromTypeIndex(ytext, sel.head);
        const was = local.cursor;
        if (
          was == null ||
          !Y.compareRelativePositions(Y.createRelativePositionFromJSON(was.anchor), anchor) ||
          !Y.compareRelativePositions(Y.createRelativePositionFromJSON(was.head), head)
        ) {
          awareness.setLocalStateField("cursor", { anchor, head });
        }
      }

      private draw(update: ViewUpdate): DecorationSet {
        const doc = ytext.doc;
        if (doc === null) return RangeSet.empty;
        const ranges: Range<Decoration>[] = [];
        const lines = update.view.state.doc;
        awareness.getStates().forEach((raw, client) => {
          if (client === awareness.clientID) return;
          const state = raw as State;
          const cursor = state.cursor;
          if (cursor == null || cursor.anchor == null || cursor.head == null) return;
          const anchor = Y.createAbsolutePositionFromRelativePosition(
            Y.createRelativePositionFromJSON(cursor.anchor),
            doc,
          );
          const head = Y.createAbsolutePositionFromRelativePosition(Y.createRelativePositionFromJSON(cursor.head), doc);
          if (anchor === null || head === null || anchor.type !== ytext || head.type !== ytext) return;
          const max = lines.length;
          const a = Math.min(anchor.index, max);
          const h = Math.min(head.index, max);
          const accent = accentOf(state.user);
          const who = `cm-who-${accent}`;
          const from = Math.min(a, h);
          const to = Math.max(a, h);
          if (from !== to) {
            const first = lines.lineAt(from);
            const last = lines.lineAt(to);
            const mark = Decoration.mark({ class: `cm-who-sel ${who}` });
            if (first.number === last.number) {
              ranges.push(mark.range(from, to));
            } else {
              if (first.to > from) ranges.push(mark.range(from, first.to));
              for (let n = first.number + 1; n < last.number; n++) {
                const line = lines.line(n);
                ranges.push(Decoration.line({ class: `cm-who-line ${who}` }).range(line.from));
              }
              if (to > last.from) ranges.push(mark.range(last.from, to));
            }
          }
          ranges.push(
            Decoration.widget({
              widget: new Caret(accent, state.user?.name ?? "?"),
              // Outside the selection it ends.
              side: h - a > 0 ? -1 : 1,
            }).range(h),
          );
        });
        return Decoration.set(ranges, true);
      }
    },
    { decorations: (v) => v.decorations },
  );
}

const colours = Object.fromEntries(
  ACCENTS.map((a) => [
    `.cm-who-${a}`,
    { "--who": `var(--person-${a})`, "--who-wash": `var(--person-${a}-wash)` },
  ]),
);

const theme = EditorView.theme({
  ...colours,
  ".cm-who-sel": { backgroundColor: "var(--who-wash)" },
  ".cm-who-line": { backgroundColor: "var(--who-wash)" },
  ".cm-who-caret": {
    position: "relative",
    borderLeft: "2px solid var(--who)",
    marginLeft: "-1px",
    marginRight: "-1px",
    boxSizing: "border-box",
  },
  ".cm-who-name": {
    position: "absolute",
    bottom: "100%",
    left: "-2px",
    padding: "0 5px",
    borderRadius: "4px 4px 4px 0",
    backgroundColor: "var(--who)",
    color: "var(--surface)",
    font: "500 11px/16px var(--font-ui)",
    whiteSpace: "nowrap",
    userSelect: "none",
    pointerEvents: "none",
    zIndex: "10",
  },
});

/** The cursors of everyone else in `awareness`, and this editor's own
 * published into it. */
export function remoteCursors(ytext: Y.Text, awareness: Awareness) {
  return [theme, plugin(ytext, awareness)];
}
