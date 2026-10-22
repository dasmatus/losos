/* What the bar that floats over a selection offers: picked text in LosOS
 * Office and Notes, cards and arrows in LosOS Diagrams.
 *
 * Like the right-click menu (lib/context-menu.ts), the bar is described as
 * data and drawn by one component, components/quick-actions.tsx: sections
 * of actions, each a button, a choice of one value (a colour), a short
 * menu of commands (the alignments) or a prompt that takes the bar's place (a link, a mention).
 * Each app builds the sections for its own selection; the parts of LosOS
 * that act on another app's things add theirs with `addQuickActions`, the
 * same way `addMenuItems` adds to a menu:
 *
 *   addQuickActions("text.selection", (picked) => [{ id: "send-mail", … }]);
 *
 * An action the box cannot do right now is `hidden`, never greyed. */

import type { IconSvgElement } from "@hugeicons/react";

interface Base {
  id: string;
  /** The button's accessible name and its tooltip. */
  label: string;
  icon: IconSvgElement;
  hidden?: boolean;
}

export interface QuickCommand extends Base {
  kind?: "command";
  /** The keys that do the same outside the bar, as `keyCaps` spells them. */
  keys?: readonly string[];
  /** Shown pressed: the selection already has it (bold text). */
  pressed?: boolean;
  /** Red, for what loses something. */
  danger?: boolean;
  onSelect: () => void;
}

export interface QuickChoice extends Base {
  kind: "choice";
  value: string;
  options: readonly {
    value: string;
    label: string;
    icon?: IconSvgElement;
    /** A CSS colour drawn as a dot, for a colour picker. */
    swatch?: string;
  }[];
  onChange: (value: string) => void;
}

/** A button that opens a short list of commands (align lefts, centres…). */
export interface QuickMenu extends Base {
  kind: "menu";
  items: readonly { id: string; label: string; icon?: IconSvgElement; onSelect: () => void }[];
}

/** One line a prompt offers under its field. */
export interface QuickSuggestion {
  id: string;
  label: string;
  detail?: string;
  icon?: IconSvgElement;
}

export interface QuickPrompt extends Base {
  kind: "prompt";
  placeholder: string;
  /** What the field starts with. */
  initial?: string;
  /** Lines to pick from for what is typed so far. */
  suggest?: (query: string, signal: AbortSignal) => readonly QuickSuggestion[] | Promise<readonly QuickSuggestion[]>;
  /** Enter on the field, or a picked line. */
  onSubmit: (value: string, picked?: QuickSuggestion) => void;
}

export type QuickAction = QuickCommand | QuickChoice | QuickMenu | QuickPrompt;

/** A run of actions between two rules. */
export type QuickSection = readonly QuickAction[];

export function visibleQuickSections(sections: readonly QuickSection[]): QuickSection[] {
  return sections.map((section) => section.filter((action) => action.hidden !== true)).filter((section) => section.length > 0);
}

// ── Other parts' actions ──────────────────────────────────────────────────

/** The selections other parts add actions to, and what each one hands them.
 *  Each app that draws a bar declares its subject by declaration merging. */
export interface QuickSubjects {
  /** Text picked in a document or a note. */
  "text.selection": {
    text: string;
    /** Where it was picked. */
    app: "office" | "notes";
    /** The file it is in, a path in LosOS Cloud. */
    path?: string;
  };
}

type Contributor<K extends keyof QuickSubjects> = (subject: QuickSubjects[K]) => QuickSection;

const contributors = new Map<string, Contributor<never>[]>();

export function addQuickActions<K extends keyof QuickSubjects>(subject: K, actions: Contributor<K>): void {
  const list = contributors.get(subject) ?? [];
  list.push(actions as Contributor<never>);
  contributors.set(subject, list);
}

/** What the other parts add for this selection, as one section. */
export function quickActionsFor<K extends keyof QuickSubjects>(subject: K, value: QuickSubjects[K]): QuickSection {
  return (contributors.get(subject) ?? []).flatMap((actions) => (actions as Contributor<K>)(value));
}

// ── Where it goes ─────────────────────────────────────────────────────────

/** The keys that put the focus on the bar, as the toolbar pattern has it. */
export const QUICK_KEYS = ["Alt", "F10"] as const;

export function isQuickKey(event: KeyboardEvent): boolean {
  return event.key === "F10" && event.altKey && !event.ctrlKey && !event.metaKey && !event.shiftKey;
}

/** Where the bar sits for a selection drawn at `rect`, a bar of `w` by `h`,
 *  in a window of `vw` by `vh`: centred above it, or below when there is no
 *  room above, and always inside the window by `pad`. */
export function placeBar(
  rect: { left: number; top: number; right: number; bottom: number },
  w: number,
  h: number,
  vw: number,
  vh: number,
  gap = 8,
  pad = 8,
): { x: number; y: number; side: "top" | "bottom" } {
  const above = rect.top - gap - h;
  const side = above >= pad ? "top" : "bottom";
  const y = side === "top" ? above : Math.min(rect.bottom + gap, vh - h - pad);
  const centre = (rect.left + rect.right) / 2;
  const x = Math.max(pad, Math.min(centre - w / 2, vw - w - pad));
  return { x: Math.round(x), y: Math.round(y), side };
}
