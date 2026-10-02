import * as React from "react";
import {
  getLocale,
  getLocaleChoice,
  interpolate,
  setLocale,
  subscribeLocale,
  t,
  template,
  type Locale,
  type LocaleChoice,
  type MessageKey,
  type Vars,
} from "@/lib/i18n";

/* The React binding for lib/i18n.ts.
 *
 * A component that shows translated text calls `useT()` (or `useLocale()`)
 * once at the top; that subscribes it, so a language change re-renders it,
 * and every `t()` it — or a helper it calls — makes during that render reads
 * the new language. Values cached in useMemo/useCallback that hold text must
 * list the locale among their deps. */

/** The language on screen, subscribed. */
export function useLocale(): Locale {
  return React.useSyncExternalStore(subscribeLocale, getLocale, () => "en" as Locale);
}

/** The owner's pick ("auto" included) and the setter, subscribed. */
export function useLocaleChoice(): [LocaleChoice, (next: LocaleChoice) => void] {
  const choice = React.useSyncExternalStore(
    subscribeLocale,
    getLocaleChoice,
    () => "auto" as LocaleChoice,
  );
  return [choice, setLocale];
}

/** Subscribe the caller and hand back `t`. */
export function useT(): typeof t {
  useLocale();
  return t;
}

export type RichVars = Record<string, React.ReactNode>;

/**
 * Translate a message whose placeholders are elements — a link or a <strong>
 * in the middle of a sentence, which word order moves around between
 * languages and so cannot be stitched from fragments.
 *
 *   <Rich k="wizard.trust.body" vars={{ name: <strong>{host}</strong> }} />
 *
 * String and number values are filled in as text; anything else is placed as
 * a React node. `count`, if given, also picks the plural form.
 */
export function Rich({ k, vars }: { k: MessageKey; vars: RichVars }) {
  const locale = useLocale();
  const plain: Vars = {};
  for (const [name, value] of Object.entries(vars)) {
    if (typeof value === "string" || typeof value === "number") plain[name] = value;
  }
  const text = interpolate(template(k, plain, locale), plain, locale);
  const parts = text.split(/(\{\w+\})/g);
  return (
    <>
      {parts.map((part, i) => {
        const match = /^\{(\w+)\}$/.exec(part);
        const name = match?.[1];
        if (name === undefined || !(name in vars)) return part;
        return <React.Fragment key={i}>{vars[name]}</React.Fragment>;
      })}
    </>
  );
}
