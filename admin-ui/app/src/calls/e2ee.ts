/* End-to-end encryption of call media, one encoded frame at a time.
 *
 * Every frame a browser sends is encrypted here before WebRTC packetizes it,
 * and decrypted here after the receiving browser reassembles it, so the SFU
 * on the box (and anything between) forwards only ciphertext. It is the
 * shape of SFrame (RFC 9605) fitted to WebRTC's encoded transforms, as the
 * established E2EE SFUs do it:
 *
 *   | clear prefix | AES-256-GCM ciphertext + 16-byte tag | ctr(4) | index(1) | 0x4C |
 *
 * The clear prefix is the few bytes a packetizer and the SFU's keyframe
 * logic read: VP8's payload header (10 bytes on a keyframe, 3 otherwise)
 * and Opus's TOC byte. They carry frame type and size, never picture or
 * sound, and they are authenticated as additional data with the trailer, so
 * changing any of them makes the frame fail to decrypt.
 *
 * Keys. Each member has a sender key of 32 random bytes and an index. The
 * frame key and IV salt are derived from it per media kind with HKDF, so
 * audio and video never share a (key, IV) pair; the IV is the salt XOR the
 * frame counter. A sender key moves forward in two ways:
 *
 *   ratchet  next = HKDF(key, "losos-call ratchet"), index + 1. Done when
 *            someone joins (they get the new key and cannot read what came
 *            before) and every few minutes. Receivers follow on their own
 *            when they see the next index, so nothing is sent.
 *   rotate   a fresh random key, index + 1, sent to the members who remain
 *            when someone leaves, so the leaver cannot read what comes next.
 *
 * Keys live in memory only and are dropped with the call; old ones are
 * forgotten as the index moves on. That, with the keys travelling over Olm
 * (see keys.ts), is the forward secrecy: a key taken later opens nothing
 * already sent.
 */

export const KEY_BYTES = 32;
export const TAG_BYTES = 16;
const IV_BYTES = 12;
const TRAILER_BYTES = 6;
const MAGIC = 0x4c;

/* A receiver jumps at most this far forward by ratcheting on its own. */
export const MAX_RATCHET_AHEAD = 16;
/* Old indexes a receiver keeps, for frames that arrive late. */
const KEEP_BEHIND = 2;

export type Kind = 'audio' | 'video';

const enc = new TextEncoder();

/* WebCrypto takes BufferSource; a Uint8Array view over a SharedArrayBuffer
 * is not one, so every buffer handed in is copied into a fresh ArrayBuffer. */
function own(b: Uint8Array): Uint8Array<ArrayBuffer> {
  const out = new Uint8Array(b.byteLength);
  out.set(b);
  return out;
}

async function hkdf(key: Uint8Array, info: string, bits: number): Promise<Uint8Array> {
  const base = await crypto.subtle.importKey('raw', own(key), 'HKDF', false, ['deriveBits']);
  const out = await crypto.subtle.deriveBits(
    { name: 'HKDF', hash: 'SHA-256', salt: new Uint8Array(32), info: enc.encode(info) },
    base,
    bits,
  );
  return new Uint8Array(out);
}

export function randomKey(): Uint8Array {
  return crypto.getRandomValues(new Uint8Array(KEY_BYTES));
}

/* The sender key after `key`. */
export function ratchet(key: Uint8Array): Promise<Uint8Array> {
  return hkdf(key, 'losos-call ratchet', KEY_BYTES * 8);
}

export interface FrameKey {
  key: CryptoKey;
  salt: Uint8Array;
}

export interface FrameKeys {
  audio: FrameKey;
  video: FrameKey;
}

async function frameKey(sender: Uint8Array, kind: Kind): Promise<FrameKey> {
  const bytes = await hkdf(sender, `losos-call ${kind}`, (KEY_BYTES + IV_BYTES) * 8);
  const key = await crypto.subtle.importKey(
    'raw',
    own(bytes.subarray(0, KEY_BYTES)),
    'AES-GCM',
    false,
    ['encrypt', 'decrypt'],
  );
  return { key, salt: bytes.slice(KEY_BYTES) };
}

export async function frameKeys(sender: Uint8Array): Promise<FrameKeys> {
  return { audio: await frameKey(sender, 'audio'), video: await frameKey(sender, 'video') };
}

