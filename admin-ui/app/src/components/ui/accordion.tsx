import type * as React from "react";
import { Accordion as AccordionPrimitive } from "@base-ui/react/accordion";
import { HugeiconsIcon } from "@hugeicons/react";
import { ArrowDown01Icon } from "@hugeicons/core-free-icons";
import { cn } from "@/lib/utils";

/* shadcn/ui's Accordion from its base registry, on Base UI, for the same
 * reason as collapsible.tsx: the panel's measured height arrives as
 * --accordion-panel-height through CSSOM, which the admin CSP allows, and
 * nothing injects a <style> element, which it refuses.
 *
 * Laid out for a settings pane rather than as shadcn's bordered list: each
 * trigger is a card the width of the pane (the look of a `Group` holding one
 * row), and its panel holds ordinary sections under it. The panel stays
 * mounted while closed, `hidden`, so a folded section keeps its draft and
 * is out of the tab order. */

function Accordion({ className, ...props }: AccordionPrimitive.Root.Props) {
  return (
    <AccordionPrimitive.Root
      data-slot="accordion"
      multiple
      className={cn("mb-6 flex flex-col gap-3 last:mb-0", className)}
      {...props}
    />
  );
}

function AccordionItem({ className, ...props }: AccordionPrimitive.Item.Props) {
  return <AccordionPrimitive.Item data-slot="accordion-item" className={cn("flex flex-col", className)} {...props} />;
}

interface AccordionTriggerProps extends AccordionPrimitive.Trigger.Props {
  /** A line under the title: what is inside, at a glance. */
  detail?: React.ReactNode;
}

function AccordionTrigger({ className, children, detail, ...props }: AccordionTriggerProps) {
  return (
    <AccordionPrimitive.Header className="m-0">
      <AccordionPrimitive.Trigger
        data-slot="accordion-trigger"
        className={cn(
          "group/accordion flex w-full items-center justify-between gap-4 px-3.5 py-3 text-left",
          "rounded-card border border-line bg-surface text-ink shadow-card",
          "transition-colors duration-150 hover:bg-sunk/60",
          "outline-none focus-visible:ring-2 focus-visible:ring-accent/40",
          className,
        )}
        {...props}
      >
        <span className="min-w-0 flex-1">
          <span className="block text-sm leading-snug font-medium text-ink">{children}</span>
          {detail !== undefined && detail !== null && (
            <span className="mt-0.5 block text-[12.5px] leading-snug text-muted">{detail}</span>
          )}
        </span>
        <HugeiconsIcon
          icon={ArrowDown01Icon}
          size={16}
          strokeWidth={1.5}
          color="currentColor"
          aria-hidden="true"
          className="shrink-0 text-faint transition-transform duration-200 group-data-[panel-open]/accordion:rotate-180"
        />
      </AccordionPrimitive.Trigger>
    </AccordionPrimitive.Header>
  );
}

function AccordionContent({ className, keepMounted = true, children, ...props }: AccordionPrimitive.Panel.Props) {
  return (
    <AccordionPrimitive.Panel
      data-slot="accordion-content"
      keepMounted={keepMounted}
      className={cn(
        "h-(--accordion-panel-height) overflow-hidden transition-[height] duration-200 ease-out",
        "data-starting-style:h-0 data-ending-style:h-0",
      )}
      {...props}
    >
      <div className={cn("pt-4 pb-2", className)}>{children}</div>
    </AccordionPrimitive.Panel>
  );
}

export { Accordion, AccordionItem, AccordionTrigger, AccordionContent };
