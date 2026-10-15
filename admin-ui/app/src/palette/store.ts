/* Whether the command palette is open, and what it opens with.
 *
 * A module-level store rather than React context, the same shape as the
 * tour's (src/tour/store.ts): the sidebar's search button, the two keyboard
 * shortcuts and the palette itself sit in different branches of the tree, and
 * none of them should re-render the shell to say "open". */

interface PaletteState {
  readonly open: boolean;
  /** What the input holds when it opens: whatever was typed into the
   *  sidebar's box before it handed over. */
  readonly query: string;
  /** Counts openings, so each one starts from a fresh body. */
  readonly serial: number;
}

let state: PaletteState = { open: false, query: "", serial: 0 };
const listeners = new Set<() => void>();

function set(next: PaletteState): void {
  state = next;
  for (const listener of listeners) listener();
}

export function subscribePalette(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

export function paletteState(): PaletteState {
  return state;
}

export function openPalette(query = ""): void {
  set({ open: true, query, serial: state.serial + 1 });
}

export function closePalette(): void {
  if (state.open) set({ ...state, open: false, query: "" });
}

/* Ctrl+Shift+P, or Cmd+Shift+P on a Mac, as in code editors; and a colon
 * typed anywhere but a text field. The colon is also the way in where the
 * browser keeps Ctrl+Shift+P for itself (a private window in Firefox). */

/** The chord, which opens and closes the palette. */
export function isPaletteShortcut(event: KeyboardEvent): boolean {
  return (event.ctrlKey || event.metaKey) && event.shiftKey && !event.altKey && event.key.toLowerCase() === "p";
}

/** A colon typed outside anything that takes text: opens the palette. */
export function isPaletteColon(event: KeyboardEvent): boolean {
  if (event.key !== ":" || event.ctrlKey || event.metaKey || event.altKey || event.isComposing) return false;
  return !takesText(event.target);
}

function takesText(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  if (target.isContentEditable) return true;
  if (target instanceof HTMLTextAreaElement || target instanceof HTMLSelectElement) return true;
  if (!(target instanceof HTMLInputElement)) return false;
  return !["button", "checkbox", "radio", "range", "color", "file", "submit", "reset", "image"].includes(target.type);
}

function onMac(): boolean {
  const platform =
    (navigator as Navigator & { userAgentData?: { platform?: string } }).userAgentData?.platform ?? navigator.platform;
  return /mac|iphone|ipad/i.test(platform);
}

/** How the chord is written on this machine, for the hint in the box. */
export function shortcutLabel(): string {
  return onMac() ? "⇧⌘P" : "Ctrl ⇧ P";
}

/** The same, for aria-keyshortcuts. */
export const SHORTCUT_ARIA = "Control+Shift+P Meta+Shift+P :";