function iv(salt: Uint8Array, counter: number): Uint8Array<ArrayBuffer> {
  const out = own(salt);
  const view = new DataView(out.buffer);
  view.setUint32(8, view.getUint32(8) ^ counter);
  return out;
}

/* How many leading bytes stay in the clear. `keyframe` is what the encoder
 * said; a VP8 frame also says it in bit 0 of its first byte. */
export function clearBytes(kind: Kind, frame: Uint8Array, keyframe?: boolean): number {
  if (kind === 'audio') return Math.min(1, frame.length);
  const isKey = keyframe ?? (frame.length > 0 && ((frame[0] ?? 1) & 1) === 0);
  return Math.min(isKey ? 10 : 3, frame.length);
}

export async function encryptFrame(
  key: FrameKey,
  index: number,
  counter: number,
  frame: Uint8Array,
  clear: number,
): Promise<Uint8Array> {
  const trailer = new Uint8Array(TRAILER_BYTES);
  const tv = new DataView(trailer.buffer);
  tv.setUint32(0, counter >>> 0);
  trailer[4] = index & 0xff;
  trailer[5] = MAGIC;
  const head = frame.subarray(0, clear);
  const aad = new Uint8Array(clear + TRAILER_BYTES);
  aad.set(head);
  aad.set(trailer, clear);
  const sealed = new Uint8Array(
    await crypto.subtle.encrypt(
      { name: 'AES-GCM', iv: iv(key.salt, counter >>> 0), additionalData: aad, tagLength: TAG_BYTES * 8 },
      key.key,
      own(frame.subarray(clear)),
    ),
  );
  const out = new Uint8Array(clear + sealed.length + TRAILER_BYTES);
  out.set(head);
  out.set(sealed, clear);
  out.set(trailer, clear + sealed.length);
  return out;
}

export interface Trailer {
  counter: number;
  index: number;
}

export function readTrailer(frame: Uint8Array): Trailer | null {
  if (frame.length < TRAILER_BYTES + TAG_BYTES || frame[frame.length - 1] !== MAGIC) return null;
  const t = frame.subarray(frame.length - TRAILER_BYTES);
  return { counter: new DataView(t.buffer, t.byteOffset).getUint32(0), index: t[4] ?? 0 };
}

/* The plain frame, or null when it does not open with `key`. */
export async function decryptFrame(
  key: FrameKey,
  frame: Uint8Array,
  clear: number,
): Promise<Uint8Array | null> {
  const t = readTrailer(frame);
  if (!t || frame.length < clear + TAG_BYTES + TRAILER_BYTES) return null;
  const body = frame.subarray(clear, frame.length - TRAILER_BYTES);
  const aad = new Uint8Array(clear + TRAILER_BYTES);
  aad.set(frame.subarray(0, clear));
  aad.set(frame.subarray(frame.length - TRAILER_BYTES), clear);
  try {
    const plain = new Uint8Array(
      await crypto.subtle.decrypt(
        { name: 'AES-GCM', iv: iv(key.salt, t.counter), additionalData: aad, tagLength: TAG_BYTES * 8 },
        key.key,
        own(body),
      ),
    );
    const out = new Uint8Array(clear + plain.length);
    out.set(frame.subarray(0, clear));
    out.set(plain, clear);
    return out;
  } catch {
    return null;
  }
}

/* The clear prefix a received frame was sent with. The receiver cannot ask
 * its own encoder, so it reads the VP8 keyframe bit, which stayed clear. */
export function receivedClearBytes(kind: Kind, frame: Uint8Array): number {
  return clearBytes(kind, frame);
}

/* What one sender's frames are decrypted with: its current index and the
 * few before, derived on demand. */
export class ReceiveRing {
  private keys = new Map<number, Promise<FrameKeys>>();
  private raw = new Map<number, Uint8Array>();
  /* Indexes whose key this ring worked out itself rather than was given. */
  private derived = new Set<number>();
  private newest: number | null = null;

  /* A key the sender handed over (rotate, or the one a joiner gets). */
  set(index: number, key: Uint8Array): void {
    const i = index & 0xff;
    this.raw.set(i, key.slice());
    this.keys.set(i, frameKeys(key));
    this.derived.delete(i);
    // A late message for an older index is kept but does not move back.
    if (this.newest === null || (i - this.newest + 256) % 256 < 128) this.newest = i;
    this.forget();
  }

