/* The LosOS components an app is written with.
 *
 * An app the builder writes (backend/src/builder-app-prompt.md) or an owner
 * types is one React component that uses these by name: the box binds them
 * before the app's code runs (backend/src/jsx.rs, COMPONENTS). They are
 * deliberately few and plain, with the props a small model guesses right:
 * a label on every field, `onChange` with the browser's own event, `tone`
 * for colour. Everything is styled by ui.css on the box's palette, so an app
 * follows the light and dark theme without a colour of its own, and plain
 * HTML elements look the same as the components.
 *
 * Keep this list, the prompt's and jsx.rs's COMPONENTS in step: a component
 * the prompt names that is not here is a crash, and one here that the check
 * does not list is reported as a mistake.
 */

import * as React from "react";

type Children = { children?: React.ReactNode };

function cx(...names: (string | false | null | undefined)[]): string {
  return names.filter(Boolean).join(" ");
}

let fieldSeq = 0;
/** A stable id per field, so a label is tied to its control. */
function useFieldId(): string {
  const ref = React.useRef<string | null>(null);
  if (ref.current === null) {
    fieldSeq += 1;
    ref.current = `lx-field-${fieldSeq}`;
  }
  return ref.current;
}

const GAP_MAX = 64;
function gap(value: unknown, fallback: number): number {
  const n = typeof value === "number" && Number.isFinite(value) ? value : fallback;
  return Math.max(0, Math.min(GAP_MAX, n));
}

export function Page({ title, children }: Children & { title?: React.ReactNode }) {
  React.useEffect(() => {
    if (typeof title === "string" && title.length > 0) document.title = title;
  }, [title]);
  return (
    <main className="lx-page">
      {title !== undefined && title !== null && title !== "" && <h1 className="lx-page-title">{title}</h1>}
      <div className="lx-stack" style={{ gap: 16 }}>
        {children}
      </div>
    </main>
  );
}

export function Card({ title, children }: Children & { title?: React.ReactNode }) {
  return (
    <section className="lx-card">
      {title !== undefined && title !== null && title !== "" && <h2 className="lx-card-title">{title}</h2>}
      {children}
    </section>
  );
}

export function Stack({ gap: g, children }: Children & { gap?: number }) {
  return (
    <div className="lx-stack" style={{ gap: gap(g, 12) }}>
      {children}
    </div>
  );
}

const ALIGN: Record<string, string> = { start: "flex-start", center: "center", end: "flex-end", baseline: "baseline", stretch: "stretch" };
const JUSTIFY: Record<string, string> = {
  start: "flex-start",
  center: "center",
  end: "flex-end",
  between: "space-between",
  "space-between": "space-between",
  around: "space-around",
};

export function Row({
  gap: g,
  align,
  justify,
  wrap = true,
  children,
}: Children & { gap?: number; align?: string; justify?: string; wrap?: boolean }) {
  return (
    <div
      className="lx-row"
      style={{
        gap: gap(g, 8),
        alignItems: (align !== undefined && ALIGN[align]) || undefined,
        justifyContent: (justify !== undefined && JUSTIFY[justify]) || undefined,
        flexWrap: wrap === false ? "nowrap" : "wrap",
      }}
    >
      {children}
    </div>
  );
}

export function Text({
  muted,
  small,
  children,
}: Children & { muted?: boolean; small?: boolean }) {
  return <p className={cx("lx-text", muted === true && "lx-muted", small === true && "lx-small")}>{children}</p>;
}

export function Heading({ children }: Children) {
  return <h3 className="lx-heading">{children}</h3>;
}

type ButtonProps = Children & {
  onClick?: React.MouseEventHandler<HTMLButtonElement>;
  variant?: "primary" | "secondary" | "danger" | "ghost";
  small?: boolean;
  disabled?: boolean;
  type?: "button" | "submit" | "reset";
  title?: string;
};

export function Button({ onClick, variant = "primary", small, disabled, type, title, children }: ButtonProps) {
  return (
    <button
      // In a <form> with no onClick, a button submits, as the prompt's
      // example shows; with an onClick it is a plain button.
      type={type ?? (onClick === undefined ? "submit" : "button")}
      className={cx("lx-button", `lx-button-${variant}`, small === true && "lx-button-small")}
      onClick={onClick}
      disabled={disabled}
      title={title}
    >
      {children}
    </button>
  );
}

type FieldProps<E> = {
  label?: React.ReactNode;
  value?: string | number;
  onChange?: React.ChangeEventHandler<E>;
  placeholder?: string;
  disabled?: boolean;
};

function Field({ id, label, children }: { id: string; label?: React.ReactNode; children: React.ReactNode }) {
  return (
    <div className="lx-field">
      {label !== undefined && label !== null && label !== "" && (
        <label className="lx-label" htmlFor={id}>
          {label}
        </label>
      )}
      {children}
    </div>
  );
}

export function Input({
  label,
  value,
  onChange,
  placeholder,
  disabled,
  type = "text",
  min,
  max,
  step,
  onKeyDown,
}: FieldProps<HTMLInputElement> & {
  type?: string;
  min?: number | string;
  max?: number | string;
  step?: number | string;
  onKeyDown?: React.KeyboardEventHandler<HTMLInputElement>;
}) {
  const id = useFieldId();
  return (
    <Field id={id} label={label}>
      <input
        id={id}
        className="lx-input"
        type={type}
        value={value ?? ""}
        onChange={onChange ?? (() => undefined)}
        placeholder={placeholder}
        disabled={disabled}
        min={min}
        max={max}
        step={step}
        onKeyDown={onKeyDown}
      />
    </Field>
  );
}

