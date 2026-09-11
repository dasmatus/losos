/* Passkey registration — the browser half, written; the box half, pending.
 *
 *   PENDING  POST /api/passkey/register/begin
 *   PENDING  POST /api/passkey/register/finish
 *
 * NEITHER ROUTE EXISTS. backend/src/http.rs serves nine routes and none of
 * them is this; there is no WebAuthn relying-party code in the crate at all.
 * Until there is, `createPasskey` fails on its first request with a 404 and
 * step 2 says, in words, that this box cannot register a passkey yet — see
 * `isMissingRoute` in ./api.ts, which is what distinguishes that from an
 * outage. What it must never do is report a passkey the box has never heard
 * of: the owner would find out at the next sign-in, with no password set,
 * on an appliance that has no shell.
 *
 * The ceremony below is not speculative — it is the WebAuthn registration
 * dance as the spec defines it, so it should need no change when the routes
 * land. What the routes must return is pinned by `CreationOptionsJson`.
 *
 * # Why the password is not optional
 *
 * A passkey is scoped to the browser that made it. The desktop and phone sync
 * clients authenticate with a name and a password (or an app password derived
 * from one), and Nextcloud's `twofactor_webauthn` — the app
 * modules/nextcloud-stack.nix installs — is a *second* factor layered on a
 * password, not a replacement for one. So step 2 collects a password whatever
 * happens here, and this file is an addition to it.
 *
 * # Why it is offered rather than pushed
 *
 * The relying-party ID would be `<hostName>.local`, and whether a browser
 * accepts an mDNS name as one is untested across the field. A SecurityError
 * on that is a normal outcome here, not a bug, and it gets its own message.
 */

import { ApiError, dropToken, getToken } from "@/lib/api";

// ── Is this even possible in this browser, on this connection? ────────────

export type PasskeySupport =
  | {
      kind: "available";
      /* True when the device has a built-in authenticator — Touch ID, Windows
       * Hello, an Android screen lock. False means the owner would need a
       * security key on a USB port, which is worth saying before they press a
       * button and get a dialog asking for hardware they do not have. */
      platformAuthenticator: boolean;
    }
  | { kind: "unavailable"; reason: string };

/* Can step 2 offer a passkey *from this page*, right now?
 *
 * Synchronous, and deliberately the same two facts `probePasskeySupport`
 * checks before it does anything async, because step 1 has to predict what
 * step 2 will do and the two must not disagree.
 *
 * They did disagree. Step 1 predicted from the BOX's `tls` flag — "not serving
 * https, so no passkey" — while step 2 decided from the BROWSER's secure
 * context. Those are different questions, and they part company whenever the
 * page is already in a secure context for a reason that has nothing to do with
 * the box's certificate: `http://localhost` and `http://127.0.0.1` are secure
 * contexts by definition, which is exactly how the wizard is developed and
 * demonstrated. Step 1 then promised "the passkey option will not be offered"
 * and step 2 offered it on the next screen.
 *
 * So the prediction is made here, from the browser, and step 1 reads it. */
export function passkeysPossibleHere(): boolean {
  return (
    window.isSecureContext &&
    typeof window.PublicKeyCredential !== "undefined" &&
    navigator.credentials !== undefined
  );
}

/* Two hard requirements, checked in the order the owner can act on them.
 *
 * The secure-context check comes first even though a browser without WebAuthn
 * would also fail the second: over plain http the fix is step 1 of this very
 * wizard, and "install the certificate and come back" is advice, where "this
 * browser cannot do passkeys" is a dead end. Reporting the dead end to someone
 * who is one step away from the fix would be wrong.
 *
 * Async because the platform-authenticator probe is. */
