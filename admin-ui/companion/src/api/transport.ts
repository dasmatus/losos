/* The one place a box request goes through, TLS and all.
 *
 * The box's admin pages serve a self-signed certificate on the LAN until
 * the owner enrols a CA (BoxStore.kt's BoxTrust: a permissive TrustManager
 * installed per HttpURLConnection). Compose's HttpURLConnection let that
 * per-instance hook be pure Kotlin; React Native's fetch is native and
 * offers no per-request trust hook, so this port carries the hook as a
 * transport choice rather than a library-wide one:
 *
 *   http://  the platform's fetch carries it (cleartext to the LAN is
 *            permitted - the port of the old manifest's res/xml/network.xml
 *            base-config, spelled by expo-build-properties in
 *            app.config.ts). Redirects are not followed, as ApiClient.kt
 *            set instanceFollowRedirects false.
 *
 *   https:// react-native-blob-util's fetch with trusty: true. Under its
 *            1.x semantics the chain is not verified but hostname and
 *            validity dates still are - closer than BoxTrust, which also
 *            let the hostname check go, and strictly permissive enough
 *            where the cert pinning libraries could not follow: pinning is
 *            stricter than an unclaimed box's certificate, which the box
 *            mints before its owner ever sees a pin to bake in, so a pin
 *            would fail exactly the connection the connect screen exists
 *            to make.
 *
 * First-party sources stay TypeScript; the native internals belong to the
 * third-party library, as the plan allows.
 */

import { config, type ReactNativeBlobUtilConfig } from "react-native-blob-util";

/** The connect/read timeouts, ApiClient.kt's 4000/20000 pair. On the
 *  native-fetch path they are two phases: the connection must have begun
 *  answering within Connect, and the body must arrive within Read of that.
 *  The blob-util path has one wall-clock timer, so it uses the sum and
 *  speaks a single "the box did not answer in time" on expiry; the
 *  connect phase can only abort while headers have not begun, which is
 *  close to the HttpURLConnection pairing this comes from. */
export const CONNECT_TIMEOUT_MS = 4_000;
export const READ_TIMEOUT_MS = 20_000;

export interface RawResponse {
  status: number;
  /** The whole body as text; JSON parsing stays up in src/api/api.ts, where
   *  the non-2xx `{"error": ...}` word is read as well. */
  text: string;
}

/** The shape both transports agree to deliver; ApiError lives above this
 *  layer (src/api/api.ts), HTTP word and all. */
export class TransportError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "TransportError";
  }
}

/** GET/POST one box URL. No caching on either path (the SPA's call() asks
 *  no-store); JSON bodies go in as `body` text, Content-Type included.
 *  `deadlineMs` overrides the connect+read sum for callers with sharper
 *  clocks (the probe); the connect phase stays CONNECT_TIMEOUT_MS in any
 *  case where headers have not begun by then. */
export async function boxFetch(
  url: string,
  options: {
    method?: "GET" | "POST";
    headers?: Record<string, string>;
    body?: string;
    deadlineMs?: number;
  } = {},
): Promise<RawResponse> {
  return url.startsWith("https://")
    ? throughBlob(url, options)
    : throughNative(url, options);
}

/* ── http: the platform fetch, with the two timeout phases ──────────────── */

async function throughNative(
  url: string,
  options: { method?: "GET" | "POST"; headers?: Record<string, string>; body?: string; deadlineMs?: number },
): Promise<RawResponse> {
  // Single-threaded discipline: `began` is set the instant the fetch
  // promise resolves (that is, after the response's status line), so the
  // connect timer firing later than that means nothing and stops.
  const controller = new AbortController();
  let began = false;
  const abort = (why: string) => {
    if (began && why === "connect") return;
    controller.abort();
  };
  const connect = setTimeout(() => abort("connect"), CONNECT_TIMEOUT_MS);
  const read = setTimeout(
    () => abort("read"),
    options.deadlineMs ?? CONNECT_TIMEOUT_MS + READ_TIMEOUT_MS,
  );
  try {
    const response = await fetch(url, {
      method: options.method ?? "GET",
      headers: options.headers,
      body: options.body,
      redirect: "manual",
      signal: controller.signal,
      cache: "no-store",
    } as RequestInit);
    began = true;
    const text = await response.text();
    return { status: response.status, text };
  } catch (e) {
    const word = beginWord(e as Error | undefined, began);
    throw new TransportError(word);
  } finally {
    clearTimeout(connect);
    clearTimeout(read);
  }
}

function beginWord(e: Error | undefined, began: boolean): string {
  const message = e?.message ?? "";
  if (!began) return `could not reach the box${message ? `: ${message}` : ""}`;
  if (message.includes("Aborted")) return "the answer came apart: the box answered too slowly";
  return message === "" ? "the answer came apart" : message;
}

/* ── https: react-native-blob-util, trusty, one wall-clock timer ────────── */


async function throughBlob(
  url: string,
  options: { method?: "GET" | "POST"; headers?: Record<string, string>; body?: string; deadlineMs?: number },
): Promise<RawResponse> {
  const method = options.method ?? "GET";
  const headers: Record<string, string> = { ...options.headers, "Cache-Control": "no-store" };
  try {
    // The util's own builder: config(options).fetch — one per request, so a
    // changed timeout between calls can not stick to a cached instance.
    const resp = await config({
      timeout: options.deadlineMs ?? CONNECT_TIMEOUT_MS + READ_TIMEOUT_MS,
      trusty: true,
    }).fetch(method, url, headers, options.body);
    return { status: resp.info().status, text: await resp.text() };
  } catch (e) {
    throw new TransportError(blobWord(e as Error | undefined));
  }
}

function blobWord(e: Error | undefined): string {
  const message = e?.message ?? "";
  if (/ESSL/i.test(message)) return "the box's certificate could not be trusted";
  return message === "" ? "could not reach the box" : message;
}
