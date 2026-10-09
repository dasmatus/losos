import * as React from "react";
import { HugeiconsIcon } from "@hugeicons/react";
import { ArrowRight01Icon, Search01Icon } from "@hugeicons/core-free-icons";
import { Input } from "@/components/ui/input";
import { InputGroup, InputGroupAddon, InputGroupInput } from "@/components/ui/input-group";
import { NativeSelect } from "@/components/ui/native-select";
import { Switch } from "@/components/ui/switch";
import { Textarea } from "@/components/ui/textarea";
import { useT } from "@/lib/i18n-react";
import { cn } from "@/lib/utils";

/* An app's properties, as a form.
 *
 * The defaults are the app's own values document, so every app gets a form
 * whether or not it ships a schema. A schema, when there is one, adds what
 * the defaults cannot say: a description, a list of allowed values, the
 * type of a property whose default is empty.
 *
 * Only what the owner changes is kept, as `Changes` keyed by path, and only
 * that is sent: the box layers it over the app's defaults, so a default the
 * app's next version changes is not pinned by a form that never touched it.
 *
 * Objects fold into native <details>, closed, so an app with hundreds of
 * properties opens as a short list of sections. The search field flattens
 * the tree to the properties whose path matches. */

export type Path = readonly string[];
export type Changes = ReadonlyMap<string, unknown>;

export const pathKey = (path: Path): string => JSON.stringify(path);

type Json = null | boolean | number | string | Json[] | { [key: string]: Json };

function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

/** The changes as the one object the box layers over the defaults. */
export function changesToValues(changes: Changes): Record<string, unknown> {
  const out: Record<string, unknown> = {};
  for (const [key, value] of changes) {
    const path = JSON.parse(key) as string[];
    let at: Record<string, unknown> = out;
    path.forEach((part, i) => {
      if (i === path.length - 1) {
        at[part] = value;
      } else {
        const next = at[part];
        if (!isObject(next)) at[part] = {};
        at = at[part] as Record<string, unknown>;
      }
    });
  }
  return out;
}

/** An earlier install's values, back into changes. Lists and scalars are
 *  leaves; only plain objects are walked. */
export function valuesToChanges(values: unknown): Map<string, unknown> {
  const out = new Map<string, unknown>();
  const walk = (value: unknown, path: string[]) => {
    if (isObject(value) && (path.length === 0 || Object.keys(value).length > 0)) {
      for (const [k, v] of Object.entries(value)) walk(v, [...path, k]);
    } else if (path.length > 0) {
      out.set(pathKey(path), value);
    }
  };
  walk(values, []);
  return out;
}

interface SchemaHint {
  description: string | null;
  type: string | null;
  enum: readonly Json[] | null;
}

/** What the schema says about one path, walking `properties`. */
function hintAt(schema: unknown, path: Path): SchemaHint {
  let at: unknown = schema;
  for (const part of path) {
    if (!isObject(at)) break;
    const props = at["properties"];
    at = isObject(props) ? props[part] : undefined;
  }
  if (!isObject(at)) return { description: null, type: null, enum: null };
  const type = Array.isArray(at["type"])
    ? ((at["type"] as unknown[]).find((x) => x !== "null") as string | undefined)
    : at["type"];
  return {
    description: typeof at["description"] === "string" ? at["description"] : null,
    type: typeof type === "string" ? type : null,
    enum: Array.isArray(at["enum"]) ? (at["enum"] as Json[]) : null,
  };
}

interface Leaf {
  path: Path;
  value: unknown;
}

function leaves(value: unknown, path: string[] = []): Leaf[] {
  if (isObject(value) && Object.keys(value).length > 0) {
    return Object.entries(value).flatMap(([k, v]) => leaves(v, [...path, k]));
  }
  return path.length > 0 ? [{ path, value }] : [];
}

export interface ValuesFormProps {
  defaults: unknown;
  schema: unknown;
  changes: Changes;
  onChange: (path: Path, value: unknown) => void;
  onReset: (path: Path) => void;
  disabled?: boolean;
}

