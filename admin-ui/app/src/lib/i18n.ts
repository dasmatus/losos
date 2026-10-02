/* Interface language: English, Slovak, German.
 *
 * The same shape as lib/theme.ts on purpose — a tiny framework-agnostic store
 * with a cached snapshot, so useSyncExternalStore can bind it (see
 * lib/i18n-react.tsx) and plain helpers (formatters, catalogues) can read it
 * while a subscribed component renders.
 *
 * Resolution order: the owner's explicit pick (localStorage), then the first
 * browser language we carry, then English. "auto" is a real choice, as it is
 * for the theme: Nextcloud and Forgejo both follow the browser's
 * Accept-Language too, so leaving the picker on auto keeps all three in step.
 *
 * Messages live in src/i18n/, one module per area, with all three languages
 * side by side under each key. The catalogue type demands every language for
 * every key, so a missing translation is a tsc error rather than a blank on
 * screen. */

import { MESSAGES, type MessageKey, type Text } from "@/i18n";

export type Locale = "en" | "sk" | "de";
export type LocaleChoice = Locale | "auto";

export const LOCALES: readonly Locale[] = ["en", "sk", "de"] as const;
export const LOCALE_KEY = "losos-lang";

/* Each language named in itself: someone who cannot read the current
 * language must still be able to find their own in the list. */
export const LOCALE_NAMES: Record<Locale, string> = {
  en: "English",
  sk: "Slovenčina",
  de: "Deutsch",
};

/* The tag handed to Intl. en-GB rather than en-US: day-month dates and a
 * 24-hour clock match the rest of the copy, which is British English. */
export const INTL_TAG: Record<Locale, string> = {
  en: "en-GB",
  sk: "sk-SK",
  de: "de-DE",
};

export function isLocale(value: unknown): value is Locale {
  return value === "en" || value === "sk" || value === "de";
}

function readStored(): LocaleChoice {
  try {
    const raw = window.localStorage.getItem(LOCALE_KEY);
    return isLocale(raw) ? raw : "auto";
  } catch {
    return "auto";
  }
}

function writeStored(choice: LocaleChoice): void {
  try {
    if (choice === "auto") window.localStorage.removeItem(LOCALE_KEY);
    else window.localStorage.setItem(LOCALE_KEY, choice);
  } catch {
    /* not fatal; the choice just will not survive the tab */
  }
}

/** The first browser language we have a catalogue for, else English. */
export function detectLocale(): Locale {
  const wanted =
    typeof navigator === "undefined"
      ? []
      : navigator.languages.length > 0
        ? navigator.languages
        : [navigator.language];
  for (const tag of wanted) {
    const base = tag.toLowerCase().split("-")[0];
    if (isLocale(base)) return base;
  }
  return "en";
}

const listeners = new Set<() => void>();
let choice: LocaleChoice | null = null;
let current: Locale | null = null;

function resolve(next: LocaleChoice): Locale {
  return next === "auto" ? detectLocale() : next;
}

function stamp(locale: Locale): void {
  if (typeof document !== "undefined") document.documentElement.lang = locale;
}

/** What the owner picked, "auto" included. */
export function getLocaleChoice(): LocaleChoice {
  choice ??= readStored();
  return choice;
}

/** The language actually on screen. Cached for useSyncExternalStore. */
export function getLocale(): Locale {
  if (current === null) {
    current = resolve(getLocaleChoice());
    stamp(current);
  }
  return current;
}

export function setLocale(next: LocaleChoice): void {
  if (getLocaleChoice() === next) return;
  choice = next;
  current = resolve(next);
  writeStored(next);
  stamp(current);
  for (const fn of listeners) fn();
}

/* Fires on a pick in this tab, on another tab writing the key, and on the
 * browser's language list changing while the choice is auto. */
export function subscribeLocale(onChange: () => void): () => void {
  listeners.add(onChange);
  const refresh = (): void => {
    choice = readStored();
    const next = resolve(choice);
    if (next !== current) {
      current = next;
      stamp(next);
    }
    onChange();
  };
  const onStorage = (event: StorageEvent): void => {
    if (event.key === null || event.key === LOCALE_KEY) refresh();
  };
  window.addEventListener("storage", onStorage);
  window.addEventListener("languagechange", refresh);
  return () => {
    listeners.delete(onChange);
    window.removeEventListener("storage", onStorage);
    window.removeEventListener("languagechange", refresh);
  };
}

// ── Lookup ────────────────────────────────────────────────────────────────

export type Vars = Record<string, string | number>;

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

/** Replace `{name}` placeholders. Numbers are formatted for the locale. */
export function interpolate(template: string, vars: Vars | undefined, locale: Locale): string {
  if (vars === undefined) return template;
  return template.replace(/\{(\w+)\}/g, (whole, name: string) => {
    const value = vars[name];
    if (value === undefined) return whole;
    return typeof value === "number" ? formatNumber(value, locale) : value;
  });
}

/** The raw template for `key` in `locale`, plural form already chosen. */
export function template(key: MessageKey, vars?: Vars, locale: Locale = getLocale()): string {
  const entry = MESSAGES[key];
  return pick(entry[locale], locale, vars);
}

/**
 * Translate `key` into the current language.
 *
 * Plural entries choose their form from `vars.count` (Intl.PluralRules, so
 * Slovak gets its one/few/many/other); `{name}` placeholders are filled from
 * `vars`. Call it while a component that used `useT()` or `useLocale()` is
 * rendering, so a language change re-renders the caller.
 */
export function t(key: MessageKey, vars?: Vars): string {
  const locale = getLocale();
  return interpolate(template(key, vars, locale), vars, locale);
}

export function formatNumber(
  n: number,
  locale: Locale = getLocale(),
  options?: Intl.NumberFormatOptions,
): string {
  return n.toLocaleString(INTL_TAG[locale], options);
}

/** The Intl tag for the current language, for Date/Intl calls. */
export function intlTag(): string {
  return INTL_TAG[getLocale()];
}

export type { MessageKey };
