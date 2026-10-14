/* One call through the signed and encrypted endpoint.
 * The contract is docs/terminal-access.md, "One call".
 *
 * The request's plaintext is {v, host, token, method, path, ts, nonce, body}.
 * It is signed by the user's key and encrypted to the pinned host key in one
 * OpenPGP message, POSTed as application/pgp-encrypted. A sealed answer is
 * decrypted with the user's key, must carry a valid signature by the pinned
 * host key, and must echo the nonce. Anything else is a plain JSON error the
 * server gave before it could trust anyone. */

import { randomBytes } from "node:crypto";
import * as openpgp from "openpgp";
import { t } from "./lang";
import type { PrivateKey, PublicKey } from "./pgp";
import type { HostKind } from "./vault";

export const PGP_TYPE = "application/pgp-encrypted";

export interface Session {
  url: string;
  kind: HostKind;
  hostKey: PublicKey;
  /** 40 hex, upper case: what goes into the envelope's `host`. */
  hostFingerprint: string;
  token: string;
  /** Unlocked. */
  userKey: PrivateKey;
}

export interface Answer {
  /** The inner call's status, or the HTTP status of a plain refusal. */
  status: number;
  /** The inner answer's JSON (a string when it was not JSON), or the plain
   *  refusal's `{error}`. */
  body: unknown;
  /** True when the answer was sealed and checked. */
  sealed: boolean;
}

/** The answer failed a check: not signed by the pinned key, a nonce that is
 *  not ours, or something that is not an answer at all. */
export class EnvelopeError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "EnvelopeError";
  }
}

export interface Plaintext {
  v: 1;
  host: string;
  token: string;
  method: string;
  path: string;
  ts: number;
  nonce: string;
  body: unknown;
}

export function callUrl(url: string, kind: HostKind): string {
  return `${url}${kind === "edge" ? "/operator/call" : "/api/access/call"}`;
}

export function hostKeyUrl(url: string, kind: HostKind): string {
  return `${url}${kind === "edge" ? "/operator/host-key" : "/api/access/host-key"}`;
}

export function newNonce(): string {
  return randomBytes(16).toString("hex");
}

/** For tests: pin the clock or the nonce. */
export interface SealOverrides {
  ts?: number;
  nonce?: string;
}

export async function seal(
  session: Session,
  method: string,
  path: string,
  body: unknown,
  overrides: SealOverrides = {},
): Promise<{ armored: string; plaintext: Plaintext }> {
  const plaintext: Plaintext = {
    v: 1,
    host: session.hostFingerprint,
    token: session.token,
    method: method.toUpperCase(),
    path,
    ts: overrides.ts ?? Math.floor(Date.now() / 1000),
    nonce: overrides.nonce ?? newNonce(),
    body: body === undefined ? null : body,
  };
  const message = await openpgp.createMessage({ text: JSON.stringify(plaintext) });
  const armored = await openpgp.encrypt({
    message,
    encryptionKeys: session.hostKey,
    signingKeys: session.userKey,
    format: "armored",
  });
  return { armored, plaintext };
}

export async function open(session: Session, armored: string, nonce: string): Promise<{ status: number; body: unknown }> {
  let data: string;
  try {
    const message = await openpgp.readMessage({ armoredMessage: armored });
    const result = await openpgp.decrypt({
      message,
      decryptionKeys: session.userKey,
      verificationKeys: session.hostKey,
      expectSigned: true,
      format: "utf8",
    });
    data = result.data as string;
  } catch {
    throw new EnvelopeError(t("tui.envelope.badSignature"));
  }
  let inner: unknown;
  try {
    inner = JSON.parse(data);
  } catch {
    throw new EnvelopeError(t("tui.envelope.notAnswer"));
  }
  if (typeof inner !== "object" || inner === null) throw new EnvelopeError(t("tui.envelope.notAnswer"));
  const fields = inner as Record<string, unknown>;
  if (fields["v"] !== 1 || typeof fields["status"] !== "number") {
    throw new EnvelopeError(t("tui.envelope.notAnswer"));
  }
  if (fields["nonce"] !== nonce) throw new EnvelopeError(t("tui.envelope.wrongNonce"));
  return { status: fields["status"], body: fields["body"] ?? null };
}

export type Fetch = typeof fetch;

export async function call(
  session: Session,
  method: string,
  path: string,
  body?: unknown,
  options: { fetch?: Fetch; overrides?: SealOverrides; signal?: AbortSignal } = {},
): Promise<Answer> {
  const doFetch = options.fetch ?? fetch;
  const { armored, plaintext } = await seal(session, method, path, body, options.overrides);
  const init: RequestInit = {
    method: "POST",
    headers: { "Content-Type": PGP_TYPE, Accept: `${PGP_TYPE}, application/json` },
    body: armored,
  };
  if (options.signal !== undefined) init.signal = options.signal;
  let response: Response;
  try {
    response = await doFetch(callUrl(session.url, session.kind), init);
  } catch (error) {
    throw new EnvelopeError(t("tui.envelope.unreachable", { url: session.url, detail: describe(error) }));
  }
  const type = response.headers.get("content-type") ?? "";
  const text = await response.text();
  if (type.toLowerCase().startsWith(PGP_TYPE)) {
    const inner = await open(session, text, plaintext.nonce);
    return { ...inner, sealed: true };
  }
  let parsed: unknown = text;
  try {
    parsed = JSON.parse(text);
  } catch {
    /* an HTML error page from a proxy: keep the text */
  }
  return { status: response.status, body: parsed, sealed: false };
}

function describe(error: unknown): string {
  if (error instanceof Error) {
    const cause = (error as Error & { cause?: unknown }).cause;
    if (cause instanceof Error && cause.message.length > 0) return cause.message;
    return error.message;
  }
  return String(error);
}

/** The error sentence of a refusal, plain or sealed. */
export function errorText(answer: Answer): string {
  const body = answer.body;
  if (typeof body === "object" && body !== null && typeof (body as { error?: unknown }).error === "string") {
    return (body as { error: string }).error;
  }
  if (typeof body === "string" && body.trim().length > 0) return body.trim().slice(0, 300);
  return `HTTP ${answer.status}`;
}
