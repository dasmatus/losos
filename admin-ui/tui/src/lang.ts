/* The words on screen.
 *
 * Two catalogues: the web UI's own (admin-ui/app/src/i18n), so a pane name,
 * a row title or a caption reads exactly as it does in the browser, and this
 * client's few strings of its own (./i18n.ts, every key in en, sk and de).
 * Lookup and plural choice are the web UI's (admin-ui/app/src/lib/i18n.ts):
 * Intl.PluralRules picks the form from `vars.count`, `{name}` placeholders are
 * filled from `vars`, numbers are formatted for the language.
 *
 * The language comes from --lang, else LC_ALL, LC_MESSAGES, LANG, the way a
 * terminal program reads it: Slovak, German, else English. */

import { MESSAGES } from "@app/i18n/index";
import type { Text } from "@app/i18n/define";
import { TUI_MESSAGES } from "./i18n";

export type Locale = "en" | "sk" | "de";
export const LOCALES: readonly Locale[] = ["en", "sk", "de"];

/* British English, as the web UI: day-month dates and a 24-hour clock. */
export const INTL_TAG: Record<Locale, string> = { en: "en-GB", sk: "sk-SK", de: "de-DE" };

export type WebKey = keyof typeof MESSAGES;
export type TuiKey = keyof typeof TUI_MESSAGES;
export type Key = WebKey | TuiKey;
export type Vars = Record<string, string | number>;

let current: Locale = "en";

export function isLocale(value: unknown): value is Locale {
  return value === "en" || value === "sk" || value === "de";
}

/** --lang wins; then the POSIX locale variables, in their precedence order. */
export function detectLocale(env: NodeJS.ProcessEnv, flag?: string): Locale {
  if (flag !== undefined) {
    const base = flag.toLowerCase().slice(0, 2);
    if (isLocale(base)) return base;
  }
  for (const name of ["LC_ALL", "LC_MESSAGES", "LANG"]) {
    const value = env[name];
    if (value === undefined || value === "") continue;
    const base = value.toLowerCase().slice(0, 2);
    return isLocale(base) ? base : "en";
  }
  return "en";
}

export function setLocale(locale: Locale): void {
  current = locale;
}

export function getLocale(): Locale {
  return current;
}

const pluralRules = new Map<Locale, Intl.PluralRules>();

function pick(text: Text, locale: Locale, vars: Vars | undefined): string {
  if (typeof text === "string") return text;
  const count = vars?.["count"];
  if (typeof count !== "number") return text.other;
  let rules = pluralRules.get(locale);
  if (rules === undefined) {
    rules = new Intl.PluralRules(INTL_TAG[locale]);
    pluralRules.set(locale, rules);
  }
  return text[rules.select(count)] ?? text.other;
}

export function formatNumber(n: number, locale: Locale = current, options?: Intl.NumberFormatOptions): string {
  return n.toLocaleString(INTL_TAG[locale], options);
}

function interpolate(template: string, vars: Vars | undefined, locale: Locale): string {
  if (vars === undefined) return template;
  return template.replace(/\{(\w+)\}/g, (whole, name: string) => {
    const value = vars[name];
    if (value === undefined) return whole;
    return typeof value === "number" ? formatNumber(value, locale) : value;
  });
}

function entryOf(key: Key): { en: Text; sk: Text; de: Text } {
  if (key in TUI_MESSAGES) return TUI_MESSAGES[key as TuiKey];
  return MESSAGES[key as WebKey];
}

/** The template for `key` in `locale`, plural form chosen, nothing filled. */
export function template(key: Key, vars?: Vars, locale: Locale = current): string {
  return pick(entryOf(key)[locale], locale, vars);
}

/** Translate `key` into the language on screen. */
export function t(key: Key, vars?: Vars): string {
  const text = interpolate(template(key, vars, current), vars, current);
  return key in TUI_MESSAGES ? text : plainDashes(text);
}

/** A few of the web catalogue's sentences still carry an em dash; the
 *  terminal prints them with a comma instead (the plain-style rule). */
export function plainDashes(text: string): string {
  return text.replace(/\s*—\s*/g, ", ");
}

export function hasKey(key: string): key is Key {
  return key in TUI_MESSAGES || key in MESSAGES;
}

/** Lower-case and strip diacritics: "Úložisko" -> "ulozisko". */
export function fold(text: string): string {
  return text
    .toLowerCase()
    .normalize("NFD")
    .replace(/[̀-ͯ]/g, "");
}

const KIB = 1024;
const UNITS = ["B", "KB", "MB", "GB", "TB", "PB"] as const;

/** Bytes as `df -h` counts them, the web UI's formatBytes. */
export function formatBytes(bytes: number | null, digits = 1): string {
  if (bytes === null || !Number.isFinite(bytes) || bytes < 0) return "-";
  if (bytes < KIB) return `${formatNumber(Math.round(bytes))} B`;
  let value = bytes;
  let unit = 0;
  while (value >= KIB && unit < UNITS.length - 1) {
    value /= KIB;
    unit += 1;
  }
  const places = value >= 10 ? 0 : digits;
  const shown = formatNumber(value, current, {
    minimumFractionDigits: places,
    maximumFractionDigits: places,
    useGrouping: false,
  });
  return `${shown} ${UNITS[unit] ?? "B"}`;
}

/** A Unix time as a short local date and time. */
export function formatWhen(unixSeconds: number): string {
  return new Date(unixSeconds * 1000).toLocaleString(INTL_TAG[current], {
    dateStyle: "medium",
    timeStyle: "short",
  });
}

/** Seconds ago, as words: "12 s", "4 min", "3 h", "2 d". */
export function formatAgo(seconds: number): string {
  if (!Number.isFinite(seconds) || seconds < 0) return "-";
  if (seconds < 60) return t("tui.ago.seconds", { count: Math.round(seconds) });
  if (seconds < 3600) return t("tui.ago.minutes", { count: Math.round(seconds / 60) });
  if (seconds < 86400) return t("tui.ago.hours", { count: Math.round(seconds / 3600) });
  return t("tui.ago.days", { count: Math.round(seconds / 86400) });
}
