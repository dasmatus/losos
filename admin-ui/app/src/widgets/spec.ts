/* A widget the owner wrote, as data.
 *
 * THE SHAPE OF THE PROBLEM
 * ------------------------
 * The scripting API is `async (losos) => ({ type, title, data, foot })`. The
 * five built-ins are exactly that — real functions, compiled into the bundle
 * by Vite, with the whole language available to them.
 *
 * A widget typed into this page in a browser cannot be. `script-src 'self'`
 * without 'unsafe-eval' means `eval` and `new Function` throw, and every way
 * of turning a string into a function goes through one of them. See expr.ts
 * for the full argument and for the alternatives that also do not work.
 *
 * So a custom widget is a WidgetSpec: which reading to take, which shape to
 * draw it in, and a handful of expressions in the small language expr.ts
 * parses. `compileSpec` turns one into a WidgetFn — the same type the
 * built-ins have — so from the board's point of view there is exactly one
 * kind of widget and exactly one execution path.
 *
 * What this costs, honestly: a spec cannot branch across metrics, cannot loop,
 * and cannot hold state between runs. What it buys: the thing the owner types
 * cannot reach the network, the DOM, storage, or any function the two
 * allow-listed namespaces do not expose — and the answer to "what happens
 * when the box's CSP refuses this" is "it does not, because there is nothing
 * to refuse".
 *
 * A widget that wants the full function form belongs in widgets/builtin.ts
 * and arrives with the next system update, like the rest of the admin page.
 */

import { t, type MessageKey } from "@/lib/i18n";
import { evaluate, ExprError, parseExpr, toNumberOrNull, toText, type Node } from "./expr";
import { fmt, stat } from "./format";
import {
  isFormatName,
  isMetricName,
  WidgetError,
  type FormatName,
  type HeatmapDay,
  type ListItem,
  type LososApi,
  type MetricName,
  type NumberData,
  type Texture,
  type Tone,
  type WidgetFn,
  type WidgetKind,
  type WidgetResult,
} from "./types";

// ── The spec ──────────────────────────────────────────────────────────────

/** A user-authored widget. Plain JSON: it round-trips through localStorage,
 *  through a copy-paste of the text, and through a future export. */
export interface WidgetSpec {
  readonly version: 1;
  readonly title: string;
  readonly type: WidgetKind;
  /** Which reading to take. Bound to the name `m` in every expression. */
  readonly metric: MetricName;
  readonly days?: number;
  readonly limit?: number;
  /** Literal text under the tile. Not an expression: a foot is a caption. */
  readonly foot?: string;

  /** number, bar: the figure. Expression. */
  readonly value?: string;
  readonly format?: FormatName;
  readonly caption?: string;

  /** bar: the full extent, and the hatched part after the fill. Expressions. */
  readonly max?: string;
  readonly reserve?: string;
  readonly fillLabel?: string;
  readonly reserveLabel?: string;

  /** list: an expression yielding an array, then two per-item expressions
   *  with the element bound to `it`. */
  readonly items?: string;
  readonly itemLabel?: string;
  readonly itemDetail?: string;
  readonly empty?: string;

  /** heatmap: an expression yielding `{date, value}` records. */
  readonly daysExpr?: string;
  readonly lowLabel?: string;
  readonly highLabel?: string;
}

export const WIDGET_KINDS: readonly WidgetKind[] = ["number", "bar", "list", "heatmap"] as const;

function isWidgetKind(value: unknown): value is WidgetKind {
  return typeof value === "string" && (WIDGET_KINDS as readonly string[]).includes(value);
}

// ── Validation ────────────────────────────────────────────────────────────

const MAX_TEXT = 80;

/* Field names in the messages below are message keys, translated only when a
 * message is actually thrown — so the sentence is in the language on screen
 * at the moment the owner reads it. */
function text(
  record: Record<string, unknown>,
  key: string,
  labelKey: MessageKey,
): string | undefined {
  const value = record[key];
  if (value === undefined || value === null) return undefined;
  if (typeof value !== "string") {
    throw new WidgetError(t("widgets.spec.mustBeText", { label: t(labelKey) }));
  }
  const trimmed = value.trim();
  if (trimmed.length === 0) return undefined;
  if (trimmed.length > MAX_TEXT) {
    throw new WidgetError(t("widgets.spec.tooLong", { label: t(labelKey) }));
  }
  return trimmed;
}

function expression(
  record: Record<string, unknown>,
  key: string,
  labelKey: MessageKey,
): string | undefined {
  const value = record[key];
  if (value === undefined || value === null) return undefined;
  if (typeof value !== "string") {
    throw new WidgetError(t("widgets.spec.mustBeText", { label: t(labelKey) }));
  }
  const trimmed = value.trim();
  if (trimmed.length === 0) return undefined;
  try {
    parseExpr(trimmed);
  } catch (error) {
    throw new WidgetError(
      t("widgets.spec.fieldProblem", {
        label: t(labelKey),
        problem: error instanceof ExprError ? error.message : t("widgets.spec.notValid"),
      }),
    );
  }
  return trimmed;
}

