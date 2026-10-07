import type * as React from "react";
import { cn } from "@/lib/utils";

/* shadcn/ui Textarea, on the palette: the Input's chrome, several lines
 * tall, resizable downwards only. Used for a list option (one item per
 * line) on the Advanced pane. */
export function Textarea({ className, ...props }: React.ComponentPropsWithoutRef<"textarea">) {
  return (
    <textarea
      data-slot="textarea"
      className={cn(
        "min-h-[4.5rem] w-full min-w-0 resize-y rounded-control border border-line bg-surface px-3 py-2",
        "text-sm leading-snug text-ink placeholder:text-faint",
        "transition-[border-color,box-shadow] duration-150",
        "hover:border-muted/60",
        "focus-visible:border-accent focus-visible:outline-none",
        "focus-visible:ring-2 focus-visible:ring-accent/35",
        "disabled:cursor-not-allowed disabled:bg-sunk disabled:text-faint",
        "aria-invalid:border-crit aria-invalid:ring-2 aria-invalid:ring-crit/25",
        className,
      )}
      {...props}
    />
  );
}
