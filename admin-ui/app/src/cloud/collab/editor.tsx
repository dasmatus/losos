import * as React from "react";
import { defaultKeymap, indentWithTab } from "@codemirror/commands";
import { markdown } from "@codemirror/lang-markdown";
import { HighlightStyle, syntaxHighlighting } from "@codemirror/language";
import { Compartment, EditorState } from "@codemirror/state";
import { drawSelection, EditorView, keymap, placeholder } from "@codemirror/view";
import { tags } from "@lezer/highlight";
import { yCollab, yUndoManagerKeymap } from "y-codemirror.next";
import { WebsocketProvider } from "y-websocket";
import * as Y from "yjs";
import { cn } from "@/lib/utils";
import { collab } from "./api";
import { remoteCursors, type PresenceUser } from "./cursors";

/* The editor itself: CodeMirror on one Yjs text, kept in step with the box
 * over y-websocket (backend/src/cloud/collab/routes.rs).
 *
 * In a shadow root, as components/code-editor.tsx is and for the same
 * reason: CodeMirror's styles then go in as constructed stylesheets, which
 * the page's `style-src 'self'` allows, where a <style> element is refused.
 * The text is the file: what is typed here is written back to the file on
 * the box that holds it a moment after the typing stops. */

/** The name of the shared text; the box uses the same (room.rs, TEXT). */
const TEXT = "content";

export type Connection = "connecting" | "live" | "offline";

export interface Edited {
  name: string;
  address: string;
  at: number;
}

export interface Presence {
  client: number;
  user: PresenceUser;
}

const theme = EditorView.theme({
  "&": {
    height: "100%",
    color: "var(--ink)",
    backgroundColor: "var(--surface)",
    fontSize: "15px",
  },
  "&.cm-focused": { outline: "none" },
  ".cm-scroller": {
    fontFamily: "var(--font-ui)",
    lineHeight: "1.65",
  },
  // Room above the first line for a name beside someone's caret.
  ".cm-content": { padding: "22px 0 48px", caretColor: "var(--accent)" },
  ".cm-line": { padding: "0 28px" },
  ".cm-cursor, .cm-dropCursor": { borderLeftColor: "var(--accent)", borderLeftWidth: "2px" },
  "&.cm-focused > .cm-scroller > .cm-selectionLayer .cm-selectionBackground, .cm-selectionBackground, ::selection": {
    backgroundColor: "var(--accent-wash)",
  },
  ".cm-placeholder": { color: "var(--faint)" },
});

const highlight = HighlightStyle.define([
  { tag: tags.heading1, fontSize: "1.45em", fontWeight: "650", letterSpacing: "-0.01em" },
  { tag: tags.heading2, fontSize: "1.2em", fontWeight: "650" },
  { tag: [tags.heading3, tags.heading4, tags.heading5, tags.heading6], fontWeight: "650" },
  { tag: tags.strong, fontWeight: "650" },
  { tag: tags.emphasis, fontStyle: "italic" },
  { tag: tags.strikethrough, textDecoration: "line-through" },
  { tag: tags.link, color: "var(--accent)", textDecoration: "underline" },
  { tag: tags.url, color: "var(--muted)" },
  { tag: [tags.monospace], fontFamily: "var(--font-code)", fontSize: "0.9em" },
  { tag: [tags.processingInstruction, tags.meta, tags.contentSeparator], color: "var(--faint)" },
  { tag: tags.quote, color: "var(--muted)" },
  { tag: tags.list, color: "var(--accent)" },
]);

export interface CollabEditorProps {
  file: number;
  share?: string;
  me: PresenceUser;
  readOnly: boolean;
  label: string;
  placeholderText: string;
  onConnection: (state: Connection) => void;
  onPresence: (people: Presence[]) => void;
  onEdited: (edit: Edited | null) => void;
  className?: string;
}