function counted(record: Record<string, unknown>, key: string, high: number): number | undefined {
  const value = record[key];
  if (value === undefined || value === null) return undefined;
  if (typeof value !== "number" || !Number.isFinite(value)) return undefined;
  return Math.min(high, Math.max(1, Math.trunc(value)));
}

/* Validate an unknown value into a WidgetSpec, or throw a sentence a person
 * can act on. This runs on the editor's Save, on every load out of
 * localStorage, and on the paste-JSON path — three doors, one validator. */
export function parseSpec(input: unknown): WidgetSpec {
  if (typeof input !== "object" || input === null) {
    throw new WidgetError(t("widgets.spec.notRecord"));
  }
  const record = input as Record<string, unknown>;

  const title = text(record, "title", "widgets.spec.label.title");
  if (title === undefined) throw new WidgetError(t("widgets.spec.needTitle"));

  const type = record["type"];
  if (!isWidgetKind(type)) {
    throw new WidgetError(t("widgets.spec.pickKind", { kinds: WIDGET_KINDS.join(", ") }));
  }

  const metric = record["metric"];
  if (!isMetricName(metric)) {
    throw new WidgetError(t("widgets.spec.pickMetric"));
  }

  const format = record["format"];
  if (format !== undefined && format !== null && !isFormatName(format)) {
    throw new WidgetError(t("widgets.spec.badFormat"));
  }

  const spec: Mutable<WidgetSpec> = { version: 1, title, type, metric };

  const days = counted(record, "days", 371);
  if (days !== undefined) spec.days = days;
  const limit = counted(record, "limit", 200);
  if (limit !== undefined) spec.limit = limit;

  assign(spec, "foot", text(record, "foot", "widgets.spec.label.foot"));
  assign(spec, "caption", text(record, "caption", "widgets.spec.label.caption"));
  if (typeof format === "string") spec.format = format as FormatName;

  switch (type) {
    case "number": {
      const value = expression(record, "value", "widgets.spec.label.value");
      if (value === undefined) throw new WidgetError(t("widgets.spec.needValue"));
      spec.value = value;
      break;
    }
    case "bar": {
      const value = expression(record, "value", "widgets.spec.label.value");
      const max = expression(record, "max", "widgets.spec.label.max");
      if (value === undefined) throw new WidgetError(t("widgets.spec.needValue"));
      if (max === undefined) throw new WidgetError(t("widgets.spec.needMax"));
      spec.value = value;
      spec.max = max;
      assign(spec, "reserve", expression(record, "reserve", "widgets.spec.label.reserve"));
      assign(spec, "fillLabel", text(record, "fillLabel", "widgets.spec.label.fillLabel"));
      assign(
        spec,
        "reserveLabel",
        text(record, "reserveLabel", "widgets.spec.label.reserveLabel"),
      );
      break;
    }
    case "list": {
      const items = expression(record, "items", "widgets.spec.label.items");
      if (items === undefined) throw new WidgetError(t("widgets.spec.needItems"));
      spec.items = items;
      assign(spec, "itemLabel", expression(record, "itemLabel", "widgets.spec.label.itemLabel"));
      assign(
        spec,
        "itemDetail",
        expression(record, "itemDetail", "widgets.spec.label.itemDetail"),
      );
      assign(spec, "empty", text(record, "empty", "widgets.spec.label.empty"));
      break;
    }
    case "heatmap": {
      const daysExpr = expression(record, "daysExpr", "widgets.spec.label.days");
      if (daysExpr === undefined) throw new WidgetError(t("widgets.spec.needDays"));
      spec.daysExpr = daysExpr;
      assign(spec, "lowLabel", text(record, "lowLabel", "widgets.spec.label.lowLabel"));
      assign(spec, "highLabel", text(record, "highLabel", "widgets.spec.label.highLabel"));
      break;
    }
  }

  return spec;
}

type Mutable<T> = { -readonly [K in keyof T]: T[K] };

function assign<K extends keyof WidgetSpec>(
  spec: Mutable<WidgetSpec>,
  key: K,
  value: WidgetSpec[K] | undefined,
): void {
  if (value !== undefined) spec[key] = value;
}

// ── Compilation ───────────────────────────────────────────────────────────

/* Parse each expression ONCE, when the spec is compiled, and keep the tree.
 * A tile refreshes on a timer; re-tokenising the same forty characters every
 * thirty seconds for the life of the page is work nobody asked for, and it
 * would put a syntax error's first report thirty seconds after Save rather
 * than at it. */
