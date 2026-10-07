import { useSyncExternalStore } from "react";
import type { HandbookEntry } from "@/lib/handbook";

/* The confirmation store behind shadcn's Toast (toaster.tsx): the list of
 * "done" messages on screen, newest first. shadcn's own use-toast.ts is the
 * model, trimmed: no update() or action buttons, a cap of five rather than
 * one, and a lifetime chosen per tone by the house API (toast.tsx) rather
 * than a near-infinite default. Radix owns the countdown, the pause while
 * the pointer rests on the stack, the swipe and the exit animation; this
 * store only knows which toasts exist and whether each is still open. */

export type ConfirmationTone = "done" | "success" | "error";

export interface Confirmation {
  id: string;
  tone: ConfirmationTone;
  title: string;
  description?: string;
  /** Milliseconds before Radix closes it; Infinity never does. */
  duration: number;
  /** The handbook page an error leads to, as a "What to do" link. */
  help?: HandbookEntry;
  open: boolean;
}

/* Five is a stack, six is a wall. The oldest is closed to make room. */
const LIMIT = 5;
/* Long enough for the exit animation (240ms) to finish before the entry is
 * dropped; under reduced motion Radix unmounts at once and this just lags. */
const EXIT_MS = 400;

let items: readonly Confirmation[] = [];
let sequence = 0;
const listeners = new Set<() => void>();

function emit(next: readonly Confirmation[]): void {
  items = next;
  for (const listener of listeners) listener();
}

function subscribe(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

export function confirm(input: Omit<Confirmation, "id" | "open">): string {
  sequence += 1;
  const id = `confirmation-${sequence}`;
  const open = items.filter((item) => item.open);
  for (const stale of open.slice(LIMIT - 1)) dismissConfirmation(stale.id);
  emit([{ ...input, id, open: true }, ...items]);
  return id;
}

export function dismissConfirmation(id: string): void {
  if (!items.some((item) => item.id === id && item.open)) return;
  emit(items.map((item) => (item.id === id ? { ...item, open: false } : item)));
  window.setTimeout(() => emit(items.filter((item) => item.id !== id)), EXIT_MS);
}

export function dismissAllConfirmations(): void {
  for (const item of items) dismissConfirmation(item.id);
}

export function useConfirmations(): readonly Confirmation[] {
  return useSyncExternalStore(
    subscribe,
    () => items,
    () => items,
  );
}
