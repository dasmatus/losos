import * as React from "react";
import { isAbort } from "@/lib/api";
import { readStatistics, type Statistics } from "./types";

export type StatisticsState =
  | { kind: "loading" }
  | { kind: "failed"; message: string }
  | { kind: "ready"; stats: Statistics };

/** How often the page and the widget ask again. Traefik's counters move every
 * request; the per-minute ring moves once a minute. */
export const STATISTICS_REFRESH_MS = 15_000;

/** Load the statistics now and every 15 s while mounted. `load` is the place's
 * own fetch (the box's /api/statistics, the edge's /manage/api/statistics);
 * keep it stable, a new function starts over. */
export function useStatistics(
  load: (signal: AbortSignal) => Promise<unknown>,
  everyMs: number = STATISTICS_REFRESH_MS,
): StatisticsState {
  const [state, setState] = React.useState<StatisticsState>({ kind: "loading" });

  React.useEffect(() => {
    const controller = new AbortController();
    const once = () =>
      load(controller.signal)
        .then((json) => setState({ kind: "ready", stats: readStatistics(json) }))
        .catch((error: unknown) => {
          if (isAbort(error)) return;
          // Keep the last good figures on screen through one failed poll.
          setState((previous) =>
            previous.kind === "ready"
              ? previous
              : { kind: "failed", message: error instanceof Error ? error.message : "" },
          );
        });
    void once();
    const timer = window.setInterval(() => void once(), everyMs);
    return () => {
      window.clearInterval(timer);
      controller.abort();
    };
  }, [load, everyMs]);

  return state;
}
