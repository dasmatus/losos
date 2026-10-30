/* The strip under the bar for the part of the document that is selected:
 * a figure's file and caption, a callout's kind and title, a link's
 * address, a comment's text, a table's caption and its rows and columns.
 * Each field writes its component's prop as it is typed. What the strip
 * offers for each part is the table below, the same props losos-mdx
 * checks (backend/mdx/src/components.rs). */

import * as React from "react";
import { addColumnAfter, addRowAfter, deleteColumn, deleteRow, deleteTable, mergeCells, splitCell } from "prosemirror-tables";
import type { Mark, MarkType, Node as PMNode } from "prosemirror-model";
import { NodeSelection, type Command, type EditorState } from "prosemirror-state";
import type { EditorView } from "prosemirror-view";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { NativeSelect, NativeSelectOption } from "@/components/ui/native-select";
import type { MessageKey } from "@/lib/i18n";
import { useT } from "@/lib/i18n-react";
import { ALIGNS, CALLOUT_TYPES, CHART_TYPES, DATE_FORMATS, SLIDE_LAYOUTS, schema } from "@/mdx/schema";
import { markRange } from "./commands";
import { STEP } from "./sync";

type Field =
  | { name: string; label: MessageKey; kind: "text" | "date" | "code" }
  | { name: string; label: MessageKey; kind: "flag" }
  | { name: string; label: MessageKey; kind: "choice"; choices: readonly string[]; words?: (v: string) => MessageKey; optional?: boolean };

const CALLOUT_WORD = (v: string) => `office.md.callout.${v}` as MessageKey;

const NODE_FIELDS: Record<string, Field[]> = {
  callout: [
    { name: "type", label: "paper.field.type", kind: "choice", choices: CALLOUT_TYPES, words: CALLOUT_WORD },
    { name: "title", label: "paper.field.title", kind: "text" },
  ],
  details: [
    { name: "summary", label: "paper.field.summary", kind: "text" },
    { name: "open", label: "paper.field.open", kind: "flag" },
  ],
  tab: [{ name: "label", label: "paper.field.label", kind: "text" }],
  slide: [{ name: "layout", label: "paper.field.layout", kind: "choice", choices: SLIDE_LAYOUTS, words: (v) => `paper.layout.${v}` as MessageKey }],
  figure: [
    { name: "src", label: "paper.field.src", kind: "text" },
    { name: "alt", label: "paper.field.alt", kind: "text" },
    { name: "caption", label: "paper.field.caption", kind: "text" },
  ],
  image: [
    { name: "src", label: "paper.field.src", kind: "text" },
    { name: "alt", label: "paper.field.alt", kind: "text" },
  ],
  chart: [
    { name: "src", label: "paper.field.src", kind: "text" },
    { name: "range", label: "paper.field.range", kind: "text" },
    { name: "type", label: "paper.field.chartType", kind: "choice", choices: CHART_TYPES, words: (v) => `paper.chart.${v}` as MessageKey },
    { name: "title", label: "paper.field.title", kind: "text" },
  ],
  toc: [{ name: "title", label: "paper.field.title", kind: "text" }],
  equation: [{ name: "tex", label: "paper.field.tex", kind: "code" }],
  math: [{ name: "tex", label: "paper.field.tex", kind: "code" }],
  mention: [{ name: "login", label: "paper.field.login", kind: "text" }],
  date: [
    { name: "value", label: "paper.field.value", kind: "date" },
    { name: "format", label: "paper.field.format", kind: "choice", choices: DATE_FORMATS, words: (v) => `paper.format.${v}` as MessageKey, optional: true },
  ],
  embed: [
    { name: "href", label: "paper.field.href", kind: "text" },
    { name: "title", label: "paper.field.title", kind: "text" },
  ],
  code_block: [{ name: "language", label: "paper.field.language", kind: "text" }],
  table: [{ name: "caption", label: "paper.field.caption", kind: "text" }],
  raw: [{ name: "source", label: "paper.field.source", kind: "code" }],
  raw_inline: [{ name: "source", label: "paper.field.source", kind: "code" }],
};

