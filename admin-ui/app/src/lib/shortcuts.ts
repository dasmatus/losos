/* Keyboard shortcuts every page shares.
 *
 * Signing out is Alt+Shift+Q (Option+Shift+Q on a Mac): no browser binds it,
 * the palette's Ctrl/Cmd+Shift+P does not overlap it, and Q for quit is the
 * letter people look for. The match is on the physical key (`code`), because
 * Option+Shift+Q types "Œ" on a Mac and `key` would never be "Q". It works in
 * text fields too: with Alt held it types nothing anyone means. */

export function isMac(): boolean {
  return typeof navigator !== "undefined" && /Mac|iPhone|iPad/.test(navigator.platform || navigator.userAgent);
}

export function isSignOutShortcut(event: KeyboardEvent): boolean {
  return event.altKey && event.shiftKey && !event.ctrlKey && !event.metaKey && event.code === "KeyQ";
}

/** The keys as this computer's keyboard labels them. */
export function signOutKeys(): readonly string[] {
  return isMac() ? ["⌥", "⇧", "Q"] : ["Alt", "Shift", "Q"];
}
