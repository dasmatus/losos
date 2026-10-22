/* What a right-click menu holds, and which part of a page answers one.
 *
 * Anything a person can right-click describes its menu as data: sections
 * of items, an item either a command, a submenu or a choice of one value.
 * components/context-menu.tsx draws it. An element takes part by its ref:
 *
 *   <li ref={contextMenu(() => ({ label: name, sections: [[open, pin]] }))}>
 *
 * The innermost element with a menu answers, so a file row answers before
 * the folder around it and the folder before the page. An item the box
 * cannot do right now is `hidden` (or left out): a menu lists what can be
 * done, it does not grey out what cannot. `disabled` is for the passing
 * reason, a full sidebar, with a `title` that says it.
 *
 * Parts of LosOS that act on another part's things add their items with
 * `addMenuItems`, as openers do in cloud/openers.ts: a subject names the
 * thing ("cloud.entry"), and whoever draws that thing's menu asks
 * `menuItemsFor` for the extra section. */

import type { IconSvgElement } from "@hugeicons/react";

export interface MenuCommand {
  kind?: "command";
  id: string;
  label: string;
  icon?: IconSvgElement;
  /** The keys that do the same outside the menu, as `keyCaps` spells them. */
  keys?: readonly string[];
  /** Red, for what loses something. */
  danger?: boolean;
  disabled?: boolean;
  title?: string;
  hidden?: boolean;
  onSelect: () => void;
}

export interface MenuSubmenu {
  kind: "submenu";
  id: string;
  label: string;
  icon?: IconSvgElement;
  hidden?: boolean;
  items: readonly MenuItem[];
}

export interface MenuChoice {
  kind: "choice";
  id: string;
  label: string;
  icon?: IconSvgElement;
  hidden?: boolean;
  value: string;
  options: readonly { value: string; label: string; icon?: IconSvgElement }[];
  onChange: (value: string) => void;
}

export type MenuItem = MenuCommand | MenuSubmenu | MenuChoice;

/** A run of items between two separators. */
export type MenuSection = readonly MenuItem[];

export interface MenuSpec {
  /** What was right-clicked, the menu's accessible name. */
  label: string;
  sections: readonly MenuSection[];
}

/** Where the menu was asked for. */
export interface MenuHit {
  target: Element;
  /** Opened with Shift+F10 or the Menu key rather than the mouse. */
  keyboard: boolean;
}

export type MenuBuilder = (hit: MenuHit) => MenuSpec | null;

// ── Who answers ───────────────────────────────────────────────────────────

const builders = new WeakMap<Element, MenuBuilder>();

/** A ref that gives its element a right-click menu. */
export function contextMenu(build: MenuBuilder): (element: Element | null) => () => void {
  return (element) => {
    if (element !== null) builders.set(element, build);
    return () => {
      if (element !== null && builders.get(element) === build) builders.delete(element);
    };
  };
}

/** The menus from `target` outwards, innermost first. */
export function buildersAt(target: Element): MenuBuilder[] {
  const out: MenuBuilder[] = [];
  for (let el: Element | null = target; el !== null; el = el.parentElement) {
    const build = builders.get(el);
    if (build !== undefined) out.push(build);
  }
  return out;
}

/** Drop what is hidden, the submenus left with nothing, and empty sections. */
export function visibleSections(sections: readonly MenuSection[]): MenuSection[] {
  const keep = (items: readonly MenuItem[]): MenuItem[] =>
    items.flatMap((item): MenuItem[] => {
      if (item.hidden === true) return [];
      if (item.kind === "submenu") {
        const inner = keep(item.items);
        return inner.length === 0 ? [] : [{ ...item, items: inner }];
      }
      return [item];
    });
  return sections.map(keep).filter((section) => section.length > 0);
}

// ── Other parts' items ────────────────────────────────────────────────────

/** The things other parts add items to, and what each one hands them. Each
 *  part that draws a menu declares its subject here by declaration merging. */
export interface MenuSubjects {}

type Contributor<K extends keyof MenuSubjects> = (subject: MenuSubjects[K]) => MenuSection;

const contributors = new Map<string, Contributor<never>[]>();

export function addMenuItems<K extends keyof MenuSubjects>(subject: K, items: Contributor<K>): void {
  const list = contributors.get(subject) ?? [];
  list.push(items as Contributor<never>);
  contributors.set(subject, list);
}

/** What the other parts add for this thing, as one section. */
export function menuItemsFor<K extends keyof MenuSubjects>(subject: K, value: MenuSubjects[K]): MenuSection {
  return (contributors.get(subject) ?? []).flatMap((items) => (items as Contributor<K>)(value));
}

// ── Keys ──────────────────────────────────────────────────────────────────

export function onMac(): boolean {
  const platform =
    (navigator as Navigator & { userAgentData?: { platform?: string } }).userAgentData?.platform ?? navigator.platform;
  return /mac|iphone|ipad/i.test(platform);
}

const MAC: Record<string, string> = { Mod: "⌘", Shift: "⇧", Alt: "⌥", Enter: "↩", Delete: "⌫", Left: "←", Right: "→" };
const PC: Record<string, string> = { Mod: "Ctrl", Shift: "⇧", Alt: "Alt", Enter: "Enter", Delete: "Del", Left: "←", Right: "→" };

/** One key cap per key, as this machine writes them: "Mod" is Ctrl or ⌘. */
export function keyCaps(keys: readonly string[]): string[] {
  const names = onMac() ? MAC : PC;
  return keys.map((key) => names[key] ?? key);
}

/** The same keys for aria-keyshortcuts. */
export function ariaKeys(keys: readonly string[]): string {
  const mac = onMac();
  const names: Record<string, string> = {
    Mod: mac ? "Meta" : "Control",
    Left: "ArrowLeft",
    Right: "ArrowRight",
  };
  return keys.map((key) => names[key] ?? key).join("+");
}

// ── Where the menu stays out of the way ───────────────────────────────────

/** A place that takes text keeps the browser's own menu: spelling, paste. */
export function takesText(target: Element): boolean {
  const field = target.closest("input, textarea, select, [contenteditable]");
  if (field === null) return false;
  if (field instanceof HTMLInputElement) {
    return !["button", "checkbox", "radio", "range", "color", "file", "submit", "reset", "image"].includes(field.type);
  }
  if (field instanceof HTMLElement && field.hasAttribute("contenteditable")) return field.isContentEditable;
  return true;
}

/** The text picked on the page, when the click landed on it. */
export function pickedText(target: Element): string {
  const selection = window.getSelection();
  if (selection === null || selection.isCollapsed || selection.rangeCount === 0) return "";
  const text = selection.toString().trim();
  if (text === "") return "";
  return selection.getRangeAt(0).intersectsNode(target) ? text : "";
}

/** Copy, with the old way where the page is not a secure context. */
export async function copyText(text: string): Promise<boolean> {
  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch {
    const area = document.createElement("textarea");
    area.value = text;
    area.setAttribute("readonly", "");
    area.className = "fixed -left-[9999px] top-0";
    document.body.append(area);
    area.select();
    const done = document.execCommand("copy");
    area.remove();
    return done;
  }
}