export function ValuesForm({ defaults, schema, changes, onChange, onReset, disabled }: ValuesFormProps) {
  const t = useT();
  const searchId = React.useId();
  const [filter, setFilter] = React.useState("");
  const needle = filter.trim().toLowerCase();

  const all = React.useMemo(() => leaves(defaults), [defaults]);
  if (all.length === 0) {
    return <p className="text-[13px] text-muted">{t("install.noProperties")}</p>;
  }

  const shown = needle.length === 0 ? null : all.filter((l) => l.path.join(".").toLowerCase().includes(needle));
  const common = { schema, changes, onChange, onReset, disabled: disabled ?? false };

  return (
    <div className="flex flex-col gap-3">
      <label htmlFor={searchId} className="sr-only">
        {t("install.findProperty")}
      </label>
      <InputGroup>
        <InputGroupAddon>
          <HugeiconsIcon
            icon={Search01Icon}
            size={15}
            strokeWidth={1.5}
            color="currentColor"
            className="text-faint"
            aria-hidden="true"
          />
        </InputGroupAddon>
        <InputGroupInput
          id={searchId}
          type="search"
          value={filter}
          placeholder={t("install.findProperty")}
          autoComplete="off"
          spellCheck={false}
          onChange={(event) => setFilter(event.target.value)}
          className="[&::-webkit-search-cancel-button]:hidden"
        />
      </InputGroup>
      <div className="flex flex-col">
        {shown === null ? (
          <Branch value={defaults} path={[]} {...common} />
        ) : shown.length === 0 ? (
          <p className="py-2 text-[13px] text-muted">{t("install.noMatch")}</p>
        ) : (
          shown.map((leaf) => (
            <Property key={pathKey(leaf.path)} path={leaf.path} label={leaf.path.join(".")} fallback={leaf.value} {...common} />
          ))
        )}
      </div>
    </div>
  );
}

interface CommonProps {
  schema: unknown;
  changes: Changes;
  onChange: (path: Path, value: unknown) => void;
  onReset: (path: Path) => void;
  disabled: boolean;
}

function Branch({ value, path, ...common }: CommonProps & { value: unknown; path: Path }) {
  if (!isObject(value)) return null;
  const entries = Object.entries(value);
  // Scalars first, then the sections, so a short app's form reads as a list.
  const scalars = entries.filter(([, v]) => !isObject(v) || Object.keys(v).length === 0);
  const sections = entries.filter(([, v]) => isObject(v) && Object.keys(v).length > 0);
  return (
    <>
      {scalars.map(([key, v]) => (
        <Property key={key} path={[...path, key]} label={key} fallback={v} {...common} />
      ))}
      {sections.map(([key, v]) => {
        const sub = [...path, key];
        const prefix = pathKey(sub).slice(0, -1);
        const touched = [...common.changes.keys()].some((k) => k.startsWith(prefix));
        return (
          <details key={key} className="group/section border-t border-hair first:border-t-0">
            <summary
              className={cn(
                "flex cursor-pointer list-none items-center gap-2 py-2.5 text-[13px] text-ink",
                "[&::-webkit-details-marker]:hidden",
              )}
            >
              <HugeiconsIcon
                icon={ArrowRight01Icon}
                size={14}
                strokeWidth={1.5}
                color="currentColor"
                className="text-faint transition-transform duration-150 group-open/section:rotate-90"
                aria-hidden="true"
              />
              <span className="font-mono text-[12.5px]">{key}</span>
              {touched && <span aria-hidden="true" className="size-1.5 rounded-full bg-accent" />}
            </summary>
            <div className="mb-2 ml-[7px] border-l border-hair pl-4">
              <Branch value={v} path={sub} {...common} />
            </div>
          </details>
        );
      })}
    </>
  );
}

function Property({
  path,
  label,
  fallback,
  schema,
  changes,
  onChange,
  onReset,
  disabled,
}: CommonProps & { path: Path; label: string; fallback: unknown }) {
  const t = useT();
  const id = React.useId();
  const key = pathKey(path);
  const changed = changes.has(key);
  const value = changed ? changes.get(key) : fallback;
  const hint = hintAt(schema, path);

  return (
    <div className="flex flex-col gap-1.5 border-t border-hair py-2.5 first:border-t-0">
      <div className="flex items-center justify-between gap-3">
        <label htmlFor={id} className="flex min-w-0 items-center gap-2">
          <span className="truncate font-mono text-[12.5px] text-ink">{label}</span>
          {changed && <span aria-hidden="true" className="size-1.5 shrink-0 rounded-full bg-accent" />}
        </label>
        {changed && (
          <button
            type="button"
            disabled={disabled}
            onClick={() => onReset(path)}
            className="shrink-0 rounded-control px-1.5 py-0.5 text-[12px] text-accent hover:bg-accent-wash"
          >
            {t("install.resetProperty")}
          </button>
        )}
      </div>
      <Editor id={id} value={value} fallback={fallback} hint={hint} disabled={disabled} onChange={(v) => onChange(path, v)} />
      {hint.description !== null && (
        <p className="text-[12px] leading-snug text-muted">{hint.description}</p>
      )}
    </div>
  );
}

