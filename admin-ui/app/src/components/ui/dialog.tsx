import * as React from "react";
import { cn } from "@/lib/utils";

export interface DialogProps extends Omit<React.ComponentPropsWithoutRef<"div">, "onCancel"> {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  /** Escape and a backdrop click close it. Off for a prompt that must be
   *  answered — the sign-in overlay, a confirmation that is already running. */
  dismissible?: boolean;
  /** id of the DialogTitle. Wire it, or the dialog announces as unlabelled. */
  labelledBy?: string;
  describedBy?: string;
  /** Classes for the <dialog> box itself (width, mostly). */
  dialogClassName?: string;
}

/* Built on the native <dialog> element.
 *
 * showModal() brings the top layer, the ::backdrop pseudo-element, the focus
 * trap, the inert background and the Escape handling — all of it, from the
 * platform. The alternative was Radix Dialog, which drags in a scroll lock
 * that injects a <style> element; under the appliance's `style-src 'self'`
 * that element is ignored and the page behind the modal keeps scrolling, with
 * nothing but a console warning on a box nobody has a console for.
 *
 * The old plain-JS UI hand-rolled the same thing with `inert` plus a Tab
 * handler (admin-ui/common.js). Native <dialog> is that, minus the code. */
export function Dialog({
  open,
  onOpenChange,
  dismissible = true,
  labelledBy,
  describedBy,
  className,
  dialogClassName,
  children,
  ...props
}: DialogProps) {
  const ref = React.useRef<HTMLDialogElement>(null);

  React.useEffect(() => {
    const dialog = ref.current;
    if (dialog === null) return;
    if (open && !dialog.open) dialog.showModal();
    else if (!open && dialog.open) dialog.close();
  }, [open]);

  return (
    <dialog
      ref={ref}
      aria-labelledby={labelledBy}
      aria-describedby={describedBy}
      className={cn("w-[min(26rem,calc(100vw-2rem))]", dialogClassName)}
      // Fires for Escape and for form method="dialog". Both are the user
      // asking to leave; a non-dismissible dialog refuses.
      onCancel={(event) => {
        if (!dismissible) {
          event.preventDefault();
          return;
        }
        onOpenChange(false);
      }}
      // Whatever closed it — Escape, .close(), a submit — the parent's state
      // has to follow, or `open` stays true and the dialog never reopens.
      onClose={() => onOpenChange(false)}
      // A click that lands on the <dialog> itself landed on the backdrop: the
      // panel below covers the box edge to edge.
      onClick={(event) => {
        if (dismissible && event.target === ref.current) onOpenChange(false);
      }}
    >
      <div
        className={cn(
          "animate-pop rounded-card border border-line bg-surface text-ink shadow-pop",
          "max-h-[calc(100dvh-4rem)] overflow-y-auto",
          className,
        )}
        {...props}
      >
        {children}
      </div>
    </dialog>
  );
}

export function DialogHeader({ className, ...props }: React.ComponentPropsWithoutRef<"div">) {
  return <div className={cn("flex flex-col gap-1.5 px-5 pt-5 pb-2", className)} {...props} />;
}

export function DialogTitle({ className, ...props }: React.ComponentPropsWithoutRef<"h2">) {
  return <h2 className={cn("text-base leading-tight font-semibold", className)} {...props} />;
}

export function DialogDescription({ className, ...props }: React.ComponentPropsWithoutRef<"p">) {
  return <p className={cn("text-[13px] leading-snug text-muted", className)} {...props} />;
}

export function DialogBody({ className, ...props }: React.ComponentPropsWithoutRef<"div">) {
  return <div className={cn("px-5 py-3", className)} {...props} />;
}

export function DialogFooter({ className, ...props }: React.ComponentPropsWithoutRef<"div">) {
  return (
    <div
      className={cn(
        "flex flex-wrap items-center justify-end gap-2 border-t border-hair px-5 py-3.5",
        className,
      )}
      {...props}
    />
  );
}
