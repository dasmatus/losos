/* Small shapes the four steps share.
 *
 * Not candidates for src/components/ui: a callout with a tone and a
 * definition row are wizard furniture, and promoting them would make two
 * places to change one look. If a fifth screen wants them, move them then.
 */

import type * as React from "react";
import { HugeiconsIcon, type IconSvgElement } from "@hugeicons/react";
import { cn } from "@/lib/utils";

export type Tone = "info" | "ok" | "warn" | "crit";

const TONE_BOX: Record<Tone, string> = {
  info: "border-line bg-sunk",
  ok: "border-ok/25 bg-ok/10",
  warn: "border-warn/25 bg-warn/10",
  crit: "border-crit/25 bg-crit/10",
};

const TONE_ICON: Record<Tone, string> = {
  info: "text-accent",
  ok: "text-ok",
  warn: "text-warn",
  crit: "text-crit",
};

export interface CalloutProps extends React.ComponentPropsWithoutRef<"div"> {
  tone?: Tone;
  icon: IconSvgElement;
  title?: string;
}

/** A bordered note with an icon. The icon carries the tone; the text does not,
 *  so a paragraph inside stays as readable as the rest of the card. */
export function Callout({
  tone = "info",
  icon,
  title,
  className,
  children,
  ...props
}: CalloutProps) {
  return (
    <div
      className={cn(
        "flex items-start gap-3 rounded-control border px-3.5 py-3",
        "animate-fade-in",
        TONE_BOX[tone],
        className,
      )}
      {...props}
    >
      <HugeiconsIcon
        icon={icon}
        size={19}
        strokeWidth={1.5}
        color="currentColor"
        className={cn("mt-px shrink-0", TONE_ICON[tone])}
        aria-hidden="true"
      />
      <div className="min-w-0 flex-1 text-[13px] leading-snug text-muted">
        {title !== undefined && <p className="font-medium text-ink">{title}</p>}
        {children}
      </div>
    </div>
  );
}

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
