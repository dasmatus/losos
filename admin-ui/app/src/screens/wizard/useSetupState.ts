/* One fetch of /setup/state.json, shared by every step that needs it.
 *
 * Step 1 wants the certificate and the https address, step 3 prints the box's
 * name onto the recovery sheet, and step 4 names the address the sign-in page
 * is on. Fetching it per step would be three requests for a file written once
 * per boot, and — worse — three independent error states for one failure.
 *
 * The document is served by Nginx from disk, without a token, guarded only by
 * source address (`lanOnly` in modules/setup.nix). That is deliberate: the
 * wizard has to name the box and fingerprint its certificate *before* the
 * owner trusts the connection, so it cannot be behind anything that trust
 * would be a prerequisite for.
 */

import * as React from "react";
import { getSetupState, isForbidden, isMissingRoute, type SetupState } from "./api";

export type SetupQuery =
  | { kind: "loading" }
  | { kind: "ready"; state: SetupState }
  /** The LAN guard refused. Reached over the master proxy, or from loopback. */
  | { kind: "blocked" }
  /** This box's software predates modules/setup.nix. Nothing to install. */
  | { kind: "absent" }
  | { kind: "failed"; message: string };

export function useSetupState(): SetupQuery {
  const [query, setQuery] = React.useState<SetupQuery>({ kind: "loading" });

  React.useEffect(() => {
    const controller = new AbortController();
    let live = true;

    void (async () => {
      try {
        const state = await getSetupState({ signal: controller.signal });
        if (live) setQuery({ kind: "ready", state });
      } catch (error) {
        if (!live || controller.signal.aborted) return;
        if (isForbidden(error)) setQuery({ kind: "blocked" });
        else if (isMissingRoute(error)) setQuery({ kind: "absent" });
        else setQuery({ kind: "failed", message: message(error) });
      }
    })();

    return () => {
      live = false;
      controller.abort();
    };
  }, []);

  return query;
}

function message(error: unknown): string {
  if (error instanceof Error && error.message.length > 0) return error.message;
  return "This box did not answer.";
}

/** The box's name when it is known, and a neutral word when it is not. Used
 *  in prose, where "this box" reads better than an empty gap. */
export function boxName(query: SetupQuery): string {
  return query.kind === "ready" ? query.state.hostName : "this box";
}