interface Compiled {
  value?: Node;
  max?: Node;
  reserve?: Node;
  items?: Node;
  itemLabel?: Node;
  itemDetail?: Node;
  days?: Node;
  caption?: Node;
}

function compileExpressions(spec: WidgetSpec): Compiled {
  const out: Compiled = {};
  if (spec.value !== undefined) out.value = parseExpr(spec.value);
  if (spec.max !== undefined) out.max = parseExpr(spec.max);
  if (spec.reserve !== undefined) out.reserve = parseExpr(spec.reserve);
  if (spec.items !== undefined) out.items = parseExpr(spec.items);
  if (spec.itemLabel !== undefined) out.itemLabel = parseExpr(spec.itemLabel);
  if (spec.itemDetail !== undefined) out.itemDetail = parseExpr(spec.itemDetail);
  if (spec.daysExpr !== undefined) out.days = parseExpr(spec.daysExpr);
  // A caption is text unless it looks like it wants to be an expression. The
  // editor labels the field "Caption"; `{ ... }` is the opt-in, and a plain
  // sentence never accidentally parses as one.
  const caption = spec.caption;
  if (caption !== undefined && caption.startsWith("{") && caption.endsWith("}")) {
    out.caption = parseExpr(caption.slice(1, -1));
  }
  return out;
}

const CALLABLE: readonly unknown[] = [stat, fmt];

function evalNode(node: Node | undefined, env: Record<string, unknown>): unknown {
  if (node === undefined) return undefined;
  try {
    return evaluate(node, env, { callable: CALLABLE });
  } catch (error) {
    throw new WidgetError(error instanceof ExprError ? error.message : t("widgets.spec.didNotWork"));
  }
}

const MAX_ROWS = 200;

/** Turn a spec into the same kind of function the built-ins are. */
export function compileSpec(spec: WidgetSpec): WidgetFn {
  const compiled = compileExpressions(spec);

  return async (losos: LososApi): Promise<WidgetResult> => {
    const options: { days?: number; limit?: number } = {};
    if (spec.days !== undefined) options.days = spec.days;
    if (spec.limit !== undefined) options.limit = spec.limit;

    const m = await losos.metric(spec.metric, options);
    const env: Record<string, unknown> = { m, stat: losos.stat, fmt: losos.fmt };

    const caption =
      compiled.caption !== undefined ? toText(evalNode(compiled.caption, env)) : spec.caption;

    switch (spec.type) {
      case "number": {
        const data: NumberData = {
          value: toNumberOrNull(evalNode(compiled.value, env)),
          format: spec.format ?? "number",
          ...(caption === undefined ? {} : { caption }),
        };
        return withFoot({ type: "number", title: spec.title, data }, spec.foot);
      }

      case "bar": {
        const max = toNumberOrNull(evalNode(compiled.max, env)) ?? 0;
        const reserve = toNumberOrNull(evalNode(compiled.reserve, env));
        const data = {
          value: toNumberOrNull(evalNode(compiled.value, env)),
          max,
          format: spec.format ?? "number",
          ...(reserve === null ? {} : { reserve }),
          ...(caption === undefined ? {} : { caption }),
          ...(spec.fillLabel === undefined ? {} : { fillLabel: spec.fillLabel }),
          ...(spec.reserveLabel === undefined ? {} : { reserveLabel: spec.reserveLabel }),
        };
        return withFoot({ type: "bar", title: spec.title, data }, spec.foot);
      }

      case "list": {
        const raw = evalNode(compiled.items, env);
        if (!Array.isArray(raw)) {
          throw new WidgetError(t("widgets.spec.rowsNotList"));
        }
        const items: ListItem[] = [];
        for (const element of raw.slice(0, MAX_ROWS)) {
          const rowEnv = { ...env, it: element };
          const label =
            compiled.itemLabel === undefined
              ? toText(element)
              : toText(evalNode(compiled.itemLabel, rowEnv));
          const detail =
            compiled.itemDetail === undefined
              ? undefined
              : toText(evalNode(compiled.itemDetail, rowEnv));
          items.push({
            label: label.length === 0 ? "—" : label.slice(0, 120),
            ...(detail === undefined || detail.length === 0 ? {} : { detail: detail.slice(0, 120) }),
            ...readRowMarkers(element),
          });
        }
        const data = {
          items,
          ...(spec.empty === undefined ? {} : { empty: spec.empty }),
        };
        return withFoot({ type: "list", title: spec.title, data }, spec.foot);
      }

      case "heatmap": {
        const raw = evalNode(compiled.days, env);
        if (!Array.isArray(raw)) {
          throw new WidgetError(t("widgets.spec.daysNotList"));
        }
        const days: HeatmapDay[] = [];
        for (const element of raw.slice(0, 371)) {
          const day = readDay(element);
          if (day !== null) days.push(day);
        }
        if (days.length === 0) {
          throw new WidgetError(t("widgets.spec.noDates"));
        }
        const data = {
          days,
          ...(spec.lowLabel === undefined ? {} : { lowLabel: spec.lowLabel }),
          ...(spec.highLabel === undefined ? {} : { highLabel: spec.highLabel }),
        };
        return withFoot({ type: "heatmap", title: spec.title, data }, spec.foot);
      }
    }
  };
}

