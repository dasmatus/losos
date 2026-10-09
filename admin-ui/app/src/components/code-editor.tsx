/* A code editor with completions, for the files of a hand-written widget.
 *
 * CodeMirror 6, not Monaco, because of the admin page's CSP (`style-src
 * 'self'`, modules/containers.nix): Monaco writes its theme into `<style>`
 * elements it creates at runtime, which that policy refuses, and it wants
 * web workers besides. CodeMirror's styles go through style-mod, which uses
 * a constructable stylesheet (a CSSOM write the policy allows) when the
 * editor lives in a shadow root and a `<style>` element otherwise. So the
 * editor is mounted in a shadow root of its own, and the page's classes do
 * not reach inside: everything visible in there is the theme below, drawn
 * from the box's colour variables, which do inherit through the boundary.
 *
 * ID references do not cross a shadow boundary either, so the editor is
 * named with `aria-label`, not `aria-labelledby`.
 */

import * as React from "react";
import {
  autocompletion,
  closeBrackets,
  closeBracketsKeymap,
  completionKeymap,
  type CompletionSource,
} from "@codemirror/autocomplete";
import { defaultKeymap, history, historyKeymap, indentWithTab } from "@codemirror/commands";
import { css, cssLanguage } from "@codemirror/lang-css";
import { html, htmlLanguage } from "@codemirror/lang-html";
import { javascript, javascriptLanguage } from "@codemirror/lang-javascript";
import { json } from "@codemirror/lang-json";
import { markdown } from "@codemirror/lang-markdown";
import {
  bracketMatching,
  HighlightStyle,
  indentOnInput,
  syntaxHighlighting,
} from "@codemirror/language";
import { Compartment, EditorState, type Extension } from "@codemirror/state";
import {
  drawSelection,
  EditorView,
  highlightActiveLine,
  highlightActiveLineGutter,
  keymap,
  lineNumbers,
} from "@codemirror/view";
import { tags } from "@lezer/highlight";
import { cn } from "@/lib/utils";
import { cssCompletions, htmlFileNames, scriptCompletions } from "@/widgets/completions";
import type { FileLanguage } from "@/widgets/files";

const theme = EditorView.theme({
  "&": {
    height: "100%",
    color: "var(--ink)",
    backgroundColor: "var(--surface)",
    fontSize: "12.5px",
  },
  "&.cm-focused": { outline: "none" },
  ".cm-scroller": {
    fontFamily: "var(--font-code, ui-monospace, SFMono-Regular, Menlo, monospace)",
    lineHeight: "1.6",
  },
  ".cm-content": { caretColor: "var(--accent)", padding: "8px 0" },
  ".cm-cursor, .cm-dropCursor": { borderLeftColor: "var(--accent)" },
  ".cm-gutters": {
    backgroundColor: "var(--sunk)",
    color: "var(--faint)",
    border: "none",
    borderRight: "1px solid var(--line)",
  },
  ".cm-activeLine": { backgroundColor: "color-mix(in srgb, var(--accent) 6%, transparent)" },
  ".cm-activeLineGutter": { backgroundColor: "transparent", color: "var(--muted)" },
  "&.cm-focused > .cm-scroller > .cm-selectionLayer .cm-selectionBackground, .cm-selectionBackground": {
    backgroundColor: "color-mix(in srgb, var(--accent) 24%, transparent)",
  },
  ".cm-matchingBracket": {
    backgroundColor: "color-mix(in srgb, var(--accent) 16%, transparent)",
    outline: "1px solid color-mix(in srgb, var(--accent) 40%, transparent)",
  },
  ".cm-tooltip": {
    backgroundColor: "var(--surface)",
    color: "var(--ink)",
    border: "1px solid var(--line)",
    borderRadius: "6px",
    boxShadow: "0 6px 24px rgb(0 0 0 / 0.14)",
    overflow: "hidden",
  },
  ".cm-tooltip.cm-tooltip-autocomplete > ul": {
    fontFamily: "var(--font-code, ui-monospace, monospace)",
    maxHeight: "16em",
  },
  ".cm-tooltip.cm-tooltip-autocomplete > ul > li": { padding: "2px 8px" },
  ".cm-tooltip.cm-tooltip-autocomplete > ul > li[aria-selected]": {
    backgroundColor: "var(--accent-wash)",
    color: "var(--ink)",
  },
  ".cm-completionDetail": { color: "var(--muted)", fontStyle: "normal", marginLeft: "0.8em" },
  ".cm-completionMatchedText": { textDecoration: "none", fontWeight: "600", color: "var(--accent)" },
  ".cm-tooltip.cm-completionInfo": {
    padding: "6px 10px",
    maxWidth: "22em",
    fontFamily: "var(--font-ui, system-ui)",
    fontSize: "12px",
    lineHeight: "1.45",
  },
});

