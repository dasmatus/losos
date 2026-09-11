/* Widget shapes — the contract between a widget and the board that draws it.
 *
 * A widget is a function of the sandboxed `losos` object and nothing else:
 *
 *   const widget: WidgetFn = async (losos) => ({
 *     type: "number",
 *     title: "Free room",
 *     data: { value: 412e9, format: "bytes" },
 *     foot: "on this box",
 *   });
 *
 * It returns data, never markup. That is the whole reason the sandbox holds:
 * a widget cannot reach the DOM, so it cannot smuggle a script tag, a remote
 * image or an inline style past the appliance CSP. The four renderers in
 * render/ own every pixel, and they are the only things that touch React.
 *
 * Built-ins are compiled into the bundle and written directly against this
 * type. User-authored widgets come in as a declarative WidgetSpec (see
 * spec.ts) whose expressions are parsed and interpreted by expr.ts — never
 * eval'd, because `script-src 'self'` with no 'unsafe-eval' means eval and
 * new Function both throw.
 */

/** The four things a widget can be. Adding a fifth means a new renderer. */
export type WidgetKind = "heatmap" | "number" | "bar" | "list";

/** State colour. Never identity: "this box" vs "the mesh" is `Texture`. */
export type Tone = "neutral" | "ok" | "warn" | "crit";

/** One accent, two textures. Filled is this box; hatched is the mesh. */
export type Texture = "local" | "mesh" | "none";

/** How a raw number becomes a string. `plain` prints it unchanged. */
export type FormatName = "bytes" | "percent" | "number" | "duration" | "plain";

export const FORMAT_NAMES: readonly FormatName[] = [
  "bytes",
  "percent",
  "number",
  "duration",
  "plain",
] as const;

export function isFormatName(value: unknown): value is FormatName {
  return typeof value === "string" && (FORMAT_NAMES as readonly string[]).includes(value);
}

/** A short coloured pill beside a big number. */
export interface Chip {
  text: string;
  tone?: Tone;
}

// ── heatmap ───────────────────────────────────────────────────────────────

/** One square. `value` is 0–1; `null` means the day was never observed, which
 *  is a different thing from an outage and is drawn differently. */
export interface HeatmapDay {
  /** Local calendar day, `YYYY-MM-DD`. */
  date: string;
  value: number | null;
  /** Replaces the generated tooltip when present. */
  note?: string;
}

export interface HeatmapData {
  days: HeatmapDay[];
  /** Legend ends, low to high. Defaults to "Down" / "Up". */
  lowLabel?: string;
  highLabel?: string;
}

// ── number ────────────────────────────────────────────────────────────────

export interface NumberData {
  /** `null` draws an em-dash: the figure is not known, which is not an error. */
  value: number | null;
  format?: FormatName;
  /** Sits under the number, muted. */
  caption?: string;
  chip?: Chip;
  /** A sparkline is not a fifth type; a number may carry one. 0–1 each. */
  trend?: readonly number[];
}

// ── bar ───────────────────────────────────────────────────────────────────

export interface BarData {
  value: number | null;
  /** Drawn hatched, immediately after the fill. The mesh half of the pair, or
   *  — for storage — the reserve this box is holding back to grow into. */
  reserve?: number;
  max: number;
  format?: FormatName;
  caption?: string;
  chip?: Chip;
  fillLabel?: string;
  reserveLabel?: string;
}

// ── list ──────────────────────────────────────────────────────────────────

export interface ListItem {
  label: string;
  detail?: string;
  tone?: Tone;
  texture?: Texture;
  badge?: string;
}

export interface ListData {
  items: readonly ListItem[];
  /** Shown instead of the rows when `items` is empty. */
  empty?: string;
}

// ── the result ────────────────────────────────────────────────────────────

/* A union rather than `{ type, data }` with a loose `data`: narrowing on
 * `type` is what lets the board hand each renderer a fully typed payload. */
export type WidgetResult =
  | { type: "heatmap"; title: string; data: HeatmapData; foot?: string }
  | { type: "number"; title: string; data: NumberData; foot?: string }
  | { type: "bar"; title: string; data: BarData; foot?: string }
  | { type: "list"; title: string; data: ListData; foot?: string };

/** The scripting API's signature. Every widget in the app has this shape. */
export type WidgetFn = (losos: LososApi) => Promise<WidgetResult> | WidgetResult;

// ── the sandbox ───────────────────────────────────────────────────────────

/* Everything a widget is allowed to see.
 *
 * Allow-list, not deny-list: there is no `fetch`, no `window`, no `document`,
 * no storage and no import. A widget that wants a number asks for it by name,
 * and `metric()` refuses a name that is not in MetricMap. Adding a source
 * means adding a key there, on purpose, in a reviewed diff.
 */
export interface LososApi {
  metric: <K extends MetricName>(name: K, opts?: MetricOptions) => Promise<MetricMap[K]>;
  stat: StatApi;
  fmt: FmtApi;
}

export interface MetricOptions {
  /** Heatmap window, in days. Clamped to 1–371. */
  days?: number;
  /** Longest list a metric will return. Clamped to 1–200. */
  limit?: number;
  /** Skip the short-lived cache. Use sparingly: it exists so eight tiles
   *  asking for the same settings document make one request. */
  fresh?: boolean;
}

