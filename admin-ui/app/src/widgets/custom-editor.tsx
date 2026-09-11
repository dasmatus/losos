/* Build a widget of your own.
 *
 * WHAT THIS DIALOG IS NOT: a JavaScript editor.
 *
 * The scripting API is `async (losos) => ({ type, title, data, foot })`, and
 * the five widgets that ship with the box are written exactly that way. A
 * widget typed in here cannot be, because the appliance serves this page under
 * `script-src 'self'` with no 'unsafe-eval': `eval` and `new Function` throw,
 * and every route from a string to a function goes through one of them. A text
 * box that accepted an arrow function would fail on Save, on a box with no
 * console to read the failure in.
 *
 * So what the owner fills in is the shape of a widget plus a few small
 * expressions — arithmetic, text, and calls into `stat.` and `fmt.` — parsed
 * and walked by widgets/expr.ts. Same result type, same renderers, same tile.
 * The preview below runs the real compiled widget against the real sandbox, so
 * what it draws is what the board will draw.
 */

import * as React from "react";
import { HugeiconsIcon } from "@hugeicons/react";
import { Alert02Icon } from "@hugeicons/core-free-icons";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogBody,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { FieldError, Input, Select } from "@/components/ui/input";
import { Label, LabelHint } from "@/components/ui/label";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { cn } from "@/lib/utils";
import { CUSTOM_REFRESH_MS } from "./catalogue";
import { createSandbox } from "./sandbox";
import { blankSpec, compileSpec, EXAMPLES, parseSpec, WIDGET_KINDS, type WidgetSpec } from "./spec";
import { WidgetBody } from "./tile";
import {
  FORMAT_NAMES,
  METRIC_NAMES,
  WidgetError,
  type FormatName,
  type MetricName,
  type WidgetKind,
  type WidgetResult,
} from "./types";

// ── Vocabulary ────────────────────────────────────────────────────────────

/* Nothing in this dialog may name a scheduler, a runtime or a unit of either.
 * These labels are the user-facing spelling of each metric key, and the key
 * itself never appears on screen. */
const METRIC_LABEL: Record<MetricName, string> = {
  "uptime.days": "Whether this box answered, day by day",
  "storage.bytes": "Room on this box",
  "mesh.compute": "Sharing with other boxes",
  "apps.list": "The apps this box serves",
  "rebuilds.recent": "Changes this box has applied",
  "box.settings": "How this box is set up",
  "box.status": "What this box is doing right now",
};

const KIND_LABEL: Record<WidgetKind, string> = {
  number: "One big figure",
  bar: "A figure with a bar",
  list: "A list of rows",
  heatmap: "A square for every day",
};

const FORMAT_LABEL: Record<FormatName, string> = {
  number: "Plain number",
  bytes: "Size, like 412 GB",
  percent: "Percentage",
  duration: "Length of time",
  plain: "Exactly as given",
};

/* The names an expression may use. Shown in the dialog, because a language
 * nobody can see the vocabulary of is a language nobody will use. */
const VOCABULARY = [
  ["m", "the reading you picked above, e.g. m.totalBytes"],
  ["it", "one row, in the two row fields"],
  ["stat.", "sum, mean, min, max, median, p95, count, last, ratio"],
  ["fmt.", "bytes, percent, number, duration, date, time, ago, plural"],
] as const;

// ── The editor ────────────────────────────────────────────────────────────

export interface CustomEditorProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  /** Return true if the widget was kept. False leaves the dialog open. */
  onSave: (spec: WidgetSpec) => boolean;
}

type Draft = Record<string, string>;

function draftFrom(spec: WidgetSpec): Draft {
  return {
    title: spec.title,
    type: spec.type,
    metric: spec.metric,
    format: spec.format ?? "number",
    value: spec.value ?? "",
    caption: spec.caption ?? "",
    foot: spec.foot ?? "",
    max: spec.max ?? "",
    reserve: spec.reserve ?? "",
    fillLabel: spec.fillLabel ?? "",
    reserveLabel: spec.reserveLabel ?? "",
    items: spec.items ?? "",
    itemLabel: spec.itemLabel ?? "",
    itemDetail: spec.itemDetail ?? "",
    empty: spec.empty ?? "",
    daysExpr: spec.daysExpr ?? "",
    lowLabel: spec.lowLabel ?? "",
    highLabel: spec.highLabel ?? "",
  };
}

