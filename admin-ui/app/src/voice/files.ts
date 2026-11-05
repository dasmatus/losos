/* The voice models are big (the ears 57 MB, the mouth 92 MB) and fixed for
 * a release, so a page fetches each once and keeps it in the Cache API under
 * its URL; the next call starts without downloading anything. Nothing here
 * leaves the box: every URL is the page's own origin. */

const CACHE = "losos-voice-v1";

export type Progress = (loaded: number, total: number) => void;

async function cache(): Promise<Cache | null> {
  try {
    return await caches.open(CACHE);
  } catch {
    // Private windows and blocked site data refuse the Cache API.
    return null;
  }
}

/* The bytes at `url`, from the cache when this page has them. */
export async function fetchBytes(url: string, progress?: Progress): Promise<Uint8Array<ArrayBuffer>> {
  const store = await cache();
  const hit = await store?.match(url);
  const response = hit ?? (await fetch(url, { credentials: "same-origin" }));
  if (!response.ok) throw new Error(`${url}: ${response.status}`);
  if (hit === undefined && store !== null) {
    // Cached from a copy, so reading the body below does not consume it.
    await store.put(url, response.clone()).catch(() => undefined);
  }
  const total = Number(response.headers.get("content-length") ?? 0);
  if (response.body === null || progress === undefined) return new Uint8Array(await response.arrayBuffer());
  const reader = response.body.getReader();
  const parts: Uint8Array[] = [];
  let loaded = 0;
  for (;;) {
    const { done, value } = await reader.read();
    if (done) break;
    parts.push(value);
    loaded += value.length;
    progress(loaded, total);
  }
  const out = new Uint8Array(new ArrayBuffer(loaded));
  let at = 0;
  for (const p of parts) {
    out.set(p, at);
    at += p.length;
  }
  return out;
}

/* Where the build puts the voice files (vite.config.ts `voiceFiles`). */
export const VOICE_BASE = `${import.meta.env.BASE_URL}voice`;
