import * as React from "react";
import { cva, type VariantProps } from "class-variance-authority";
import { cn } from "@/lib/utils";

/* shadcn/ui Toggle Group, Radix-free.
 *
 * A single-select segmented control: Auto / Light / Dark, Linux & macOS /
 * Windows. The semantics are the ones Radix's `type="single"` group renders
 * — role="radiogroup" with one role="radio" per item, aria-checked on the
 * chosen one — and the keyboard behaviour is the WAI-ARIA radio pattern:
 * one Tab stop, arrows to move the choice, Home and End to jump. Radix is not
 * used anywhere in this folder; see button.tsx for why.
 *
 * Two looks, both already in the app before this file existed:
 *   - `default` is the theme toggle: a sunk tray, the chosen item lifted to
 *     the surface with the card shadow.
 *   - `outline` is the wizard's OS picker: a bordered tray on the surface,
 *     the chosen item filled with the accent. */

export const toggleVariants = cva(
  cn(
    "inline-flex items-center justify-center gap-1.5 whitespace-nowrap select-none",
    "text-[12.5px] font-medium transition-colors duration-150",
    "focus-visible:ring-2 focus-visible:ring-accent/40 focus-visible:outline-none",
    "disabled:pointer-events-none disabled:opacity-45",
    "[&_svg]:pointer-events-none [&_svg]:shrink-0",
  ),
  {
    variants: {
      variant: {
        default: cn(
          "text-faint hover:text-ink",
          "data-[state=on]:bg-surface data-[state=on]:text-ink data-[state=on]:shadow-card",
        ),
        outline: cn(
          "text-muted hover:text-ink",
          "data-[state=on]:bg-accent data-[state=on]:text-surface",
        ),
      },
      size: {
        default: "h-7 min-w-7 px-2.5",
        sm: "h-6 min-w-6 px-2 text-[12px]",
        icon: "size-7 p-0",
      },
    },
    defaultVariants: { variant: "default", size: "default" },
  },
);

interface ToggleGroupContextValue extends VariantProps<typeof toggleVariants> {
  value: string;
  setValue: (value: string) => void;
}

const ToggleGroupContext = React.createContext<ToggleGroupContextValue | null>(null);

export interface ToggleGroupProps
  extends Omit<React.ComponentPropsWithoutRef<"div">, "onChange" | "defaultValue">,
    VariantProps<typeof toggleVariants> {
  /** Controlled value. Omit for an uncontrolled group. */
  value?: string;
  defaultValue?: string;
  onValueChange?: (value: string) => void;
}

export function ToggleGroup({
  className,
  variant = "default",
  size = "default",
  value,
  defaultValue = "",
  onValueChange,
  children,
  onKeyDown,
  ...props
}: ToggleGroupProps) {
  const [inner, setInner] = React.useState(defaultValue);
  const current = value ?? inner;
  const setValue = React.useCallback(
    (next: string) => {
      if (value === undefined) setInner(next);
      onValueChange?.(next);
    },
    [value, onValueChange],
  );

  /* Arrows move the choice itself, not just focus (activation follows focus,
   * as in a native radio group). The items are whatever role="radio" buttons
   * are inside the group, read at keypress time so a conditional item costs
   * nothing to add. */
  const handleKeyDown = (event: React.KeyboardEvent<HTMLDivElement>): void => {
    onKeyDown?.(event);
    if (event.defaultPrevented) return;
    const keys = ["ArrowRight", "ArrowDown", "ArrowLeft", "ArrowUp", "Home", "End"];
    if (!keys.includes(event.key)) return;
    const items = Array.from(
      event.currentTarget.querySelectorAll<HTMLButtonElement>('[role="radio"]:not(:disabled)'),
    );
    if (items.length === 0) return;
    const at = items.findIndex((item) => item === document.activeElement);
    let next: number;
    if (event.key === "Home") next = 0;
    else if (event.key === "End") next = items.length - 1;
    else if (event.key === "ArrowRight" || event.key === "ArrowDown")
      next = at < 0 ? 0 : (at + 1) % items.length;
    else next = at <= 0 ? items.length - 1 : at - 1;
    event.preventDefault();
    const target = items[next];
    if (target === undefined) return;
    target.focus();
    const chosen = target.dataset["value"];
    if (chosen !== undefined) setValue(chosen);
  };

  return (
    <ToggleGroupContext.Provider value={{ value: current, setValue, variant, size }}>
      <div
        role="radiogroup"
        data-slot="toggle-group"
        data-variant={variant}
        data-size={size}
        className={cn(
          "group/toggle-group inline-flex w-fit items-center gap-0.5 rounded-control border p-0.5",
          variant === "outline" ? "border-line bg-surface" : "border-line bg-sunk",
          className,
        )}
        onKeyDown={handleKeyDown}
        {...props}
      >
        {children}
      </div>
    </ToggleGroupContext.Provider>
  );
}

export interface ToggleGroupItemProps
  extends Omit<React.ComponentPropsWithoutRef<"button">, "value">,
    VariantProps<typeof toggleVariants> {
  value: string;
}

export function ToggleGroupItem({
  className,
  value,
  variant,
  size,
  onClick,
  disabled,
  ...props
}: ToggleGroupItemProps) {
  const context = React.useContext(ToggleGroupContext);
  if (context === null) throw new Error("<ToggleGroupItem> must be inside <ToggleGroup>");
  const on = context.value === value;
  const resolvedVariant = variant ?? context.variant;
  const resolvedSize = size ?? context.size;
  // One Tab stop: the chosen item, or the first one while nothing is chosen.
  const tabStop = on || context.value === "";

  return (
    <button
      type="button"
      role="radio"
      aria-checked={on}
      data-slot="toggle-group-item"
      data-state={on ? "on" : "off"}
      data-value={value}
      data-variant={resolvedVariant}
      data-size={resolvedSize}
      tabIndex={tabStop ? 0 : -1}
      disabled={disabled}
      onClick={(event) => {
        onClick?.(event);
        if (!event.defaultPrevented) context.setValue(value);
      }}
      className={cn(
        toggleVariants({ variant: resolvedVariant, size: resolvedSize }),
        "min-w-0 shrink-0 rounded-[calc(var(--control-radius)-2px)] focus-visible:z-10",
        className,
      )}
      {...props}
    />
  );
}
