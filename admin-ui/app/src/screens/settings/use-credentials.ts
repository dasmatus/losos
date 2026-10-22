import * as React from "react";
import {
  ApiError,
  getCredentials,
  isAbort,
  isUnauthorized,
  postCredentialRotate,
  type Credential,
  type CredentialsResponse,
} from "@/lib/api";
import { toast } from "@/components/ui/toast";
import { t } from "@/lib/i18n";
import { credentialTitle } from "./pane-credentials-words";
import { notPresent } from "./http";

/* The box's credentials as lososd lists them (`GET /api/credentials`).
 *
 * Nothing here is a setting of the box's configuration, so the pane sits
 * outside the Apply bar, and the one write, a new value, is its own request.
 * No value ever reaches this page: the list has none, and a new value is
 * made and sealed on the box.
 *
 * A 404 is a box whose lososd predates the routes, said plainly rather than
 * painted red. */

export type CredentialsState =
  | { kind: "loading" }
  | { kind: "unavailable" }
  | { kind: "failed"; message: string }
  | { kind: "ready"; view: CredentialsResponse };

export interface CredentialsData {
  state: CredentialsState;
  /** The name being made new, while its request runs. */
  rotating: string | null;
  refresh: () => void;
  rotate: (credential: Credential) => Promise<boolean>;
}

function viewOf(reply: Partial<CredentialsResponse>): CredentialsResponse {
  return {
    key: reply.key ?? null,
    credentials: Array.isArray(reply.credentials) ? reply.credentials : [],
  };
}

function messageOf(error: unknown): string {
  return error instanceof ApiError || error instanceof Error ? error.message : "";
}

export function useCredentials(enabled: boolean): CredentialsData {
  const [state, setState] = React.useState<CredentialsState>({ kind: "loading" });
  const [rotating, setRotating] = React.useState<string | null>(null);
  const [tick, setTick] = React.useState(0);
  const refresh = React.useCallback(() => setTick((n) => n + 1), []);

  React.useEffect(() => {
    if (!enabled) return;
    const controller = new AbortController();
    void (async () => {
      try {
        const view = viewOf(await getCredentials({ signal: controller.signal }));
        if (!controller.signal.aborted) setState({ kind: "ready", view });
      } catch (error) {
        if (isAbort(error) || isUnauthorized(error)) return;
        setState(notPresent(error) ? { kind: "unavailable" } : { kind: "failed", message: messageOf(error) });
      }
    })();
    return () => controller.abort();
  }, [enabled, tick]);

  const rotate = React.useCallback(
    async (credential: Credential): Promise<boolean> => {
      setRotating(credential.name);
      try {
        await postCredentialRotate(credential.name);
        toast.done(t("credentials.rotated", { name: credentialTitle(credential) }));
        return true;
      } catch (error) {
        if (!isUnauthorized(error)) {
          toast.error(t("credentials.rotateFailed"), messageOf(error), { help: "credentials" });
        }
        return false;
      } finally {
        setRotating(null);
        refresh();
      }
    },
    [refresh],
  );

  return { state, rotating, refresh, rotate };
}
