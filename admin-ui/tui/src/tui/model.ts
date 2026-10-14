/* A pane is data: sections of rows, the web UI's settings vocabulary
 * (admin-ui/app/src/screens/settings/rows.tsx). A group title above, a card
 * of rows (title on the left, the value or control on the right, a detail
 * line under the title), a caption under the card. The renderer lays that out
 * into terminal lines, and the shell moves a cursor over the rows.
 *
 * Keeping it as data is what makes the panes testable without a terminal and
 * scrollable without measuring rendered output. */

import type { ColourName } from "../generated/palette";

export interface Span {
  text: string;
  color?: ColourName;
  bg?: ColourName;
  bold?: boolean;
  /** A literal colour (the logo's pixels, a meter's fill). */
  rawColor?: string;
  rawBg?: string;
}

export type Tone = "ink" | "muted" | "faint" | "accent" | "ok" | "warn" | "crit";

export interface ConfirmSpec {
  title: string;
  body: string[];
  confirm: string;
  cancel: string;
}

export type RowAction =
  | { kind: "toggle"; on: boolean; disabled?: boolean; set: (next: boolean) => void }
  | {
      kind: "choice";
      value: string;
      options: { value: string; label: string }[];
      disabled?: boolean;
      set: (next: string) => void;
    }
  | {
      kind: "edit";
      value: string;
      prompt: string;
      masked?: boolean;
      disabled?: boolean;
      submit: (next: string) => void | string | null | Promise<void | string | null>;
    }
  | {
      kind: "run";
      label: string;
      disabled?: boolean;
      confirm?: ConfirmSpec | (() => Promise<ConfirmSpec | null>);
      run: () => Promise<string | null | void>;
    };

export interface Row {
  key: string;
  title: string;
  detail?: string;
  /** Shown on the right. */
  value?: string;
  tone?: Tone;
  action?: RowAction;
  /** Lines drawn under the row's text, full width (a meter, an hour strip). */
  extra?: Span[][];
}

export interface Section {
  key: string;
  title?: string;
  /** A notice above the card, with a tone (the web UI's Alert). */
  notice?: { title?: string; text: string; tone: Tone };
  rows: Row[];
  caption?: string;
  /** Lines drawn in place of a card (a table). */
  lines?: Span[][];
}

export interface PaneModel {
  title: string;
  summary?: string;
  sections: Section[];
}

/** A laid-out line. `row` is the index into the pane's rows when the line
 *  belongs to one, so the cursor can be painted and kept in view. */
export interface Line {
  spans: Span[];
  bg: ColourName;
  row?: number;
}

export function width(text: string): number {
  return [...text].length;
}

export function truncate(text: string, max: number): string {
  if (max <= 0) return "";
  const chars = [...text];
  if (chars.length <= max) return text;
  return `${chars.slice(0, Math.max(0, max - 1)).join("")}…`;
}

/** Word wrap, hard-splitting a word longer than the width. */
export function wrap(text: string, max: number): string[] {
  if (max <= 1) return [text];
  const out: string[] = [];
  for (const paragraph of text.split("\n")) {
    let line = "";
    for (const word of paragraph.split(/\s+/).filter((w) => w.length > 0)) {
      let rest = word;
      while (width(rest) > max) {
        if (line.length > 0) {
          out.push(line);
          line = "";
        }
        out.push([...rest].slice(0, max).join(""));
        rest = [...rest].slice(max).join("");
      }
      if (line.length === 0) line = rest;
      else if (width(line) + 1 + width(rest) <= max) line = `${line} ${rest}`;
      else {
        out.push(line);
        line = rest;
      }
    }
    out.push(line);
  }
  return out;
}

const TONE: Record<Tone, ColourName> = {
  ink: "ink",
  muted: "muted",
  faint: "faint",
  accent: "accent",
  ok: "ok",
  warn: "warn",
  crit: "crit",
};

export function toneColour(tone: Tone | undefined, fallback: ColourName = "muted"): ColourName {
  return tone === undefined ? fallback : TONE[tone];
}