function withFoot(result: WidgetResult, foot: string | undefined): WidgetResult {
  return foot === undefined ? result : { ...result, foot };
}

/* A row may carry its own tone and texture when it came straight out of a
 * metric that has them — apps.list does. Read defensively: the element is
 * whatever the expression produced, which is not necessarily a record. */
function readRowMarkers(element: unknown): { tone?: Tone; texture?: Texture; badge?: string } {
  if (typeof element !== "object" || element === null) return {};
  const record = element as Record<string, unknown>;
  const out: { tone?: Tone; texture?: Texture; badge?: string } = {};
  const tone = record["tone"];
  if (tone === "ok" || tone === "warn" || tone === "crit" || tone === "neutral") out.tone = tone;
  const texture = record["texture"];
  if (texture === "local" || texture === "mesh" || texture === "none") out.texture = texture;
  const badge = record["badge"];
  if (typeof badge === "string" && badge.length > 0) out.badge = badge.slice(0, 24);
  return out;
}

function readDay(element: unknown): HeatmapDay | null {
  if (typeof element !== "object" || element === null) return null;
  const record = element as Record<string, unknown>;
  const date = record["date"];
  if (typeof date !== "string" || !/^\d{4}-\d{2}-\d{2}$/.test(date)) return null;
  const value = record["value"];
  const numeric = typeof value === "number" && Number.isFinite(value) ? value : null;
  const note = record["note"];
  return {
    date,
    value: numeric === null ? null : Math.min(1, Math.max(0, numeric)),
    ...(typeof note === "string" && note.length > 0 ? { note: note.slice(0, 120) } : {}),
  };
}

// ── A starting point for the editor ───────────────────────────────────────

/** What the Custom… dialog opens with: a valid spec that draws something.
 *  The title and caption are written in the language on screen; once saved
 *  they are the owner's own text and stay as they are. */
export function blankSpec(): WidgetSpec {
  return {
    version: 1,
    title: t("widgets.spec.blank.title"),
    type: "number",
    metric: "storage.bytes",
    value: "m.reserveBytes",
    format: "bytes",
    caption: t("widgets.spec.blank.caption"),
  };
}

/* Worked examples, shown beside the expression field.
 *
 * Every one of these is valid against the metric named beside it. They exist
 * because the language is small and unfamiliar, and a blank box with a
 * grammar reference beside it teaches nobody anything. */
export interface SpecExample {
  /** Message keys, translated where the example is shown. */
  readonly label: MessageKey;
  readonly metric: MetricName;
  readonly type: WidgetKind;
  readonly expression: string;
  readonly note: MessageKey;
}

export const EXAMPLES: readonly SpecExample[] = [
  {
    label: "widgets.spec.example.daysWatched.label",
    metric: "uptime.days",
    type: "number",
    expression: "m.observedDays",
    note: "widgets.spec.example.daysWatched.note",
  },
  {
    label: "widgets.spec.example.answeredShare.label",
    metric: "uptime.days",
    type: "number",
    expression: "m.upRatio",
    note: "widgets.spec.example.answeredShare.note",
  },
  {
    label: "widgets.spec.example.longestQuiet.label",
    metric: "uptime.days",
    type: "number",
    expression: "m.longestOutageSeconds",
    note: "widgets.spec.example.longestQuiet.note",
  },
  {
    label: "widgets.spec.example.roomLeft.label",
    metric: "storage.bytes",
    type: "number",
    expression: "m.totalBytes - (m.usedBytes ?? 0)",
    note: "widgets.spec.example.roomLeft.note",
  },
  {
    label: "widgets.spec.example.hoursLent.label",
    metric: "mesh.compute",
    type: "number",
    expression: "m.windowHours",
    note: "widgets.spec.example.hoursLent.note",
  },
  {
    label: "widgets.spec.example.appsAnswering.label",
    metric: "apps.list",
    type: "list",
    expression: "m.apps",
    note: "widgets.spec.example.appsAnswering.note",
  },
  {
    label: "widgets.spec.example.typicalDay.label",
    metric: "uptime.days",
    type: "number",
    expression: "stat.median([1, 2, 3])",
    note: "widgets.spec.example.typicalDay.note",
  },
];
