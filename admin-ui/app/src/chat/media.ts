/* Pictures and files in chat.
 *
 * The homeserver serves media only to a signed-in device (authenticated
 * media, /_matrix/client/v1/media/), so an <img src> cannot point at it:
 * the page fetches the bytes with its token and shows them as a blob: URL,
 * which the chat page's policy allows for images and media. In an encrypted
 * room a file is encrypted on this page before it is uploaded (AES-CTR, as
 * the spec's EncryptedFile describes) and decrypted here after download, so
 * the box stores only ciphertext. */

import type { MatrixClient } from "matrix-js-sdk";
import { sendContent } from "./model";

/** The spec's EncryptedFile. */
export interface EncryptedFile {
  v: "v2";
  url: string;
  key: JsonWebKey & { alg: "A256CTR"; k: string; kty: "oct"; ext: true; key_ops: string[] };
  iv: string;
  hashes: { sha256: string };
}

/** What an m.image, m.file, m.audio or m.video event points at. */
export interface MediaSource {
  url?: string;
  file?: EncryptedFile;
}

const unpadded = (bytes: Uint8Array): string => btoa(String.fromCharCode(...bytes)).replace(/=+$/, "");

function fromBase64(text: string): Uint8Array<ArrayBuffer> {
  const normal = text.replace(/-/g, "+").replace(/_/g, "/");
  const padded = normal + "=".repeat((4 - (normal.length % 4)) % 4);
  const raw = atob(padded);
  const out = new Uint8Array(new ArrayBuffer(raw.length));
  for (let i = 0; i < raw.length; i += 1) out[i] = raw.charCodeAt(i);
  return out;
}

/** Encrypt an attachment for an encrypted room. */
export async function encryptAttachment(data: ArrayBuffer): Promise<{ data: ArrayBuffer; info: Omit<EncryptedFile, "url"> }> {
  const iv = new Uint8Array(16);
  // The upper half random, the lower half a zero counter (the spec's shape).
  crypto.getRandomValues(iv.subarray(0, 8));
  const key = await crypto.subtle.generateKey({ name: "AES-CTR", length: 256 }, true, ["encrypt", "decrypt"]);
  const sealed = await crypto.subtle.encrypt({ name: "AES-CTR", counter: iv, length: 64 }, key, data);
  const jwk = await crypto.subtle.exportKey("jwk", key);
  const hash = new Uint8Array(await crypto.subtle.digest("SHA-256", sealed));
  return {
    data: sealed,
    info: {
      v: "v2",
      key: { alg: "A256CTR", ext: true, k: jwk.k ?? "", kty: "oct", key_ops: ["encrypt", "decrypt"] },
      iv: unpadded(iv),
      hashes: { sha256: unpadded(hash) },
    },
  };
}

async function decryptAttachment(data: ArrayBuffer, file: EncryptedFile): Promise<ArrayBuffer> {
  const hash = new Uint8Array(await crypto.subtle.digest("SHA-256", data));
  if (unpadded(hash) !== file.hashes.sha256.replace(/=+$/, "")) throw new Error("attachment hash mismatch");
  const key = await crypto.subtle.importKey(
    "jwk",
    { kty: "oct", k: file.key.k, alg: "A256CTR", ext: true },
    { name: "AES-CTR" },
    false,
    ["decrypt"],
  );
  return crypto.subtle.decrypt({ name: "AES-CTR", counter: fromBase64(file.iv), length: 64 }, key, data);
}

const cache = new Map<string, Promise<string>>();

/** A blob: URL for an event's media, fetched once per page load. `thumb`
 *  asks for a server-made thumbnail of that width (unencrypted only). */
export function mediaUrl(client: MatrixClient, source: MediaSource, mime: string, thumb?: number): Promise<string> {
  const mxc = source.file?.url ?? source.url ?? "";
  const key = `${mxc}#${thumb ?? 0}`;
  let found = cache.get(key);
  if (found === undefined) {
    found = fetchMedia(client, source, mime, thumb);
    cache.set(key, found);
    found.catch(() => cache.delete(key));
  }
  return found;
}

async function fetchMedia(client: MatrixClient, source: MediaSource, mime: string, thumb?: number): Promise<string> {
  return URL.createObjectURL(await fetchBlob(client, source, mime, thumb));
}

/** An event's file, decrypted, as `name`: what "Save to LosOS Cloud"
 *  uploads. */
