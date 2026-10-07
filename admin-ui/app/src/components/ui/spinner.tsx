import type * as React from "react";
import { HugeiconsIcon } from "@hugeicons/react";
import { Loading03Icon } from "@hugeicons/core-free-icons";
import { useT } from "@/lib/i18n-react";
import { cn } from "@/lib/utils";

export interface SpinnerProps extends React.ComponentPropsWithoutRef<"span"> {
  size?: number;
  label?: string;
}

/* shadcn/ui Spinner, on the palette: a spinning line icon with role="status".
 * The label is the only thing a screen reader gets, so it is never empty —
 * unlabelled, it says "Working" in the interface language. */
export function Spinner({ size = 18, label, className, ...props }: SpinnerProps) {
  const t = useT();
  return (
    <span
      role="status"
      data-slot="spinner"
      className={cn("inline-flex items-center", className)}
      {...props}
    >
      <HugeiconsIcon
        icon={Loading03Icon}
        size={size}
        strokeWidth={1.5}
        color="currentColor"
        className="animate-spin-slow"
        aria-hidden="true"
      />
      <span className="sr-only">{label ?? t("ui.working")}</span>
    </span>
  );
}
