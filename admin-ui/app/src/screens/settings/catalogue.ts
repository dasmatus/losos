import * as React from "react";
import { isAbort, isUnauthorized } from "@/lib/api";
import { authedGet, isRecord, nonEmptyString, notPresent } from "./http";

/* Searching for more apps.
 *
 * The search HAS to run on the box. The admin pages are served under
 * `connect-src 'self'`, so this browser cannot reach a catalogue directly —
 * a fetch to any other host is refused before it leaves the tab, and that is
 * the CSP working, not a bug to route around. lososd is the only thing here
 * with an outbound path, so the box searches and the page renders what it
 * found.
 *
 * That route does not exist yet (backend/schema.json serves nine, none of
 * them this one), so the contract this screen expects is written down here
 * and its absence is handled: a 404 leaves the field disabled with a plain
 * sentence about why, and no request is made again. Shape wanted from
 *
 *     GET /api/apps/search?q=<query>
 *     { "sources": ["Artifact Hub"],
 *       "results": [ { "id": "...", "name": "...", "source": "Artifact Hub",
 *                      "summary": "...", "version": "1.2.3",
 *                      "homepage": "https://..." } ] }
 *
 * `id`, `name` and `source` are required on a result; the rest may be absent.
 * `source` is required because it is the only thing on the row that tells the
 * reader who wrote what they are about to install, and a result with nowhere
 * to attribute it is dropped rather than shown as if this box vouched for it.
 */

export const CATALOGUE_SEARCH_URL = "/api/apps/search";

export interface CatalogueApp {
  id: string;
  name: string;
  /** Who published it — named on every row, never summarised away. */
  source: string;
  summary: string | null;
  version: string | null;
  homepage: string | null;
}

export interface CatalogueResults {
  sources: readonly string[];
  apps: readonly CatalogueApp[];
}

/* `unsupported` is this box, answering honestly. `failed` is the search
 * itself going wrong — no network on the box, the catalogue down — which is
 * worth retrying and worth distinguishing. */
export type CatalogueState =
  | { kind: "idle" }
  | { kind: "searching" }
  | { kind: "results"; results: CatalogueResults }
  | { kind: "unsupported" }
  | { kind: "failed"; message: string };

function parseApp(value: unknown): CatalogueApp | null {
  if (!isRecord(value)) return null;
  const id = nonEmptyString(value["id"]);
  const name = nonEmptyString(value["name"]);
  const source = nonEmptyString(value["source"]);
  if (id === null || name === null || source === null) return null;
  return {
    id,
    name,
    source,
    summary: nonEmptyString(value["summary"]),
    version: nonEmptyString(value["version"]),
    homepage: nonEmptyString(value["homepage"]),
  };
}

function parseResults(body: unknown): CatalogueResults {
  if (!isRecord(body)) return { sources: [], apps: [] };

  const rawResults = body["results"];
  const apps: CatalogueApp[] = [];
  if (Array.isArray(rawResults)) {
    for (const entry of rawResults) {
      const app = parseApp(entry);
      if (app !== null) apps.push(app);
    }
  }

  const rawSources = body["sources"];
  const sources: string[] = [];
  if (Array.isArray(rawSources)) {
    for (const entry of rawSources) {
      const name = nonEmptyString(entry);
      if (name !== null) sources.push(name);
    }
  }

  // Falling back to the sources actually cited by the rows keeps the footer
  // truthful when the box reports results but forgets to list where from.
  if (sources.length === 0) {
    for (const app of apps) if (!sources.includes(app.source)) sources.push(app.source);
  }

  return { sources, apps };
}

function describe(error: unknown): string {
  if (error instanceof Error && error.message.length > 0) return error.message;
  return "The search did not come back.";
}

/** Debounce: long enough that typing a word is one search, not seven. */
const DEBOUNCE_MS = 350;

export interface Catalogue {
  query: string;
  setQuery: (value: string) => void;
  state: CatalogueState;
}

export function useCatalogue(enabled: boolean): Catalogue {
  const [query, setQuery] = React.useState("");
  const [state, setState] = React.useState<CatalogueState>({ kind: "idle" });

  /* Once the box has said it does not serve the route, stop asking. Held in
   * a ref rather than derived from `state` so it survives the state going
   * back to `idle` when the field is cleared. */
  const unsupported = React.useRef(false);

  React.useEffect(() => {
    const trimmed = query.trim();
    if (!enabled || unsupported.current || trimmed.length < 2) {
      setState(unsupported.current ? { kind: "unsupported" } : { kind: "idle" });
      return;
    }

    const controller = new AbortController();
    let cancelled = false;

    const timer = window.setTimeout(() => {
      setState({ kind: "searching" });
      void (async () => {
        try {
          const url = `${CATALOGUE_SEARCH_URL}?q=${encodeURIComponent(trimmed)}`;
          const body = await authedGet(url, { signal: controller.signal });
          if (cancelled) return;
          setState({ kind: "results", results: parseResults(body) });
        } catch (error) {
          if (cancelled || isAbort(error)) return;
          if (isUnauthorized(error)) {
            setState({ kind: "idle" });
            return;
          }
          if (notPresent(error)) {
            unsupported.current = true;
            setState({ kind: "unsupported" });
            return;
          }
          setState({ kind: "failed", message: describe(error) });
        }
      })();
    }, DEBOUNCE_MS);

    return () => {
      cancelled = true;
      controller.abort();
      window.clearTimeout(timer);
    };
  }, [query, enabled]);

  return { query, setQuery, state };
}
