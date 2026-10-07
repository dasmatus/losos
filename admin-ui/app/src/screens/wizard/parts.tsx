/* Small shapes the four steps share.
 *
 * Not candidates for src/components/ui: a definition row and a step's body
 * copy are wizard furniture, and promoting them would make two places to
 * change one look. The callout that used to live here is gone — it was a
 * shadcn Alert in all but name, and now it is one (components/ui/alert.tsx).
 */

import type * as React from "react";
import { cn } from "@/lib/utils";

/** A label above a value that the owner is meant to read character by
 *  character — a fingerprint, an account name, an address. */
export function ReadoutRow({
  label,
  children,
  className,
  ...props
}: React.ComponentPropsWithoutRef<"div"> & { label: string }) {
  return (
    <div className={cn("flex flex-col gap-1", className)} {...props}>
      <span className="text-[12px] font-medium tracking-wide text-faint uppercase">{label}</span>
      {children}
    </div>
  );
}

/** The body copy of a step: one or two sentences, never a wall. */
export function StepText({ className, ...props }: React.ComponentPropsWithoutRef<"p">) {
  return <p className={cn("text-[13.5px] leading-relaxed text-muted", className)} {...props} />;
}