export function CustomEditor({ open, onOpenChange, onSave }: CustomEditorProps) {
  const [draft, setDraft] = React.useState<Draft>(() => draftFrom(blankSpec()));
  const [pasted, setPasted] = React.useState("");
  const [pasteProblem, setPasteProblem] = React.useState<string | null>(null);
  const titleId = React.useId();
  const hintId = React.useId();

  const set = (key: string, value: string): void => {
    setDraft((previous) => ({ ...previous, [key]: value }));
  };

  /* Validate on every keystroke, not on Save.
   *
   * The expression language is small and unfamiliar; finding out at Save that
   * a bracket is missing means having lost the place you were in. parseSpec
   * is cheap — it parses at most six short expressions — so it runs on each
   * render and its message sits under the form. */
  const attempt = React.useMemo<{ spec: WidgetSpec } | { problem: string }>(() => {
    try {
      return { spec: parseSpec(draftToSpec(draft)) };
    } catch (error) {
      return { problem: error instanceof Error ? error.message : "That does not add up yet." };
    }
  }, [draft]);

  const spec = "spec" in attempt ? attempt.spec : null;
  const kind = (draft["type"] ?? "number") as WidgetKind;

  const save = (): void => {
    if (spec === null) return;
    if (onSave(spec)) setDraft(draftFrom(blankSpec()));
  };

  const usePaste = (): void => {
    let parsed: unknown;
    try {
      parsed = JSON.parse(pasted);
    } catch {
      setPasteProblem("That is not a complete record. Check the brackets.");
      return;
    }
    try {
      setDraft(draftFrom(parseSpec(parsed)));
      setPasteProblem(null);
    } catch (error) {
      setPasteProblem(error instanceof Error ? error.message : "That widget could not be read.");
    }
  };

  return (
    <Dialog
      open={open}
      onOpenChange={onOpenChange}
      labelledBy={titleId}
      describedBy={hintId}
      dialogClassName="w-[min(52rem,calc(100vw-2rem))] max-w-[52rem]"
    >
      <DialogHeader>
        <DialogTitle id={titleId}>Build a widget</DialogTitle>
        <DialogDescription id={hintId}>
          Pick a reading and a shape, then say what to show. Only readings this box publishes are
          available, and a widget can only read them.
        </DialogDescription>
      </DialogHeader>

      <DialogBody>
        <Tabs defaultValue="build">
          <TabsList>
            <TabsTrigger value="build">Build</TabsTrigger>
            <TabsTrigger value="paste">Paste</TabsTrigger>
            <TabsTrigger value="help">What you can write</TabsTrigger>
          </TabsList>

          <TabsContent value="build">
            <div className="grid gap-5 md:grid-cols-[minmax(0,1fr)_minmax(0,18rem)]">
              <div className="flex flex-col gap-3.5">
                <Field label="Title">
                  <Input
                    value={draft["title"] ?? ""}
                    maxLength={80}
                    onChange={(event) => set("title", event.target.value)}
                  />
                </Field>

                <div className="grid gap-3.5 sm:grid-cols-2">
                  <Field label="Reading">
                    <Select
                      className="w-full"
                      value={draft["metric"] ?? "storage.bytes"}
                      onChange={(event) => set("metric", event.target.value)}
                    >
                      {METRIC_NAMES.map((name) => (
                        <option key={name} value={name}>
                          {METRIC_LABEL[name]}
                        </option>
                      ))}
                    </Select>
                  </Field>

                  <Field label="Shape">
                    <Select
                      className="w-full"
                      value={kind}
                      onChange={(event) => set("type", event.target.value)}
                    >
                      {WIDGET_KINDS.map((name) => (
                        <option key={name} value={name}>
                          {KIND_LABEL[name]}
                        </option>
                      ))}
                    </Select>
                  </Field>
                </div>

                {(kind === "number" || kind === "bar") && (
                  <>
                    <Field label="The figure" hint="For example: m.totalBytes - m.usedBytes">
                      <Input
                        className="numeric"
                        spellCheck={false}
                        value={draft["value"] ?? ""}
                        onChange={(event) => set("value", event.target.value)}
                      />
                    </Field>

                    <Field label="Shown as">
                      <Select
                        className="w-full"
                        value={draft["format"] ?? "number"}
                        onChange={(event) => set("format", event.target.value)}
                      >
                        {FORMAT_NAMES.map((name) => (
                          <option key={name} value={name}>
                            {FORMAT_LABEL[name]}
                          </option>
                        ))}
                      </Select>
                    </Field>
                  </>
                )}

                {kind === "bar" && (
                  <>
                    <Field label="Full at" hint="What counts as a full bar, e.g. m.totalBytes">
                      <Input
                        className="numeric"
                        spellCheck={false}
                        value={draft["max"] ?? ""}
                        onChange={(event) => set("max", event.target.value)}
                      />
                    </Field>
                    <Field
                      label="Held back"
                      hint="Drawn hatched, right after the fill. Leave empty for none."
                    >
                      <Input
                        className="numeric"
                        spellCheck={false}
                        value={draft["reserve"] ?? ""}
                        onChange={(event) => set("reserve", event.target.value)}
                      />
                    </Field>
                    <div className="grid gap-3.5 sm:grid-cols-2">
                      <Field label="Label for the fill">
                        <Input
                          value={draft["fillLabel"] ?? ""}
                          onChange={(event) => set("fillLabel", event.target.value)}
                        />
                      </Field>
                      <Field label="Label for the held-back part">
                        <Input
                          value={draft["reserveLabel"] ?? ""}
                          onChange={(event) => set("reserveLabel", event.target.value)}
                        />
                      </Field>
                    </div>
                  </>
                )}

                {kind === "list" && (
                  <>
                    <Field label="The rows" hint="A list, for example: m.apps">
                      <Input
                        className="numeric"
                        spellCheck={false}
                        value={draft["items"] ?? ""}
                        onChange={(event) => set("items", event.target.value)}
                      />
                    </Field>
                    <div className="grid gap-3.5 sm:grid-cols-2">
                      <Field label="Row label" hint="it.name">
                        <Input
                          className="numeric"
                          spellCheck={false}
                          value={draft["itemLabel"] ?? ""}
                          onChange={(event) => set("itemLabel", event.target.value)}
                        />
                      </Field>
                      <Field label="Row detail" hint="it.path">
                        <Input
                          className="numeric"
                          spellCheck={false}
                          value={draft["itemDetail"] ?? ""}
                          onChange={(event) => set("itemDetail", event.target.value)}
                        />
                      </Field>
                    </div>
                    <Field label="When there is nothing">
                      <Input
                        value={draft["empty"] ?? ""}
                        onChange={(event) => set("empty", event.target.value)}
                      />
                    </Field>
                  </>
                )}

                {kind === "heatmap" && (
                  <>
                    <Field label="The days" hint="A list of days, for example: m.days">
                      <Input
                        className="numeric"
                        spellCheck={false}
                        value={draft["daysExpr"] ?? ""}
                        onChange={(event) => set("daysExpr", event.target.value)}
                      />
                    </Field>
                    <div className="grid gap-3.5 sm:grid-cols-2">
                      <Field label="Low end">
                        <Input
                          value={draft["lowLabel"] ?? ""}
                          onChange={(event) => set("lowLabel", event.target.value)}
                        />
                      </Field>
                      <Field label="High end">
                        <Input
                          value={draft["highLabel"] ?? ""}
                          onChange={(event) => set("highLabel", event.target.value)}
                        />
                      </Field>
                    </div>
                  </>
                )}

                {kind !== "list" && kind !== "heatmap" && (
                  <Field label="Caption" hint="A line under the figure.">
                    <Input
                      value={draft["caption"] ?? ""}
                      onChange={(event) => set("caption", event.target.value)}
                    />
                  </Field>
                )}

                <Field label="Note" hint="The small line at the bottom of the tile.">
                  <Input
                    value={draft["foot"] ?? ""}
                    onChange={(event) => set("foot", event.target.value)}
                  />
                </Field>

                {"problem" in attempt && (
                  <p
                    role="alert"
                    className="flex items-start gap-2 text-[12.5px] leading-snug text-warn"
                  >
                    <HugeiconsIcon
                      icon={Alert02Icon}
                      size={16}
                      strokeWidth={1.5}
                      color="currentColor"
                      className="mt-px shrink-0"
                      aria-hidden="true"
                    />
                    {attempt.problem}
                  </p>
                )}
              </div>

              <div className="flex flex-col gap-3">
                <p className="text-[12px] font-medium tracking-wide text-faint uppercase">
                  Preview
                </p>
                <Preview spec={spec} />
                <Examples
                  onPick={(example) => {
                    setDraft((previous) => ({
                      ...previous,
                      metric: example.metric,
                      type: example.type,
                      value: example.type === "list" ? (previous["value"] ?? "") : example.expression,
                      items: example.type === "list" ? example.expression : (previous["items"] ?? ""),
                      title: example.label,
                    }));
                  }}
                />
              </div>
            </div>
          </TabsContent>

          <TabsContent value="paste">
            <div className="flex flex-col gap-3">
              <Label htmlFor="widget-json">A widget, as text</Label>
              <textarea
                id="widget-json"
                rows={10}
                spellCheck={false}
                value={pasted}
                onChange={(event) => {
                  setPasted(event.target.value);
                  setPasteProblem(null);
                }}
                className={cn(
                  "numeric w-full rounded-control border border-line bg-surface p-3",
                  "text-[12.5px] leading-relaxed text-ink placeholder:text-faint",
                  "focus-visible:border-accent focus-visible:outline-none",
                  "focus-visible:ring-2 focus-visible:ring-accent/35",
                )}
                placeholder={JSON.stringify(blankSpec(), null, 2)}
              />
              <FieldError>{pasteProblem}</FieldError>
              <LabelHint>
                Widgets are plain records. Copy one out of another browser, or write one here, then
                load it into the form to check it.
              </LabelHint>
              <Button
                variant="secondary"
                className="self-start"
                disabled={pasted.trim().length === 0}
                onClick={usePaste}
              >
                Load into the form
              </Button>

              {spec !== null && (
                <>
                  <Label htmlFor="widget-json-out">What the form holds now</Label>
                  <textarea
                    id="widget-json-out"
                    readOnly
                    rows={8}
                    value={JSON.stringify(spec, null, 2)}
                    className={cn(
                      "numeric w-full rounded-control border border-line bg-sunk p-3",
                      "text-[12.5px] leading-relaxed text-muted",
                    )}
                  />
                </>
              )}
            </div>
          </TabsContent>

          <TabsContent value="help">
            <div className="flex flex-col gap-4 text-[13px] leading-relaxed text-muted">
              <p>
                A widget here is not a program. It is a reading, a shape, and a few short
                expressions. That is a deliberate limit: this page is served with a rule that
                forbids the browser from running code it was handed at the last moment, and a box
                you cannot log in to is not the place to discover that the rule applies.
              </p>

              <dl className="flex flex-col gap-2">
                {VOCABULARY.map(([name, meaning]) => (
                  <div key={name} className="flex flex-wrap items-baseline gap-2">
                    <dt className="numeric rounded-[4px] bg-sunk px-1.5 py-0.5 text-[12.5px] text-ink">
                      {name}
                    </dt>
                    <dd className="text-[12.5px]">{meaning}</dd>
                  </div>
                ))}
              </dl>

              <p>
                Between them you can use <Code>+ - * / %</Code>, comparisons, <Code>&amp;&amp;</Code>{" "}
                and <Code>||</Code>, <Code>a ? b : c</Code>, and <Code>??</Code> for a fallback when
                a figure has not been measured. Dividing by nothing gives nothing rather than an
                error.
              </p>

              <p>
                A widget that wants more than that, with its own requests, its own state or a loop,
                belongs in the box&apos;s own admin page and arrives with the next system update,
                the same way these five did.
              </p>

              <p className="text-faint">
                Widgets are kept in this browser only, and refresh about once a minute.
              </p>
            </div>
          </TabsContent>
        </Tabs>
      </DialogBody>

      <DialogFooter>
        <Button variant="ghost" onClick={() => onOpenChange(false)}>
          Cancel
        </Button>
        <Button disabled={spec === null} onClick={save}>
          Add to the board
        </Button>
      </DialogFooter>
    </Dialog>
  );
}

