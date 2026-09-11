import * as React from "react";
import { HugeiconsIcon } from "@hugeicons/react";
import {
  Alert02Icon,
  Cancel01Icon,
  CheckmarkCircle02Icon,
  InformationCircleIcon,
} from "@hugeicons/core-free-icons";
import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";

/* A toast stack, sonner-shaped but hand-rolled.
 *
 * sonner positions its stack with inline styles and ships a stylesheet it
 * injects at runtime; both are refused by `style-src 'self'`. The API here is
 * the subset worth keeping: a module-level `toast()` any module can call
 * without a hook or a provider, and one <Toaster/> mounted in the shell. */

export type ToastTone = "info" | "success" | "error";

export interface Toast {
  id: number;
  tone: ToastTone;
  title: string;
  description?: string;
}

const DISMISS_MS = 6000;

let toasts: Toast[] = [];
let nextId = 1;
const listeners = new Set<() => void>();

function emit(): void {
  for (const fn of listeners) fn();
}

function push(tone: ToastTone, title: string, description?: string): number {
  const id = nextId++;
  const entry: Toast = description === undefined ? { id, tone, title } : { id, tone, title, description };
  toasts = [...toasts, entry];
  emit();
  window.setTimeout(() => dismiss(id), DISMISS_MS);
  return id;
}

export function dismiss(id: number): void {
  const next = toasts.filter((t) => t.id !== id);
  if (next.length === toasts.length) return;
  toasts = next;
  emit();
}

export const toast = Object.assign(
  (title: string, description?: string) => push("info", title, description),
  {
    info: (title: string, description?: string) => push("info", title, description),
    success: (title: string, description?: string) => push("success", title, description),
    error: (title: string, description?: string) => push("error", title, description),
    dismiss,
  },
);

function subscribe(onChange: () => void): () => void {
  listeners.add(onChange);
  return () => {
    listeners.delete(onChange);
  };
}

const TONE_ICON = {
  info: InformationCircleIcon,
  success: CheckmarkCircle02Icon,
  error: Alert02Icon,
} as const;

const TONE_CLASS = {
  info: "text-accent",
  success: "text-ok",
  error: "text-crit",
} as const;

/** Mount once, in the app shell. */
export function Toaster() {
  const items = React.useSyncExternalStore(
    subscribe,
    () => toasts,
    () => toasts,
  );

  return (
    <div
      // Polite, not assertive: these report what just finished, and an
      // assertive region interrupts whatever the user was reading.
      aria-live="polite"
      aria-atomic="false"
      className={cn(
        "pointer-events-none fixed inset-x-0 bottom-0 z-50",
        "flex flex-col items-center gap-2 p-4 sm:items-end",
      )}
    >
      {items.map((item) => (
        <div
          key={item.id}
          className={cn(
            "pointer-events-auto flex w-full max-w-sm items-start gap-3",
            "animate-rise rounded-card border border-line bg-surface p-3 pr-2 shadow-pop",
          )}
        >
          <HugeiconsIcon
            icon={TONE_ICON[item.tone]}
            size={20}
            strokeWidth={1.5}
            color="currentColor"
            className={cn("mt-0.5", TONE_CLASS[item.tone])}
            aria-hidden="true"
          />
          <div className="min-w-0 flex-1">
            <p className="text-[13.5px] leading-snug font-medium text-ink">{item.title}</p>
            {item.description !== undefined && (
              <p className="mt-0.5 text-[12.5px] leading-snug break-words text-muted">
                {item.description}
              </p>
            )}
          </div>
          <Button
            variant="ghost"
            size="icon"
            className="size-7"
            aria-label="Dismiss"
            onClick={() => dismiss(item.id)}
          >
            <HugeiconsIcon
              icon={Cancel01Icon}
              size={16}
              strokeWidth={1.5}
              color="currentColor"
              aria-hidden="true"
            />
          </Button>
        </div>
      ))}
    </div>
  );
}
