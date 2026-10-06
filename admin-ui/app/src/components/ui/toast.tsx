import { toast as sonner } from "sonner";

/* Status updates, in the shape of a macOS notification.
 *
 * Every "saved", "failed", "applying", "copied" and "paid" in the admin UI
 * and the setup wizard goes through here: one `toast()` any module can call
 * without a hook or a provider, and one <Toaster/> mounted in each shell.
 * The stack hangs off the top-right corner, a new one slides in from the
 * right and the older ones gather behind it, each dismisses itself after a
 * few seconds (an error stays longer, and any of them waits while the
 * pointer is over the stack), and each carries a close button for whoever
 * reads faster.
 *
 * The component is shadcn's Sonner (./sonner.tsx). This file is the house
 * API over it: a title, an optional second line and an optional lifetime,
 * with the lifetime chosen per tone rather than per call site, so an error
 * never leaves as fast as a success by somebody forgetting a number.
 */

export type ToastTone = "info" | "success" | "error";

export interface ToastOptions {
  /** How long the toast stays, in milliseconds. Defaults per tone. */
  duration?: number;
}

/* Long enough to read a sentence, short enough that a run of three does
 * not pile up. An error gets longer: it is the one the owner may need to
 * act on, and it may carry the box's own words as a second line. */
export const TOAST_MS: Record<ToastTone, number> = {
  info: 5000,
  success: 5000,
  error: 9000,
};

function show(tone: ToastTone, title: string, description?: string, options: ToastOptions = {}) {
  const data = {
    duration: options.duration ?? TOAST_MS[tone],
    ...(description !== undefined && description.length > 0 ? { description } : {}),
  };
  return sonner[tone](title, data);
}

export const toast = Object.assign(
  (title: string, description?: string, options?: ToastOptions) =>
    show("info", title, description, options),
  {
    info: (title: string, description?: string, options?: ToastOptions) =>
      show("info", title, description, options),
    success: (title: string, description?: string, options?: ToastOptions) =>
      show("success", title, description, options),
    error: (title: string, description?: string, options?: ToastOptions) =>
      show("error", title, description, options),
    dismiss: (id: string | number) => sonner.dismiss(id),
    dismissAll: () => sonner.dismiss(),
  },
);

export { Toaster } from "./sonner";
