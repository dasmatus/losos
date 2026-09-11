import type * as React from "react";
import { cn } from "@/lib/utils";

export interface ScrollAreaProps extends React.ComponentPropsWithoutRef<"div"> {
  orientation?: "vertical" | "horizontal" | "both";
}

/* A scroll container with the palette's own scrollbar.
 *
 * Radix ScrollArea replaces the native scrollbar with two absolutely
 * positioned divs whose size and offset it writes as inline styles — under
 * `style-src 'self'` the thumb would sit at 0×0 forever. `scrollbar-width`
 * and `scrollbar-color` (set globally in styles/index.css) get most of the
 * look from the platform, and the platform keeps keyboard scrolling,
 * overscroll and touch momentum working. */
export function ScrollArea({
  className,
  orientation = "vertical",
  tabIndex,
  ...props
}: ScrollAreaProps) {
  return (
    <div
      // A scrollable region must be reachable by keyboard or its content is
      // unreadable without a mouse.
      tabIndex={tabIndex ?? 0}
      className={cn(
        "overscroll-contain focus-visible:outline-none",
        orientation === "vertical" && "overflow-x-hidden overflow-y-auto",
        orientation === "horizontal" && "overflow-x-auto overflow-y-hidden",
        orientation === "both" && "overflow-auto",
        className,
      )}
      {...props}
    />
  );
}

/* A rebuild log line, or any machine text: monospace, wrapped, scrollable,
 * and announced politely so a running rebuild narrates itself. */
export function LogView({ className, ...props }: React.ComponentPropsWithoutRef<"div">) {
  return (
    <ScrollArea
      aria-live="polite"
      className={cn(
        "numeric max-h-40 rounded-control bg-sunk px-3 py-2",
        "text-[12.5px] leading-relaxed break-words whitespace-pre-wrap text-muted",
        className,
      )}
      {...props}
    />
  );
}
