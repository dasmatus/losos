/* An authed GET for the two routes this screen needs and backend/schema.json
 * does not yet define.
 *
 * @/lib/api covers the nine routes lososd serves today; its transport is
 * private to that module and rightly so. The Storage pane wants a capacity
 * reading and the Apps pane wants a catalogue search, and neither exists on
 * the box yet (see storage.ts and catalogue.ts for the exact shapes each
 * expects). Rather than leave two dead controls on the page, both are wired
 * to their real route and both degrade when the box answers 404 — so the day
 * lososd grows the route, the pane lights up with no change here.
 *
 * `notPresent` is the whole point of this file: a 404 or a 501 from the box
 * means "this appliance does not do that yet", which is a thing to say
 * plainly in the UI, not an error to paint red.
 */

import { ApiError, dropToken, getToken } from "@/lib/api";

/** The box answered, and its answer was "I do not serve that route". */
export function notPresent(error: unknown): boolean {
  return error instanceof ApiError && (error.status === 404 || error.status === 501);
}

export interface GetOptions {
  signal?: AbortSignal;
}

/* Same 401 handling as @/lib/api's own transport: the stored token is
 * evicted once, here, and the shell's unlock prompt wakes through
 * subscribeAuth. Anything else comes back as an ApiError carrying the box's
 * own `{"error": "..."}` message. */
export async function authedGet(path: string, options: GetOptions = {}): Promise<unknown> {
  const token = getToken();
  const headers: Record<string, string> = {};
  if (token !== null) headers["Authorization"] = `Bearer ${token}`;

  const init: RequestInit = { method: "GET", headers, cache: "no-store" };
  if (options.signal !== undefined) init.signal = options.signal;

  const response = await fetch(path, init);

  if (response.status === 401) {
    dropToken();
    throw new ApiError(401, "unauthorized");
  }
  if (!response.ok) {
    throw new ApiError(response.status, await errorMessage(response));
  }
  return (await response.json()) as unknown;
}

async function errorMessage(response: Response): Promise<string> {
  try {
    const body = (await response.json()) as { error?: unknown };
    if (typeof body.error === "string" && body.error.length > 0) return body.error;
  } catch {
    /* nginx's own 404 page is HTML, not JSON. */
  }
  return `HTTP ${response.status}`;
}

// ── Narrowing helpers for the boundary ────────────────────────────────────

/* Everything that comes back over the wire is `unknown` until one of these
 * has looked at it. A route that does not exist yet is a route whose eventual
 * response nobody has seen, so the parsers below accept a missing field and
 * refuse a wrong one rather than trusting a shape. */

export function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

/** A finite, non-negative byte count. Anything else — including a string of
 *  digits — is treated as "the box did not report this". */
export function byteCount(value: unknown): number | null {
  return typeof value === "number" && Number.isFinite(value) && value >= 0 ? value : null;
}

export function nonEmptyString(value: unknown): string | null {
  return typeof value === "string" && value.trim().length > 0 ? value.trim() : null;
}