const MARK_FIELDS: Record<string, Field[]> = {
  link: [{ name: "href", label: "paper.field.href", kind: "text" }],
  comment: [{ name: "text", label: "paper.field.text", kind: "text" }],
  abbr: [{ name: "title", label: "paper.field.abbr", kind: "text" }],
};

type Target =
  | { kind: "node"; node: PMNode; pos: number; fields: Field[] }
  | { kind: "mark"; mark: Mark; from: number; to: number; fields: Field[] };

/** What the strip is about: a selected component, a mark the cursor is
 * in, or the nearest container around the cursor. */
export function targetOf(state: EditorState): Target | null {
  const sel = state.selection;
  if (sel instanceof NodeSelection) {
    const fields = NODE_FIELDS[sel.node.type.name];
    if (fields !== undefined) return { kind: "node", node: sel.node, pos: sel.from, fields };
  }
  for (const name of ["link", "comment", "abbr"]) {
    const range = markRange(state, schema.marks[name] as MarkType);
    if (range !== null) return { kind: "mark", mark: range.mark, from: range.from, to: range.to, fields: MARK_FIELDS[name]! };
  }
  const $from = sel.$from;
  for (let d = $from.depth; d > 0; d--) {
    const n = $from.node(d);
    const fields = NODE_FIELDS[n.type.name];
    if (fields !== undefined) return { kind: "node", node: n, pos: $from.before(d), fields };
  }
  return null;
}

function inTable(state: EditorState): boolean {
  const $from = state.selection.$from;
  for (let d = $from.depth; d > 0; d--) if ($from.node(d).type.name === "table") return true;
  return false;
}

export function Fields({ view, state, readOnly }: { view: EditorView | null; state: EditorState | null; readOnly: boolean }) {
  const t = useT();
  if (view === null || state === null || readOnly) return null;
  const target = targetOf(state);
  const table = inTable(state);
  if (target === null && !table) return null;

  const run = (cmd: Command) => {
    cmd(view.state, (tr) => view.dispatch(tr.setMeta(STEP, true)), view);
    view.focus();
  };
  const set = (name: string, value: unknown) => {
    const now = targetOf(view.state);
    if (now === null) return;
    const tr = view.state.tr;
    if (now.kind === "node") {
      tr.setNodeMarkup(now.pos, undefined, { ...now.node.attrs, [name]: value });
      if (view.state.selection instanceof NodeSelection) tr.setSelection(NodeSelection.create(tr.doc, now.pos));
    } else {
      tr.removeMark(now.from, now.to, now.mark.type);
      tr.addMark(now.from, now.to, now.mark.type.create({ ...now.mark.attrs, [name]: value }));
    }
    view.dispatch(tr);
  };
  const remove = () => {
    const now = targetOf(view.state);
    if (now === null) return;
    if (now.kind === "mark") view.dispatch(view.state.tr.removeMark(now.from, now.to, now.mark.type).setMeta(STEP, true));
    else view.dispatch(view.state.tr.delete(now.pos, now.pos + now.node.nodeSize).setMeta(STEP, true));
    view.focus();
  };

  const attrs = target === null ? {} : target.kind === "node" ? target.node.attrs : target.mark.attrs;
  const name = target === null ? "table" : target.kind === "node" ? target.node.type.name : target.mark.type.name;
  const removable = target !== null && (target.kind === "mark" || target.node.isAtom);

  return (
    <div
      className="flex min-h-10 shrink-0 flex-wrap items-center gap-2 border-b border-hair bg-sunk px-2 py-1.5 text-[13px] sm:px-3"
      role="group"
      aria-label={t("paper.fields")}
      data-testid="paper-fields"
      data-target={name}
    >
      {target?.fields.map((f) => (
        <FieldInput key={`${name}-${f.name}`} field={f} value={(attrs as Record<string, unknown>)[f.name]} onChange={(v) => set(f.name, v)} />
      ))}
      {table && (
        <span className="flex flex-wrap items-center gap-1" data-testid="paper-table-tools">
          {(
            [
              ["paper.table.addRow", addRowAfter],
              ["paper.table.addColumn", addColumnAfter],
              ["paper.table.deleteRow", deleteRow],
              ["paper.table.deleteColumn", deleteColumn],
              ["paper.table.merge", mergeCells],
              ["paper.table.split", splitCell],
              ["paper.table.delete", deleteTable],
            ] as const
          ).map(([label, cmd]) => (
            <Button key={label} variant="ghost" size="sm" disabled={!cmd(state)} onMouseDown={(e) => e.preventDefault()} onClick={() => run(cmd)}>
              {t(label)}
            </Button>
          ))}
          <CellAlign view={view} state={state} />
        </span>
      )}
      {removable && (
        <Button variant="ghost" size="sm" className="ml-auto" onClick={remove} data-action="remove">
          {target?.kind === "mark" && target.mark.type.name === "comment" ? t("paper.field.resolve") : t("paper.field.remove")}
        </Button>
      )}
    </div>
  );
}

