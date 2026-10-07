import { toast as sonner } from "sonner";
import { Toaster as StatusStack } from "./sonner";
import { Confirmations } from "./toaster";
import { confirm, dismissAllConfirmations } from "./use-toast";
import type { HandbookEntry } from "@/lib/handbook";

/* Status updates and confirmations, in the shape of a macOS notification.
 *
 * Two kinds, two components, one corner:
 *
 *   toast.status()  — something is happening or has to be waited for: the
 *                     box is rebuilding, is not ready for the password yet,
 *                     a payment is being finished in the other tab. Sonner
 *                     (sonner.tsx), shadcn's component for exactly this.
 *
 *   toast.success() — something is done: applied, grown, copied, unlocked,
 *   toast.error()     paid, or refused for good. shadcn's Toast (Radix,
 *   toast.done()      toaster.tsx), which the owner reads as the result.
 *
 * Both hang off the top-right corner and look the same; the confirmations
 * take the top and the status stack sits under them, never on them
 * (corner.ts). A confirmation that answers a status names it (`settles`),
 * and the status gives way the moment the result is there: "Applying your
 * changes" leaves as "Changes applied" arrives.
 *
 * Lifetimes are chosen here per kind rather than per call site, so an error
 * never leaves as fast as a success by somebody forgetting a number. A
 * status may be given an `id` and `duration: Infinity` to stay until it is
 * settled or dismissed.
 *
 * An error names its handbook page (`help`, lib/handbook.ts) and the toast
 * carries a "What to do" link to it; `index` when no page fits. Every
 * toast.error() call passes one, so no refusal leaves the owner with only
 * the box's sentence. */

export interface StatusOptions {
  /** A stable name, so a later status replaces it and a confirmation can settle it. */
  id?: string;
  /** How long it stays, in milliseconds; Infinity until settled or dismissed. */
  duration?: number;
}

export interface ConfirmOptions {
  /** How long it stays, in milliseconds. Defaults per tone. */
  duration?: number;
  /** The id of the status this confirmation answers; it is dismissed first. */
  settles?: string;
}

export interface ErrorOptions extends ConfirmOptions {
  /** The handbook page this error leads to; `index` for the chapter's start. */
  help: HandbookEntry;
}

export type ConfirmTone = "done" | "success" | "error";

/* Long enough to read a sentence, short enough that a run of three does not
 * pile up. An error gets longer: it is the one the owner may need to act on,
 * and it may carry the box's own words as a second line. */
export const STATUS_MS = 5000;
export const CONFIRM_MS: Record<ConfirmTone, number> = {
  done: 5000,
  success: 5000,
  error: 9000,
};

function status(title: string, description?: string, options: StatusOptions = {}): string | number {
  return sonner.info(title, {
    ...(options.id !== undefined ? { id: options.id } : {}),
    duration: options.duration ?? STATUS_MS,
    ...(description !== undefined && description.length > 0 ? { description } : {}),
  });
}

function settle(
  tone: ConfirmTone,
  title: string,
  description?: string,
  options: ConfirmOptions & { help?: HandbookEntry } = {},
): string {
  if (options.settles !== undefined) sonner.dismiss(options.settles);
  return confirm({
    tone,
    title,
    ...(description !== undefined && description.length > 0 ? { description } : {}),
    ...(options.help !== undefined ? { help: options.help } : {}),
    duration: options.duration ?? CONFIRM_MS[tone],
  });
}

export const toast = {
  status,
  done: (title: string, description?: string, options?: ConfirmOptions) =>
    settle("done", title, description, options),
  success: (title: string, description?: string, options?: ConfirmOptions) =>
    settle("success", title, description, options),
  error: (title: string, description: string | undefined, options: ErrorOptions) =>
    settle("error", title, description, options),
  /** Dismiss a status by its id. */
  dismiss: (id: string | number) => sonner.dismiss(id),
  dismissAll: () => {
    sonner.dismiss();
    dismissAllConfirmations();
  },
};

/** Both stacks, mounted once per shell. */
export function Toaster() {
  return (
    <>
      <Confirmations />
      <StatusStack />
    </>
  );
}
