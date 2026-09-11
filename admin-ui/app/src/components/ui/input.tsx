import type * as React from "react";
import { cn } from "@/lib/utils";

export function Input({ className, ...props }: React.ComponentPropsWithoutRef<"input">) {
  return (
    <input
      className={cn(
        "h-9 w-full rounded-control border border-line bg-surface px-3",
        "text-sm text-ink placeholder:text-faint",
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

/* A token, a hostname, a port: identifiers read character by character.
 * Monospace with tabular figures so they can be compared down a column. */
export function MonoInput({ className, ...props }: React.ComponentPropsWithoutRef<"input">) {
  return (
    <Input
      spellCheck={false}
      autoCapitalize="off"
      autoCorrect="off"
      className={cn("numeric tracking-[0.01em]", className)}
      {...props}
    />
  );
}

export function Select({ className, children, ...props }: React.ComponentPropsWithoutRef<"select">) {
  return (
    <select
      className={cn(
        "h-9 rounded-control border border-line bg-surface px-3 pr-8",
        "text-sm text-ink",
        "transition-[border-color,box-shadow] duration-150",
        "focus-visible:border-accent focus-visible:outline-none",
        "focus-visible:ring-2 focus-visible:ring-accent/35",
        "disabled:cursor-not-allowed disabled:bg-sunk disabled:text-faint",
        className,
      )}
      {...props}
    >
      {children}
    </select>
  );
}

/** The error line under a field. Rendered as a live region, so a validation
 *  failure is announced when it happens rather than on the next focus. */
export function FieldError({
  className,
  children,
  ...props
}: React.ComponentPropsWithoutRef<"p">) {
  if (children === undefined || children === null || children === false) return null;
  return (
    <p role="alert" className={cn("text-[12.5px] text-crit", className)} {...props}>
      {children}
    </p>
  );
}
