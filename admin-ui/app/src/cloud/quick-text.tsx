/* The bar over picked text in LosOS Office's Markdown documents and in
 * Notes (components/quick-actions.tsx draws it).
 *
 * Over the editor (`EditorQuickActions`) it formats what is picked, the way
 * the Office bar does and with the same code (office/markdown/format.ts), so
 * one undo takes a change back; links it to a web page, a note in the vault
 * or a person or file on the box; copies it; and asks the box's AI about
 * it, whose answer can replace it. Over a note being read
 * (`ReaderQuickActions`) it copies the words, links to their section and
 * asks the AI. Other parts add theirs to "text.selection" (cloud/together.tsx
 * hands the words to Mail and Chat).
 *
 * The bar waits for the pointer to let go, so it does not chase a drag, and
 * goes when the editor loses the focus to anything but the bar itself. */

import * as React from "react";
import { Compartment, EditorSelection, StateEffect } from "@codemirror/state";
import { EditorView } from "@codemirror/view";
import {
  AtIcon,
  Copy01Icon,
  File02Icon,
  Folder01Icon,
  HighlighterIcon,
  Link01Icon,
  Link04Icon,
  Note01Icon,
  SourceCodeIcon,
  TextBoldIcon,
  TextItalicIcon,
  TextStrikethroughIcon,
  UserIcon,
} from "@hugeicons/core-free-icons";
import { insideQuickActions, QuickActions } from "@/components/quick-actions";
import { toast } from "@/components/ui/toast";
import { copyText } from "@/lib/context-menu";
import { useT } from "@/lib/i18n-react";
import { mentionText, searchMentions, type Mention } from "@/lib/mentions";
import { quickActionsFor, type QuickSection, type QuickSubjects } from "@/lib/quick-actions";
import type { Vault } from "@/notes/vault";
import { linkName } from "./notes/editing";
import { activeAt, run, type Action, type Words } from "./office/markdown/format";

type App = QuickSubjects["text.selection"]["app"];

/** Redraws on every change that can move the picked text on screen. */
function useRedraw(): [number, () => void] {
  const [tick, setTick] = React.useState(0);
  const frame = React.useRef(0);
  const bump = React.useCallback(() => {
    if (frame.current !== 0) return;
    frame.current = window.requestAnimationFrame(() => {
      frame.current = 0;
      setTick((n) => n + 1);
    });
  }, []);
  React.useEffect(() => () => window.cancelAnimationFrame(frame.current), []);
  return [tick, bump];
}

/* Whether a mouse button or a finger is down on the page: the bar waits
 * for the selection to settle. Kept once for the page, so an editor that
 * starts again mid-drag (its connection came back) still knows. */
let pressed = false;
const watchers = new Set<() => void>();
if (typeof document !== "undefined") {
  const set = (down: boolean) => (event: PointerEvent) => {
    if (down && insideQuickActions(event.target)) return;
    if (pressed === down) return;
    pressed = down;
    for (const w of watchers) w();
  };
  document.addEventListener("pointerdown", set(true), true);
  document.addEventListener("pointerup", set(false), true);
  document.addEventListener("pointercancel", set(false), true);
}

function usePointerDown(onChange: () => void): { readonly current: boolean } {
  React.useEffect(() => {
    watchers.add(onChange);
    return () => {
      watchers.delete(onChange);
    };
  }, [onChange]);
  return {
    get current() {
      return pressed;
    },
  };
}

function rectOf(left: number, top: number, right: number, bottom: number): DOMRect {
  return DOMRect.fromRect({ x: left, y: top, width: Math.max(0, right - left), height: Math.max(0, bottom - top) });
}

/** The words a format button would leave in an empty selection. The bar
 *  only shows over picked text, so these are rarely seen. */
function useWords(): Words {
  const t = useT();
  return React.useMemo(
    () => ({
      text: t("office.md.word.text"),
      code: t("office.md.word.code"),
      formula: "E = mc^2",
      key: "Ctrl",
      url: "https://",
      picture: t("office.md.word.picture"),
      note: t("office.md.word.note"),
      tag: t("office.md.word.tag"),
      title: t("office.md.word.title"),
      summary: t("office.md.word.summary"),
      column: t("office.md.word.column"),
      footnote: t("office.md.word.footnote"),
      callout: t("office.md.word.callout"),
    }),
    [t],
  );
}

