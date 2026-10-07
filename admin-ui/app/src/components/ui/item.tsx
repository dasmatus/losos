import type * as React from "react";
import { cva, type VariantProps } from "class-variance-authority";
import { Separator } from "@/components/ui/separator";
import { cn } from "@/lib/utils";

/* shadcn/ui Item, on the palette.
 *
 * A picture, a title, a sentence and an action, in a row that wraps on a
 * phone. The widget gallery's cards are this shape — the Windows 7 gadget
 * gallery the board's header comment names is a grid of exactly these.
 *
 * Not the settings rows. screens/settings/rows.tsx keeps its own Row because
 * its hairline is inset 14px from the left, which is the tell that says
 * "these rows belong together"; Item's separator runs edge to edge. */
const itemVariants = cva(
  cn(
    "group/item flex flex-wrap items-center gap-x-4 gap-y-2 border border-transparent text-sm",
    "rounded-card transition-colors duration-150 outline-none",
    "[a]:hover:bg-sunk [a]:transition-colors",
    "focus-visible:ring-2 focus-visible:ring-accent/40",
  ),
  {
    variants: {
      variant: {
        default: "bg-transparent",
        outline: "border-line bg-surface hover:border-accent",
        muted: "bg-sunk/60",
        /** The gallery's "build your own" card: a dashed frame. */
        dashed: "border-dashed border-line hover:border-accent",
      },
      size: {
        default: "gap-4 p-4",
        sm: "gap-2.5 px-4 py-3",
      },
    },
    defaultVariants: { variant: "default", size: "default" },
  },
);

export interface ItemProps
  extends React.ComponentPropsWithoutRef<"div">,
    VariantProps<typeof itemVariants> {}

export function Item({ className, variant = "default", size = "default", ...props }: ItemProps) {
  return (
    <div
      data-slot="item"
      data-variant={variant}
      data-size={size}
      className={cn(itemVariants({ variant, size }), className)}
      {...props}
    />
  );
}

export function ItemGroup({ className, ...props }: React.ComponentPropsWithoutRef<"div">) {
  return (
    <div
      role="list"
      data-slot="item-group"
      className={cn("group/item-group flex flex-col", className)}
      {...props}
    />
  );
}

export function ItemSeparator({
  className,
  ...props
}: React.ComponentPropsWithoutRef<typeof Separator>) {
  return (
    <Separator
      data-slot="item-separator"
      orientation="horizontal"
      className={cn("my-0", className)}
      {...props}
    />
  );
}

const itemMediaVariants = cva(
  cn(
    "flex shrink-0 items-center justify-center gap-2",
    "group-has-[[data-slot=item-description]]/item:self-start",
    "group-has-[[data-slot=item-description]]/item:translate-y-0.5",
    "[&_svg]:pointer-events-none",
  ),
  {
    variants: {
      variant: {
        default: "bg-transparent",
        icon: cn(
          "size-9 rounded-control bg-accent-wash text-accent",
          "[&_svg:not([class*='size-'])]:size-[21px]",
        ),
        /** The same hue, hatched: the mesh's half of a pair. */
        hatched: "hatched size-9 rounded-control text-accent [&_svg:not([class*='size-'])]:size-[21px]",
        image: cn(
          "size-10 overflow-hidden rounded-control",
          "[&_img]:size-full [&_img]:object-cover",
        ),
      },
    },
    defaultVariants: { variant: "default" },
  },
);

export interface ItemMediaProps
  extends React.ComponentPropsWithoutRef<"div">,
    VariantProps<typeof itemMediaVariants> {}

export function ItemMedia({ className, variant = "default", ...props }: ItemMediaProps) {
  return (
    <div
      data-slot="item-media"
      data-variant={variant}
      className={cn(itemMediaVariants({ variant }), className)}
      {...props}
    />
  );
}

export function ItemContent({ className, ...props }: React.ComponentPropsWithoutRef<"div">) {
  return (
    <div
      data-slot="item-content"
      className={cn(
        "flex min-w-0 flex-1 flex-col gap-1 [&+[data-slot=item-content]]:flex-none",
        className,
      )}
      {...props}
    />
  );
}

export function ItemTitle({ className, ...props }: React.ComponentPropsWithoutRef<"div">) {
  return (
    <div
      data-slot="item-title"
      className={cn("flex w-fit items-center gap-2 text-[14px] leading-snug font-semibold text-ink", className)}
      {...props}
    />
  );
}

export function ItemDescription({ className, ...props }: React.ComponentPropsWithoutRef<"p">) {
  return (
    <p
      data-slot="item-description"
      className={cn(
        "text-[12.5px] leading-snug font-normal text-balance text-muted",
        "[&>a]:text-accent [&>a]:underline [&>a]:underline-offset-4",
        className,
      )}
      {...props}
    />
  );
}

export function ItemActions({ className, ...props }: React.ComponentPropsWithoutRef<"div">) {
  return (
    <div
      data-slot="item-actions"
      className={cn("flex items-center gap-2", className)}
      {...props}
    />
  );
}

export function ItemHeader({ className, ...props }: React.ComponentPropsWithoutRef<"div">) {
  return (
    <div
      data-slot="item-header"
      className={cn("flex basis-full items-center justify-between gap-2", className)}
      {...props}
    />
  );
}

export function ItemFooter({ className, ...props }: React.ComponentPropsWithoutRef<"div">) {
  return (
    <div
      data-slot="item-footer"
      className={cn("flex basis-full items-center justify-between gap-2", className)}
      {...props}
    />
  );
}