export async function probePasskeySupport(): Promise<PasskeySupport> {
  if (!window.isSecureContext) {
    return {
      kind: "unavailable",
      reason:
        "Passkeys need an encrypted connection. Go back a step, install this box's certificate, and reopen this page at its https address.",
    };
  }
  if (typeof window.PublicKeyCredential === "undefined" || navigator.credentials === undefined) {
    return { kind: "unavailable", reason: "This browser cannot create passkeys." };
  }

  let platformAuthenticator = false;
  try {
    platformAuthenticator =
      await window.PublicKeyCredential.isUserVerifyingPlatformAuthenticatorAvailable();
  } catch {
    // Some browsers throw rather than resolving false. Not a reason to hide
    // the button — a plugged-in security key still works.
    platformAuthenticator = false;
  }
  return { kind: "available", platformAuthenticator };
}

// ── base64url, both ways ──────────────────────────────────────────────────

/* WebAuthn's JS API speaks ArrayBuffer and its JSON encoding speaks base64url,
 * so a conversion at the boundary is unavoidable.
 *
 * Done by hand rather than through `PublicKeyCredential.parseCreationOptionsFromJSON`
 * and `credential.toJSON()`, which would be shorter: those landed in Chrome
 * 119, Safari 17.4 and Firefox 135, and the appliance's owner is whoever walks
 * up to it. A feature-detected fast path would mean two code paths where only
 * one of them is ever exercised in testing. */

function fromBase64Url(value: string): ArrayBuffer {
  const padded = value.replace(/-/g, "+").replace(/_/g, "/");
  const binary = window.atob(padded.padEnd(Math.ceil(padded.length / 4) * 4, "="));
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i += 1) bytes[i] = binary.charCodeAt(i);
  return bytes.buffer;
}

function toBase64Url(value: ArrayBuffer): string {
  const bytes = new Uint8Array(value);
  let binary = "";
  for (const byte of bytes) binary += String.fromCharCode(byte);
  return window.btoa(binary).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
}

// ── What POST /api/passkey/register/begin must return ─────────────────────

/* PublicKeyCredentialCreationOptions, JSON-shaped: every field that is a
 * BufferSource in the JS API is base64url here. This is the contract the
 * backend has to meet; a Rust relying-party library (webauthn-rs, say) emits
 * exactly this shape.
 *
 * `rp.id` is omitted deliberately from the required set: leaving it out makes
 * the browser use the current origin's host, which is the behaviour that has
 * the best chance of working on a `.local` name. The backend should send it
 * only if it has a reason to. */

export interface CreationOptionsJson {
  rp: { name: string; id?: string };
  user: { id: string; name: string; displayName: string };
  /** base64url, at least 16 bytes, minted per attempt and not reused. */
  challenge: string;
  pubKeyCredParams: readonly { type: "public-key"; alg: number }[];
  timeout?: number;
  /** Credentials already registered, so a second attempt on the same device
   *  is refused by the authenticator instead of making a duplicate. */
  excludeCredentials?: readonly { type: "public-key"; id: string; transports?: string[] }[];
  authenticatorSelection?: {
    authenticatorAttachment?: "platform" | "cross-platform";
    residentKey?: "discouraged" | "preferred" | "required";
    requireResidentKey?: boolean;
    userVerification?: "discouraged" | "preferred" | "required";
  };
  attestation?: "none" | "indirect" | "direct" | "enterprise";
}

export interface PasskeyBeginResponse {
  publicKey: CreationOptionsJson;
}

/** What the browser sends back. The backend verifies the challenge, the
 *  origin and the RP ID hash against these. */
export interface PasskeyFinishRequest {
  id: string;
  rawId: string;
  type: "public-key";
  response: {
    clientDataJSON: string;
    attestationObject: string;
    transports: string[];
  };
  /** What the owner will see in the list of their passkeys. */
  label: string;
}

export interface PasskeyFinishResponse {
  /** Echoed back so the wizard can name the key it just made. */
  label: string;
  /** True on the one call that registered it. A failure is an error reply. */
  registered: boolean;
}

// ── Transport ─────────────────────────────────────────────────────────────