/** A link as Markdown writes it: a bare host gets https, a space <…>. */
export function linkTarget(value: string): string {
  const v = value.trim();
  const full = /^[a-z][a-z0-9+.-]*:/i.test(v) || v.startsWith("/") || v.startsWith("#") ? v : `https://${v}`;
  return /[\s()]/.test(full) ? `<${full}>` : full;
}

/** Where `text` is now, near `from`: the others may have typed meanwhile. */
function locate(view: EditorView, from: number, to: number, text: string): { from: number; to: number } | null {
  const doc = view.state.doc;
  if (to <= doc.length && view.state.sliceDoc(from, to) === text) return { from, to };
  const all = doc.toString();
  let best = -1;
  for (let i = all.indexOf(text); i >= 0; i = all.indexOf(text, i + 1)) {
    if (best < 0 || Math.abs(i - from) < Math.abs(best - from)) best = i;
  }
  return best < 0 ? null : { from: best, to: best + text.length };
}

const FORMATS: ReadonlyArray<readonly [Action, typeof TextBoldIcon, "office.tool.bold" | "office.tool.italic" | "office.tool.strikethrough" | "office.md.highlight" | "office.md.code", readonly string[] | undefined]> = [
  ["bold", TextBoldIcon, "office.tool.bold", ["Mod", "B"]],
  ["italic", TextItalicIcon, "office.tool.italic", ["Mod", "I"]],
  ["strike", TextStrikethroughIcon, "office.tool.strikethrough", ["Mod", "Shift", "X"]],
  ["highlight", HighlighterIcon, "office.md.highlight", ["Mod", "Shift", "H"]],
  ["code", SourceCodeIcon, "office.md.code", ["Mod", "E"]],
];

