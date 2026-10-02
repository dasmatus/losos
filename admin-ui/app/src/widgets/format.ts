/* Formatting and summary statistics — the `losos.fmt` and `losos.stat` halves
 * of the widget sandbox.
 *
 * Pure functions over plain values. Nothing here reads the network, the DOM
 * or storage, which is what makes handing them to a user-authored expression
 * safe: the worst a caller can do is get an ugly string back.
 *
 * Every function is total. A widget that divides by zero, formats NaN or
 * hands `duration` a negative should get "—" and a tile that still renders,
 * not an exception that greys the tile over an arithmetic edge case.
 *
 * Text follows the language on screen: decimal separators, unit words, month
 * names and "ago" are all read at call time, never frozen at import. A widget
 * runs while its tile is mounted, and tile.tsx re-runs every tile when the
 * language changes.
 */

import { formatNumber as localeNumber, getLocale, t, INTL_TAG, type Locale } from "@/lib/i18n";
import type { FmtApi, FormatName, StatApi } from "./types";

const DASH = "—";

function finite(value: unknown): number | null {
  return typeof value === "number" && Number.isFinite(value) ? value : null;
}

/* Only the numbers. A heatmap year is mostly nulls on a box installed last
 * week, and a mean that counts those as zero says the box was down. */
function numbers(values: readonly (number | null)[]): number[] {
  const out: number[] = [];
  for (const value of values) {
    const n = finite(value);
    if (n !== null) out.push(n);
  }
  return out;
}

// ── stat ──────────────────────────────────────────────────────────────────

export const stat: StatApi = {
  sum(values) {
    let total = 0;
    for (const n of numbers(values)) total += n;
    return total;
  },
  mean(values) {
    const list = numbers(values);
    if (list.length === 0) return 0;
    return stat.sum(list) / list.length;
  },
  min(values) {
    const list = numbers(values);
    if (list.length === 0) return 0;
    return list.reduce((a, b) => (b < a ? b : a), list[0] as number);
  },
  max(values) {
    const list = numbers(values);
    if (list.length === 0) return 0;
    return list.reduce((a, b) => (b > a ? b : a), list[0] as number);
  },
  count(values) {
    return Array.isArray(values) ? values.length : 0;
  },
  last(values) {
    const list = numbers(values);
    return list.length === 0 ? 0 : (list[list.length - 1] as number);
  },
  median(values) {
    const list = numbers(values).sort((a, b) => a - b);
    if (list.length === 0) return 0;
    const mid = Math.floor(list.length / 2);
    if (list.length % 2 === 1) return list[mid] as number;
    return ((list[mid - 1] as number) + (list[mid] as number)) / 2;
  },
  p95(values) {
    const list = numbers(values).sort((a, b) => a - b);
    if (list.length === 0) return 0;
    // Nearest-rank: the smallest value at or above the 95th percentile.
    const rank = Math.ceil(0.95 * list.length) - 1;
    return list[Math.min(list.length - 1, Math.max(0, rank))] as number;
  },
  ratio(a, b) {
    const x = finite(a);
    const y = finite(b);
    if (x === null || y === null || y === 0) return 0;
    return x / y;
  },
};

// ── fmt ───────────────────────────────────────────────────────────────────

/* Base 1024 with the short names, which is what `df -h` prints and what the
 * owner will have seen if they ever looked at the disk. Mixing SI sizes with
 * binary names is worse than picking one and staying with it. */
const BYTE_UNITS = ["B", "KB", "MB", "GB", "TB", "PB"] as const;

function formatBytes(value: number, digits?: number): string {
  const n = finite(value);
  if (n === null) return DASH;
  const negative = n < 0;
  let size = Math.abs(n);
  let unit = 0;
  while (size >= 1024 && unit < BYTE_UNITS.length - 1) {
    size /= 1024;
    unit += 1;
  }
  // Bytes and kilobytes have no useful decimal; terabytes need one.
  const places = digits ?? (unit <= 1 ? 0 : size < 10 ? 1 : size < 100 ? 1 : 0);
  const digitsText = localeNumber(size, getLocale(), {
    minimumFractionDigits: places,
    maximumFractionDigits: places,
    useGrouping: false,
  });
  return `${negative ? "-" : ""}${digitsText} ${BYTE_UNITS[unit] ?? "B"}`;
}

function formatPercent(value: number, digits = 1): string {
  const n = finite(value);
  if (n === null) return DASH;
  const places = Math.min(4, Math.max(0, Math.trunc(digits)));
  // 99.96% rounding to "100%" is a lie a heatmap summary should not tell.
  const scaled = n * 100;
  const shown = scaled >= 100 ? scaled : Math.min(scaled, 99.999);
  const text = localeNumber(shown, getLocale(), {
    minimumFractionDigits: places,
    maximumFractionDigits: places,
  });
  return t("widgets.format.percent", { n: text });
}

function formatNumber(value: number, digits = 0): string {
  const n = finite(value);
  if (n === null) return DASH;
  const places = Math.min(6, Math.max(0, Math.trunc(digits)));
  return localeNumber(n, getLocale(), {
    minimumFractionDigits: places,
    maximumFractionDigits: places,
  });
}