/* The same rules as ./api.ts' `authedJson`, and for the same reason: a 401
 * evicts the stored admin token so the shell's unlock dialog reappears.
 * Duplicated rather than imported because ./api.ts keeps its transport
 * private, and both files are small enough that a shared one would be a third
 * file to keep in step. */
async function authedJson<T>(path: string, body: unknown, signal?: AbortSignal): Promise<T> {
  const headers: Record<string, string> = { "Content-Type": "application/json" };
  const token = getToken();
  if (token !== null) headers["Authorization"] = `Bearer ${token}`;

  const init: RequestInit = {
    method: "POST",
    headers,
    cache: "no-store",
    body: JSON.stringify(body),
  };
  if (signal !== undefined) init.signal = signal;

  const response = await fetch(path, init);
  if (response.status === 401) {
    dropToken();
    throw new ApiError(401, "unauthorized");
  }
  if (!response.ok) {
    let message = `HTTP ${response.status}`;
    try {
      const parsed = (await response.json()) as { error?: unknown };
      if (typeof parsed.error === "string" && parsed.error.length > 0) message = parsed.error;
    } catch {
      /* Nginx's own error bodies are HTML. */
    }
    throw new ApiError(response.status, message);
  }
  return (await response.json()) as T;
}

export const PASSKEY_BEGIN_URL = "/api/passkey/register/begin";
export const PASSKEY_FINISH_URL = "/api/passkey/register/finish";

// ── The ceremony ──────────────────────────────────────────────────────────

/* What went wrong, in a shape step 2 can branch on.
 *
 * `cancelled` is not an error the owner needs an explanation for — they
 * dismissed a dialog — so it is separated from the rest rather than being
 * given a message that reads like a failure. */
export type PasskeyOutcome =
  | { kind: "registered"; label: string }
  | { kind: "cancelled" }
  | { kind: "unsupported-here"; message: string }
  | { kind: "not-implemented" }
  | { kind: "failed"; message: string };

export interface CreatePasskeyOptions {
  /** Shown in the owner's list of passkeys. Something they will recognise. */
  label: string;
  signal?: AbortSignal;
}

/** Run the whole registration. Never throws: every outcome is a variant. */
export async function createPasskey(options: CreatePasskeyOptions): Promise<PasskeyOutcome> {
  const support = await probePasskeySupport();
  if (support.kind === "unavailable") {
    return { kind: "unsupported-here", message: support.reason };
  }

  let begun: PasskeyBeginResponse;
  try {
    begun = await authedJson<PasskeyBeginResponse>(
      PASSKEY_BEGIN_URL,
      { label: options.label },
      options.signal,
    );
  } catch (error) {
    // The expected outcome today: the route is not served.
    if (error instanceof ApiError && (error.status === 404 || error.status === 501)) {
      return { kind: "not-implemented" };
    }
    return { kind: "failed", message: describe(error) };
  }

  let credential: Credential | null;
  try {
    const init: CredentialCreationOptions = {
      publicKey: toCreationOptions(begun.publicKey),
    };
    if (options.signal !== undefined) init.signal = options.signal;
    credential = await navigator.credentials.create(init);
  } catch (error) {
    return fromCeremonyError(error);
  }

  if (credential === null || !isAttestation(credential)) {
    return { kind: "failed", message: "This browser returned a passkey this box cannot read." };
  }

  const attestation = credential.response;
  try {
    const finished = await authedJson<PasskeyFinishResponse>(
      PASSKEY_FINISH_URL,
      {
        id: credential.id,
        rawId: toBase64Url(credential.rawId),
        type: "public-key",
        response: {
          clientDataJSON: toBase64Url(attestation.clientDataJSON),
          attestationObject: toBase64Url(attestation.attestationObject),
          transports: readTransports(attestation),
        },
        label: options.label,
      } satisfies PasskeyFinishRequest,
      options.signal,
    );
    return { kind: "registered", label: finished.label };
  } catch (error) {
    if (error instanceof ApiError && (error.status === 404 || error.status === 501)) {
      return { kind: "not-implemented" };
    }
    /* The authenticator has already stored a key the box then failed to
     * record. Say so — the owner will otherwise see it offered at the next
     * sign-in and be refused. */
    return {
      kind: "failed",
      message: `${describe(error)} Your device may have saved a passkey this box did not keep; remove it from your device's passkey list before trying again.`,
    };
  }
}

