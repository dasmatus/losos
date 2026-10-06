import type * as React from "react";
import { cva, type VariantProps } from "class-variance-authority";
import { cn } from "@/lib/utils";

/* shadcn/ui Alert, on the palette.
 *
 * The grid is shadcn's: an icon as the first child takes the first column and
 * the title and description line up in the second, so a note with and without
 * an icon keeps the same text edge. @hugeicons renders a plain <svg>, which is
 * what `has-[>svg]` and `[&>svg]` match.
 *
 * Four tones instead of shadcn's two. The icon carries the tone and the text
 * does not, so a paragraph inside stays as readable as the rest of the card;
 * `crit` is kept rather than renamed `destructive` because it is the word
 * tokens.css uses for the colour. */
export const alertVariants = cva(
  cn(
    "relative grid w-full grid-cols-[0_1fr] items-start gap-y-0.5 rounded-control border px-3.5 py-3",
    "text-[13px] leading-snug",
    "has-[>svg]:grid-cols-[19px_1fr] has-[>svg]:gap-x-3",
    "[&>svg]:mt-px [&>svg]:size-[19px] [&>svg]:shrink-0 [&>svg]:text-current",
    "animate-fade-in",
  ),
  {
    variants: {
      variant: {
        default: "border-line bg-sunk text-ink [&>svg]:text-accent",
        ok: "border-ok/25 bg-ok/10 text-ink [&>svg]:text-ok",
        warn: "border-warn/25 bg-warn/10 text-ink [&>svg]:text-warn",
        crit: "border-crit/25 bg-crit/10 text-ink [&>svg]:text-crit",
      },
    },
    defaultVariants: { variant: "default" },
  },
);

export interface AlertProps
  extends React.ComponentPropsWithoutRef<"div">,
    VariantProps<typeof alertVariants> {}

export function Alert({ className, variant, ...props }: AlertProps) {
  return (
    <div
      data-slot="alert"
      data-variant={variant ?? "default"}
      role="alert"
      className={cn(alertVariants({ variant }), className)}
      {...props}
    />
  );
}

export function AlertTitle({ className, ...props }: React.ComponentPropsWithoutRef<"p">) {
  return (
    <p
      data-slot="alert-title"
      className={cn("col-start-2 min-h-4 font-medium tracking-tight text-ink", className)}
      {...props}
    />
  );
}

export function AlertDescription({ className, ...props }: React.ComponentPropsWithoutRef<"div">) {
  return (
    <div
      data-slot="alert-description"
      className={cn(
        "col-start-2 grid min-w-0 gap-1 text-[13px] text-muted",
        "[&_p]:leading-snug [&>*]:min-w-0",
        className,
      )}
      {...props}
    />
  );
}