export function CollabEditor({
  file,
  share,
  me,
  readOnly,
  label,
  placeholderText,
  onConnection,
  onPresence,
  onEdited,
  className,
}: CollabEditorProps) {
  const hostRef = React.useRef<HTMLDivElement>(null);
  const viewRef = React.useRef<EditorView | null>(null);
  const editable = React.useRef(new Compartment());
  const callbacks = React.useRef({ onConnection, onPresence, onEdited });
  callbacks.current = { onConnection, onPresence, onEdited };
  const meRef = React.useRef(me);
  meRef.current = me;

  React.useEffect(() => {
    const host = hostRef.current;
    if (host === null) return;
    const doc = new Y.Doc();
    const ytext = doc.getText(TEXT);
    const meta = doc.getMap<Edited>("meta");
    const provider = new WebsocketProvider(collab.server(), "ws", doc, {
      params: collab.params(file, share),
      // One room name for every file: the file is in the parameters, so
      // tabs must not pass updates to each other by the room's name.
      disableBc: true,
    });
    const awareness = provider.awareness;
    awareness.setLocalStateField("user", meRef.current);

    const status = ({ status }: { status: string }) => {
      if (status === "connected") return;
      callbacks.current.onConnection(status === "connecting" ? "connecting" : "offline");
    };
    const synced = (isSynced: boolean) => {
      if (isSynced) callbacks.current.onConnection("live");
    };
    provider.on("status", status);
    provider.on("sync", synced);
    provider.on("connection-close", () => callbacks.current.onConnection("offline"));

    const people = () => {
      const out: Presence[] = [];
      awareness.getStates().forEach((state, client) => {
        const user = (state as { user?: PresenceUser }).user;
        if (user !== undefined && typeof user.name === "string") out.push({ client, user });
      });
      callbacks.current.onPresence(out);
    };
    awareness.on("change", people);
    people();

    const edited = () => callbacks.current.onEdited(meta.get("edited") ?? null);
    meta.observe(edited);

    // Say who typed last, at most every few seconds.
    let stamped = 0;
    const stamp = (_: Y.YTextEvent, tr: Y.Transaction) => {
      if (!tr.local || Date.now() - stamped < 4000) return;
      stamped = Date.now();
      const { name, address } = meRef.current;
      meta.set("edited", { name, address, at: stamped });
    };
    ytext.observe(stamp);

    const shadow = host.shadowRoot ?? host.attachShadow({ mode: "open" });
    const undo = new Y.UndoManager(ytext);
    const view = new EditorView({
      root: shadow,
      parent: shadow,
      state: EditorState.create({
        doc: ytext.toString(),
        extensions: [
          drawSelection(),
          EditorView.lineWrapping,
          markdown(),
          syntaxHighlighting(highlight),
          theme,
          placeholder(placeholderText),
          keymap.of([...yUndoManagerKeymap, ...defaultKeymap, indentWithTab]),
          editable.current.of([EditorView.editable.of(!readOnly), EditorState.readOnly.of(readOnly)]),
          EditorView.contentAttributes.of({ "aria-label": label, "aria-multiline": "true" }),
          // Sync and undo from y-codemirror.next; the cursors are ours.
          yCollab(ytext, null, { undoManager: undo }),
          remoteCursors(ytext, awareness),
        ],
      }),
    });
    viewRef.current = view;
    host.dataset["ready"] = "";

    return () => {
      ytext.unobserve(stamp);
      meta.unobserve(edited);
      awareness.off("change", people);
      provider.destroy();
      view.destroy();
      doc.destroy();
      viewRef.current = null;
      delete host.dataset["ready"];
    };
    // The editor is for one file; the page keys it by file and share.
  }, [file, share, label, placeholderText]);

  React.useEffect(() => {
    viewRef.current?.dispatch({
      effects: editable.current.reconfigure([
        EditorView.editable.of(!readOnly),
        EditorState.readOnly.of(readOnly),
      ]),
    });
  }, [readOnly]);

  return (
    <div
      ref={hostRef}
      data-testid="collab-editor"
      className={cn("min-h-[60dvh] overflow-hidden rounded-card border border-line bg-surface shadow-card", className)}
    />
  );
}
