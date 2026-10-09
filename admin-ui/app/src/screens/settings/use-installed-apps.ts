import * as React from "react";
import {
  getApps,
  isAbort,
  isUnauthorized,
  postAppInstall,
  postAppRemove,
  type AppInstallRequest,
  type AppsResponse,
  type InstalledApp,
} from "@/lib/api";
import { toast } from "@/components/ui/toast";
import { t } from "@/lib/i18n";

/* The apps the owner installed from the search, and the two things they can
 * do to one: install (or change) it, and remove it.
 *
 * An install is a job on the box that fetches the app and waits for it to
 * start, which takes as long as the download does. The pane asks again
 * while any app is installing or being removed, and stops once they all
 * settle. */

export type InstalledState =
  | { kind: "loading" }
  | { kind: "unavailable" }
  | { kind: "failed"; message: string }
  | { kind: "ready"; sharedAvailable: boolean; apps: InstalledApp[] };

export interface InstalledApps {
  state: InstalledState;
  refresh: () => void;
  /** Resolves to the app's record, or throws the box's refusal for the
   *  dialog to show beside the button. */
  install: (request: AppInstallRequest) => Promise<InstalledApp>;
  remove: (release: string) => Promise<boolean>;
}

const SETTLING_POLL_MS = 4_000;

function fromResponse(reply: AppsResponse): InstalledState {
  return reply.available
    ? { kind: "ready", sharedAvailable: reply.sharedAvailable, apps: reply.apps }
    : { kind: "unavailable" };
}

export function useInstalledApps(enabled: boolean): InstalledApps {
  const [state, setState] = React.useState<InstalledState>({ kind: "loading" });
  const [tick, setTick] = React.useState(0);
  const refresh = React.useCallback(() => setTick((n) => n + 1), []);

  React.useEffect(() => {
    if (!enabled) return;
    const controller = new AbortController();
    void (async () => {
      try {
        const reply = await getApps({ signal: controller.signal });
        if (!controller.signal.aborted) setState(fromResponse(reply));
      } catch (error) {
        if (isAbort(error) || isUnauthorized(error)) return;
        setState({ kind: "failed", message: error instanceof Error ? error.message : "" });
      }
    })();
    return () => controller.abort();
  }, [enabled, tick]);

  const busy =
    state.kind === "ready" &&
    state.apps.some((app) => app.phase === "installing" || app.phase === "removing");
  React.useEffect(() => {
    if (!enabled || !busy) return;
    const timer = window.setTimeout(refresh, SETTLING_POLL_MS);
    return () => window.clearTimeout(timer);
  }, [enabled, busy, refresh, state]);

  const install = React.useCallback(
    async (request: AppInstallRequest) => {
      const app = await postAppInstall(request);
      refresh();
      return app;
    },
    [refresh],
  );

  const remove = React.useCallback(
    async (release: string) => {
      try {
        await postAppRemove(release);
        refresh();
        return true;
      } catch (error) {
        if (!isUnauthorized(error) && !isAbort(error)) {
          const said = error instanceof Error ? error.message : "";
          toast.error(t("install.removeFailedTitle"), said.length > 0 ? said : t("install.failedFallback"), {
            help: "apps",
          });
        }
        return false;
      }
    },
    [refresh],
  );

  return { state, refresh, install, remove };
}
