import type * as React from "react";
import { cva, type VariantProps } from "class-variance-authority";
import { Separator } from "@/components/ui/separator";
import { cn } from "@/lib/utils";

/* shadcn/ui Button Group, on the palette.
 *
 * Adjacent controls that belong to one action share their edges: the first
 * keeps its left corners, the last its right, and the border between two
 * neighbours is drawn once. It takes buttons, inputs and a text cell alike,
 * which is what lets the market's quantity box and its Order button, and the
 * mesh pane's "from · until · to" pair of time inputs, read as one control
 * each rather than as a row of unrelated widgets. */
export const buttonGroupVariants = cva(
  cn(
    "flex w-fit items-stretch",
    "[&>*]:focus-visible:relative [&>*]:focus-visible:z-10",
    "[&>input]:flex-1",
    "has-[>[data-slot=button-group]]:gap-2",
  ),
  {
    variants: {
      orientation: {
        horizontal: cn(
          "[&>*:not(:first-child)]:rounded-l-none [&>*:not(:first-child)]:border-l-0",
          "[&>*:not(:last-child)]:rounded-r-none",
        ),
        vertical: cn(
          "flex-col",
          "[&>*:not(:first-child)]:rounded-t-none [&>*:not(:first-child)]:border-t-0",
          "[&>*:not(:last-child)]:rounded-b-none",
        ),
      },
    },
    defaultVariants: { orientation: "horizontal" },
  },
);

export interface ButtonGroupProps
  extends React.ComponentPropsWithoutRef<"div">,
    VariantProps<typeof buttonGroupVariants> {}

export function ButtonGroup({ className, orientation, ...props }: ButtonGroupProps) {
  return (
    <div
      role="group"
      data-slot="button-group"
      data-orientation={orientation ?? "horizontal"}
      className={cn(buttonGroupVariants({ orientation }), className)}
      {...props}
    />
  );
}

/** A non-interactive cell in the group: a unit, a word between two inputs. */
export function ButtonGroupText({ className, ...props }: React.ComponentPropsWithoutRef<"div">) {
  return (
    <div
      data-slot="button-group-text"
      className={cn(
        "flex items-center gap-2 rounded-control border border-line bg-sunk px-3 text-[12.5px] text-muted",
        "[&_svg:not([class*='size-'])]:size-4 [&_svg]:pointer-events-none",
        className,
      )}
      {...props}
    />
  );
}

export function ButtonGroupSeparator({
  className,
  orientation = "vertical",
  ...props
}: React.ComponentPropsWithoutRef<typeof Separator>) {
  return (
    <Separator
      data-slot="button-group-separator"
      orientation={orientation}
      className={cn(
        "relative !m-0 self-stretch bg-line data-[orientation=vertical]:h-auto",
        className,
      )}
      {...props}
    />
  );
}
