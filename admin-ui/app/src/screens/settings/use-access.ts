import * as React from "react";
import {
  ApiError,
  getAccess,
  isAbort,
  isUnauthorized,
  postAccessRevoke,
  postAccessToken,
  type AccessResponse,
  type AccessTokenInput,
} from "@/lib/api";
import { toast } from "@/components/ui/toast";
import { t } from "@/lib/i18n";
import { notPresent } from "./http";

/* Terminal access as lososd sees it (`GET /api/access`): the box's own key,
 * the GPG keys seeded at install, and the tokens bound to them.
 *
 * Nothing here is a setting of the box's configuration, so the pane sits
 * outside the Apply bar and every button is its own request. Nothing polls
 * either: the view changes only when this pane makes or revokes a token, and
 * "last used" moving while the pane is open is not worth a request every few
 * seconds.
 *
 * A 404 is a box whose lososd predates the routes, said plainly rather than
 * painted red. */

export type AccessState =
  | { kind: "loading" }
  | { kind: "unavailable" }
  | { kind: "failed"; message: string }
  | { kind: "ready"; view: AccessResponse };

/** What making a token came to: the secret, shown once, or lososd's refusal. */
export type CreateOutcome = { ok: true; token: string } | { ok: false; message: string };

export interface AccessData {
  state: AccessState;
  busy: boolean;
  refresh: () => void;
  create: (input: AccessTokenInput) => Promise<CreateOutcome>;
  revoke: (id: string, name: string) => Promise<boolean>;
}

/* Every field has a fallback, so a box that answers with less shows an
 * empty list rather than a broken pane. */
function viewOf(reply: Partial<AccessResponse>): AccessResponse {
  return {
    hostKey: reply.hostKey ?? null,
    keys: Array.isArray(reply.keys) ? reply.keys : [],
    tokens: Array.isArray(reply.tokens) ? reply.tokens : [],
  };
}

function messageOf(error: unknown): string {
  return error instanceof ApiError || error instanceof Error ? error.message : "";
}

export function useAccess(enabled: boolean): AccessData {
  const [state, setState] = React.useState<AccessState>({ kind: "loading" });
  const [busy, setBusy] = React.useState(false);
  const [tick, setTick] = React.useState(0);
  const refresh = React.useCallback(() => setTick((n) => n + 1), []);

  React.useEffect(() => {
    if (!enabled) return;
    const controller = new AbortController();
    void (async () => {
      try {
        const view = viewOf(await getAccess({ signal: controller.signal }));
        if (!controller.signal.aborted) setState({ kind: "ready", view });
      } catch (error) {
        if (isAbort(error) || isUnauthorized(error)) return;
        setState(notPresent(error) ? { kind: "unavailable" } : { kind: "failed", message: messageOf(error) });
      }
    })();
    return () => controller.abort();
  }, [enabled, tick]);

  /* The refusal is returned, not toasted: the form shows it beside itself,
   * where the field it is about still is. */
  const create = React.useCallback(
    async (input: AccessTokenInput): Promise<CreateOutcome> => {
      setBusy(true);
      try {
        const reply = await postAccessToken(input);
        return { ok: true, token: reply.token };
      } catch (error) {
        if (isUnauthorized(error)) return { ok: false, message: "" };
        const said = messageOf(error);
        return { ok: false, message: said.length > 0 ? said : t("access.form.failed") };
      } finally {
        setBusy(false);
        refresh();
      }
    },
    [refresh],
  );

  const revoke = React.useCallback(
    async (id: string, name: string): Promise<boolean> => {
      setBusy(true);
      try {
        await postAccessRevoke(id);
        toast.done(t("access.tokens.revoked", { name }));
        return true;
      } catch (error) {
        if (!isUnauthorized(error)) {
          const said = messageOf(error);
          toast.error(t("access.tokens.revokeFailed"), said, { help: "terminal" });
        }
        return false;
      } finally {
        setBusy(false);
        refresh();
      }
    },
    [refresh],
  );

  return { state, busy, refresh, create, revoke };
}
