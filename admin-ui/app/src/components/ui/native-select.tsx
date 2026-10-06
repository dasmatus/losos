import type * as React from "react";
import { HugeiconsIcon } from "@hugeicons/react";
import { ArrowDown01Icon } from "@hugeicons/core-free-icons";
import { cn } from "@/lib/utils";

/* shadcn/ui Native Select, on the palette.
 *
 * The platform's <select> with the app's chrome and one chevron. A native
 * select, not a custom menu, on purpose: it is the one control every phone
 * already renders well, and the lists on this box (two run modes, three
 * languages, a handful of metrics) do not need more. The custom shadcn
 * Select is Radix and portals a listbox; nothing here wants that. */
export function NativeSelect({
  className,
  children,
  ...props
}: React.ComponentPropsWithoutRef<"select">) {
  return (
    <div
      data-slot="native-select-wrapper"
      className={cn(
        "group/native-select relative w-fit has-[select:disabled]:opacity-50",
        className,
      )}
    >
      <select
        data-slot="native-select"
        className={cn(
          "h-9 w-full appearance-none rounded-control border border-line bg-surface py-1 pr-9 pl-3",
          "text-sm text-ink",
          "transition-[border-color,box-shadow] duration-150",
          "hover:border-muted/60",
          "focus-visible:border-accent focus-visible:outline-none",
          "focus-visible:ring-2 focus-visible:ring-accent/35",
          "disabled:cursor-not-allowed disabled:bg-sunk disabled:text-faint",
          "aria-invalid:border-crit aria-invalid:ring-2 aria-invalid:ring-crit/25",
        )}
        {...props}
      >
        {children}
      </select>
      <HugeiconsIcon
        icon={ArrowDown01Icon}
        size={16}
        strokeWidth={1.5}
        color="currentColor"
        className="pointer-events-none absolute top-1/2 right-3 -translate-y-1/2 text-muted select-none"
        aria-hidden="true"
      />
    </div>
  );
}

export function NativeSelectOption(props: React.ComponentPropsWithoutRef<"option">) {
  return <option data-slot="native-select-option" {...props} />;
}

export function NativeSelectOptGroup({
  className,
  ...props
}: React.ComponentPropsWithoutRef<"optgroup">) {
  return (
    <optgroup
      data-slot="native-select-optgroup"
      className={cn("bg-surface text-ink", className)}
      {...props}
    />
  );
}
