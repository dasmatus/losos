import {
  composeRenderProps,
  Switch as SwitchPrimitive,
  type SwitchProps as SwitchPrimitiveProps,
} from "react-aria-components";
import { cn } from "@/lib/utils";

export interface SwitchProps extends Omit<SwitchPrimitiveProps, "className"> {
  className?: string;
  size?: "sm" | "default";
}

/* shadcn/ui's Switch, React Aria flavour, on the palette.
 *
 * React Aria renders a <label> holding a visually hidden
 * <input type="checkbox" role="switch">, so the control is a real form
 * input: Space toggles it, a screen reader hears "switch, on", and an
 * outside <label htmlFor> or FieldLabel reaches it through `id`, which
 * React Aria puts on the input. The track and thumb are the two spans
 * below, styled off the data attributes React Aria sets on the label
 * (data-selected, data-disabled, data-focus-visible).
 *
 * Under the appliance CSP (`style-src 'self'`), checked in Chromium against
 * the real header: VisuallyHidden hides the input through a React style
 * prop, which is a CSSOM write and allowed, and a page full of switches
 * raises no violation beyond the two Sonner already logs at load. React
 * Aria's press handling can also add a `<style>` element of its own (a
 * `touch-action` rule for `[data-react-aria-pressable]`); the Switch does
 * not trigger it in 1.21, and index.css ships the same rule from 'self' in
 * case a later version or another pressable does.
 *
 * The look is the hand-made switch it replaced: a 44x24 track on --sunk
 * that fills with the accent, a --surface thumb. `sm` is shadcn's second size. */
export function Switch({ className, size = "default", children, ...props }: SwitchProps) {
  return (
    <SwitchPrimitive
      data-slot="switch"
      data-size={size}
      className={cn(
        "group/switch relative inline-flex shrink-0 cursor-pointer items-center rounded-full",
        "border border-line bg-sunk p-0.5 outline-none",
        "transition-colors duration-200 ease-out",
        "data-[size=default]:h-6 data-[size=default]:w-11",
        "data-[size=sm]:h-5 data-[size=sm]:w-9",
        "data-selected:border-transparent data-selected:bg-accent",
        "data-focus-visible:ring-2 data-focus-visible:ring-accent/40",
        "data-disabled:cursor-not-allowed data-disabled:opacity-45",
        // A bigger hit area than the track, as shadcn's has, without moving it.
        "after:absolute after:-inset-x-2 after:-inset-y-2 after:content-['']",
        className,
      )}
      {...props}
    >
      {composeRenderProps(children, (children, { isSelected }) => (
        <>
          <span
            data-slot="switch-thumb"
            data-selected={isSelected || undefined}
            aria-hidden="true"
            className={cn(
              "pointer-events-none block rounded-full bg-surface shadow-sm",
              "transition-transform duration-200 ease-out",
              "group-data-[size=default]/switch:size-5 group-data-[size=sm]/switch:size-4",
              "group-data-[size=default]/switch:data-selected:translate-x-5",
              "group-data-[size=sm]/switch:data-selected:translate-x-4",
            )}
          />
          {children}
        </>
      ))}
    </SwitchPrimitive>
  );
}
