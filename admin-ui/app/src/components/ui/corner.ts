import { useSyncExternalStore } from "react";

/* The top-right corner is shared by two stacks: the confirmations (shadcn's
 * Toast, toaster.tsx) at the very top, and the status updates (Sonner,
 * sonner.tsx) under them. So that neither ever lands on the other, the
 * confirmation list reports its height here as it changes, and the Sonner
 * stack offsets itself by it. A store rather than context, because the
 * two live in different React trees of the same shell and the house toast
 * API is a plain function any module calls. */

let height = 0;
const listeners = new Set<() => void>();

function subscribe(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

/** Called by the confirmation list whenever its box changes size. */
export function reportConfirmationsHeight(px: number): void {
  const rounded = Math.round(px);
  if (rounded === height) return;
  height = rounded;
  for (const listener of listeners) listener();
}

/** The confirmation list's current height in CSS pixels, 0 when empty. */
export function useConfirmationsHeight(): number {
  return useSyncExternalStore(
    subscribe,
    () => height,
    () => 0,
  );
}
