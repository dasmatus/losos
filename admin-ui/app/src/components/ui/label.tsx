import type * as React from "react";
import { cn } from "@/lib/utils";

export function Label({ className, ...props }: React.ComponentPropsWithoutRef<"label">) {
  return (
    <label
      className={cn(
        "text-sm leading-snug font-medium text-ink select-none",
        "has-[+:disabled]:text-faint",
        className,
      )}
      {...props}
    />
  );
}

/** The explanatory line under a label. Not a placeholder, not a tooltip. */
export function LabelHint({ className, ...props }: React.ComponentPropsWithoutRef<"p">) {
  return <p className={cn("text-[12.5px] leading-snug text-muted", className)} {...props} />;
}
