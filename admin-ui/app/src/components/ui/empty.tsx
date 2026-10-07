import type * as React from "react";
import { cva, type VariantProps } from "class-variance-authority";
import { cn } from "@/lib/utils";

/* shadcn/ui Empty, on the palette.
 *
 * The honest shape for "there is nothing here yet": an icon, a sentence
 * saying so, and the one button that changes it. The widget board before a
 * widget is added, a catalogue search with no hits, an empty market shelf
 * and an unknown address all share it, so an owner learns the shape once. */
export function Empty({ className, ...props }: React.ComponentPropsWithoutRef<"div">) {
  return (
    <div
      data-slot="empty"
      className={cn(
        "flex min-w-0 flex-1 flex-col items-center justify-center gap-5 p-6 text-center text-balance",
        "rounded-card border border-dashed border-line md:p-10",
        "animate-fade-in",
        className,
      )}
      {...props}
    />
  );
}

export function EmptyHeader({ className, ...props }: React.ComponentPropsWithoutRef<"div">) {
  return (
    <div
      data-slot="empty-header"
      className={cn("flex max-w-sm flex-col items-center gap-2 text-center", className)}
      {...props}
    />
  );
}

const emptyMediaVariants = cva(
  "flex shrink-0 items-center justify-center mb-1 [&_svg]:pointer-events-none [&_svg]:shrink-0",
  {
    variants: {
      variant: {
        default: "bg-transparent",
        icon: cn(
          "size-10 rounded-control bg-accent-wash text-accent",
          "[&_svg:not([class*='size-'])]:size-[22px]",
        ),
      },
    },
    defaultVariants: { variant: "default" },
  },
);

export interface EmptyMediaProps
  extends React.ComponentPropsWithoutRef<"div">,
    VariantProps<typeof emptyMediaVariants> {}

export function EmptyMedia({ className, variant = "default", ...props }: EmptyMediaProps) {
  return (
    <div
      data-slot="empty-icon"
      data-variant={variant}
      className={cn(emptyMediaVariants({ variant }), className)}
      {...props}
    />
  );
}

export function EmptyTitle({ className, ...props }: React.ComponentPropsWithoutRef<"h2">) {
  return (
    <h2
      data-slot="empty-title"
      className={cn("text-[15px] leading-tight font-semibold tracking-tight text-ink", className)}
      {...props}
    />
  );
}

export function EmptyDescription({ className, ...props }: React.ComponentPropsWithoutRef<"p">) {
  return (
    <p
      data-slot="empty-description"
      className={cn(
        "text-[13px] leading-snug text-muted",
        "[&>a]:text-accent [&>a]:underline [&>a]:underline-offset-4",
        className,
      )}
      {...props}
    />
  );
}

export function EmptyContent({ className, ...props }: React.ComponentPropsWithoutRef<"div">) {
  return (
    <div
      data-slot="empty-content"
      className={cn(
        "flex w-full max-w-sm min-w-0 flex-col items-center gap-3 text-sm text-balance",
        className,
      )}
      {...props}
    />
  );
}
