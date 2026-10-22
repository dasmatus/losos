import type * as React from "react";
import { cn } from "@/lib/utils";

/* A key, as the palette's footer and the menus name them. Pure classes. */
export function Kbd({ className, ...props }: React.ComponentPropsWithoutRef<"kbd">) {
  return (
    <kbd
      data-slot="kbd"
      className={cn(
        "inline-flex min-w-5 items-center justify-center rounded-[4px] border border-line bg-sunk px-1",
        "font-sans text-[11px] leading-[18px] text-muted",
        className,
      )}
      {...props}
    />
  );
}

/** Keys pressed together, one Kbd each. */
export function KbdGroup({ keys, className }: { keys: readonly string[]; className?: string }) {
  return (
    <span data-slot="kbd-group" className={cn("inline-flex items-center gap-0.5", className)}>
      {keys.map((key) => (
        <Kbd key={key}>{key}</Kbd>
      ))}
    </span>
  );
}
