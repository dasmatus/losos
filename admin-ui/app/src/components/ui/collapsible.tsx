import { Collapsible as CollapsiblePrimitive } from "@base-ui/react/collapsible";
import { cn } from "@/lib/utils";

/* shadcn/ui's Collapsible from its base registry, on Base UI.
 *
 * Base UI rather than Radix because Base UI writes every style it needs
 * through CSSOM (the panel's measured height arrives as
 * --collapsible-panel-height on a React style prop), which the admin CSP
 * allows, and injects no <style> element, which it refuses. The trigger
 * carries aria-expanded and aria-controls and gets `data-panel-open` while
 * its panel is open; the panel carries `data-open`/`data-closed`.
 *
 * The content stays mounted when closed, with the `hidden` attribute, so a
 * folded entry is out of the tab order and the accessibility tree but its
 * height can animate on the way out (`data-ending-style`). */

function Collapsible(props: CollapsiblePrimitive.Root.Props) {
  return <CollapsiblePrimitive.Root data-slot="collapsible" {...props} />;
}

function CollapsibleTrigger(props: CollapsiblePrimitive.Trigger.Props) {
  return <CollapsiblePrimitive.Trigger data-slot="collapsible-trigger" {...props} />;
}

function CollapsibleContent({ className, keepMounted = true, ...props }: CollapsiblePrimitive.Panel.Props) {
  return (
    <CollapsiblePrimitive.Panel
      data-slot="collapsible-content"
      keepMounted={keepMounted}
      className={cn(
        "h-(--collapsible-panel-height) overflow-hidden transition-[height] duration-200 ease-out",
        "data-starting-style:h-0 data-ending-style:h-0",
        className,
      )}
      {...props}
    />
  );
}

export { Collapsible, CollapsibleTrigger, CollapsibleContent };