export async function mediaFile(client: MatrixClient, source: MediaSource, mime: string, name: string): Promise<File> {
  const blob = await fetchBlob(client, source, mime);
  return new File([blob], name, { type: blob.type });
}

async function fetchBlob(client: MatrixClient, source: MediaSource, mime: string, thumb?: number): Promise<Blob> {
  const mxc = source.file?.url ?? source.url;
  if (mxc === undefined || !mxc.startsWith("mxc://")) throw new Error("no media");
  const sized = source.file === undefined && thumb !== undefined;
  const http = client.mxcUrlToHttp(
    mxc,
    sized ? thumb : undefined,
    sized ? thumb : undefined,
    sized ? "scale" : undefined,
    false,
    true,
    true,
  );
  if (http === null) throw new Error("no media");
  const response = await fetch(http, { headers: { Authorization: `Bearer ${client.getAccessToken() ?? ""}` } });
  if (!response.ok) throw new Error(`HTTP ${response.status}`);
  let bytes = await response.arrayBuffer();
  if (source.file !== undefined) bytes = await decryptAttachment(bytes, source.file);
  return new Blob([bytes], { type: source.file !== undefined ? safeMime(mime) : (response.headers.get("Content-Type") ?? safeMime(mime)) });
}

/* A decrypted blob carries the type the sender claimed. Only types a
 * browser shows inertly are kept; anything else (HTML, SVG with script)
 * downloads as bytes. */
function safeMime(mime: string): string {
  return /^(image\/(png|jpeg|gif|webp|avif)|video\/(mp4|webm|ogg)|audio\/(mpeg|ogg|webm|mp4|wav|x-wav|aac|flac))$/.test(mime)
    ? mime
    : "application/octet-stream";
}

/** Save a file to the person's computer. */
export async function download(client: MatrixClient, source: MediaSource, mime: string, name: string): Promise<void> {
  const url = await mediaUrl(client, source, mime);
  const link = document.createElement("a");
  link.href = url;
  link.download = name;
  link.rel = "noopener";
  document.body.append(link);
  link.click();
  link.remove();
}

export type MsgKind = "m.image" | "m.video" | "m.audio" | "m.file";

export function kindOf(mime: string): MsgKind {
  if (/^image\/(png|jpeg|gif|webp|avif)$/.test(mime)) return "m.image";
  if (mime.startsWith("video/")) return "m.video";
  if (mime.startsWith("audio/")) return "m.audio";
  return "m.file";
}

function imageSize(file: Blob): Promise<{ w: number; h: number } | null> {
  return new Promise((resolve) => {
    const url = URL.createObjectURL(file);
    const img = new Image();
    img.onload = () => {
      resolve({ w: img.naturalWidth, h: img.naturalHeight });
      URL.revokeObjectURL(url);
    };
    img.onerror = () => {
      resolve(null);
      URL.revokeObjectURL(url);
    };
    img.src = url;
  });
}

/** Upload a file (encrypted when the room is) and send it as a message.
 *  `extra` goes into the content as is (a voice message's flags). */
export async function sendFile(
  client: MatrixClient,
  roomId: string,
  file: Blob,
  name: string,
  options: { threadRoot?: string; extra?: Record<string, unknown>; durationMs?: number } = {},
): Promise<void> {
  const mime = file.type || "application/octet-stream";
  const msgtype = kindOf(mime);
  const info: Record<string, unknown> = { mimetype: mime, size: file.size };
  if (msgtype === "m.image") {
    const size = await imageSize(file);
    if (size !== null) Object.assign(info, size);
  }
  if (options.durationMs !== undefined) info["duration"] = Math.round(options.durationMs);
  const encrypted = client.getCrypto() !== undefined && (await client.getCrypto()?.isEncryptionEnabledInRoom(roomId)) === true;
  const content: Record<string, unknown> = { msgtype, body: name, filename: name, info, ...options.extra };
  if (encrypted) {
    const sealed = await encryptAttachment(await file.arrayBuffer());
    const upload = await client.uploadContent(new Blob([sealed.data], { type: "application/octet-stream" }), {
      type: "application/octet-stream",
      includeFilename: false,
    });
    content["file"] = { ...sealed.info, url: upload.content_uri, mimetype: mime };
  } else {
    const upload = await client.uploadContent(file, { name, type: mime });
    content["url"] = upload.content_uri;
  }
  if (options.threadRoot !== undefined) {
    content["m.relates_to"] = { rel_type: "m.thread", event_id: options.threadRoot, is_falling_back: true };
  }
  await sendContent(client, roomId, content);
}
