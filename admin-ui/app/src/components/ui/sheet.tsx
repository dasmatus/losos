import * as React from "react";
import { Dialog as SheetPrimitive } from "@base-ui/react/dialog";
import { HugeiconsIcon } from "@hugeicons/react";
import { Cancel01Icon } from "@hugeicons/core-free-icons";
import { cn } from "@/lib/utils";

/* shadcn/ui's Sheet from its base registry: a Base UI Dialog that slides in
 * from an edge. Used by the Sidebar on a phone.
 *
 * Base UI and not Radix, deliberately: Radix's Dialog locks the page's
 * scroll with react-remove-scroll, which injects a <style> element that the
 * admin CSP refuses, so the page behind kept scrolling. Base UI's scroll
 * lock is CSSOM, which the policy allows. dialog.tsx stays on the native
 * <dialog>; this is the one modal on Base UI. */

function Sheet(props: SheetPrimitive.Root.Props) {
  return <SheetPrimitive.Root data-slot="sheet" {...props} />;
}

function SheetTrigger(props: SheetPrimitive.Trigger.Props) {
  return <SheetPrimitive.Trigger data-slot="sheet-trigger" {...props} />;
}

function SheetClose(props: SheetPrimitive.Close.Props) {
  return <SheetPrimitive.Close data-slot="sheet-close" {...props} />;
}

const SIDE_CLASSES = {
  left: "inset-y-0 left-0 h-full w-3/4 max-w-[300px] border-r data-starting-style:-translate-x-full data-ending-style:-translate-x-full",
  right: "inset-y-0 right-0 h-full w-3/4 max-w-[300px] border-l data-starting-style:translate-x-full data-ending-style:translate-x-full",
} as const;

function SheetContent({
  className,
  children,
  side = "left",
  closeLabel,
  ...props
}: SheetPrimitive.Popup.Props & {
  side?: keyof typeof SIDE_CLASSES;
  /** The close button's accessible name; no button when omitted. */
  closeLabel?: string;
}) {
  return (
    <SheetPrimitive.Portal>
      <SheetPrimitive.Backdrop
        data-slot="sheet-overlay"
        className={cn(
          "fixed inset-0 z-50 bg-black/30 transition-opacity duration-200 ease-out",
          "data-starting-style:opacity-0 data-ending-style:opacity-0",
        )}
      />
      <SheetPrimitive.Popup
        data-slot="sheet-content"
        className={cn(
          "fixed z-50 flex flex-col gap-3 border-line bg-sunk p-3 shadow-xl outline-none",
          "transition-transform duration-250 ease-out",
          SIDE_CLASSES[side],
          className,
        )}
        {...props}
      >
        {children}
        {closeLabel !== undefined && (
          <SheetPrimitive.Close
            data-slot="sheet-close"
            aria-label={closeLabel}
            className={cn(
              "absolute top-3 right-3 flex size-8 items-center justify-center rounded-control text-muted",
              "transition-colors duration-150 hover:bg-surface hover:text-ink",
              "focus-visible:ring-2 focus-visible:ring-accent/40 focus-visible:outline-none",
            )}
          >
            <HugeiconsIcon icon={Cancel01Icon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
          </SheetPrimitive.Close>
        )}
      </SheetPrimitive.Popup>
    </SheetPrimitive.Portal>
  );
}

function SheetHeader({ className, ...props }: React.ComponentProps<"div">) {
  return <div data-slot="sheet-header" className={cn("flex flex-col gap-1 pr-10", className)} {...props} />;
}

function SheetTitle({ className, ...props }: SheetPrimitive.Title.Props) {
  return (
    <SheetPrimitive.Title
      data-slot="sheet-title"
      className={cn("text-[14px] font-semibold text-ink", className)}
      {...props}
    />
  );
}

function SheetDescription({ className, ...props }: SheetPrimitive.Description.Props) {
  return (
    <SheetPrimitive.Description
      data-slot="sheet-description"
      className={cn("text-[12.5px] text-muted", className)}
      {...props}
    />
  );
}

export { Sheet, SheetTrigger, SheetClose, SheetContent, SheetHeader, SheetTitle, SheetDescription };
