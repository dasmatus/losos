/* The first-run wizard's own client calls.
 *
 * Three of these routes exist today and one does not. The distinction is
 * marked on every declaration below and it is load-bearing — a wizard that
 * pretends to have minted a recovery code is worse than one that says it
 * cannot, because the owner writes down a number that proves nothing.
 *
 *   SHIPPING  GET  /setup/state.json   modules/setup.nix, static, no token
 *   SHIPPING  GET  /setup/losos-ca.crt modules/setup.nix, static, no token
 *   SHIPPING  POST /api/set-password   backend/src/http.rs + setup.rs
 *   PENDING   GET  /api/recovery       backend/src/recovery.rs exists; no HTTP
 *                                      route is wired to it yet
 *
 * The passkey half is pending too and lives in ./passkey.ts.
 *
 * Why a transport of its own rather than @/lib/api's: that module's `call()`
 * is private and its exported surface is one function per route it already
 * knows. Adding routes to it would mean editing a file this change does not
 * own. The duplication is ~30 lines and it borrows lib/api's `ApiError`,
 * `getToken` and `dropToken`, so authentication behaves identically — in
 * particular a 401 here evicts the stored token, which is what makes the
 * shell's sign-in dialog reappear.
 */

import { ApiError, dropToken, getToken } from "@/lib/api";

// ── Transport ─────────────────────────────────────────────────────────────

export interface RequestOptions {
  signal?: AbortSignal;
}

/** A request that never carries the admin token. The two /setup routes are
 *  served by Nginx from disk and are guarded by source address, not by a
 *  Bearer header — the wizard reads them before it has been unlocked. */
async function anonymousJson<T>(path: string, options: RequestOptions = {}): Promise<T> {
  const init: RequestInit = { method: "GET", cache: "no-store" };
  if (options.signal !== undefined) init.signal = options.signal;

  const response = await fetch(path, init);
  if (!response.ok) throw new ApiError(response.status, await errorMessage(response));
  return (await response.json()) as T;
}

interface AuthedRequest extends RequestOptions {
  method: "GET" | "POST";
  /** Serialized as JSON. Omit for a request with no body. */
  body?: unknown;
}

async function authedJson<T>(path: string, request: AuthedRequest): Promise<T> {
  const headers: Record<string, string> = {};
  const token = getToken();
  if (token !== null) headers["Authorization"] = `Bearer ${token}`;
  if (request.body !== undefined) headers["Content-Type"] = "application/json";

  const init: RequestInit = { method: request.method, headers, cache: "no-store" };
  if (request.body !== undefined) init.body = JSON.stringify(request.body);
  if (request.signal !== undefined) init.signal = request.signal;

  const response = await fetch(path, init);

  if (response.status === 401) {
    // Same rule as @/lib/api: the stored token stopped working, so drop it and
    // let the shell re-prompt. Nothing in the wizard probes a candidate token,
    // so there is no case here where eviction would be wrong.
    dropToken();
    throw new ApiError(401, "unauthorized");
  }
  if (!response.ok) throw new ApiError(response.status, await errorMessage(response));
  return (await response.json()) as T;
}

async function errorMessage(response: Response): Promise<string> {
  try {
    const body = (await response.json()) as { error?: unknown };
    if (typeof body.error === "string" && body.error.length > 0) return body.error;
  } catch {
    /* Nginx's own 403/404 bodies are HTML, and lososd restarting mid-rebuild
     * answers through Nginx rather than as JSON. */
  }
  return `HTTP ${response.status}`;
}

/** A route this box's software does not serve — as opposed to one that failed.
 *  The wizard says so in words rather than showing an error. */
export function isMissingRoute(error: unknown): boolean {
  return error instanceof ApiError && (error.status === 404 || error.status === 501);
}

/** The LAN guard on /setup/* refused this request. Reached from loopback —
 *  which, on a box with the master proxy on, is where tunnelled traffic
 *  arrives. See the `lanOnly` note in modules/setup.nix. */
export function isForbidden(error: unknown): boolean {
  return error instanceof ApiError && error.status === 403;
}

// ── GET /setup/state.json ─────────────────────────────────────────────────

/* SHIPPING. Written by the `losos-setup-state` unit in modules/setup.nix on
 * every boot, from the certificate itself. Static, JSON, no token: the wizard
 * needs the box's name and its certificate's fingerprint before the owner can
 * reach anything over HTTPS at all, and everything token-gated is behind a
 * connection they are only about to start trusting. */

export const SETUP_STATE_URL = "/setup/state.json";

export interface SetupCertificate {
  /** Where to download the certificate. `/setup/losos-ca.crt` today; read it
   *  from here rather than hard-coding, so the two cannot drift. */
  url: string;
  /** `sha256:` followed by 64 lowercase hex characters. */
  fingerprint: string;
  /** The same digest colon-separated and upper-case, which is how every
   *  browser's certificate viewer prints it — so the owner can compare the two
   *  strings character for character instead of transcribing. */
  fingerprintDisplay: string;
  /** ISO 8601 in UTC. The certificate is minted for two years. */
  expires: string;
}

export interface SetupState {
  hostName: string;
  /** `<hostName>.local` — the name the certificate is valid for. */
  fqdn: string;
  /** False when `losos.tls.enable` is off. There is then no certificate to
   *  install and step 1 has nothing to ask for. */
  tls: boolean;
  certificate: SetupCertificate | null;
}

export async function getSetupState(options: RequestOptions = {}): Promise<SetupState> {
  return parseSetupState(await anonymousJson<unknown>(SETUP_STATE_URL, options));
}