const highlight = HighlightStyle.define([
  { tag: [tags.keyword, tags.operatorKeyword, tags.modifier], color: "var(--accent)" },
  { tag: [tags.tagName, tags.angleBracket], color: "var(--accent)" },
  { tag: [tags.attributeName, tags.propertyName], color: "var(--ink)", fontWeight: "500" },
  { tag: [tags.string, tags.attributeValue, tags.special(tags.string)], color: "var(--ok)" },
  { tag: [tags.number, tags.bool, tags.null, tags.atom, tags.unit], color: "var(--warn)" },
  { tag: [tags.comment, tags.meta], color: "var(--faint)", fontStyle: "italic" },
  { tag: [tags.variableName, tags.definition(tags.variableName)], color: "var(--ink)" },
  { tag: [tags.function(tags.variableName), tags.function(tags.propertyName)], color: "var(--ink)" },
  { tag: [tags.className, tags.typeName, tags.labelName], color: "var(--crit)" },
  { tag: tags.heading, fontWeight: "600" },
  { tag: tags.invalid, color: "var(--crit)" },
]);

function languageExtension(language: FileLanguage, files: () => readonly string[]): Extension {
  const script = scriptCompletions(files).map((source: CompletionSource) =>
    javascriptLanguage.data.of({ autocomplete: source }),
  );
  const style = cssLanguage.data.of({ autocomplete: cssCompletions(files) });
  switch (language) {
    case "html":
      // Script and style inside index.html get the same help as in their
      // own files: html() nests these very language objects.
      return [html(), htmlLanguage.data.of({ autocomplete: htmlFileNames(files) }), script, style];
    case "css":
      return [css(), style];
    case "javascript":
      return [javascript(), script];
    case "json":
      return json();
    case "markdown":
      return markdown();
    default:
      return [];
  }
}

export interface CodeEditorProps {
  value: string;
  onChange: (value: string) => void;
  language: FileLanguage;
  /** The accessible name; see the header for why not an id. */
  label: string;
  /** Names of the other files, offered where a file can be named. */
  files?: readonly string[];
  disabled?: boolean;
  invalid?: boolean;
  className?: string;
}

export function CodeEditor({
  value,
  onChange,
  language,
  label,
  files = [],
  disabled = false,
  invalid = false,
  className,
}: CodeEditorProps) {
  const hostRef = React.useRef<HTMLDivElement>(null);
  const viewRef = React.useRef<EditorView | null>(null);
  const onChangeRef = React.useRef(onChange);
  onChangeRef.current = onChange;
  const filesRef = React.useRef(files);
  filesRef.current = files;
  const editable = React.useRef(new Compartment());
  const attributes = React.useRef(new Compartment());

  /* One editor per mount; the caller keys this component by file, so a
   * different file is a different editor with its own undo history. */
  React.useEffect(() => {
    const host = hostRef.current;
    if (host === null) return;
    // Strict mode mounts twice, and a host keeps its shadow root.
    const shadow = host.shadowRoot ?? host.attachShadow({ mode: "open" });
    const view = new EditorView({
      root: shadow,
      parent: shadow,
      state: EditorState.create({
        doc: value,
        extensions: [
          lineNumbers(),
          highlightActiveLineGutter(),
          highlightActiveLine(),
          drawSelection(),
          history(),
          indentOnInput(),
          bracketMatching(),
          closeBrackets(),
          autocompletion({ icons: false }),
          syntaxHighlighting(highlight),
          theme,
          EditorState.tabSize.of(2),
          keymap.of([
            ...closeBracketsKeymap,
            ...completionKeymap,
            ...historyKeymap,
            ...defaultKeymap,
            indentWithTab,
          ]),
          languageExtension(language, () => filesRef.current),
          editable.current.of([EditorView.editable.of(!disabled), EditorState.readOnly.of(disabled)]),
          attributes.current.of(
            EditorView.contentAttributes.of({ "aria-label": label, "aria-invalid": String(invalid) }),
          ),
          EditorView.updateListener.of((update) => {
            if (update.docChanged) onChangeRef.current(update.state.doc.toString());
          }),
        ],
      }),
    });
    viewRef.current = view;
    return () => {
      view.destroy();
      viewRef.current = null;
    };
    // The language is fixed for the life of the editor; value, label and
    // the flags are followed by the effects below.
  }, [language]);

  // A value from outside (a build that finished) replaces the text.
  React.useEffect(() => {
    const view = viewRef.current;
    if (view === null) return;
    const current = view.state.doc.toString();
    if (current !== value) {
      view.dispatch({ changes: { from: 0, to: current.length, insert: value } });
    }
  }, [value]);

  React.useEffect(() => {
    viewRef.current?.dispatch({
      effects: [
        editable.current.reconfigure([
          EditorView.editable.of(!disabled),
          EditorState.readOnly.of(disabled),
        ]),
        attributes.current.reconfigure(
          EditorView.contentAttributes.of({ "aria-label": label, "aria-invalid": String(invalid) }),
        ),
      ],
    });
  }, [disabled, label, invalid]);

  return (
    <div
      ref={hostRef}
      data-testid="code-editor"
      data-invalid={invalid ? "" : undefined}
      className={cn(
        "h-80 overflow-hidden rounded-control border border-line bg-surface",
        "focus-within:border-accent focus-within:ring-2 focus-within:ring-accent/35",
        "data-invalid:border-crit",
        disabled && "opacity-70",
        className,
      )}
    />
  );
}
