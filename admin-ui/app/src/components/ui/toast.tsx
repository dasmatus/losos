import * as React from "react";
import { HugeiconsIcon } from "@hugeicons/react";
import {
  Alert02Icon,
  Cancel01Icon,
  CheckmarkCircle02Icon,
  InformationCircleIcon,
} from "@hugeicons/core-free-icons";
import { useT } from "@/lib/i18n-react";
import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";

/* Status updates, in the shape of a macOS notification.
 *
 * Every "saved", "failed", "applying", "copied" and "paid" in the admin UI
 * and the setup wizard goes through here: one `toast()` any module can call
 * without a hook or a provider, and one <Toaster/> mounted in each shell.
 * The stack hangs off the top-right corner, a new one slides in from the
 * right and pushes the older ones down, each dismisses itself after a few
 * seconds (an error stays longer, and any of them waits while the pointer
 * is over it), and each carries a close button for whoever reads faster.
 *
 * Hand-rolled rather than sonner: sonner positions its stack with inline
 * styles and injects a stylesheet at runtime, and the appliance CSP
 * (`style-src 'self'`) refuses both. Motion is two keyframes in
 * src/styles/index.css, and the stylesheet's prefers-reduced-motion block
 * switches them off like every other animation in the app, so a toast then
 * simply appears and disappears.
 */

export type ToastTone = "info" | "success" | "error";

export interface ToastOptions {
  /** How long the toast stays, in milliseconds. Defaults per tone. */
  duration?: number;
}

export interface Toast {
  id: number;
  tone: ToastTone;
  title: string;
  description?: string;
  /** The exit animation is running; the entry is gone on its next tick. */
  leaving: boolean;
}

/* Long enough to read a sentence, short enough that a run of three does
 * not pile up. An error gets longer: it is the one the owner may need to
 * act on, and it may carry the box's own words as a second line. */
export const TOAST_MS: Record<ToastTone, number> = {
  info: 5000,
  success: 5000,
  error: 9000,
};

/** The exit keyframe's length. The entry is removed from the store after it. */
const EXIT_MS = 240;

/* A sixth toast would push the stack below the fold on a phone. The oldest
 * goes when a new one arrives past this. */
const MAX_SHOWN = 5;

let toasts: Toast[] = [];
let nextId = 1;
const listeners = new Set<() => void>();

/* One countdown per toast, pausable: `expiresAt` is when it would fire,
 * `remaining` is what was left when the pointer paused it. */
interface Countdown {
  timer: number | null;
  remaining: number;
  startedAt: number;
}
const countdowns = new Map<number, Countdown>();

function emit(): void {
  for (const fn of listeners) fn();
}

function startCountdown(id: number, ms: number): void {
  const timer = window.setTimeout(() => dismiss(id), ms);
  countdowns.set(id, { timer, remaining: ms, startedAt: Date.now() });
}

export function pause(id: number): void {
  const countdown = countdowns.get(id);
  if (countdown === undefined || countdown.timer === null) return;
  window.clearTimeout(countdown.timer);
  countdown.remaining = Math.max(0, countdown.remaining - (Date.now() - countdown.startedAt));
  countdown.timer = null;
}

export function resume(id: number): void {
  const countdown = countdowns.get(id);
  if (countdown === undefined || countdown.timer !== null) return;
  // A toast the pointer rested on stays a moment after it leaves, so the
  // close button does not vanish from under a hand reaching for it.
  const ms = Math.max(countdown.remaining, 1200);
  countdown.timer = window.setTimeout(() => dismiss(id), ms);
  countdown.remaining = ms;
  countdown.startedAt = Date.now();
}

function remove(id: number): void {
  const next = toasts.filter((t) => t.id !== id);
  if (next.length === toasts.length) return;
  toasts = next;
  emit();
}