export function EditorQuickActions({
  view,
  app,
  path,
  title,
  vault,
  readOnly = false,
  deck = false,
}: {
  view: EditorView | null;
  app: App;
  /** The file, a path in LosOS Cloud. */
  path?: string;
  /** What the AI is told the words come from. */
  title: string;
  vault?: () => Vault | null;
  readOnly?: boolean;
  deck?: boolean;
}) {
  const t = useT();
  const words = useWords();
  const [tick, redraw] = useRedraw();
  const down = usePointerDown(redraw);
  const [dismissed, setDismissed] = React.useState<string | null>(null);
  const picks = React.useRef(new Map<string, Mention>());

  // Watch the editor from outside: selection, focus, scrolling, layout.
  React.useEffect(() => {
    if (view === null) return;
    const slot = new Compartment();
    view.dispatch({
      effects: StateEffect.appendConfig.of(
        slot.of(
          EditorView.updateListener.of((u) => {
            if (u.selectionSet || u.docChanged || u.focusChanged || u.geometryChanged || u.viewportChanged) redraw();
          }),
        ),
      ),
    });
    const scroller = view.scrollDOM;
    scroller.addEventListener("scroll", redraw, { passive: true });
    window.addEventListener("scroll", redraw, { passive: true, capture: true });
    window.addEventListener("resize", redraw);
    // Leaving the bar for the page closes it; into the bar keeps it.
    const blur = (): void => {
      window.setTimeout(redraw, 0);
    };
    document.addEventListener("focusin", blur);
    return () => {
      scroller.removeEventListener("scroll", redraw);
      window.removeEventListener("scroll", redraw, { capture: true });
      window.removeEventListener("resize", redraw);
      document.removeEventListener("focusin", blur);
      try {
        view.dispatch({ effects: slot.reconfigure([]) });
      } catch {
        // The editor went first.
      }
    };
  }, [view, redraw]);

  void tick;
  const state = view?.state;
  const main = state?.selection.main;
  const key = main === undefined ? "" : `${main.from}:${main.to}`;
  const text = state !== undefined && main !== undefined ? state.sliceDoc(main.from, main.to) : "";

  let anchor: DOMRect | null = null;
  if (view !== null && main !== undefined && !main.empty && text.trim() !== "" && !down.current && dismissed !== key) {
    const focused = view.hasFocus || insideQuickActions(document.activeElement);
    const a = focused ? view.coordsAtPos(main.from, 1) : null;
    const b = focused ? view.coordsAtPos(main.to, -1) : null;
    const box = view.scrollDOM.getBoundingClientRect();
    if (a !== null && b !== null && b.bottom > box.top && a.top < box.bottom) {
      const content = view.contentDOM.getBoundingClientRect();
      const oneLine = Math.abs(a.top - b.top) < 4;
      anchor = oneLine
        ? rectOf(a.left, Math.max(a.top, box.top), b.right, b.bottom)
        : rectOf(Math.max(content.left, box.left), Math.max(a.top, box.top), Math.min(content.right, box.right), Math.min(b.bottom, box.bottom));
    }
  }

  // A new selection brings the bar back after Escape.
  React.useEffect(() => {
    if (dismissed !== null && dismissed !== key) setDismissed(null);
  }, [dismissed, key]);

  const from = main?.from ?? 0;
  const to = main?.to ?? 0;
  const replace = (insert: string, select: "after" | "inside" = "after"): void => {
    if (view === null) return;
    const at = locate(view, from, to, text);
    if (at === null) return;
    view.dispatch({
      changes: { from: at.from, to: at.to, insert },
      selection: select === "after" ? EditorSelection.cursor(at.from + insert.length) : EditorSelection.range(at.from, at.from + insert.length),
      scrollIntoView: true,
      userEvent: "input.format",
    });
    view.focus();
  };

  const active = state !== undefined ? activeAt(state) : new Set<Action>();
  const editing = !readOnly;
  const sections: QuickSection[] = [
    FORMATS.map(([action, icon, label, keys]) => ({
      id: action,
      label: t(label),
      icon,
      keys,
      pressed: active.has(action),
      hidden: !editing,
      onSelect: () => view !== null && run(view, action, words, deck),
    })),
    [
      {
        kind: "prompt",
        id: "link",
        label: t("quick.link"),
        icon: Link01Icon,
        placeholder: t("quick.link.placeholder"),
        hidden: !editing,
        onSubmit: (value) => replace(`[${text}](${linkTarget(value)})`),
      },
      {
        kind: "prompt",
        id: "wikilink",
        label: t("quick.note"),
        icon: Link04Icon,
        placeholder: t("quick.note.placeholder"),
        initial: text.trim().split("\n")[0]?.slice(0, 80),
        hidden: !editing || vault === undefined,
        suggest: (q) => {
          const v = vault?.();
          if (v === null || v === undefined) return [];
          const needle = q.trim().toLocaleLowerCase();
          return v.notes
            .map((n) => ({ n, name: linkName(v, n.path) }))
            .filter(({ name, n }) => needle === "" || name.toLocaleLowerCase().includes(needle) || n.aliases.some((a) => a.toLocaleLowerCase().includes(needle)))
            .sort((x, y) => Number(!x.name.toLocaleLowerCase().startsWith(needle)) - Number(!y.name.toLocaleLowerCase().startsWith(needle)))
            .slice(0, 6)
            .map(({ n, name }) => ({ id: n.path, label: name, detail: n.path.includes("/") ? n.path.slice(0, n.path.lastIndexOf("/")) : "", icon: Note01Icon }));
        },
        onSubmit: (value) => {
          const target = value.replace(/[[\]|]/g, "").trim();
          if (target === "") return;
          const shown = text.replace(/[[\]|\n]/g, " ").trim();
          replace(shown === target || shown === "" ? `[[${target}]]` : `[[${target}|${shown}]]`);
        },
      },
      {
        kind: "prompt",
        id: "mention",
        label: t("quick.mention"),
        icon: AtIcon,
        placeholder: t("quick.mention.placeholder"),
        hidden: !editing,
        suggest: async (q, signal) => {
          const list = await searchMentions(q, signal);
          picks.current = new Map(list.map((m) => [m.id, m]));
          return list.map((m) => ({ id: m.id, label: m.label, detail: m.detail, icon: m.kind === "person" ? UserIcon : m.dir ? Folder01Icon : File02Icon }));
        },
        onSubmit: (_value, picked) => {
          const m = picked === undefined ? undefined : picks.current.get(picked.id);
          if (m === undefined) return;
          // A file links the words; a person is named after them.
          replace(m.kind === "file" ? `[${text}](${m.href})` : `${text} ${mentionText(m)}`);
        },
      },
    ],
    [
      {
        id: "copy",
        label: t("quick.copy"),
        icon: Copy01Icon,
        keys: ["Mod", "C"],
        onSelect: () => void copied(text, t("menu.copied"), t("menu.copyFailed")),
      },
    ],
    quickActionsFor("text.selection", { text, app, path }),
  ];

  return (
    <QuickActions
      anchor={anchor}
      label={t("quick.text")}
      sections={sections}
      ask={{
        context: app,
        sources: () => [{ title, text }],
        onUse: editing ? (answer) => replace(answer.trim(), "inside") : undefined,
      }}
      onReturn={() => view?.focus()}
      onDismiss={() => setDismissed(key)}
      ceiling={() => view?.scrollDOM.getBoundingClientRect().top ?? 0}
    />
  );
}

