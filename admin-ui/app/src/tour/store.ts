/* Whether the tour is showing, and whether this browser still owes it.
 *
 * The tour opens once, right after the setup wizard: the shell calls
 * `startTour()` from the wizard's `onDone`, which also writes "pending" to
 * localStorage so a reload while it is up brings it back. Finishing it or
 * skipping it writes "seen", and nothing opens it by itself again; the About
 * pane's "Take the tour" opens it on request.
 *
 * Per browser on purpose. The flag is a convenience, not a record: an owner
 * who signs in from a second laptop is not shown the tour unasked, and a
 * browser that refuses storage just never reopens it after a reload. Same
 * store shape as lib/theme.ts, so useSyncExternalStore can bind it. */

const KEY = "losos-tour";

function read(): string | null {
  try {
    return window.localStorage.getItem(KEY);
  } catch {
    return null;
  }
}

function write(value: "pending" | "seen"): void {
  try {
    window.localStorage.setItem(KEY, value);
  } catch {
    // Storage refused (a private window): the tour still opens and closes,
    // it just will not survive a reload.
  }
}

let open = typeof window !== "undefined" && read() === "pending";
const listeners = new Set<() => void>();

function set(next: boolean): void {
  if (next === open) return;
  open = next;
  for (const listener of listeners) listener();
}

export function subscribeTour(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

export function isTourOpen(): boolean {
  return open;
}

/** The wizard has just finished: show the tour, and keep owing it until it is closed. */
export function startTour(): void {
  write("pending");
  set(true);
}

/** Open it again on request (the About pane). */
export function openTour(): void {
  set(true);
}

/** Finished or skipped: it does not open by itself again in this browser. */
export function closeTour(): void {
  write("seen");
  set(false);
}
