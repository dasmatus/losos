import type * as React from "react";
import { cn } from "@/lib/utils";

/* shadcn/ui Input, on the palette. The select lives in native-select.tsx and
 * the error line in field.tsx now (shadcn's file layout); FieldError is
 * re-exported from here so its callers keep compiling. */
export function Input({ className, ...props }: React.ComponentPropsWithoutRef<"input">) {
  return (
    <input
      data-slot="input"
      className={cn(
        "h-9 w-full min-w-0 rounded-control border border-line bg-surface px-3",
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

export { FieldError, type FieldErrorProps } from "@/components/ui/field";
