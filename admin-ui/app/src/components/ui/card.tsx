import type * as React from "react";
import { cn } from "@/lib/utils";

/** A surface panel. 9px radius, hairline border, light-mode-only depth. */
export function Card({ className, ...props }: React.ComponentPropsWithoutRef<"div">) {
  return (
    <div
      className={cn(
        "rounded-card border border-line bg-surface text-ink shadow-card",
        "animate-rise",
        className,
      )}
      {...props}
    />
  );
}

export function CardHeader({ className, ...props }: React.ComponentPropsWithoutRef<"div">) {
  return <div className={cn("flex flex-col gap-1 px-5 pt-5 pb-3", className)} {...props} />;
}

export function CardTitle({ className, ...props }: React.ComponentPropsWithoutRef<"h2">) {
  return <h2 className={cn("text-[15px] leading-tight font-semibold", className)} {...props} />;
}

export function CardDescription({ className, ...props }: React.ComponentPropsWithoutRef<"p">) {
  return <p className={cn("text-[13px] leading-snug text-muted", className)} {...props} />;
}

export function CardContent({ className, ...props }: React.ComponentPropsWithoutRef<"div">) {
  return <div className={cn("px-5 pb-5", className)} {...props} />;
}

export function CardFooter({ className, ...props }: React.ComponentPropsWithoutRef<"div">) {
  return (
    <div
      className={cn("flex items-center gap-2 border-t border-hair px-5 py-3.5", className)}
      {...props}
    />
  );
}

/* A row inside a card, the settings-list shape: label on the left, control on
 * the right, hairline between rows and none after the last. */
export function CardRow({ className, ...props }: React.ComponentPropsWithoutRef<"div">) {
  return (
    <div
      className={cn(
        "flex items-center justify-between gap-4 px-5 py-3.5",
        "border-b border-hair last:border-b-0",
        className,
      )}
      {...props}
    />
  );
}
