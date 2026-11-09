/* The box's JSON API, driven the way the admin SPA drives it
 * (admin-ui/app/src/lib/api.ts, `call()`): every document answers at
 * <base>/api/… and every route except /api/health wants
 * `Authorization: Bearer <token>` - the 64-hex key the box minted at first
 * boot (modules/credentials.nix). A 401 is the session ending: the token
 * goes and the app asks again, like the SPA's eviction (api.ts dropToken,
 * which the app shell subscribed to; here onUnauthorized is wired once per
 * BoxApi by the screen that made it, and the store's update performs it).
 */

import { boxFetch, TransportError } from "./transport";

/** What one route answered with: a JSON object. The box answers no
 *  arrays at the top level; anything else is a `{}`. */
export type JsonBody = Record<string, unknown>;

/** A non-2xx from lososd; the message carries the box's own error word. */
export class ApiError extends Error {
  readonly status: number;

  constructor(status: number, message: string) {
    super(message);
    this.name = "ApiError";
    this.status = status;
  }

  /** The token this session holds is not the one lososd minted. Re-prompt
   *  (the SPA's ApiError.unauthorized getter, used by the eviction). */
  get unauthorized(): boolean {
    return this.status === 401;
  }
}

/** The API of one box, with the owner's token in every call it makes. */
export class BoxApi {
  /** The base URL as the config carries it (BoxApi.bas): `http://`
   *  addresses are taken as typed, everything else rides https. */
  readonly base: string;
  private readonly token: string;
  /** A rejected token's fate, wired by the caller: flip to the connect
   *  screen (Panes.kt.boxApi remembered exactly this closure). */
  private readonly onUnauthorized: () => void;

  constructor(address: string, token: string, onUnauthorized: () => void = () => {}) {
    const trimmed = address.trim().replace(/\/+$/, "");
    this.base = trimmed.startsWith("http") ? trimmed : `https://${trimmed}`;
    this.token = token;
    this.onUnauthorized = onUnauthorized;
  }

  /** GET a document; /api/health is fetched anonymous, like the SPA's
   *  own health call (api.ts getHealth). */
  get(path: string, options: { anonymous?: boolean } = {}): Promise<JsonBody> {
    return this.call(path, "GET", undefined, options.anonymous === true);
  }

  /** POST a bodyless action (/api/grow, /api/config/sync, /api/backup/run). */
  post(path: string): Promise<JsonBody> {
    return this.call(path, "POST", undefined, false);
  }

  /** POST JSON; app install/remove and other actions carry a body
   *  (Panes.kt's app install used exactly this shape). */
  postJson(path: string, body: JsonBody): Promise<JsonBody> {
    return this.call(path, "POST", JSON.stringify(body), false);
  }

  private async call(
    path: string,
    method: "GET" | "POST",
    body: string | undefined,
    anonymous: boolean,
  ): Promise<JsonBody> {
    if (this.base === "https://" || this.base === "http://") {
      throw new ApiError(0, "No address set");
    }
    const headers: Record<string, string> = {
      // The token rides only real API calls; reachability probes never see
      // it (BoxStore.kt's BoxProbe) - `anonymous` is that switch here too.
      Accept: "application/json",
    };
    if (!anonymous && this.token !== "") headers.Authorization = `Bearer ${this.token}`;
    if (body !== undefined) headers["Content-Type"] = "application/json";
    const raw = await boxFetch(`${this.base}${path}`, { method, headers, body });

    if (raw.status === 401) {
      // The token got thrown back; the SPA's eviction (api.ts, dropToken on
      // 401 only when it was the stored one) applies with no candidate
      // concept here - the connect screen probed anonymously, so no probe
      // ever lands here.
      this.onUnauthorized();
      throw new ApiError(401, "The box wants its token again");
    }
    if (raw.status < 200 || raw.status >= 300) {
      throw new ApiError(raw.status, errorWord(raw));
    }
    if (raw.text === "") return {};
    try {
      const parsed = JSON.parse(raw.text);
      return parsed !== null && typeof parsed === "object" && !Array.isArray(parsed)
        ? (parsed as JsonBody)
        : {};
    } catch (e) {
      throw new ApiError(0, `The answer was not JSON: ${(e as Error).message}`);
    }
  }
}

/** The box's error body is {"error": "..."}; anything else stays the HTTP
 *  word (ApiClient.kt errorText, the SPA's errorBody). */
function errorWord(raw: { status: number; text: string }): string {
  try {
    const body = JSON.parse(raw.text) as { error?: unknown };
    if (typeof body.error === "string" && body.error.trim() !== "") return body.error;
  } catch {
    /* lososd restarting mid-rebuild answers through nginx, not as JSON */
  }
  return `HTTP ${raw.status}`;
}

/** Whether an unknown failure is an ApiError. Not all callers want the
 *  import; panes mostly read `.message` and are done (Panes.kt Read.Failed). */
export function isApiError(e: unknown): e is ApiError {
  return e instanceof ApiError;
}

/** The word a pane's Read.Failed carries: the box's, or the transport's
 *  (Panes.kt: e.message ?: e.javaClass.simpleName). */
export function failWord(e: unknown): string {
  if (e instanceof TransportError) return e.message;
  if (e instanceof ApiError) return e.message;
  const message = (e as Error | undefined)?.message;
  return typeof message === "string" && message !== "" ? message : "something went wrong";
}