/** What a row's control looks like on the right. */
export function controlText(row: Row, words: { on: string; off: string }): Span[] {
  const action = row.action;
  if (action?.kind === "toggle") {
    const dim = action.disabled === true;
    return action.on
      ? [{ text: ` ● ${words.on} `, color: dim ? "faint" : "accent", bold: !dim }]
      : [{ text: ` ○ ${words.off} `, color: "faint" }];
  }
  if (action?.kind === "choice") {
    const label = action.options.find((o) => o.value === action.value)?.label ?? action.value;
    return [{ text: `‹ ${label} ›`, color: action.disabled === true ? "faint" : "ink" }];
  }
  if (action?.kind === "edit") {
    const shown = action.masked === true ? "••••••" : action.value.length > 0 ? action.value : "-";
    return [{ text: `[ ${shown} ]`, color: action.disabled === true ? "faint" : "ink" }];
  }
  if (action?.kind === "run") {
    return [{ text: `[ ${action.label} ]`, color: action.disabled === true ? "faint" : "accent", bold: action.disabled !== true }];
  }
  if (row.value !== undefined) return [{ text: row.value, color: toneColour(row.tone, "muted") }];
  return [];
}

/* Lay a pane out at `cols` wide. Rows are numbered in order across sections;
 * that number is what the shell's cursor holds. */
export function layout(
  model: PaneModel,
  cols: number,
  words: { on: string; off: string },
  cursor: number | null,
): { lines: Line[]; rowCount: number } {
  const lines: Line[] = [];
  const blank = (): void => {
    lines.push({ spans: [], bg: "ground" });
  };
  const inner = Math.max(20, cols - 4);

  lines.push({ spans: [{ text: model.title, color: "ink", bold: true }], bg: "ground" });
  if (model.summary !== undefined && model.summary.length > 0) {
    for (const text of wrap(model.summary, cols)) lines.push({ spans: [{ text, color: "muted" }], bg: "ground" });
  }

  let index = 0;
  for (const section of model.sections) {
    blank();
    if (section.title !== undefined) {
      lines.push({ spans: [{ text: section.title, color: "muted", bold: true }], bg: "ground" });
    }
    if (section.notice !== undefined) {
      const colour = toneColour(section.notice.tone, "ink");
      if (section.notice.title !== undefined) {
        lines.push({ spans: [{ text: "▌ ", color: colour }, { text: section.notice.title, color: "ink", bold: true }], bg: "sunk" });
      }
      for (const text of wrap(section.notice.text, cols - 2)) {
        lines.push({ spans: [{ text: "▌ ", color: colour }, { text, color: "muted" }], bg: "sunk" });
      }
      if (section.rows.length > 0 || section.lines !== undefined) blank();
    }
    if (section.lines !== undefined) {
      for (const spans of section.lines) lines.push({ spans, bg: "surface" });
    }
    section.rows.forEach((row, i) => {
      const selected = cursor === index;
      const bg: ColourName = selected ? "accentWash" : "surface";
      const control = controlText(row, words);
      const controlWidth = control.reduce((n, s) => n + width(s.text), 0);
      const titleRoom = Math.max(8, inner - controlWidth - 2);
      const titleLines = wrap(row.title, titleRoom);
      const marker: Span = { text: selected ? "› " : "  ", color: "accent", bold: true };
      titleLines.forEach((text, n) => {
        const spans: Span[] = [marker, { text, color: selected ? "accent" : "ink", bold: selected }];
        if (n === 0 && controlWidth > 0) {
          spans.push({ text: " ".repeat(Math.max(1, inner - width(text) - controlWidth)) });
          spans.push(...control);
        }
        lines.push({ spans: n === 0 ? spans : [{ text: "  " }, ...spans.slice(1)], bg, row: index });
      });
      if (row.detail !== undefined && row.detail.length > 0) {
        for (const text of wrap(row.detail, inner)) {
          lines.push({ spans: [{ text: "  " }, { text, color: "muted" }], bg, row: index });
        }
      }
      for (const spans of row.extra ?? []) lines.push({ spans: [{ text: "  " }, ...spans], bg, row: index });
      if (i < section.rows.length - 1) {
        lines.push({ spans: [{ text: "  " }, { text: "─".repeat(inner), color: "hair" }], bg: "surface" });
      }
      index += 1;
    });
    if (section.caption !== undefined && section.caption.length > 0) {
      for (const text of wrap(section.caption, cols)) lines.push({ spans: [{ text, color: "faint" }], bg: "ground" });
    }
  }
  return { lines, rowCount: index };
}

/** Every row of a model, flat, in cursor order. */
export function rowsOf(model: PaneModel): Row[] {
  return model.sections.flatMap((section) => section.rows);
}