function push(tone: ToastTone, title: string, description?: string, options: ToastOptions = {}): number {
  const id = nextId++;
  const entry: Toast =
    description === undefined || description.length === 0
      ? { id, tone, title, leaving: false }
      : { id, tone, title, description, leaving: false };

  const shown = toasts.filter((t) => !t.leaving);
  if (shown.length >= MAX_SHOWN) {
    const oldest = shown[0];
    if (oldest !== undefined) dismiss(oldest.id);
  }

  toasts = [...toasts, entry];
  emit();
  startCountdown(id, options.duration ?? TOAST_MS[tone]);
  return id;
}

/** Start the exit; the entry leaves the store once the animation has run. */
export function dismiss(id: number): void {
  const countdown = countdowns.get(id);
  if (countdown !== undefined) {
    if (countdown.timer !== null) window.clearTimeout(countdown.timer);
    countdowns.delete(id);
  }
  const entry = toasts.find((t) => t.id === id);
  if (entry === undefined || entry.leaving) return;
  toasts = toasts.map((t) => (t.id === id ? { ...t, leaving: true } : t));
  emit();
  window.setTimeout(() => remove(id), EXIT_MS);
}

export function dismissAll(): void {
  for (const entry of toasts) dismiss(entry.id);
}

export const toast = Object.assign(
  (title: string, description?: string, options?: ToastOptions) =>
    push("info", title, description, options),
  {
    info: (title: string, description?: string, options?: ToastOptions) =>
      push("info", title, description, options),
    success: (title: string, description?: string, options?: ToastOptions) =>
      push("success", title, description, options),
    error: (title: string, description?: string, options?: ToastOptions) =>
      push("error", title, description, options),
    dismiss,
    dismissAll,
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

const TONE_TEXT = {
  info: "text-accent",
  success: "text-ok",
  error: "text-crit",
} as const;

const TONE_STRIPE = {
  info: "bg-accent",
  success: "bg-ok",
  error: "bg-crit",
} as const;

/** Mount once per shell: the app's and the wizard's. */
export function Toaster() {
  const t = useT();
  const items = React.useSyncExternalStore(
    subscribe,
    () => toasts,
    () => toasts,
  );

  // Newest on top, the way the corner of a Mac stacks them.
  const ordered = React.useMemo(() => [...items].reverse(), [items]);

  return (
    <section
      aria-label={t("ui.notifications")}
      /* Polite, not assertive: these report what just finished, and an
         assertive region interrupts whatever the owner was reading. An
         error is the exception and says so on its own element. */
      aria-live="polite"
      data-testid="toaster"
      className={cn(
        "pointer-events-none fixed top-3 right-3 left-3 z-[60]",
        "flex flex-col items-stretch gap-2 sm:top-4 sm:right-4 sm:left-auto sm:w-[22rem]",
      )}
    >
      {ordered.map((item) => (
        <ToastCard key={item.id} item={item} closeLabel={t("ui.dismiss")} />
      ))}
    </section>
  );
}

function ToastCard({ item, closeLabel }: { item: Toast; closeLabel: string }) {
  return (
    <div
      role={item.tone === "error" ? "alert" : "status"}
      data-toast=""
      data-tone={item.tone}
      data-leaving={item.leaving ? "" : undefined}
      onMouseEnter={() => pause(item.id)}
      onMouseLeave={() => resume(item.id)}
      onFocus={() => pause(item.id)}
      onBlur={() => resume(item.id)}
      className={cn(
        "pointer-events-auto relative flex w-full items-start gap-3 overflow-hidden",
        "rounded-card border border-line bg-surface/95 p-3 pr-2 shadow-pop backdrop-blur-md",
        item.leaving ? "animate-toast-out" : "animate-toast-in",
      )}
    >
      <span
        aria-hidden="true"
        className={cn("absolute inset-y-2.5 left-0 w-[3px] rounded-r-full", TONE_STRIPE[item.tone])}
      />
      <HugeiconsIcon
        icon={TONE_ICON[item.tone]}
        size={20}
        strokeWidth={1.5}
        color="currentColor"
        className={cn("mt-0.5 shrink-0", TONE_TEXT[item.tone])}
        aria-hidden="true"
      />
      <div className="min-w-0 flex-1 py-0.5">
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
        className="size-7 shrink-0"
        aria-label={closeLabel}
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
  );
}
