import * as React from "react";
import {
  getEdge,
  hasToken,
  isAbort,
  isUnauthorized,
  subscribeAuth,
  type EdgeResponse,
} from "@/lib/api";

/* The edge proxy, as the box last saw it.
 *
 * lososd looks for an edge every few seconds, on the LAN (DNS-SD) and at the
 * configured registrar, and GET /api/edge is its latest answer. The Mesh pane
 * shows it and the two sharing switches read it, because the daemon refuses
 * to turn sharing on without one (a 409 with `edgeRequired`): a switch that
 * is going to be refused should say so before it is touched, in the same
 * words the refusal would use.
 *
 * Polled, not fetched once: the whole point of the reading is that it moves
 * when an edge appears or goes away, and the demo of exactly that is what
 * the pane exists to show. The period is the daemon's own scan cadence,
 * roughly; faster would only re-read the same cached scan. */

export const EDGE_PERIOD_MS = 10000;

export type EdgeState =
  | { kind: "loading" }
  | { kind: "known"; edge: EdgeResponse }
  | { kind: "failed"; message: string };

export interface EdgeView {
  state: EdgeState;
  /** `null` until the first answer; then the box's reading. */
  edge: EdgeResponse | null;
  /** False only when the box has answered and found nothing. Before the
   *  first answer the switches stay usable: the daemon is the gate, and a
   *  pane that greys everything while loading reads as broken. */
  blocked: boolean;
  refresh: () => void;
}

function describe(error: unknown): string {
  if (error instanceof Error && error.message.length > 0) return error.message;
  return "HTTP";
}

export function useEdge(): EdgeView {
  const signedIn = React.useSyncExternalStore(subscribeAuth, hasToken, () => false);
  const [state, setState] = React.useState<EdgeState>({ kind: "loading" });
  const [tick, setTick] = React.useState(0);

  React.useEffect(() => {
    if (!signedIn) {
      setState({ kind: "loading" });
      return;
    }
    const controller = new AbortController();
    let cancelled = false;
    let timer: ReturnType<typeof setTimeout> | null = null;

    const ask = async (): Promise<void> => {
      try {
        const edge = await getEdge({ signal: controller.signal });
        if (cancelled) return;
        setState({ kind: "known", edge });
      } catch (error) {
        if (cancelled || isAbort(error) || isUnauthorized(error)) return;
        setState({ kind: "failed", message: describe(error) });
      }
      if (!cancelled) timer = setTimeout(() => void ask(), EDGE_PERIOD_MS);
    };
    void ask();

    return () => {
      cancelled = true;
      controller.abort();
      if (timer !== null) clearTimeout(timer);
    };
  }, [signedIn, tick]);

  const refresh = React.useCallback(() => setTick((n) => n + 1), []);
  const edge = state.kind === "known" ? state.edge : null;
  return { state, edge, blocked: edge !== null && !edge.reachable, refresh };
}
