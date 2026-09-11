import type * as React from "react";
import { cn } from "@/lib/utils";

/* A loading placeholder.
 *
 * aria-hidden with the surrounding region marked aria-busy is the right
 * pairing: the shapes are decoration, and announcing "loading" once beats
 * announcing six grey boxes. */
export function Skeleton({ className, ...props }: React.ComponentPropsWithoutRef<"div">) {
  return (
    <div
      aria-hidden="true"
      className={cn("animate-breathe rounded-control bg-sunk", className)}
      {...props}
    />
  );
}

/** A skeleton sized to a line of text. `w` is any Tailwind width class. */
export function SkeletonLine({ className, ...props }: React.ComponentPropsWithoutRef<"div">) {
  return <Skeleton className={cn("h-3.5 w-full", className)} {...props} />;
}