export interface StatApi {
  sum: (values: readonly (number | null)[]) => number;
  mean: (values: readonly (number | null)[]) => number;
  min: (values: readonly (number | null)[]) => number;
  max: (values: readonly (number | null)[]) => number;
  count: (values: readonly unknown[]) => number;
  last: (values: readonly (number | null)[]) => number;
  median: (values: readonly (number | null)[]) => number;
  /** 95th percentile, nearest-rank. */
  p95: (values: readonly (number | null)[]) => number;
  /** `a / b`, and 0 rather than Infinity when b is 0. */
  ratio: (a: number, b: number) => number;
}

export interface FmtApi {
  /** Base-1024, the way `df -h` counts. */
  bytes: (value: number, digits?: number) => string;
  /** Takes 0–1, prints "99.4%". */
  percent: (value: number, digits?: number) => string;
  number: (value: number, digits?: number) => string;
  /** Seconds in, "6 h 12 min" out. */
  duration: (seconds: number) => string;
  /** `YYYY-MM-DD` or an epoch-millisecond number in, "12 Mar" out. */
  date: (value: string | number) => string;
  time: (value: string | number) => string;
  /** Epoch milliseconds in, "4 min ago" out. */
  ago: (millis: number) => string;
  plural: (count: number, one: string, many?: string) => string;
}

// ── metrics ───────────────────────────────────────────────────────────────

/** Where a widget's numbers come from. Every key is a read; there is no
 *  write, so no widget — built-in or user-written — can change this box. */
export interface MetricMap {
  "uptime.days": UptimeMetric;
  "storage.bytes": StorageMetric;
  "mesh.compute": MeshMetric;
  "apps.list": AppsMetric;
  "rebuilds.recent": RebuildsMetric;
  "box.settings": BoxSettingsMetric;
  "box.status": BoxStatusMetric;
}

export type MetricName = keyof MetricMap;

export const METRIC_NAMES: readonly MetricName[] = [
  "uptime.days",
  "storage.bytes",
  "mesh.compute",
  "apps.list",
  "rebuilds.recent",
  "box.settings",
  "box.status",
] as const;

export function isMetricName(value: unknown): value is MetricName {
  return typeof value === "string" && (METRIC_NAMES as readonly string[]).includes(value);
}

export interface UptimeMetric {
  days: HeatmapDay[];
  /** Fraction of observed time the box answered. 0–1. */
  upRatio: number;
  outages: number;
  longestOutageSeconds: number;
  /** How many of `days` carry an observation at all. */
  observedDays: number;
  /** Where the numbers came from — the box itself, or this browser watching. */
  source: MetricSource;
}

export interface StorageMetric {
  usedBytes: number | null;
  totalBytes: number | null;
  /** Unallocated space the box is holding back, claimable without opening it. */
  reserveBytes: number | null;
  freeBytes: number | null;
  /** 0–1, used over total. */
  usedRatio: number | null;
  source: MetricSource;
}

export interface MeshMetric {
  joined: boolean;
  sharingStorage: boolean;
  sharingCompute: boolean;
  windowStart: string;
  windowEnd: string;
  /** Hours a day this box lends, from the window. Wraps midnight correctly. */
  windowHours: number;
  /** Seconds of work this box ran for others, when the box reports it. */
  givenSeconds: number | null;
  /** Seconds of this box's work that ran elsewhere. */
  takenSeconds: number | null;
  source: MetricSource;
}

export interface AppInfo {
  id: string;
  name: string;
  path: string;
  reachable: boolean | null;
  onMesh: boolean;
}

export interface AppsMetric {
  apps: AppInfo[];
  hostName: string;
}

export interface RebuildRecord {
  job: string;
  state: "idle" | "building" | "done" | "failed";
  message: string;
  /** Epoch milliseconds, first time this browser saw the job. */
  startedAt: number;
  /** Epoch milliseconds of the last observation. */
  seenAt: number;
}

export interface RebuildsMetric {
  entries: RebuildRecord[];
  current: RebuildRecord | null;
  busy: boolean;
}

export interface BoxSettingsMetric {
  hostName: string;
  https: boolean;
  mode: "local" | "mesh";
  gpu: boolean;
  proxy: boolean;
  clusterEnable: boolean;
  shareCompute: boolean;
  computeWindowStart: string;
  computeWindowEnd: string;
}

export interface BoxStatusMetric {
  state: "idle" | "building" | "done" | "failed";
  progress: number;
  message: string;
  job: string | null;
}

/** Whether a figure was reported by the box or observed by this browser. A
 *  widget that shows a number should be able to say which, and the tiles do. */
export type MetricSource = "box" | "browser" | "unknown";

// ── errors ────────────────────────────────────────────────────────────────

/* A widget failing is normal — a metric the box does not report yet, a typo
 * in a custom expression. It greys out its own tile and nothing else, so the
 * message has to be readable by the person looking at the tile. */
export class WidgetError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "WidgetError";
  }
}
