import * as React from "react";
import { HugeiconsIcon } from "@hugeicons/react";
import { Loading03Icon } from "@hugeicons/core-free-icons";
import { cn, setCssVar } from "@/lib/utils";

export interface ProgressProps extends React.ComponentPropsWithoutRef<"div"> {
  /** 0–100. Omit — or pass null — for the indeterminate band. */
  value?: number | null;
  label?: string;
}

/* A progress bar whose fill width is a CSS custom property.
 *
 * The width is genuinely dynamic and there is no set of classes that covers
 * 0–100, so it goes onto the element as `--losos-progress` through the CSSOM
 * (see setCssVar) and the stylesheet reads it back with
 * `w-[var(--losos-progress,0%)]`. That is not an inline style attribute, which
 * is what the appliance CSP refuses, and the stylesheet still owns what the
 * value means.
 *
 * lososd reports 0 for the whole of a rebuild and 100 on success, so in
 * practice the indeterminate band is what a rebuild looks like. */
export function Progress({ value = null, label, className, ...props }: ProgressProps) {
  const fill = React.useRef<HTMLDivElement>(null);
  const clamped = value === null ? null : Math.min(100, Math.max(0, Math.round(value)));

  React.useEffect(() => {
    if (clamped !== null) setCssVar(fill.current, "--losos-progress", `${clamped}%`);
  }, [clamped]);

  return (
    <div
      role="progressbar"
      aria-label={label}
      aria-valuemin={0}
      aria-valuemax={100}
      aria-valuenow={clamped ?? undefined}
      aria-valuetext={clamped === null ? "Working" : `${clamped}%`}
      className={cn("h-1.5 w-full overflow-hidden rounded-full bg-sunk", className)}
      {...props}
    >
      {clamped === null ? (
        <div className="h-full w-1/3 animate-sweep rounded-full bg-accent" />
      ) : (
        <div
          ref={fill}
          className={cn(
            "h-full w-[var(--losos-progress,0%)] rounded-full bg-accent",
            "transition-[width] duration-300 ease-out",
          )}
        />
      )}
    </div>
  );
}

export interface SpinnerProps extends React.ComponentPropsWithoutRef<"span"> {
  size?: number;
  label?: string;
}

/** A spinner. The label is the only thing a screen reader gets. */
export function Spinner({ size = 18, label = "Working", className, ...props }: SpinnerProps) {
  return (
    <span role="status" className={cn("inline-flex items-center", className)} {...props}>
      <HugeiconsIcon
        icon={Loading03Icon}
        size={size}
        strokeWidth={1.5}
        color="currentColor"
        className="animate-spin-slow"
        aria-hidden="true"
      />
      <span className="sr-only">{label}</span>
    </span>
  );
}