function toCreationOptions(json: CreationOptionsJson): PublicKeyCredentialCreationOptions {
  const rp: PublicKeyCredentialRpEntity = { name: json.rp.name };
  if (json.rp.id !== undefined) rp.id = json.rp.id;

  const options: PublicKeyCredentialCreationOptions = {
    rp,
    user: {
      id: fromBase64Url(json.user.id),
      name: json.user.name,
      displayName: json.user.displayName,
    },
    challenge: fromBase64Url(json.challenge),
    pubKeyCredParams: json.pubKeyCredParams.map((param) => ({ ...param })),
  };
  if (json.timeout !== undefined) options.timeout = json.timeout;
  if (json.attestation !== undefined) options.attestation = json.attestation;
  if (json.authenticatorSelection !== undefined) {
    options.authenticatorSelection = { ...json.authenticatorSelection };
  }
  if (json.excludeCredentials !== undefined) {
    options.excludeCredentials = json.excludeCredentials.map((entry) => {
      const descriptor: PublicKeyCredentialDescriptor = {
        type: entry.type,
        id: fromBase64Url(entry.id),
      };
      if (entry.transports !== undefined) {
        descriptor.transports = entry.transports as AuthenticatorTransport[];
      }
      return descriptor;
    });
  }
  return options;
}

/* `navigator.credentials.create` resolves to the `Credential` base type, so
 * the attestation fields have to be checked rather than asserted. A guard that
 * only looked at `instanceof PublicKeyCredential` would still leave
 * `.response` typed as the base `AuthenticatorResponse`, which has neither
 * field this needs. */
interface AttestationCredential extends PublicKeyCredential {
  response: AuthenticatorAttestationResponse;
}

function isAttestation(credential: Credential): credential is AttestationCredential {
  if (!("response" in credential)) return false;
  const response = (credential as PublicKeyCredential).response;
  return "attestationObject" in response && "clientDataJSON" in response;
}

/* Optional since Level 2 and absent on plenty of authenticators. The backend
 * stores whatever arrives and uses it as a hint at sign-in time; an empty
 * list is a valid answer, not a failure. */
function readTransports(response: AuthenticatorAttestationResponse): string[] {
  if (typeof response.getTransports !== "function") return [];
  try {
    return response.getTransports();
  } catch {
    return [];
  }
}

/* The three DOMException names that mean something specific here. Everything
 * else becomes a generic failure with the browser's own message, which is
 * more use than a sentence invented for it. */
function fromCeremonyError(error: unknown): PasskeyOutcome {
  if (!(error instanceof DOMException)) {
    return { kind: "failed", message: describe(error) };
  }
  switch (error.name) {
    case "NotAllowedError":
      // Dismissed, or the dialog timed out. Both are the owner's choice.
      return { kind: "cancelled" };
    case "AbortError":
      return { kind: "cancelled" };
    case "InvalidStateError":
      return {
        kind: "failed",
        message: "This device already has a passkey for this box.",
      };
    case "SecurityError":
      // The expected failure on an mDNS name, and the reason the password
      // half of step 2 is not optional.
      return {
        kind: "failed",
        message:
          "This browser would not accept this box's local name for a passkey. Use the password instead.",
      };
    default:
      return { kind: "failed", message: describe(error) };
  }
}

function describe(error: unknown): string {
  if (error instanceof Error && error.message.length > 0) return error.message;
  return "This box did not answer.";
}
