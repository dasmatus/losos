/* The `losos` object a widget is handed, and nothing else.
 *
 * A widget is a function of one argument. That argument is this object. It
 * has three keys — `metric`, `stat`, `fmt` — and no way to reach a fourth:
 * a built-in widget is an ordinary module and could of course `import` the
 * API client directly, but none does, and a custom widget is a parsed
 * expression tree (see expr.ts) whose evaluator can only see the names bound
 * for it. There is no `fetch` here, no `window`, no `document`, no storage
 * and no import.
 *
 * `metric` is the only door out, and it is a door with a list on it:
 * MetricMap in types.ts enumerates every reachable reading, every one of them
 * a read. Adding a source means adding a key there, on purpose, in a diff
 * somebody looked at. A widget cannot name a host, a path or a method.
 *
 * The object is frozen. Not as a security boundary — a built-in that wanted
 * to could freeze nothing and import anything — but so that one widget
 * monkey-patching `fmt.bytes` cannot change what the tile below it prints.
 */

import { ApiError } from "@/lib/api";
import { fmt, stat } from "./format";
import { loadMetric, type LoadOptions } from "./metrics";
import {
  isMetricName,
  WidgetError,
  type LososApi,
  type MetricMap,
  type MetricName,
  type MetricOptions,
} from "./types";

/** Heatmap windows are clamped here, not in the renderer: an expression asking
 *  for a million days would otherwise allocate a million objects first. */
const MIN_DAYS = 1;
const MAX_DAYS = 371;
const MIN_LIMIT = 1;
const MAX_LIMIT = 200;

function clamp(value: number | undefined, low: number, high: number): number | undefined {
  if (value === undefined) return undefined;
  if (!Number.isFinite(value)) return undefined;
  return Math.min(high, Math.max(low, Math.trunc(value)));
}

export interface SandboxOptions {
  /** Aborted when the tile unmounts or refreshes. Widgets never see it. */
  signal?: AbortSignal;
}

/** Build the object handed to one widget run. Cheap; make a fresh one per run
 *  so the abort signal belongs to that run and not to the tile forever. */
export function createSandbox(options: SandboxOptions = {}): LososApi {
  async function metric<K extends MetricName>(
    name: K,
    opts: MetricOptions = {},
  ): Promise<MetricMap[K]> {
    if (!isMetricName(name)) {
      throw new WidgetError(`There is no reading called ${String(name)}`);
    }

    const days = clamp(opts.days, MIN_DAYS, MAX_DAYS);
    const limit = clamp(opts.limit, MIN_LIMIT, MAX_LIMIT);

    const load: LoadOptions = { fresh: opts.fresh === true };
    if (days !== undefined) load.days = days;
    if (limit !== undefined) load.limit = limit;
    if (options.signal !== undefined) load.signal = options.signal;

    try {
      return (await loadMetric(name, load)) as MetricMap[K];
    } catch (error) {
      if (error instanceof WidgetError) throw error;
      // The tile shows this sentence. "TypeError: Failed to fetch" is not a
      // sentence anyone reading a dashboard can act on.
      throw new WidgetError(readableReason(error));
    }
  }

  return Object.freeze({ metric, stat, fmt }) satisfies LososApi;
}

/* Turn whatever was thrown into something worth printing on a tile.
 *
 * Deliberately vague about the transport: on a box with no shell, "this box
 * did not answer" is actionable ("is it plugged in?") and "NetworkError when
 * attempting to fetch resource" is not. So is "HTTP 404", which is what the
 * API client throws for anything that is not a 401 — a status code on a
 * dashboard tile tells the one person who will read it nothing at all. */
function readableReason(error: unknown): string {
  if (error instanceof ApiError) return fromStatus(error.status);

  if (error instanceof Error) {
    const message = error.message.trim();
    if (message === "unauthorized") return fromStatus(401);
    // The API client's fallback message when lososd answered with no JSON.
    const status = /^HTTP (\d{3})$/.exec(message);
    if (status !== null) return fromStatus(Number(status[1]));
    if (message.length > 0 && message.length < 120) return message;
  }

  return "This box did not answer.";
}

function fromStatus(status: number): string {
  if (status === 401) return "This tab is no longer signed in.";
  if (status === 403) return "This page is not allowed to ask for that.";
  if (status === 404) return "This box does not report that yet.";
  if (status >= 500) return "This box is busy or restarting. It will be back.";
  return "This box did not answer.";
}

/** The two namespaces a custom expression is allowed to CALL into. Passed to
 *  the evaluator by identity — see expr.ts, which checks the object, not the
 *  name it was reached through. */
export function callableNamespaces(losos: LososApi): readonly unknown[] {
  return [losos.stat, losos.fmt];
}
