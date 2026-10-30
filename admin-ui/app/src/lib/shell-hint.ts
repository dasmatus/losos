/* What the sidebar drew last in this tab, for the first paint of the next
 * page.
 *
 * Some of the sidebar's entries wait on an answer: Cloud, Mail, Chat,
 * Contacts, Calendar and Mesh CI on GET /api/me's `shared`, Documents on
 * the Office folder. The admin page and LosOS Cloud are two documents, so
 * every move between them is a page load, and without this each one drew
 * the panel without those entries and then pushed them in a moment later.
 * sessionStorage keeps it to the tab; it decides nothing but what is drawn
 * before the answer, which replaces it. */

const KEY = "losos.shellHint";

export interface ShellHint {
  shared: readonly string[];
  office: boolean;
}

let cached: ShellHint | null | undefined;

export function shellHint(): ShellHint | null {
  if (cached !== undefined) return cached;
  cached = null;
  try {
    const raw = window.sessionStorage.getItem(KEY);
    if (raw !== null) {
      const parsed = JSON.parse(raw) as Partial<ShellHint>;
      if (Array.isArray(parsed.shared) && typeof parsed.office === "boolean") {
        cached = { shared: parsed.shared.filter((s): s is string => typeof s === "string"), office: parsed.office };
      }
    }
  } catch {
    /* no storage (a private window, a preview): no hint */
  }
  return cached;
}

export function rememberShell(hint: ShellHint): void {
  const same =
    cached != null &&
    cached.office === hint.office &&
    cached.shared.length === hint.shared.length &&
    cached.shared.every((s, i) => s === hint.shared[i]);
  if (same) return;
  cached = { shared: [...hint.shared], office: hint.office };
  try {
    window.sessionStorage.setItem(KEY, JSON.stringify(cached));
  } catch {
    /* the same */
  }
}
