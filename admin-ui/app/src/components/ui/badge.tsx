import type * as React from "react";
import { cva, type VariantProps } from "class-variance-authority";
import { cn } from "@/lib/utils";

/* One accent in the whole app. The local/mesh distinction is FILLED versus
 * HATCHED, never a second hue — `local` and `mesh` below are the same colour
 * with a different texture. ok / warn / crit are state and nothing else. */
export const badgeVariants = cva(
  cn(
    "inline-flex items-center gap-1.5 rounded-full px-2.5 py-0.5",
    "text-[12px] leading-5 font-medium whitespace-nowrap",
    "[&_svg]:pointer-events-none [&_svg]:shrink-0",
  ),
  {
    variants: {
      variant: {
        neutral: "bg-sunk text-muted",
        outline: "border border-line text-muted",
        /** Runs on this box. */
        local: "bg-accent-wash text-accent",
        /** Runs on the mesh. Same hue, hatched. */
        mesh: "hatched text-accent",
        ok: "bg-ok/12 text-ok",
        warn: "bg-warn/12 text-warn",
        crit: "bg-crit/12 text-crit",
      },
    },
    defaultVariants: { variant: "neutral" },
  },
);

export interface BadgeProps
  extends React.ComponentPropsWithoutRef<"span">,
    VariantProps<typeof badgeVariants> {}

export function Badge({ className, variant, ...props }: BadgeProps) {
  return <span className={cn(badgeVariants({ variant }), className)} {...props} />;
}

export type DotState = "ok" | "warn" | "crit" | "idle" | "pending";

/* A status dot.
 *
 * aria-hidden, always. A dot that labels itself and sits beside a text node
 * announces its state twice; the text next to it is the only channel a screen
 * reader gets, and it should be a live region. */
export function StatusDot({
  state,
  className,
  ...props
}: React.ComponentPropsWithoutRef<"span"> & { state: DotState }) {
  return (
    <span
      aria-hidden="true"
      className={cn(
        "inline-block size-2 shrink-0 rounded-full",
        state === "ok" && "bg-ok",
        state === "warn" && "bg-warn",
        state === "crit" && "bg-crit",
        state === "idle" && "bg-faint",
        state === "pending" && "bg-faint animate-breathe",
        className,
      )}
      {...props}
    />
  );
}