  has(index: number): boolean {
    return this.raw.has(index & 0xff);
  }

  /* Whether the key for `index` was ratcheted here rather than handed
   * over. A rotation is a fresh key at the next index, so a frame that
   * outruns its key meets a ratcheted guess that cannot open it: that
   * frame lacks its key, it was not tampered with. */
  guessed(index: number): boolean {
    return this.derived.has(index & 0xff);
  }

  /* The frame keys for `index`, ratcheting forward from the newest key when
   * the sender moved on by itself. */
  get(index: number): Promise<FrameKeys | null> {
    const i = index & 0xff;
    const known = this.keys.get(i);
    if (known) return known;
    // Frames decrypt concurrently; two that both see the next index must
    // not both ratchet, so the forward steps run one at a time.
    const step = this.stepping.then(() => this.forward(i));
    this.stepping = step.then(
      () => undefined,
      () => undefined,
    );
    return step;
  }

  private stepping: Promise<void> = Promise.resolve();

  private async forward(i: number): Promise<FrameKeys | null> {
    const known = this.keys.get(i);
    if (known) return known;
    const from = this.newest;
    if (from === null) return null;
    const ahead = (i - from + 256) % 256;
    if (ahead === 0 || ahead > MAX_RATCHET_AHEAD) return null;
    let key = this.raw.get(from);
    if (!key) return null;
    for (let step = 1; step <= ahead; step++) {
      key = await ratchet(key);
      const at = (from + step) & 0xff;
      // A key handed over meanwhile wins over one worked out here.
      if (!this.raw.has(at)) {
        this.raw.set(at, key);
        this.keys.set(at, frameKeys(key));
        this.derived.add(at);
      }
      if (this.newest === null || (at - this.newest + 256) % 256 < 128) this.newest = at;
    }
    this.forget();
    return this.keys.get(i) ?? null;
  }

  private forget(): void {
    if (this.newest === null) return;
    for (const i of [...this.raw.keys()]) {
      const behind = (this.newest - i + 256) % 256;
      if (behind > KEEP_BEHIND) {
        this.raw.get(i)?.fill(0);
        this.raw.delete(i);
        this.keys.delete(i);
        this.derived.delete(i);
      }
    }
  }

  clear(): void {
    for (const k of this.raw.values()) k.fill(0);
    this.raw.clear();
    this.keys.clear();
    this.derived.clear();
    this.newest = null;
  }
}

/* The key this browser sends with. Counters restart with every new key, and
 * a key is ratcheted long before its counter could wrap. */
export class SendKey {
  key: Uint8Array;
  index: number;
  private derived: Promise<FrameKeys>;
  private counters: Record<Kind, number> = { audio: 0, video: 0 };

  constructor(key: Uint8Array = randomKey(), index = 0) {
    this.key = key.slice();
    this.index = index & 0xff;
    this.derived = frameKeys(this.key);
  }

  async ratchet(): Promise<void> {
    this.replace(await ratchet(this.key), this.index + 1);
  }

  rotate(): void {
    this.replace(randomKey(), this.index + 1);
  }

  private replace(key: Uint8Array, index: number): void {
    this.key.fill(0);
    this.key = key;
    this.index = index & 0xff;
    this.derived = frameKeys(key);
    this.counters = { audio: 0, video: 0 };
  }

  async seal(kind: Kind, frame: Uint8Array, keyframe?: boolean): Promise<Uint8Array> {
    // Taken together before the await, so a ratchet that lands meanwhile
    // cannot pair one key's frame keys with the next key's index.
    const derived = this.derived;
    const index = this.index;
    const counter = this.counters[kind]++;
    if (counter >= 0xffff_fff0) throw new Error('frame counter exhausted');
    const keys = await derived;
    return encryptFrame(keys[kind], index, counter, frame, clearBytes(kind, frame, keyframe));
  }
}

export function toBase64(b: Uint8Array): string {
  let s = '';
  for (const x of b) s += String.fromCharCode(x);
  return btoa(s);
}

export function fromBase64(s: string): Uint8Array {
  const bin = atob(s);
  const out = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
  return out;
}