function Code({ children }: { children: React.ReactNode }) {
  return (
    <code className="numeric rounded-[4px] bg-sunk px-1 py-0.5 text-[12px] text-ink">
      {children}
    </code>
  );
}

/* A label WRAPPING its control, rather than htmlFor plus a generated id.
 *
 * Implicit association: no id to generate, none to get out of step, and the
 * hint sits inside the label so a screen reader reads the field and its
 * explanation as one thing. Every field in this form is a single control,
 * which is the condition wrapping requires. */
function Field({
  label,
  hint,
  children,
}: {
  label: string;
  hint?: string;
  children: React.ReactNode;
}) {
  return (
    <Label className="flex flex-col gap-1.5">
      <span>{label}</span>
      {children}
      {hint !== undefined && <LabelHint className="font-normal">{hint}</LabelHint>}
    </Label>
  );
}

// ── The live preview ──────────────────────────────────────────────────────

/* Compiles and RUNS the spec, against the real sandbox, drawing through the
 * same renderer a board tile uses. A preview that renders sample data would
 * agree with the tile right up until it mattered. */
function Preview({ spec }: { spec: WidgetSpec | null }) {
  const [result, setResult] = React.useState<WidgetResult | null>(null);
  const [problem, setProblem] = React.useState<string | null>(null);

  React.useEffect(() => {
    if (spec === null) return;

    let live = true;
    const controller = new AbortController();

    const timer = setTimeout(() => {
      void (async () => {
        try {
          const run = compileSpec(spec);
          const value = await run(createSandbox({ signal: controller.signal }));
          if (!live) return;
          setResult(value);
          setProblem(null);
        } catch (error) {
          if (!live) return;
          setResult(null);
          setProblem(
            error instanceof WidgetError || error instanceof Error
              ? error.message
              : "That did not work out.",
          );
        }
      })();
      // Typing into an expression field would otherwise fire a run per
      // keystroke, and two of the readings behind it reach the box.
    }, 350);

    return () => {
      live = false;
      controller.abort();
      clearTimeout(timer);
    };
  }, [spec]);

  return (
    <div className="rounded-card border border-line bg-surface p-4">
      {spec === null && (
        <p className="text-[12.5px] text-faint">Fill the form in and it appears here.</p>
      )}

      {spec !== null && problem !== null && (
        <p className="text-[12.5px] leading-snug text-warn">{problem}</p>
      )}

      {spec !== null && problem === null && result === null && (
        <p className="text-[12.5px] text-faint">Working…</p>
      )}

      {result !== null && problem === null && (
        <div className="flex flex-col gap-2">
          <p className="truncate text-[13px] font-semibold tracking-wide text-muted uppercase">
            {result.title}
          </p>
          <WidgetBody result={result} />
          {result.foot !== undefined && (
            <p className="text-[11.5px] leading-snug text-faint">{result.foot}</p>
          )}
        </div>
      )}
    </div>
  );
}