function FieldInput({ field, value, onChange }: { field: Field; value: unknown; onChange: (v: unknown) => void }) {
  const t = useT();
  const id = React.useId();
  const label = (
    <label htmlFor={id} className="text-muted">
      {t(field.label)}
    </label>
  );
  if (field.kind === "flag") {
    return (
      <span className="flex items-center gap-1.5">
        <input id={id} type="checkbox" checked={value === true} onChange={(e) => onChange(e.target.checked)} data-field={field.name} />
        {label}
      </span>
    );
  }
  if (field.kind === "choice") {
    return (
      <span className="flex items-center gap-1.5">
        {label}
        <NativeSelect
          id={id}
          className="w-36 [&_select]:h-8"
          value={typeof value === "string" ? value : ""}
          data-field={field.name}
          onChange={(e) => onChange(e.target.value === "" ? null : e.target.value)}
        >
          {field.optional && <NativeSelectOption value="">—</NativeSelectOption>}
          {field.choices.map((c) => (
            <NativeSelectOption key={c} value={c}>
              {field.words ? t(field.words(c)) : c}
            </NativeSelectOption>
          ))}
        </NativeSelect>
      </span>
    );
  }
  return (
    <span className={field.kind === "code" ? "flex min-w-64 flex-1 items-center gap-1.5" : "flex items-center gap-1.5"}>
      {label}
      <Input
        id={id}
        type={field.kind === "date" ? "date" : "text"}
        className={field.kind === "code" ? "h-8 flex-1 font-code" : "h-8 w-48"}
        value={typeof value === "string" ? value : ""}
        spellCheck={field.kind !== "code"}
        data-field={field.name}
        onChange={(e) => onChange(e.target.value)}
      />
    </span>
  );
}

function CellAlign({ view, state }: { view: EditorView; state: EditorState }) {
  const t = useT();
  const $from = state.selection.$from;
  let cell: { node: PMNode; pos: number } | null = null;
  for (let d = $from.depth; d > 0; d--) {
    const n = $from.node(d);
    if (n.type.name === "table_cell" || n.type.name === "table_header") {
      cell = { node: n, pos: $from.before(d) };
      break;
    }
  }
  if (cell === null) return null;
  const c = cell;
  return (
    <NativeSelect
      aria-label={t("paper.table.align")}
      className="w-36 [&_select]:h-8"
      value={(c.node.attrs["align"] as string | null) ?? ""}
      data-field="align"
      onChange={(e) => view.dispatch(view.state.tr.setNodeMarkup(c.pos, undefined, { ...c.node.attrs, align: e.target.value === "" ? null : e.target.value }).setMeta(STEP, true))}
    >
      <NativeSelectOption value="">{t("paper.table.alignNone")}</NativeSelectOption>
      {ALIGNS.map((a) => (
        <NativeSelectOption key={a} value={a}>
          {t(a === "left" ? "office.tool.alignLeft" : a === "center" ? "office.tool.alignCenter" : "office.tool.alignRight")}
        </NativeSelectOption>
      ))}
    </NativeSelect>
  );
}
