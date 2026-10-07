import type * as React from "react";
import { cva, type VariantProps } from "class-variance-authority";
import { Label } from "@/components/ui/label";
import { cn } from "@/lib/utils";

/* shadcn/ui Field, on the palette.
 *
 * A field is a label, a control, a description and an error, in that order,
 * sharing one `data-invalid` and one `data-disabled` on the wrapper so the
 * label greys out with its control and the description reddens with the
 * error. Before this the sign-in dialog, the wizard's password form and the
 * market's quantity box each wired `useId`, `aria-invalid` and
 * `aria-describedby` by hand; the composition here is the same wiring, once.
 *
 * The control is whatever child the caller puts between FieldLabel and
 * FieldDescription — an <Input>, an <InputGroup>, a <NativeSelect>. Field does
 * not clone it: the caller still sets `id`, `aria-invalid` and
 * `aria-describedby` on the control, because those have to be right for a
 * screen reader whether or not a wrapper agrees. */

export const fieldVariants = cva("group/field flex w-full gap-2 data-[invalid=true]:text-crit", {
  variants: {
    orientation: {
      vertical: "flex-col [&>*]:w-full [&>.sr-only]:w-auto",
      horizontal: cn(
        "flex-row items-center",
        "[&>[data-slot=field-label]]:flex-auto",
        "has-[>[data-slot=field-content]]:items-start has-[>[data-slot=field-content]]:[&>[data-slot=switch]]:mt-px",
      ),
      responsive: cn(
        "flex-col [&>*]:w-full [&>.sr-only]:w-auto",
        "@md/field-group:flex-row @md/field-group:items-center @md/field-group:[&>*]:w-auto",
        "@md/field-group:[&>[data-slot=field-label]]:flex-auto",
        "@md/field-group:has-[>[data-slot=field-content]]:items-start",
      ),
    },
  },
  defaultVariants: { orientation: "vertical" },
});

export interface FieldProps
  extends React.ComponentPropsWithoutRef<"div">,
    VariantProps<typeof fieldVariants> {
  /** Mirrors the control's aria-invalid onto the wrapper. */
  invalid?: boolean;
  disabled?: boolean;
}

export function Field({ className, orientation, invalid, disabled, ...props }: FieldProps) {
  return (
    <div
      role="group"
      data-slot="field"
      data-orientation={orientation ?? "vertical"}
      data-invalid={invalid === true ? "true" : undefined}
      data-disabled={disabled === true ? "true" : undefined}
      className={cn(fieldVariants({ orientation }), className)}
      {...props}
    />
  );
}

/** Several fields with one vertical rhythm. A container query root, so a
 *  `responsive` field inside it reads the group's width, not the window's. */
export function FieldGroup({ className, ...props }: React.ComponentPropsWithoutRef<"div">) {
  return (
    <div
      data-slot="field-group"
      className={cn(
        "group/field-group @container/field-group flex w-full flex-col gap-4",
        "[&>[data-slot=field-group]]:gap-3",
        className,
      )}
      {...props}
    />
  );
}

export function FieldSet({ className, ...props }: React.ComponentPropsWithoutRef<"fieldset">) {
  return (
    <fieldset
      data-slot="field-set"
      className={cn(
        "flex flex-col gap-4",
        "has-[>[data-slot=checkbox-group]]:gap-3 has-[>[data-slot=radio-group]]:gap-3",
        className,
      )}
      {...props}
    />
  );
}

export function FieldLegend({
  className,
  variant = "legend",
  ...props
}: React.ComponentPropsWithoutRef<"legend"> & { variant?: "legend" | "label" }) {
  return (
    <legend
      data-slot="field-legend"
      data-variant={variant}
      className={cn(
        "mb-2 font-medium text-ink",
        "data-[variant=legend]:text-[15px]",
        "data-[variant=label]:text-sm",
        className,
      )}
      {...props}
    />
  );
}

/** The right-hand side of a horizontal field: label and description stacked
 *  beside a control that sits on its own. */
