/* The catalogue's shape, kept apart from index.ts so the per-area modules can
 * import it without a cycle through the catalogue that imports them. */

/** A plain string, or plural forms chosen by `vars.count`. */
export type Text = string | Plural;

/* `other` is required; the rest are what Intl.PluralRules can answer for a
 * given language. English and German use one/other, Slovak one/few/many/other
 * ("many" is for fractions, so give it the same text as "other"). */
export type Plural = Partial<Record<Intl.LDMLPluralRule, string>> & { other: string };

/** Every key carries all three languages, so a gap is a type error. */
export interface Entry {
  en: Text;
  sk: Text;
  de: Text;
}

/** Identity at runtime; at compile time it checks the entries and keeps the
 * literal keys so `t("…")` only accepts keys that exist. */
export function defineMessages<const T extends Record<string, Entry>>(messages: T): T {
  return messages;
}