async function copied(text: string, done: string, failed: string): Promise<void> {
  if (await copyText(text)) toast.done(done);
  else toast.error(failed, undefined, { help: "index" });
}

/** The bar over words picked in a note being read: copy them, link to
 *  their section, ask the AI about them. */
export function ReaderQuickActions({
  container,
  app,
  path,
  title,
  sectionHref,
}: {
  container: React.RefObject<HTMLElement | null>;
  app: App;
  path?: string;
  title: string;
  /** An address for the heading with this id, on this origin. */
  sectionHref?: (heading: string) => string;
}) {
  const t = useT();
  const [tick, redraw] = useRedraw();
  const down = usePointerDown(redraw);
  const [dismissed, setDismissed] = React.useState<string | null>(null);

  React.useEffect(() => {
    document.addEventListener("selectionchange", redraw);
    window.addEventListener("scroll", redraw, { passive: true, capture: true });
    window.addEventListener("resize", redraw);
    return () => {
      document.removeEventListener("selectionchange", redraw);
      window.removeEventListener("scroll", redraw, { capture: true });
      window.removeEventListener("resize", redraw);
    };
  }, [redraw]);

  void tick;
  const root = container.current;
  const selection = typeof window === "undefined" ? null : window.getSelection();
  const range = selection !== null && !selection.isCollapsed && selection.rangeCount > 0 ? selection.getRangeAt(0) : null;
  const inside = range !== null && root !== null && root.contains(range.commonAncestorContainer);
  const text = inside ? (selection?.toString() ?? "").trim() : "";
  const key = inside && range !== null ? `${text.length}:${text.slice(0, 40)}` : "";

  React.useEffect(() => {
    if (dismissed !== null && dismissed !== key) setDismissed(null);
  }, [dismissed, key]);

  let anchor: DOMRect | null = null;
  let heading: string | null = null;
  if (inside && range !== null && text !== "" && !down.current && dismissed !== key) {
    const r = range.getBoundingClientRect();
    if (r.bottom > 0 && r.top < window.innerHeight && (r.width > 0 || r.height > 0)) anchor = r;
    // The heading above the words, for a link to their section.
    const start = range.startContainer instanceof Element ? range.startContainer : range.startContainer.parentElement;
    const headings = root?.querySelectorAll<HTMLElement>("h1[id], h2[id], h3[id], h4[id], h5[id], h6[id]") ?? [];
    for (const h of headings) {
      if (start !== null && (h === start || h.contains(start) || h.compareDocumentPosition(start) & Node.DOCUMENT_POSITION_FOLLOWING)) heading = h.id;
    }
  }

  const sections: QuickSection[] = [
    [
      {
        id: "copy",
        label: t("quick.copy"),
        icon: Copy01Icon,
        keys: ["Mod", "C"],
        onSelect: () => void copied(text, t("menu.copied"), t("menu.copyFailed")),
      },
      {
        id: "copy-link",
        label: t("quick.copyLink"),
        icon: Link01Icon,
        hidden: heading === null || sectionHref === undefined,
        onSelect: () => heading !== null && void copied(new URL(sectionHref!(heading), location.href).href, t("quick.linkCopied"), t("menu.copyFailed")),
      },
    ],
    quickActionsFor("text.selection", { text, app, path }),
  ];

  return (
    <QuickActions
      anchor={anchor}
      label={t("quick.text")}
      sections={sections}
      ask={{ context: app, sources: () => [{ title, text }] }}
      onReturn={() => container.current?.focus({ preventScroll: true })}
      onDismiss={() => setDismissed(key)}
    />
  );
}
