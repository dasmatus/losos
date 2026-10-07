import type * as React from "react";
import { cva, type VariantProps } from "class-variance-authority";
import { Button, type ButtonProps } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { cn } from "@/lib/utils";

/* shadcn/ui Input Group, on the palette.
 *
 * One bordered box that holds an input and whatever sits beside it — the
 * show/hide eye on a password, the magnifier in front of a search, a copy
 * button after a read-only value. The border, focus ring and invalid ring
 * move from the <input> to the group, so the eye button lives INSIDE the
 * field rather than being a second control bolted on next to it, which is
 * what the sign-in dialog and the wizard's password form drew before. */
export function InputGroup({ className, ...props }: React.ComponentPropsWithoutRef<"div">) {
  return (
    <div
      data-slot="input-group"
      role="group"
      className={cn(
        "group/input-group relative flex h-9 w-full min-w-0 items-center",
        "rounded-control border border-line bg-surface",
        "transition-[border-color,box-shadow] duration-150 outline-none",
        "hover:border-muted/60",
        "has-[>textarea]:h-auto",
        // The input's own padding gives way to the addon's.
        "has-[>[data-align=inline-start]]:[&>input]:pl-2",
        "has-[>[data-align=inline-end]]:[&>input]:pr-2",
        "has-[>[data-align=block-start]]:h-auto has-[>[data-align=block-start]]:flex-col has-[>[data-align=block-start]]:[&>input]:pb-3",
        "has-[>[data-align=block-end]]:h-auto has-[>[data-align=block-end]]:flex-col has-[>[data-align=block-end]]:[&>input]:pt-3",
        // The focus ring the plain Input draws, now on the group.
        "has-[[data-slot=input-group-control]:focus-visible]:border-accent",
        "has-[[data-slot=input-group-control]:focus-visible]:ring-2",
        "has-[[data-slot=input-group-control]:focus-visible]:ring-accent/35",
        "has-[[data-slot][aria-invalid=true]]:border-crit",
        "has-[[data-slot][aria-invalid=true]]:ring-2 has-[[data-slot][aria-invalid=true]]:ring-crit/25",
        "has-[[data-slot=input-group-control]:disabled]:bg-sunk",
        className,
      )}
      {...props}
    />
  );
}

const inputGroupAddonVariants = cva(
  cn(
    "flex h-auto cursor-text items-center justify-center gap-2 py-1.5 text-[13px] font-medium",
    "text-muted select-none",
    "group-data-[disabled=true]/input-group:opacity-50",
    "[&>svg:not([class*='size-'])]:size-4 [&>kbd]:rounded-[calc(var(--control-radius)-2px)]",
  ),
  {
    variants: {
      align: {
        "inline-start": "order-first pl-3 has-[>button]:ml-[-0.45rem] has-[>kbd]:ml-[-0.35rem]",
        "inline-end": "order-last pr-3 has-[>button]:mr-[-0.45rem] has-[>kbd]:mr-[-0.35rem]",
        "block-start": cn(
          "order-first w-full justify-start px-3 pt-3",
          "[.border-b]:pb-3 group-has-[>input]/input-group:pt-2.5",
        ),
        "block-end": cn(
          "order-last w-full justify-start px-3 pb-3",
          "[.border-t]:pt-3 group-has-[>input]/input-group:pb-2.5",
        ),
      },
    },
    defaultVariants: { align: "inline-start" },
  },
);

export interface InputGroupAddonProps
  extends React.ComponentPropsWithoutRef<"div">,
    VariantProps<typeof inputGroupAddonVariants> {}

/** Something beside the input: an icon, a unit, a button. Clicking the
 *  addon's empty space focuses the input, the way clicking a field's padding
 *  does on a plain input. */
export function InputGroupAddon({
  className,
  align = "inline-start",
  onClick,
  ...props
}: InputGroupAddonProps) {
  return (
    <div
      role="group"
      data-slot="input-group-addon"
      data-align={align}
      className={cn(inputGroupAddonVariants({ align }), className)}
      onClick={(event) => {
        onClick?.(event);
        if (event.defaultPrevented) return;
        if ((event.target as HTMLElement).closest("button") !== null) return;
        event.currentTarget.parentElement?.querySelector("input")?.focus();
      }}
      {...props}
    />
  );
}

const inputGroupButtonVariants = cva("flex items-center gap-2 text-[13px] shadow-none", {
  variants: {
    size: {
      xs: "h-6 gap-1 rounded-[calc(var(--control-radius)-2px)] px-2 has-[>svg]:px-2 [&>svg:not([class*='size-'])]:size-3.5",
      sm: "h-8 px-2.5",
      "icon-xs": "size-6 rounded-[calc(var(--control-radius)-2px)] p-0 has-[>svg]:p-0",
      "icon-sm": "size-8 p-0 has-[>svg]:p-0",
    },
  },
  defaultVariants: { size: "xs" },
});

export interface InputGroupButtonProps
  extends Omit<ButtonProps, "size">,
    VariantProps<typeof inputGroupButtonVariants> {}

/** A button inside the group. Ghost by default, so the group's border is
 *  the only chrome and the button reads as part of the field. */
export function InputGroupButton({
  className,
  type = "button",
  variant = "ghost",
  size = "xs",
  ...props
}: InputGroupButtonProps) {
  return (
    <Button
      type={type}
      data-size={size}
      variant={variant}
      className={cn(inputGroupButtonVariants({ size }), className)}
      {...props}
    />
  );
}

export function InputGroupText({ className, ...props }: React.ComponentPropsWithoutRef<"span">) {
  return (
    <span
      className={cn(
        "flex items-center gap-2 text-[13px] text-muted",
        "[&_svg:not([class*='size-'])]:size-4 [&_svg]:pointer-events-none",
        className,
      )}
      {...props}
    />
  );
}

/** The input itself, stripped of its own border and ring: the group draws
 *  both. */
export function InputGroupInput({ className, ...props }: React.ComponentPropsWithoutRef<"input">) {
  return (
    <Input
      data-slot="input-group-control"
      className={cn(
        "h-full flex-1 rounded-none border-0 bg-transparent shadow-none",
        "hover:border-transparent focus-visible:ring-0 focus-visible:border-transparent",
        "aria-invalid:ring-0 aria-invalid:border-transparent",
        "disabled:bg-transparent",
        className,
      )}
      {...props}
    />
  );
}
