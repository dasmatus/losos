import * as React from "react";
import {
  ApiError,
  getDomains,
  isAbort,
  isUnauthorized,
  postDomainAdd,
  postDomainRemove,
  type CustomDomain,
} from "@/lib/api";
import { toast } from "@/components/ui/toast";
import { t } from "@/lib/i18n";

/* The owner's own domains, as the official edge sees them.
 *
 * The edge does the checking: it looks the owner's TXT and CNAME records up
 * itself, every half minute while a domain waits and hourly once it is live,
 * and routes the domain when both are right. So this hook only reads the
 * edge's answer, and re-reads it on the same half-minute cadence while
 * anything is still waiting, so a domain turns live on the screen without a
 * reload. A box with nothing waiting asks once.
 *
 * Not offered is a quiet 200 with `available: false` (no official edge, no
 * proxy configured, or an edge with no DNS zone), the same shape the market
 * uses and for the same reason: the shell latches a 404 as "not served". */

export const DOMAINS_RECHECK_MS = 30000;

export type DomainsState =
  | { kind: "loading" }
  | { kind: "unavailable"; reason?: "noOfficialEdge" }
  | { kind: "failed"; message: string }
  | {
      kind: "ready";
      eligible: boolean;
      target: string | null;
      addresses: string[];
      maxDomains: number;
      domains: CustomDomain[];
    };

export interface DomainsData {
  state: DomainsState;
  busy: boolean;
  refresh: () => void;
  add: (domain: string) => Promise<boolean>;
  remove: (domain: string) => Promise<boolean>;
}

function stateOf(reply: Awaited<ReturnType<typeof getDomains>>): DomainsState {
  if (!reply.available) return { kind: "unavailable", reason: reply.reason };
  return {
    kind: "ready",
    eligible: reply.eligible,
    target: reply.target,
    addresses: reply.addresses,
    maxDomains: reply.max_domains,
    domains: reply.domains,
  };
}

export function useDomains(enabled: boolean): DomainsData {
  const [state, setState] = React.useState<DomainsState>({ kind: "loading" });
  const [busy, setBusy] = React.useState(false);
  const [tick, setTick] = React.useState(0);
  const refresh = React.useCallback(() => setTick((n) => n + 1), []);

  React.useEffect(() => {
    if (!enabled) return;
    const controller = new AbortController();
    void (async () => {
      try {
        const reply = await getDomains({ signal: controller.signal });
        if (!controller.signal.aborted) setState(stateOf(reply));
      } catch (error) {
        if (isAbort(error) || isUnauthorized(error)) return;
        setState({ kind: "failed", message: error instanceof Error ? error.message : "" });
      }
    })();
    return () => controller.abort();
  }, [enabled, tick]);

  const waiting = state.kind === "ready" && state.domains.some((d) => d.status === "waiting");
  React.useEffect(() => {
    if (!enabled || !waiting) return;
    const timer = window.setTimeout(refresh, DOMAINS_RECHECK_MS);
    return () => window.clearTimeout(timer);
  }, [enabled, waiting, tick, refresh]);

  /* One change at a time; the reply is the whole view, so it replaces the
   * state directly rather than asking again. A refusal is a toast with the
   * edge's own sentence: "taken by another box", "needs a Stripe account". */
  const act = React.useCallback(
    async (fn: () => ReturnType<typeof getDomains>, done: () => void): Promise<boolean> => {
      setBusy(true);
      try {
        setState(stateOf(await fn()));
        done();
        return true;
      } catch (error) {
        if (!isUnauthorized(error)) {
          const said = error instanceof ApiError || error instanceof Error ? error.message : "";
          toast.error(
            t("panes.network.domains.failedTitle"),
            said.length > 0 ? said : t("panes.network.domains.failed"),
            { help: "custom-domain" },
          );
        }
        return false;
      } finally {
        setBusy(false);
      }
    },
    [],
  );

  return {
    state,
    busy,
    refresh,
    add: (domain) =>
      act(
        () => postDomainAdd(domain),
        () => toast.success(t("panes.network.domains.addedTitle"), t("panes.network.domains.addedBody")),
      ),
    remove: (domain) =>
      act(
        () => postDomainRemove(domain),
        () => toast.done(t("panes.network.domains.removedTitle", { domain })),
      ),
  };
}
