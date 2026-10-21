import * as React from "react";
import {
  ApiError,
  getCloudApps,
  isAbort,
  isUnauthorized,
  postCloudApp,
  searchCloudStore,
  type CloudApp,
  type CloudAppsResponse,
  type CloudAppVerb,
  type CloudStoreApp,
} from "@/lib/api";
import { toast } from "@/components/ui/toast";
import { t } from "@/lib/i18n";

/* LosOS Cloud's own apps (backend/src/cloud_apps.rs): the list, one change
 * at a time, and the app store search.
 *
 * A change answers with the whole list after it, so the pane never shows a
 * switch in a position LosOS Cloud did not confirm. An older box that does
 * not serve the route answers 404; the pane then draws no apps section. */

export type CloudAppsState =
  | { kind: "loading" }
  | { kind: "unsupported" }
  | { kind: "unavailable"; reason: string }
  | { kind: "failed"; message: string }
  | { kind: "ready"; store: boolean; apps: CloudApp[] };

export interface CloudApps {
  state: CloudAppsState;
  /** The app a change is running for, or null. */
  busy: string | null;
  /** Resolves to whether the change went through; a refusal is a toast. */
  change: (verb: CloudAppVerb, id: string) => Promise<boolean>;
}

function fromResponse(reply: CloudAppsResponse): CloudAppsState {
  return reply.available
    ? { kind: "ready", store: reply.store, apps: reply.apps }
    : { kind: "unavailable", reason: reply.reason };
}

function said(error: unknown): string {
  return error instanceof Error ? error.message : "";
}

export function useCloudApps(enabled: boolean): CloudApps {
  const [state, setState] = React.useState<CloudAppsState>({ kind: "loading" });
  const [busy, setBusy] = React.useState<string | null>(null);

  React.useEffect(() => {
    if (!enabled) return;
    const controller = new AbortController();
    void (async () => {
      try {
        const reply = await getCloudApps({ signal: controller.signal });
        if (!controller.signal.aborted) setState(fromResponse(reply));
      } catch (error) {
        if (isAbort(error) || isUnauthorized(error)) return;
        if (error instanceof ApiError && error.status === 404) {
          setState({ kind: "unsupported" });
          return;
        }
        setState({ kind: "failed", message: said(error) });
      }
    })();
    return () => controller.abort();
  }, [enabled]);

  const change = React.useCallback(async (verb: CloudAppVerb, id: string) => {
    setBusy(id);
    try {
      setState(fromResponse(await postCloudApp(verb, id)));
      return true;
    } catch (error) {
      if (!isUnauthorized(error) && !isAbort(error)) {
        const message = said(error);
        toast.error(t("cloudApps.apps.changeFailed"), message.length > 0 ? message : t("install.failedFallback"), {
          help: "apps",
        });
      }
      return false;
    } finally {
      setBusy(null);
    }
  }, []);

  return { state, busy, change };
}

export type CloudStoreState =
  | { kind: "idle" }
  | { kind: "searching" }
  | { kind: "results"; apps: CloudStoreApp[] }
  | { kind: "failed"; message: string };

const DEBOUNCE_MS = 350;

/** The app store's matches for what is typed, debounced like the chart
 *  search in catalogue.ts. */
export function useCloudStore(enabled: boolean): {
  query: string;
  setQuery: (q: string) => void;
  state: CloudStoreState;
} {
  const [query, setQuery] = React.useState("");
  const [state, setState] = React.useState<CloudStoreState>({ kind: "idle" });

  React.useEffect(() => {
    const trimmed = query.trim();
    if (!enabled || trimmed.length < 2) {
      setState({ kind: "idle" });
      return;
    }
    const controller = new AbortController();
    let cancelled = false;
    const timer = window.setTimeout(() => {
      setState({ kind: "searching" });
      void (async () => {
        try {
          const reply = await searchCloudStore(trimmed, { signal: controller.signal });
          if (!cancelled) setState({ kind: "results", apps: reply.results });
        } catch (error) {
          if (cancelled || isAbort(error)) return;
          if (isUnauthorized(error)) {
            setState({ kind: "idle" });
            return;
          }
          setState({ kind: "failed", message: said(error) });
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
