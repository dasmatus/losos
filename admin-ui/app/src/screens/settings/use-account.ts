import * as React from "react";
import {
  ApiError,
  getAccount,
  heldBy,
  isAbort,
  isUnauthorized,
  loadMe,
  postAccount,
  type Account,
} from "@/lib/api";
import { toast } from "@/components/ui/toast";
import { t } from "@/lib/i18n";
import { postSetPassword } from "@/screens/wizard/api";

/* The signed-in person's own account (`GET /api/account`), for Your
 * account. Nothing here is part of the box's configuration: no Apply bar,
 * every button is its own request.
 *
 * A person adds an authenticator app or a passkey on LosOS Login's own page,
 * in another tab, so the account is asked again whenever this tab gets the
 * focus back. When that answer no longer holds them, /api/me is asked too,
 * and the shell opens the rest of the box. */

export type AccountState =
  | { kind: "loading" }
  | { kind: "noAccount" }
  | { kind: "failed"; message: string }
  | { kind: "ready"; account: Account };

/** What a change came to: done, or lososd's refusal in its own words. */
export type Outcome = { ok: true } | { ok: false; message: string };

export interface AccountData {
  state: AccountState;
  busy: boolean;
  refresh: () => void;
  saveDetails: (displayName: string, email: string) => Promise<Outcome>;
  changePassword: (password: string) => Promise<Outcome>;
}

function messageOf(error: unknown): string {
  return error instanceof ApiError || error instanceof Error ? error.message : "";
}

export function useAccount(enabled: boolean): AccountData {
  const [state, setState] = React.useState<AccountState>({ kind: "loading" });
  const [busy, setBusy] = React.useState(false);
  const [tick, setTick] = React.useState(0);
  const refresh = React.useCallback(() => setTick((n) => n + 1), []);

  React.useEffect(() => {
    if (!enabled) return;
    const controller = new AbortController();
    void (async () => {
      try {
        const account = await getAccount({ signal: controller.signal });
        if (controller.signal.aborted) return;
        setState({ kind: "ready", account });
        // Let go: the shell learns it from /api/me.
        if (heldBy().length > 0 && account.locked.length === 0) {
          await loadMe({ signal: controller.signal }).catch(() => null);
        }
      } catch (error) {
        if (isAbort(error) || isUnauthorized(error)) return;
        // 409: the spare key, which has no account of its own.
        setState(
          error instanceof ApiError && error.status === 409
            ? { kind: "noAccount" }
            : { kind: "failed", message: messageOf(error) },
        );
      }
    })();
    return () => controller.abort();
  }, [enabled, tick]);

  React.useEffect(() => {
    if (!enabled) return;
    const again = () => {
      if (document.visibilityState === "visible") refresh();
    };
    window.addEventListener("focus", again);
    document.addEventListener("visibilitychange", again);
    return () => {
      window.removeEventListener("focus", again);
      document.removeEventListener("visibilitychange", again);
    };
  }, [enabled, refresh]);

  const attempt = React.useCallback(
    async (send: () => Promise<unknown>, fallback: string): Promise<Outcome> => {
      setBusy(true);
      try {
        await send();
        return { ok: true };
      } catch (error) {
        if (isUnauthorized(error)) return { ok: false, message: "" };
        const said = messageOf(error);
        return { ok: false, message: said.length > 0 ? said : fallback };
      } finally {
        setBusy(false);
        refresh();
      }
    },
    [refresh],
  );

  const saveDetails = React.useCallback(
    async (displayName: string, email: string) => {
      const outcome = await attempt(() => postAccount({ displayName, email }), t("account.failed"));
      if (outcome.ok) {
        toast.done(t("account.details.saved"));
        // The name in the top bar comes from /api/me.
        void loadMe().catch(() => null);
      }
      return outcome;
    },
    [attempt],
  );

  const changePassword = React.useCallback(
    async (password: string) => {
      const outcome = await attempt(() => postSetPassword(password), t("account.failed"));
      if (outcome.ok) {
        toast.done(t("account.password.saved"));
        void loadMe().catch(() => null);
      }
      return outcome;
    },
    [attempt],
  );

  return { state, busy, refresh, saveDetails, changePassword };
}