function Editor({
  id,
  value,
  fallback,
  hint,
  disabled,
  onChange,
}: {
  id: string;
  value: unknown;
  fallback: unknown;
  hint: SchemaHint;
  disabled: boolean;
  onChange: (value: unknown) => void;
}) {
  const t = useT();

  if (hint.enum !== null && hint.enum.every((v) => typeof v !== "object" || v === null)) {
    const options = hint.enum.map((v) => JSON.stringify(v));
    const current = JSON.stringify(value ?? null);
    return (
      <NativeSelect
        id={id}
        value={options.includes(current) ? current : ""}
        disabled={disabled}
        onChange={(event) => onChange(JSON.parse(event.target.value))}
      >
        {!options.includes(current) && <option value="">{String(value ?? "")}</option>}
        {options.map((o) => (
          <option key={o} value={o}>
            {String(JSON.parse(o))}
          </option>
        ))}
      </NativeSelect>
    );
  }

  const kind =
    typeof fallback === "boolean" || hint.type === "boolean"
      ? "boolean"
      : typeof fallback === "number" || hint.type === "integer" || hint.type === "number"
        ? "number"
        : Array.isArray(fallback) || isObject(fallback) || hint.type === "array" || hint.type === "object"
          ? "json"
          : "string";

  if (kind === "boolean") {
    return (
      <Switch id={id} size="sm" isSelected={value === true} isDisabled={disabled} onChange={(on) => onChange(on)} />
    );
  }

  if (kind === "number") {
    return (
      <Input
        id={id}
        type="number"
        inputMode="decimal"
        className="numeric h-8 max-w-48"
        value={typeof value === "number" ? String(value) : ""}
        disabled={disabled}
        onChange={(event) => {
          const raw = event.target.value;
          if (raw === "") onChange(fallback);
          else if (Number.isFinite(Number(raw))) onChange(Number(raw));
        }}
      />
    );
  }

  if (kind === "json") {
    return <JsonEditor id={id} value={value} disabled={disabled} onChange={onChange} invalidText={t("install.notJson")} />;
  }

  return (
    <Input
      id={id}
      className="h-8 font-mono text-[12.5px]"
      spellCheck={false}
      autoComplete="off"
      value={value === null || value === undefined ? "" : String(value)}
      placeholder={value === null ? "null" : undefined}
      disabled={disabled}
      onChange={(event) => onChange(event.target.value === "" && fallback === null ? null : event.target.value)}
    />
  );
}

/* A list, or an empty object: edited as JSON, which is also valid YAML, and
 * kept as text until it parses. */
function JsonEditor({
  id,
  value,
  disabled,
  onChange,
  invalidText,
}: {
  id: string;
  value: unknown;
  disabled: boolean;
  onChange: (value: unknown) => void;
  invalidText: string;
}) {
  const [text, setText] = React.useState(() => JSON.stringify(value ?? null));
  const [invalid, setInvalid] = React.useState(false);
  React.useEffect(() => {
    setText(JSON.stringify(value ?? null));
    setInvalid(false);
  }, [value]);
  return (
    <>
      <Textarea
        id={id}
        rows={1}
        className="min-h-9 py-1.5 font-mono text-[12.5px]"
        spellCheck={false}
        value={text}
        disabled={disabled}
        aria-invalid={invalid || undefined}
        onChange={(event) => setText(event.target.value)}
        onBlur={() => {
          try {
            onChange(JSON.parse(text) as unknown);
            setInvalid(false);
          } catch {
            setInvalid(true);
          }
        }}
      />
      {invalid && <p className="text-[12px] text-crit">{invalidText}</p>}
    </>
  );
}
