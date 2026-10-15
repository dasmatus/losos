/* Whether the command palette is open, and what it opens with.
 *
 * A module-level store rather than React context, the same shape as the
 * tour's (src/tour/store.ts): the sidebar's search box, the Ctrl/Cmd+K
 * shortcut and the palette itself sit in different branches of the tree, and
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

/** Ctrl+K, or Cmd+K on a Mac. The same pair the sidebar uses for Ctrl/Cmd+B. */
export function isPaletteShortcut(event: KeyboardEvent): boolean {
  return (event.ctrlKey || event.metaKey) && !event.altKey && !event.shiftKey && event.key.toLowerCase() === "k";
}

/** How the shortcut is written on this machine, for the hint in the box. */
export function shortcutLabel(): string {
  const platform =
    (navigator as Navigator & { userAgentData?: { platform?: string } }).userAgentData?.platform ?? navigator.platform;
  return /mac|iphone|ipad/i.test(platform) ? "⌘K" : "Ctrl K";
}
