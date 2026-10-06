import type * as React from "react";
import * as ToastPrimitives from "@radix-ui/react-toast";
import { cva, type VariantProps } from "class-variance-authority";
import { HugeiconsIcon } from "@hugeicons/react";
import { Cancel01Icon } from "@hugeicons/core-free-icons";
import { cn } from "@/lib/utils";

/* shadcn/ui's Toast, on LosOS tokens: the Radix Toast primitives wrapped
 * with the house classes, the file shadcn ships as components/ui/toast.tsx.
 * (Here it is toast-primitives.tsx because toast.tsx is the house API every
 * screen calls.)
 *
 * This is the one Radix primitive in src/components/ui, and it is here
 * because it survives the appliance CSP (style-src 'self'): Radix Toast
 * injects no <style> element and pulls no scroll lock, and the few values it
 * sets at runtime (the swipe offset as --radix-toast-swipe-move-x) go
 * through React's style prop, a CSSOM write the policy allows. The dialog,
 * menus and sheets stay Radix-free for the reasons in button.tsx.
 *
 * The look is the same as the status stack's (sonner.tsx, styled in
 * index.css): the plain surface, the card radius, the pop shadow, the tone
 * as the icon's colour and an inset stripe down the left edge, and a round
 * close button hanging off the top-left corner. Motion is the slide from the
 * right edge (toast-in / toast-out in index.css); prefers-reduced-motion is
 * the stylesheet's global rule, and Radix then unmounts without waiting for
 * an animation that never runs. */

const ToastProvider = ToastPrimitives.Provider;

function ToastViewport({ className, ...props }: React.ComponentProps<typeof ToastPrimitives.Viewport>) {
  return (
    <ToastPrimitives.Viewport
      className={cn(
        "fixed top-4 right-4 z-[2147483000] m-0 flex w-[356px] max-w-[calc(100vw-2rem)] list-none flex-col gap-2.5 p-0 outline-none",
        "max-sm:top-3 max-sm:right-3 max-sm:w-[calc(100vw-1.5rem)] max-sm:max-w-none",
        className,
      )}
      {...props}
    />
  );
}

const toastVariants = cva(
  cn(
    "group pointer-events-auto relative flex w-full items-start gap-2.5 rounded-card border border-line bg-surface py-3 pr-3.5 pl-[18px] text-ink",
    "shadow-[inset_3px_0_0_0_var(--tone),var(--elev-pop)]",
    "data-[state=open]:animate-toast-in data-[state=closed]:animate-toast-out",
    "data-[swipe=move]:translate-x-[var(--radix-toast-swipe-move-x)] data-[swipe=move]:transition-none",
    "data-[swipe=cancel]:translate-x-0 data-[swipe=cancel]:transition-transform data-[swipe=cancel]:duration-200",
    "data-[swipe=end]:animate-toast-out",
  ),
  {
    variants: {
      tone: {
        done: "[--tone:var(--accent)]",
        success: "[--tone:var(--ok)]",
        error: "[--tone:var(--crit)]",
      },
    },
    defaultVariants: { tone: "done" },
  },
);

type ToastProps = React.ComponentProps<typeof ToastPrimitives.Root> & VariantProps<typeof toastVariants>;

function Toast({ className, tone, ...props }: ToastProps) {
  return <ToastPrimitives.Root className={cn(toastVariants({ tone }), className)} {...props} />;
}

function ToastTitle({ className, ...props }: React.ComponentProps<typeof ToastPrimitives.Title>) {
  return (
    <ToastPrimitives.Title
      className={cn("text-[13.5px] leading-[1.35] font-medium text-ink", className)}
      {...props}
    />
  );
}

function ToastDescription({ className, ...props }: React.ComponentProps<typeof ToastPrimitives.Description>) {
  return (
    <ToastPrimitives.Description
      className={cn("mt-0.5 text-[12.5px] leading-[1.35] text-muted [overflow-wrap:anywhere]", className)}
      {...props}
    />
  );
}

function ToastClose({ className, ...props }: React.ComponentProps<typeof ToastPrimitives.Close>) {
  return (
    <ToastPrimitives.Close
      toast-close=""
      className={cn(
        "absolute top-0 left-0 z-10 flex size-5 -translate-x-[35%] -translate-y-[35%] cursor-pointer items-center justify-center rounded-full border border-line bg-surface p-0 text-muted",
        "transition-[background-color,color] duration-150 hover:bg-sunk hover:text-ink focus-visible:ring-2 focus-visible:ring-accent/40 focus-visible:outline-none",
        className,
      )}
      {...props}
    >
      <HugeiconsIcon icon={Cancel01Icon} size={12} strokeWidth={1.5} color="currentColor" />
    </ToastPrimitives.Close>
  );
}

export { type ToastProps, ToastProvider, ToastViewport, Toast, ToastTitle, ToastDescription, ToastClose };
