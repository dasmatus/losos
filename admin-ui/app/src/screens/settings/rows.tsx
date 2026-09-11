import type * as React from "react";
import { cn } from "@/lib/utils";

/* The macOS System Settings row vocabulary.
 *
 * Three rules hold everywhere below, and they are what make a list of rows
 * read as one settings pane rather than as a stack of unrelated widgets:
 *
 *   1. A row is label on the left, control on the right, 42px minimum. The
 *      control is the answer; the label is the question.
 *   2. The hairline between rows is INSET 14px from the left, not edge to
 *      edge, and there is none after the last row. That inset is the tell —
 *      it is what says "these rows belong to each other" without a heading.
 *   3. The explanation lives in a caption UNDER the group, once. A sentence
 *      hung off every row turns a scannable list into a wall of prose, and
 *      the thing being explained is usually the group, not the row.
 *
 * The hairline is an ::after pseudo-element rather than a border-bottom
 * because a border cannot be inset. It is also why `Row` is `relative`.
 */

/** A card of rows. Give it `<Row>` children and nothing else. */
export function Group({ className, ...props }: React.ComponentPropsWithoutRef<"div">) {
  return (
    <div
      className={cn(
        "overflow-hidden rounded-card border border-line bg-surface text-ink shadow-card",
        className,
      )}
      {...props}
    />
  );
}

/** The small label above a group. Optional — a single group needs no title. */
export function GroupTitle({ className, ...props }: React.ComponentPropsWithoutRef<"h2">) {
  return (
    <h2
      className={cn("px-1.5 pb-1.5 text-[12.5px] font-medium text-muted", className)}
      {...props}
    />
  );
}

/** The explanation under a group. One per group, not one per row. */
export function GroupCaption({ className, ...props }: React.ComponentPropsWithoutRef<"p">) {
  return (
    <p
      className={cn("px-1.5 pt-2 text-[12.5px] leading-snug text-muted", className)}
      {...props}
    />
  );
}

export interface RowProps extends React.ComponentPropsWithoutRef<"div"> {
  /** Drop the hairline under this row even when it is not the last one. */
  last?: boolean;
}

/** One row. `px-3.5` is the 14px the hairline is inset by; keep them equal. */
export function Row({ className, last, ...props }: RowProps) {
  return (
    <div
      className={cn(
        "relative flex min-h-[42px] items-center justify-between gap-4 px-3.5 py-2",
        "after:pointer-events-none after:absolute after:right-0 after:bottom-0",
        "after:left-3.5 after:h-px after:bg-hair after:content-['']",
        "last:after:hidden",
        last === true && "after:hidden",
        className,
      )}
      {...props}
    />
  );
}

/* A row whose content stacks instead of sitting on one line — the capacity
 * meter, the hour strip. Same padding and hairline, no right-hand control. */
export function StackRow({ className, last, ...props }: RowProps) {
  return (
    <div
      className={cn(
        "relative flex flex-col gap-2.5 px-3.5 py-3",
        "after:pointer-events-none after:absolute after:right-0 after:bottom-0",
        "after:left-3.5 after:h-px after:bg-hair after:content-['']",
        "last:after:hidden",
        last === true && "after:hidden",
        className,
      )}
      {...props}
    />
  );
}

export interface RowTextProps extends React.ComponentPropsWithoutRef<"div"> {
  title: React.ReactNode;
  /* Secondary text that IS the row's content — a measured value, a
   * consequence. Not for explaining the control: that is GroupCaption. */
  detail?: React.ReactNode;
  htmlFor?: string;
}

/** The left-hand side of a row. Renders a <label> when given `htmlFor`. */
export function RowText({ title, detail, htmlFor, className, ...props }: RowTextProps) {
  return (
    <div className={cn("min-w-0 flex-1", className)} {...props}>
      {htmlFor === undefined ? (
        <p className="text-sm leading-snug text-ink">{title}</p>
      ) : (
        <label htmlFor={htmlFor} className="text-sm leading-snug text-ink select-none">
          {title}
        </label>
      )}
      {detail !== undefined && detail !== null && detail !== false && (
        <p className="mt-0.5 text-[12.5px] leading-snug text-muted">{detail}</p>
      )}
    </div>
  );
}

/** A read-only value on the right of a row: a name, a size, an address. */
export function RowValue({ className, ...props }: React.ComponentPropsWithoutRef<"span">) {
  return (
    <span
      className={cn("numeric shrink-0 text-[13px] text-muted tabular-nums", className)}
      {...props}
    />
  );
}

/** The heading of a pane. One per pane, above its first group. */
export function PaneHeader({
  title,
  summary,
}: {
  title: string;
  summary: string;
}) {
  return (
    <header className="mb-4">
      <h1 className="text-[19px] leading-tight font-semibold tracking-tight">{title}</h1>
      <p className="mt-1 max-w-prose text-[13px] leading-snug text-muted">{summary}</p>
    </header>
  );
}

/** Vertical rhythm between groups inside a pane. */
export function PaneSection({ className, ...props }: React.ComponentPropsWithoutRef<"section">) {
  return <section className={cn("mb-6 last:mb-0", className)} {...props} />;
}