function Examples({ onPick }: { onPick: (example: (typeof EXAMPLES)[number]) => void }) {
  return (
    <div className="flex flex-col gap-2">
      <p className="text-[12px] font-medium tracking-wide text-faint uppercase">Try one</p>
      <ul className="flex flex-col gap-1">
        {EXAMPLES.map((example) => (
          <li key={example.label}>
            <button
              type="button"
              onClick={() => onPick(example)}
              className={cn(
                "w-full rounded-control px-2 py-1.5 text-left",
                "transition-colors duration-150 hover:bg-sunk",
              )}
            >
              <span className="block text-[12.5px] text-ink">{example.label}</span>
              <span className="numeric block truncate text-[11.5px] text-faint">
                {example.expression}
              </span>
            </button>
          </li>
        ))}
      </ul>
    </div>
  );
}

// ── Draft → spec ──────────────────────────────────────────────────────────

/* The form holds strings; parseSpec wants a record with the empty fields
 * absent, because an empty expression is "not given" and not "a blank one". */
function draftToSpec(draft: Draft): Record<string, unknown> {
  const out: Record<string, unknown> = {
    version: 1,
    title: draft["title"] ?? "",
    type: draft["type"] ?? "number",
    metric: draft["metric"] ?? "storage.bytes",
  };
  for (const key of [
    "format",
    "value",
    "caption",
    "foot",
    "max",
    "reserve",
    "fillLabel",
    "reserveLabel",
    "items",
    "itemLabel",
    "itemDetail",
    "empty",
    "daysExpr",
    "lowLabel",
    "highLabel",
  ]) {
    const value = draft[key];
    if (value !== undefined && value.trim().length > 0) out[key] = value;
  }
  return out;
}

/** Exported for the board's "how often" line. Keeps one number in one place. */
export const CUSTOM_REFRESH_SECONDS = Math.round(CUSTOM_REFRESH_MS / 1000);
