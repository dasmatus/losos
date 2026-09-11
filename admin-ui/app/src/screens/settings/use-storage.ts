import * as React from "react";
import {
  hasToken,
  isAbort,
  isUnauthorized,
  postGrow,
  subscribeAuth,
  type GrowResponse,
} from "@/lib/api";
import { authedGet, byteCount, isRecord, notPresent } from "./http";

/* Storage: what the box says about its disk, and claiming the reserve.
 *
 * Two halves, and only one of them exists on the box today.
 *
 * GROW is real. POST /api/grow runs lvextend, then `cryptsetup resize`, then
 * resize2fs, online, and answers with `{grew, beforeBytes, afterBytes,
 * claimedBytes}`. `grew` is MEASURED — the daemon compares the filesystem
 * size before and after rather than trusting three exit statuses, because
 * every way that sequence goes wrong goes wrong quietly (resize2fs against an
 * unresized mapping prints "Nothing to do!" and exits 0). So the pane reports
 * `grew`, and when it is false it says nothing changed. It never says "grown"
 * because a request returned 200.
 *
 * THE CAPACITY READING is not real yet. There is no route on lososd that
 * reports how full /persist is; backend/schema.json's growResponse is the
 * only place a byte count appears at all, and it only appears after you have
 * already grown. So `GET /api/storage` below is a contract this screen
 * proposes and tolerates the absence of: 404 means "this box cannot measure
 * itself yet", the meter says exactly that, and nothing is invented to fill
 * the bar. The shape wanted is
 *
 *     { "totalBytes": n, "usedBytes": n, "lentBytes": n, "reserveBytes": n }
 *
 * — every field optional, every one a non-negative integer of bytes:
 *   totalBytes    size of the /persist filesystem (growResponse.afterBytes)
 *   usedBytes     of that, in use by this box's own data
 *   lentBytes     of that, holding copies for other boxes on the mesh
 *   reserveBytes  unallocated volume-group space a grow would claim
 *                 (growResponse.claimedBytes, read without growing)
 */

export const STORAGE_URL = "/api/storage";

export interface StorageFacts {
  totalBytes: number | null;
  usedBytes: number | null;
  lentBytes: number | null;
  reserveBytes: number | null;
}

const NOTHING_KNOWN: StorageFacts = {
  totalBytes: null,
  usedBytes: null,
  lentBytes: null,
  reserveBytes: null,
};

/** Whether the box can measure its own disk. `absent` is the normal answer
 *  today and is a fact to state, not a failure to report. */
export type StorageSource = "loading" | "box" | "absent" | "error";

export type GrowOutcome =
  | { kind: "grew"; beforeBytes: number; afterBytes: number }
  | { kind: "nothing"; claimedBytes: number }
  | { kind: "failed"; message: string };

export interface Storage {
  facts: StorageFacts;
  source: StorageSource;
  /** A grow is in flight. It is slow and it cannot be cancelled. */
  growing: boolean;
  /** The last grow this tab ran, reported from `grew` rather than inferred. */
  outcome: GrowOutcome | null;
  grow: () => void;
  dismissOutcome: () => void;
}

function parseFacts(body: unknown): StorageFacts {
  if (!isRecord(body)) return NOTHING_KNOWN;
  return {
    totalBytes: byteCount(body["totalBytes"]),
    usedBytes: byteCount(body["usedBytes"]),
    lentBytes: byteCount(body["lentBytes"]),
    reserveBytes: byteCount(body["reserveBytes"]),
  };
}

function describe(error: unknown): string {
  if (error instanceof Error && error.message.length > 0) return error.message;
  return "This box did not answer.";
}

export function useStorage(): Storage {
  const signedIn = React.useSyncExternalStore(subscribeAuth, hasToken, () => false);

  const [facts, setFacts] = React.useState<StorageFacts>(NOTHING_KNOWN);
  const [source, setSource] = React.useState<StorageSource>("loading");
  const [growing, setGrowing] = React.useState(false);
  const [outcome, setOutcome] = React.useState<GrowOutcome | null>(null);

  React.useEffect(() => {
    if (!signedIn) {
      setFacts(NOTHING_KNOWN);
      setSource("loading");
      setOutcome(null);
      return;
    }

    const controller = new AbortController();
    let cancelled = false;

    void (async () => {
      try {
        const body = await authedGet(STORAGE_URL, { signal: controller.signal });
        if (cancelled) return;
        setFacts(parseFacts(body));
        setSource("box");
      } catch (error) {
        if (cancelled || isAbort(error) || isUnauthorized(error)) return;
        setFacts(NOTHING_KNOWN);
        // A box that does not serve the route is not a box that is broken.
        setSource(notPresent(error) ? "absent" : "error");
      }
    })();

    return () => {
      cancelled = true;
      controller.abort();
    };
  }, [signedIn]);

  /* A grow does not start a rebuild, so there is no job to poll: the response
   * IS the outcome, and it arrives when three tools have finished running
   * against a mounted filesystem. Slow, and not cancellable — aborting the
   * fetch would abandon the answer, not the work. */
  const record = React.useCallback((result: GrowResponse) => {
    if (!result.grew) {
      // Measured, not inferred. Nothing moved, so nothing about the reading
      // changed either — and the pane says so in those words.
      setOutcome({ kind: "nothing", claimedBytes: result.claimedBytes });
      return;
    }
    setOutcome({ kind: "grew", beforeBytes: result.beforeBytes, afterBytes: result.afterBytes });
    /* The one place a byte count can be trusted without /api/storage: the
     * daemon just measured the filesystem on both sides of the resize. Fold it
     * in so the meter stops showing the pre-grow total, and zero the reserve,
     * which is what was just spent. */
    setFacts((current) => ({ ...current, totalBytes: result.afterBytes, reserveBytes: 0 }));
  }, []);

  const grow = React.useCallback(() => {
    if (growing) return;
    setGrowing(true);
    setOutcome(null);
    void (async () => {
      try {
        record(await postGrow());
      } catch (error) {
        if (isUnauthorized(error)) {
          setOutcome(null);
          return;
        }
        setOutcome({ kind: "failed", message: describe(error) });
      } finally {
        setGrowing(false);
      }
    })();
  }, [growing, record]);

  const dismissOutcome = React.useCallback(() => setOutcome(null), []);

  return { facts, source, growing, outcome, grow, dismissOutcome };
}
