import { Drawer as DrawerPrimitive } from "@base-ui/react/drawer";
import { HugeiconsIcon } from "@hugeicons/react";
import { Cancel01Icon } from "@hugeicons/core-free-icons";
import { cn } from "@/lib/utils";

/* A bottom sheet on Base UI's Drawer: the Sidebar's panel on a phone, opened
 * from the bottom bar's More (components/ui/bottom-nav.tsx), so the whole
 * list comes up under the thumb that asked for it.
 *
 * Base UI for the same reason as the Sheet beside it: its scroll lock and
 * its swipe are CSSOM writes (the swipe distance arrives as
 * --drawer-swipe-movement-y on the popup's style), which the admin CSP
 * allows, and nothing in it injects a <style> element. A flick down closes
 * it; the handle at the top is where a thumb takes hold, and is drawn only
 * because the gesture is real. It arrives on ease-snap and leaves faster on
 * ease-exit; a swipe that lets go lands from where the finger left it. */

function Drawer(props: DrawerPrimitive.Root.Props) {
  return <DrawerPrimitive.Root data-slot="drawer" swipeDirection="down" {...props} />;
}

function DrawerContent({
  className,
  children,
  closeLabel,
  ...props
}: DrawerPrimitive.Popup.Props & {
  /** The close button's accessible name; no button when omitted. */
  closeLabel?: string;
}) {
  return (
    <DrawerPrimitive.Portal>
      <DrawerPrimitive.Backdrop
        data-slot="drawer-overlay"
        className={cn(
          "fixed inset-0 z-50 bg-black/30 transition-opacity duration-200 ease-snap",
          "opacity-[calc(1-var(--drawer-swipe-progress,0))] data-swiping:duration-0",
          "data-starting-style:opacity-0 data-ending-style:opacity-0 data-ending-style:duration-150 data-ending-style:ease-exit",
        )}
      />
      <DrawerPrimitive.Viewport data-slot="drawer-viewport" className="fixed inset-0 z-50 flex items-end justify-center">
        <DrawerPrimitive.Popup
          data-slot="drawer-content"
          className={cn(
            "relative flex max-h-[85dvh] w-full flex-col gap-3 overflow-y-auto overscroll-contain",
            "rounded-t-card border-t border-line bg-sunk px-3 pt-2 pb-[calc(0.75rem+env(safe-area-inset-bottom,0px))] shadow-pop outline-none",
            "translate-y-[var(--drawer-swipe-movement-y,0px)] transition-[translate] duration-220 ease-snap data-swiping:duration-0 data-swiping:select-none",
            "data-starting-style:translate-y-full data-ending-style:translate-y-full data-ending-style:duration-150 data-ending-style:ease-exit",
            className,
          )}
          {...props}
        >
          <div aria-hidden="true" className="mx-auto h-1 w-10 shrink-0 rounded-full bg-line" />
          {children}
          {closeLabel !== undefined && (
            <DrawerPrimitive.Close
              data-slot="drawer-close"
              aria-label={closeLabel}
              className={cn(
                "absolute top-6 right-3 flex size-8 items-center justify-center rounded-control text-muted",
                "transition-colors duration-150 hover:bg-surface hover:text-ink",
                "focus-visible:ring-2 focus-visible:ring-accent/40 focus-visible:outline-none",
              )}
            >
              <HugeiconsIcon icon={Cancel01Icon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
            </DrawerPrimitive.Close>
          )}
        </DrawerPrimitive.Popup>
      </DrawerPrimitive.Viewport>
    </DrawerPrimitive.Portal>
  );
}

function DrawerTitle({ className, ...props }: DrawerPrimitive.Title.Props) {
  return <DrawerPrimitive.Title data-slot="drawer-title" className={cn("text-[14px] font-semibold text-ink", className)} {...props} />;
}

function DrawerDescription({ className, ...props }: DrawerPrimitive.Description.Props) {
  return <DrawerPrimitive.Description data-slot="drawer-description" className={cn("text-[12.5px] text-muted", className)} {...props} />;
}

export { Drawer, DrawerContent, DrawerTitle, DrawerDescription };