export function FieldContent({ className, ...props }: React.ComponentPropsWithoutRef<"div">) {
  return (
    <div
      data-slot="field-content"
      className={cn("group/field-content flex flex-1 flex-col gap-1 leading-snug", className)}
      {...props}
    />
  );
}

export function FieldLabel({ className, ...props }: React.ComponentPropsWithoutRef<"label">) {
  return (
    <Label
      data-slot="field-label"
      className={cn(
        "group/field-label peer/field-label flex w-fit gap-2 leading-snug",
        "group-data-[disabled=true]/field:text-faint",
        "has-[>[data-slot=field]]:w-full has-[>[data-slot=field]]:flex-col has-[>[data-slot=field]]:rounded-control",
        "has-[>[data-slot=field]]:border has-[>[data-slot=field]]:border-line has-[>[data-slot=field]]:p-3",
        "has-data-[state=checked]:border-accent has-data-[state=checked]:bg-accent-wash",
        className,
      )}
      {...props}
    />
  );
}

export function FieldTitle({ className, ...props }: React.ComponentPropsWithoutRef<"div">) {
  return (
    <div
      data-slot="field-label"
      className={cn(
        "flex w-fit items-center gap-2 text-sm leading-snug font-medium",
        "group-data-[disabled=true]/field:text-faint",
        className,
      )}
      {...props}
    />
  );
}

/** The explanatory line of a field. Not a placeholder, not a tooltip. */
export function FieldDescription({ className, ...props }: React.ComponentPropsWithoutRef<"p">) {
  return (
    <p
      data-slot="field-description"
      className={cn(
        "text-[12.5px] leading-snug font-normal text-muted",
        "group-has-[[data-orientation=horizontal]]/field:text-balance",
        "nth-last-2:-mt-1 [[data-variant=legend]+&]:-mt-1.5",
        "[&>a]:text-accent [&>a]:underline-offset-4 [&>a:hover]:underline",
        className,
      )}
      {...props}
    />
  );
}

export function FieldSeparator({
  children,
  className,
  ...props
}: React.ComponentPropsWithoutRef<"div">) {
  return (
    <div
      data-slot="field-separator"
      data-content={children !== undefined && children !== null ? "" : undefined}
      className={cn(
        "relative -my-2 h-5 text-sm group-data-[variant=outline]/field-group:-mb-2",
        className,
      )}
      {...props}
    >
      <div className="absolute inset-x-0 top-1/2 h-px -translate-y-1/2 bg-hair" />
      {children !== undefined && children !== null && (
        <span
          className="relative mx-auto block w-fit bg-surface px-2 text-muted"
          data-slot="field-separator-content"
        >
          {children}
        </span>
      )}
    </div>
  );
}

export interface FieldErrorProps extends React.ComponentPropsWithoutRef<"p"> {
  /** shadcn's shape: the error objects a form library hands back. One is
   *  shown as a line, several as a list, duplicates folded. */
  errors?: Array<{ message?: string } | undefined>;
}

/** The error line under a field. Rendered as a live region, so a validation
 *  failure is announced when it happens rather than on the next focus.
 *  Renders nothing when there is nothing to say, so callers can leave it
 *  mounted and only ever change its children. */
export function FieldError({ className, children, errors, ...props }: FieldErrorProps) {
  let content: React.ReactNode = children;
  if (content === undefined || content === null || content === false) {
    const messages = [
      ...new Set(
        (errors ?? [])
          .map((error) => error?.message)
          .filter((message): message is string => typeof message === "string" && message !== ""),
      ),
    ];
    if (messages.length === 0) return null;
    content =
      messages.length === 1 ? (
        messages[0]
      ) : (
        <ul className="ml-4 flex list-disc flex-col gap-1">
          {messages.map((message) => (
            <li key={message}>{message}</li>
          ))}
        </ul>
      );
  }
  return (
    <p
      role="alert"
      data-slot="field-error"
      className={cn("text-[12.5px] leading-snug font-normal text-crit", className)}
      {...props}
    >
      {content}
    </p>
  );
}