export function TextArea({
  label,
  value,
  onChange,
  placeholder,
  disabled,
  rows = 4,
}: FieldProps<HTMLTextAreaElement> & { rows?: number }) {
  const id = useFieldId();
  return (
    <Field id={id} label={label}>
      <textarea
        id={id}
        className="lx-input lx-textarea"
        value={value ?? ""}
        onChange={onChange ?? (() => undefined)}
        placeholder={placeholder}
        disabled={disabled}
        rows={rows}
      />
    </Field>
  );
}

type Option = string | number | { value: string | number; label?: React.ReactNode };

export function Select({
  label,
  value,
  onChange,
  disabled,
  options = [],
  children,
}: FieldProps<HTMLSelectElement> & { options?: Option[]; children?: React.ReactNode }) {
  const id = useFieldId();
  return (
    <Field id={id} label={label}>
      <select
        id={id}
        className="lx-input lx-select"
        value={value ?? ""}
        onChange={onChange ?? (() => undefined)}
        disabled={disabled}
      >
        {/* <option> children, the way plain HTML writes them, work too. */}
        {children}
        {options.map((option) => {
          const v = typeof option === "object" ? option.value : option;
          const text = typeof option === "object" ? (option.label ?? option.value) : option;
          return (
            <option key={String(v)} value={v}>
              {text}
            </option>
          );
        })}
      </select>
    </Field>
  );
}

export function Checkbox({
  label,
  checked,
  onChange,
  disabled,
}: {
  label?: React.ReactNode;
  checked?: boolean;
  onChange?: React.ChangeEventHandler<HTMLInputElement>;
  disabled?: boolean;
}) {
  const id = useFieldId();
  return (
    <div className="lx-check">
      <input
        id={id}
        type="checkbox"
        checked={checked === true}
        onChange={onChange ?? (() => undefined)}
        disabled={disabled}
      />
      {label !== undefined && label !== null && label !== "" && <label htmlFor={id}>{label}</label>}
    </div>
  );
}

const TONES = new Set(["ok", "warn", "crit", "accent", "neutral"]);

export function Badge({ tone = "neutral", children }: Children & { tone?: string }) {
  return <span className={cx("lx-badge", `lx-tone-${TONES.has(tone) ? tone : "neutral"}`)}>{children}</span>;
}

export function Progress({ value, tone }: { value?: number; tone?: string }) {
  const ratio = typeof value === "number" && Number.isFinite(value) ? Math.max(0, Math.min(1, value > 1 ? value / 100 : value)) : 0;
  return (
    <div className="lx-progress" role="progressbar" aria-valuemin={0} aria-valuemax={100} aria-valuenow={Math.round(ratio * 100)}>
      <div className={cx("lx-progress-fill", tone !== undefined && TONES.has(tone) && `lx-tone-${tone}`)} style={{ width: `${ratio * 100}%` }} />
    </div>
  );
}

export function Stat({ label, value }: { label?: React.ReactNode; value?: React.ReactNode }) {
  return (
    <div className="lx-stat">
      <div className="lx-stat-value">{value === null || value === undefined ? "–" : figure(value)}</div>
      {label !== undefined && <div className="lx-stat-label">{label}</div>}
    </div>
  );
}

/** A figure as text. A small model sometimes hands Stat an object of
 *  figures (a total per category); it is shown as "a: 1 · b: 2" rather
 *  than taking the app down. */
function figure(value: unknown): React.ReactNode {
  if (typeof value === "object" && value !== null && !React.isValidElement(value)) {
    if (Array.isArray(value)) return value.map((v) => String(cell(v))).join(" · ");
    return Object.entries(value)
      .map(([k, v]) => `${k}: ${String(cell(v))}`)
      .join(" · ");
  }
  return value as React.ReactNode;
}

type Column = string | { key: string; label?: React.ReactNode; render?: (row: Record<string, unknown>) => React.ReactNode };

function cell(value: unknown): React.ReactNode {
  if (value === null || value === undefined) return "";
  if (typeof value === "boolean") return value ? "✓" : "";
  if (typeof value === "object" && !React.isValidElement(value)) return JSON.stringify(value);
  return value as React.ReactNode;
}

export function Table({ columns = [], rows = [] }: { columns?: Column[]; rows?: Record<string, unknown>[] }) {
  const cols = columns.map((c) => (typeof c === "string" ? { key: c, label: c } : c));
  const list = Array.isArray(rows) ? rows : [];
  return (
    <div className="lx-table-wrap">
      <table className="lx-table">
        <thead>
          <tr>
            {cols.map((c) => (
              <th key={c.key}>{c.label ?? c.key}</th>
            ))}
          </tr>
        </thead>
        <tbody>
          {list.map((row, i) => (
            <tr key={typeof row["id"] === "string" ? row["id"] : i}>
              {cols.map((c) => (
                <td key={c.key}>{"render" in c && typeof c.render === "function" ? c.render(row) : cell(row[c.key])}</td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

export function Empty({ children }: Children) {
  return <div className="lx-empty">{children}</div>;
}

/** What the app's code is handed as `window.LosOS.ui`. */
export const UI = {
  Page,
  Card,
  Stack,
  Row,
  Text,
  Heading,
  Button,
  Input,
  TextArea,
  Select,
  Checkbox,
  Badge,
  Progress,
  Stat,
  Table,
  Empty,
} as const;
