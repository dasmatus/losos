import type * as React from "react";
import { cn } from "@/lib/utils";

/* shadcn/ui Table, on the palette.
 *
 * For lists whose rows are the same shape and whose columns carry a number:
 * the market's purchases (what, how much, until when, status) and the apps
 * catalogue's results (app, source, link). The settings panes are NOT tables
 * — a switch with a label is a Row (screens/settings/rows.tsx), and putting
 * eleven switches in a grid would promise sortable data that is not there.
 *
 * The wrapper scrolls sideways on a phone instead of the page doing it: the
 * one layout rule tests/app.browser.mjs checks on a 390px viewport. */
export function Table({ className, ...props }: React.ComponentPropsWithoutRef<"table">) {
  return (
    <div data-slot="table-container" className="relative w-full overflow-x-auto">
      <table
        data-slot="table"
        className={cn("w-full caption-bottom text-sm", className)}
        {...props}
      />
    </div>
  );
}

export function TableHeader({ className, ...props }: React.ComponentPropsWithoutRef<"thead">) {
  return (
    <thead
      data-slot="table-header"
      className={cn("[&_tr]:border-b [&_tr]:border-hair", className)}
      {...props}
    />
  );
}

export function TableBody({ className, ...props }: React.ComponentPropsWithoutRef<"tbody">) {
  return (
    <tbody
      data-slot="table-body"
      className={cn("[&_tr:last-child]:border-0", className)}
      {...props}
    />
  );
}

export function TableFooter({ className, ...props }: React.ComponentPropsWithoutRef<"tfoot">) {
  return (
    <tfoot
      data-slot="table-footer"
      className={cn("border-t border-hair bg-sunk/50 font-medium [&>tr]:last:border-b-0", className)}
      {...props}
    />
  );
}

export function TableRow({ className, ...props }: React.ComponentPropsWithoutRef<"tr">) {
  return (
    <tr
      data-slot="table-row"
      className={cn(
        "border-b border-hair transition-colors duration-150",
        "hover:bg-sunk/50 data-[state=selected]:bg-sunk",
        className,
      )}
      {...props}
    />
  );
}

export function TableHead({ className, ...props }: React.ComponentPropsWithoutRef<"th">) {
  return (
    <th
      data-slot="table-head"
      className={cn(
        "h-9 px-3.5 text-left align-middle text-[12.5px] font-medium whitespace-nowrap text-muted",
        "[&:has([role=checkbox])]:pr-0 [&>[role=checkbox]]:translate-y-[2px]",
        className,
      )}
      {...props}
    />
  );
}

export function TableCell({ className, ...props }: React.ComponentPropsWithoutRef<"td">) {
  return (
    <td
      data-slot="table-cell"
      className={cn(
        "px-3.5 py-2.5 align-middle text-[13px] whitespace-nowrap text-ink",
        "[&:has([role=checkbox])]:pr-0 [&>[role=checkbox]]:translate-y-[2px]",
        className,
      )}
      {...props}
    />
  );
}

export function TableCaption({ className, ...props }: React.ComponentPropsWithoutRef<"caption">) {
  return (
    <caption
      data-slot="table-caption"
      className={cn("mt-3 text-[12.5px] leading-snug text-muted", className)}
      {...props}
    />
  );
}
