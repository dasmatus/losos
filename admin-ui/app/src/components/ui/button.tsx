import type * as React from "react";
import { cva, type VariantProps } from "class-variance-authority";
import { cn } from "@/lib/utils";

/* ShadCN-shaped, Radix-free.
 *
 * Radix is not used anywhere in src/components/ui: its floating primitives
 * position themselves with inline styles and its dialog injects a <style>
 * element for the scroll lock, and the appliance CSP is `style-src 'self'`
 * with no 'unsafe-inline'. A <style> element it inserts is simply ignored,
 * silently, on a box with no console anyone will ever look at. So the
 * primitives here are plain elements plus Tailwind classes, and the few that
 * need real behaviour (dialog, tabs, switch) implement it against native
 * semantics. `asChild` is not supported; wrap or restyle instead. */

export const buttonVariants = cva(
  cn(
    "inline-flex shrink-0 items-center justify-center gap-2 rounded-control",
    "text-sm font-medium whitespace-nowrap select-none",
    "transition-[background-color,border-color,color,box-shadow,transform] duration-150",
    "active:translate-y-px",
    "disabled:pointer-events-none disabled:opacity-45",
    "[&_svg]:pointer-events-none [&_svg]:shrink-0",
  ),
  {
    variants: {
      variant: {
        primary: "bg-accent text-surface hover:opacity-90",
        secondary: "border border-line bg-surface text-ink hover:bg-sunk",
        ghost: "text-muted hover:bg-sunk hover:text-ink",
        outline: "border border-accent bg-transparent text-accent hover:bg-accent-wash",
        // Destructive is a state colour, not the accent: factory reset and
        // nothing else should be reaching for it. `text-surface`, not
        // `text-white` — --crit is a light coral in the dark palette, and
        // white on it does not read.
        destructive: "bg-crit text-surface hover:opacity-90",
        link: "text-accent underline-offset-4 hover:underline",
      },
      size: {
        sm: "h-8 px-3 text-[13px]",
        md: "h-9 px-4",
        lg: "h-11 px-5 text-base",
        icon: "size-9 p-0",
      },
    },
    defaultVariants: { variant: "primary", size: "md" },
  },
);

export interface ButtonProps
  extends React.ComponentPropsWithoutRef<"button">,
    VariantProps<typeof buttonVariants> {}

export function Button({ className, variant, size, type, ...props }: ButtonProps) {
  return (
    <button
      // Inside a <form> an unset type submits, and the sign-in form is not
      // the only form on this box.
      type={type ?? "button"}
      className={cn(buttonVariants({ variant, size }), className)}
      {...props}
    />
  );
}