/* Validated rather than cast.
 *
 * This document is assembled by a shell script interpolating `openssl` output
 * into a heredoc, so "it parsed as JSON" is a weaker guarantee here than for a
 * serde-generated body — a certificate the unit could not measure produces a
 * document with fields missing, not a 500. Anything unrecognised becomes a
 * thrown ApiError, which step 1 already has a branch for. */
function parseSetupState(value: unknown): SetupState {
  if (typeof value !== "object" || value === null) {
    throw new ApiError(0, "the setup document is not an object");
  }
  const raw = value as Record<string, unknown>;
  if (typeof raw["hostName"] !== "string" || typeof raw["fqdn"] !== "string") {
    throw new ApiError(0, "the setup document does not name this box");
  }
  return {
    hostName: raw["hostName"],
    fqdn: raw["fqdn"],
    tls: raw["tls"] === true,
    certificate: parseCertificate(raw["certificate"]),
  };
}

function parseCertificate(value: unknown): SetupCertificate | null {
  if (typeof value !== "object" || value === null) return null;
  const raw = value as Record<string, unknown>;
  const url = raw["url"];
  const fingerprint = raw["fingerprint"];
  if (typeof url !== "string" || typeof fingerprint !== "string") return null;
  return {
    url,
    fingerprint,
    fingerprintDisplay:
      typeof raw["fingerprintDisplay"] === "string" ? raw["fingerprintDisplay"] : fingerprint,
    expires: typeof raw["expires"] === "string" ? raw["expires"] : "",
  };
}

// ── POST /api/set-password ────────────────────────────────────────────────

/* SHIPPING. backend/src/http.rs `post_set_password`, planned in
 * backend/src/setup.rs.
 *
 * This is the reason the wizard exists. modules/nextcloud-common.nix generates
 * the install-time password from /dev/urandom, writes it 0600 and shows it to
 * nobody; the appliance has no SSH and no shell login, so on a fresh box
 * nobody can sign in at all. This route replaces that password with one the
 * owner chose. It never reads the old one back — there would be no point, the
 * owner has to type a password they want either way. */

export interface SetPasswordResponse {
  /** The account that changed, echoed back. The wizard shows it: the owner has
   *  to type this name on the sign-in page and has no other way to learn it. */
  user: string;
  /** Which route ran. Not shown to the owner — it names a runtime. */
  mode: "container" | "native";
  /** Whether occ exited 0. A failure is an error reply, never `false`, so this
   *  is true on every success path. */
  changed: boolean;
  message: string;
}

/** No `user` field on purpose: the default is the appliance's own admin
 *  account and the wizard has no business knowing its name to reset it.
 *  It learns the name from the response. */
export function postSetPassword(
  password: string,
  options: RequestOptions = {},
): Promise<SetPasswordResponse> {
  return authedJson<SetPasswordResponse>("/api/set-password", {
    ...options,
    method: "POST",
    body: { password },
  });
}

/* The server's rules, restated. backend/src/setup.rs `validate_password` is
 * the boundary — this is only here so the owner hears about a short password
 * before the request rather than as a 400 afterwards. Keep the two in step:
 * MIN_PASSWORD_CHARS / MAX_PASSWORD_BYTES are the constants' names there too.
 *
 * Length only, and that is deliberate on the server's side: composition rules
 * push people towards shorter passwords they reuse. */
export const MIN_PASSWORD_CHARS = 12;
export const MAX_PASSWORD_BYTES = 256;

/** The problem with `password`, in words the owner can act on, or null. */
export function passwordProblem(password: string): string | null {
  // Characters, not UTF-16 units — a passphrase in a non-Latin script should
  // be measured the way its owner would count it, which is what the server
  // does with `chars().count()`.
  const characters = [...password].length;
  if (characters < MIN_PASSWORD_CHARS) {
    return `Use at least ${MIN_PASSWORD_CHARS} characters. This one has ${characters}.`;
  }
  if (new TextEncoder().encode(password).length > MAX_PASSWORD_BYTES) {
    return "That is longer than this box accepts. Shorten it.";
  }
  // A newline is the one that bites: the password reaches the app through a
  // staged file read with `$(cat …)`, which would silently truncate at the
  // first one — leaving the owner certain of a password that was never set.
  if (/\p{Cc}/u.test(password)) {
    return "Remove the line breaks and control characters.";
  }
  return null;
}

// ── GET /api/recovery ─────────────────────────────────────────────────────

/* Served. `backend/src/recovery.rs` mints a v4 UUID once and keeps it at
 * /var/secrets/losos-recovery-code; `backend/src/http.rs` routes GET
 * /api/recovery to it, `backend/src/dbus.rs` exposes the same call as
 * `Recovery`, and `backend/schema.json` carries the wire shape the interface
 * below is transcribed from.
 *
 * The 404 branch is kept, and is not dead code: this SPA is served from the
 * appliance's own store path, but `losos.backend.package` can be pinned to an
 * older lososd, and a box mid-`nixos-rebuild switch` has the new UI in front of
 * the previous daemon for the length of the rebuild. Step 3 then says the box
 * cannot show a code, in words, and unblocks Continue.
 *
 * What it must never do is invent one. A UUID the box did not mint is a piece
 * of paper that proves nothing, and the owner would not find out until the
 * reinstall this code exists to survive. */

export const RECOVERY_URL = "/api/recovery";

export interface RecoveryResponse {
  /** Canonical lowercase hyphenated UUID, 36 characters. */
  code: string;
  /** True only on the call that created it, so the wizard can say "write this
   *  down now" once and "here it is again" afterwards. Describes the call, not
   *  persisted state. */
  minted: boolean;
}

export function getRecovery(options: RequestOptions = {}): Promise<RecoveryResponse> {
  return authedJson<RecoveryResponse>(RECOVERY_URL, { ...options, method: "GET" });
}
