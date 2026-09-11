import type * as React from "react";
import { cn } from "@/lib/utils";

export interface SwitchProps
  extends Omit<React.ComponentPropsWithoutRef<"button">, "onChange" | "value" | "type"> {
  checked: boolean;
  onCheckedChange?: (checked: boolean) => void;
}

/* A native button carrying role="switch".
 *
 * Not Radix: its Switch renders a hidden form input positioned with an inline
 * style attribute, and `style-src 'self'` would leave that input visible. A
 * button with role="switch" and aria-checked is what a screen reader wants
 * anyway, it is keyboard-operable for free, and there is no form to post to —
 * every write on this box goes through /api/apply as generated Nix. */
export function Switch({
  checked,
  onCheckedChange,
  className,
  disabled,
  onClick,
  ...props
}: SwitchProps) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      disabled={disabled}
      onClick={(event) => {
        onClick?.(event);
        if (!event.defaultPrevented) onCheckedChange?.(!checked);
      }}
      className={cn(
        "relative inline-flex h-6 w-11 shrink-0 items-center rounded-full",
        "border border-transparent p-0.5",
        "transition-colors duration-200 ease-out",
        "focus-visible:ring-2 focus-visible:ring-accent/40 focus-visible:outline-none",
        "disabled:cursor-not-allowed disabled:opacity-45",
        checked ? "bg-accent" : "bg-sunk border-line",
        className,
      )}
      {...props}
    >
      <span
        aria-hidden="true"
        className={cn(
          "size-5 rounded-full bg-surface shadow-sm",
          "transition-transform duration-200 ease-out",
          checked ? "translate-x-5" : "translate-x-0",
        )}
      />
    </button>
  );
}
