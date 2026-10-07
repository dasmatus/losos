import * as React from "react";
import { cn } from "@/lib/utils";

/* shadcn/ui's Collapsible, Radix-free.
 *
 * Same three parts and the same props as shadcn's (`open`, `defaultOpen`,
 * `onOpenChange`; `data-state` on every part), implemented against plain
 * elements for the reason in button.tsx. The content folds with a grid-rows
 * transition from 0fr to 1fr, which needs no measured height and so no
 * runtime style at all; prefers-reduced-motion drops the transition with
 * every other one in the stylesheet. While closed the content is `inert`
 * and, once folded, `visibility: hidden`: folded links must not be
 * reachable by Tab or by a screen reader. */

interface CollapsibleContextValue {
  open: boolean;
  toggle: () => void;
  contentId: string;
}

const CollapsibleContext = React.createContext<CollapsibleContextValue | null>(null);

function useCollapsible(): CollapsibleContextValue {
  const context = React.useContext(CollapsibleContext);
  if (context === null) throw new Error("Collapsible parts must sit inside <Collapsible>");
  return context;
}

interface CollapsibleProps extends React.ComponentProps<"div"> {
  open?: boolean;
  defaultOpen?: boolean;
  onOpenChange?: (open: boolean) => void;
}

function Collapsible({ open, defaultOpen = false, onOpenChange, className, children, ...props }: CollapsibleProps) {
  const [own, setOwn] = React.useState(defaultOpen);
  const isOpen = open ?? own;
  const contentId = React.useId();
  const value = React.useMemo<CollapsibleContextValue>(
    () => ({
      open: isOpen,
      contentId,
      toggle: () => {
        if (open === undefined) setOwn(!isOpen);
        onOpenChange?.(!isOpen);
      },
    }),
    [isOpen, open, onOpenChange, contentId],
  );
  return (
    <CollapsibleContext.Provider value={value}>
      <div data-slot="collapsible" data-state={isOpen ? "open" : "closed"} className={className} {...props}>
        {children}
      </div>
    </CollapsibleContext.Provider>
  );
}

function CollapsibleTrigger({ className, onClick, ...props }: React.ComponentProps<"button">) {
  const { open, toggle, contentId } = useCollapsible();
  return (
    <button
      type="button"
      data-slot="collapsible-trigger"
      data-state={open ? "open" : "closed"}
      aria-expanded={open}
      aria-controls={contentId}
      className={className}
      onClick={(event) => {
        onClick?.(event);
        if (!event.defaultPrevented) toggle();
      }}
      {...props}
    />
  );
}

function CollapsibleContent({ className, children, ...props }: React.ComponentProps<"div">) {
  const { open, contentId } = useCollapsible();
  return (
    <div
      id={contentId}
      data-slot="collapsible-content"
      data-state={open ? "open" : "closed"}
      inert={!open}
      className={cn(
        "grid transition-[grid-template-rows,opacity,visibility] duration-200 ease-out",
        // visibility switches discretely, at the end of a fold and at the
        // start of an unfold, so a folded entry is hidden (not merely
        // clipped) once the fold has run.
        "data-[state=closed]:invisible data-[state=closed]:grid-rows-[0fr] data-[state=closed]:opacity-0",
        "data-[state=open]:visible data-[state=open]:grid-rows-[1fr] data-[state=open]:opacity-100",
        className,
      )}
      {...props}
    >
      <div className="min-h-0 overflow-hidden">{children}</div>
    </div>
  );
}

export { Collapsible, CollapsibleTrigger, CollapsibleContent, useCollapsible };