const MINUTE = 60;
const HOUR = 60 * MINUTE;
const DAY = 24 * HOUR;

function formatDuration(seconds: number): string {
  const n = finite(seconds);
  if (n === null) return DASH;
  const total = Math.max(0, Math.round(n));
  if (total < MINUTE) return t("widgets.format.seconds", { n: total });
  if (total < HOUR) return t("widgets.format.minutes", { n: Math.round(total / MINUTE) });
  if (total < DAY) {
    const hours = Math.floor(total / HOUR);
    const mins = Math.round((total % HOUR) / MINUTE);
    return mins === 0
      ? t("widgets.format.hours", { n: hours })
      : t("widgets.format.hoursMinutes", { h: hours, m: mins });
  }
  const days = Math.floor(total / DAY);
  const hours = Math.round((total % DAY) / HOUR);
  return hours === 0
    ? t("widgets.format.days", { n: days })
    : t("widgets.format.daysHours", { d: days, h: hours });
}

/* `YYYY-MM-DD` is parsed as a LOCAL date, not UTC.
 *
 * `new Date("2026-03-12")` is midnight UTC by spec, which in any negative
 * offset is the 11th — and every day key in the journal is a local calendar
 * day. Getting this wrong shifts the whole heatmap by one column for half the
 * planet, silently. */
export function parseDayKey(key: string): Date | null {
  const match = /^(\d{4})-(\d{2})-(\d{2})$/.exec(key);
  if (match === null) return null;
  const year = Number(match[1]);
  const month = Number(match[2]);
  const day = Number(match[3]);
  const date = new Date(year, month - 1, day);
  if (date.getFullYear() !== year || date.getMonth() !== month - 1 || date.getDate() !== day) {
    return null;
  }
  return date;
}

function toDate(value: string | number): Date | null {
  if (typeof value === "number") {
    return Number.isFinite(value) ? new Date(value) : null;
  }
  if (typeof value !== "string") return null;
  const fromKey = parseDayKey(value);
  if (fromKey !== null) return fromKey;
  const parsed = new Date(value);
  return Number.isNaN(parsed.getTime()) ? null : parsed;
}

/* One formatter per (shape, language), built the first time it is asked for.
 * A heatmap labels 371 cells per render; building an Intl.DateTimeFormat per
 * cell is measurably slow, and building one at import time would freeze the
 * language the page happened to load in. */
const DATE_SHAPES = {
  date: { day: "numeric", month: "short" },
  long: { weekday: "short", day: "numeric", month: "short", year: "numeric" },
  time: { hour: "2-digit", minute: "2-digit" },
  weekday: { weekday: "short" },
  month: { month: "short" },
} as const satisfies Record<string, Intl.DateTimeFormatOptions>;

type DateShape = keyof typeof DATE_SHAPES;

const dateFormats = new Map<string, Intl.DateTimeFormat>();

/** The cached formatter for `shape` in the current language. */
export function dateFormat(shape: DateShape, locale: Locale = getLocale()): Intl.DateTimeFormat {
  const cacheKey = `${locale}:${shape}`;
  let format = dateFormats.get(cacheKey);
  if (format === undefined) {
    format = new Intl.DateTimeFormat(INTL_TAG[locale], DATE_SHAPES[shape]);
    dateFormats.set(cacheKey, format);
  }
  return format;
}

function formatDate(value: string | number): string {
  const date = toDate(value);
  return date === null ? DASH : dateFormat("date").format(date);
}

/** The tooltip form: "Thu, 12 Mar 2026". Not on the sandbox surface. */
export function formatLongDate(value: string | number): string {
  const date = toDate(value);
  return date === null ? DASH : dateFormat("long").format(date);
}

function formatTime(value: string | number): string {
  const date = toDate(value);
  return date === null ? DASH : dateFormat("time").format(date);
}

function formatAgo(millis: number): string {
  const n = finite(millis);
  if (n === null) return DASH;
  const delta = Date.now() - n;
  if (delta < 0) return t("widgets.format.justNow");
  if (delta < 45_000) return t("widgets.format.justNow");
  return t("widgets.format.ago", { duration: formatDuration(delta / 1000) });
}

/* The words are the widget author's own, so this stays an English-shaped
 * one/many choice: it cannot know the plural forms of whatever language they
 * were typed in. Built-in text uses plural messages from the catalogue. */
function plural(count: number, one: string, many?: string): string {
  const n = finite(count) ?? 0;
  return Math.abs(n) === 1 ? one : (many ?? `${one}s`);
}

export const fmt: FmtApi = {
  bytes: formatBytes,
  percent: formatPercent,
  number: formatNumber,
  duration: formatDuration,
  date: formatDate,
  time: formatTime,
  ago: formatAgo,
  plural,
};

/** Apply a named format. What the renderers use on a `FormatName`. */
export function applyFormat(value: number | null, format: FormatName = "number"): string {
  if (value === null || !Number.isFinite(value)) return DASH;
  switch (format) {
    case "bytes":
      return fmt.bytes(value);
    case "percent":
      return fmt.percent(value);
    case "duration":
      return fmt.duration(value);
    case "plain":
      return String(value);
    case "number":
      return fmt.number(value, Number.isInteger(value) ? 0 : 1);
  }
}

export { DASH };
