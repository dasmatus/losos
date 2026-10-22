/* Keys between a guest and the rest of a call. A guest has no Matrix
 * device, so there is no Olm session to carry its media keys. Instead each
 * pair in which one side is a guest agrees a key of its own through the
 * SFU's relay:
 *
 *   hello {pub, macs}   an ephemeral P-256 public key, made for this page
 *                       load only, with an HMAC of it under each invite
 *                       secret the sender knows
 *   key {iv, ct}        a KeyMessage sealed with AES-GCM under the pair key
 *
 * The invite secret is the link's fragment: the browser that made the link
 * keeps it, sends it to the other members over Olm, and the guest reads it
 * from the address bar. Neither the box nor the edge ever sees it, so the
 * SFU, which carries every hello, cannot answer one with a key of its own.
 * The pair key comes from ECDH of two keys that exist only in memory, so a
 * secret that leaks later opens nothing that was said.
 */

import { fromBase64, toBase64 } from './e2ee';

const enc = new TextEncoder();
const dec = new TextDecoder();

export interface Hello {
  t: 'hello';
  pub: string;
  macs: string[];
}

export interface Sealed {
  t: 'key';
  iv: string;
  ct: string;
}

export type RelayBody = Hello | Sealed;

/* 32 random bytes, as the link carries them after `#`. */
export function newSecret(): string {
  return toBase64Url(crypto.getRandomValues(new Uint8Array(32)));
}

export function toBase64Url(b: Uint8Array): string {
  return toBase64(b).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
}

export function fromBase64Url(s: string): Uint8Array {
  const std = s.replace(/-/g, '+').replace(/_/g, '/');
  return fromBase64(std + '='.repeat((4 - (std.length % 4)) % 4));
}

/* A secret as a member or a guest hands it in, or null when it is not 32
 * bytes of base64url. */
export function parseSecret(s: unknown): Uint8Array | null {
  if (typeof s !== 'string' || !/^[A-Za-z0-9_-]{43}$/.test(s)) return null;
  try {
    const b = fromBase64Url(s);
    return b.length === 32 ? b : null;
  } catch {
    return null;
  }
}

const helloData = (call: string, pid: number, pub: string): Uint8Array =>
  enc.encode(`losos-guest|${call}|${pid}|${pub}`);

async function hmacKey(secret: Uint8Array): Promise<CryptoKey> {
  return crypto.subtle.importKey('raw', secret as BufferSource, { name: 'HMAC', hash: 'SHA-256' }, false, [
    'sign',
    'verify',
  ]);
}

interface Pair {
  key: CryptoKey;
  pub: string;
}

/* One page's side of every guest pair in a call. `pid` is this page's
 * participant id, known once the SFU has said it. */
export class GuestKeys {
  private mine: Promise<{ pair: CryptoKeyPair; pub: string }>;
  private secrets = new Map<string, Uint8Array>();
  private pairs = new Map<number, Pair>();
  /* Hellos that no known secret opened yet; a member may get the secret
   * after the guest's hello arrives. */
  private waiting = new Map<number, Hello>();
  /* The secret set each peer was last greeted with. */
  private greeted = new Map<number, string>();

  constructor(
    private call: string,
    private send: (to: number, body: RelayBody) => void,
  ) {
    this.mine = (async () => {
      const pair = await crypto.subtle.generateKey({ name: 'ECDH', namedCurve: 'P-256' }, false, ['deriveBits']);
      const raw = new Uint8Array(await crypto.subtle.exportKey('raw', pair.publicKey));
      return { pair, pub: toBase64(raw) };
    })();
  }

  /* Add an invite secret. Returns whether it was new. */
  addSecret(s: string): boolean {
    const b = parseSecret(s);
    if (b === null || this.secrets.has(s)) return false;
    this.secrets.set(s, b);
    return true;
  }

  knownSecrets(): string[] {
    return [...this.secrets.keys()];
  }

  hasPair(pid: number): boolean {
    return this.pairs.has(pid);
  }

  forget(pid: number): void {
    this.pairs.delete(pid);
    this.waiting.delete(pid);
    this.greeted.delete(pid);
  }

  /* Say hello to `peer` unless it was already greeted with every secret
   * this page knows. Nothing is sent without a secret. */
  async greet(you: number, peer: number): Promise<void> {
    if (this.secrets.size === 0) return;
    const set = this.knownSecrets().sort().join(',');
    if (this.greeted.get(peer) === set) return;
    this.greeted.set(peer, set);
    const { pub } = await this.mine;
    const data = helloData(this.call, you, pub);
    const macs: string[] = [];
    for (const secret of this.secrets.values()) {
      const mac = await crypto.subtle.sign('HMAC', await hmacKey(secret), data as BufferSource);
      macs.push(toBase64(new Uint8Array(mac)));
    }
    this.send(peer, { t: 'hello', pub, macs });
  }

  /* A hello from `from`. Returns true when it set up a new pair key, which
   * is when the caller sends its media key over it. */
  async onHello(you: number, from: number, h: Hello): Promise<boolean> {
    if (typeof h.pub !== 'string' || !Array.isArray(h.macs) || h.macs.length > 8) return false;
    if (this.pairs.get(from)?.pub === h.pub) return false;
    if (!(await this.verify(from, h))) {
      this.waiting.set(from, h);
      return false;
    }
    this.waiting.delete(from);
    let theirs: CryptoKey;
    try {
      theirs = await crypto.subtle.importKey(
        'raw',
        fromBase64(h.pub) as BufferSource,
        { name: 'ECDH', namedCurve: 'P-256' },
        false,
        [],
      );
    } catch {
      return false;
    }
    const { pair } = await this.mine;
    const shared = await crypto.subtle.deriveBits({ name: 'ECDH', public: theirs }, pair.privateKey, 256);
    const hkdf = await crypto.subtle.importKey('raw', shared, 'HKDF', false, ['deriveKey']);
    const [lo, hi] = you < from ? [you, from] : [from, you];
    const key = await crypto.subtle.deriveKey(
      { name: 'HKDF', hash: 'SHA-256', salt: new Uint8Array(0), info: enc.encode(`losos-guest-key|${this.call}|${lo}|${hi}`) },
      hkdf,
      { name: 'AES-GCM', length: 256 },
      false,
      ['encrypt', 'decrypt'],
    );
    this.pairs.set(from, { key, pub: h.pub });
    await this.greet(you, from);
    return true;
  }

  private async verify(from: number, h: Hello): Promise<boolean> {
    const data = helloData(this.call, from, h.pub);
    for (const secret of this.secrets.values()) {
      const k = await hmacKey(secret);
      for (const mac of h.macs) {
        let m: Uint8Array;
        try {
          m = fromBase64(String(mac));
        } catch {
          continue;
        }
        if (await crypto.subtle.verify('HMAC', k, m as BufferSource, data as BufferSource)) return true;
      }
    }
    return false;
  }

  /* Try the hellos that waited for a secret this page did not have. */
  async retry(you: number): Promise<number[]> {
    const opened: number[] = [];
    for (const [from, h] of [...this.waiting]) {
      if (await this.onHello(you, from, h)) opened.push(from);
    }
    return opened;
  }

  private aad(from: number, to: number): Uint8Array {
    return enc.encode(`losos-guest-msg|${this.call}|${from}|${to}`);
  }

  async seal(you: number, to: number, value: unknown): Promise<Sealed | null> {
    const p = this.pairs.get(to);
    if (!p) return null;
    const iv = crypto.getRandomValues(new Uint8Array(12));
    const ct = await crypto.subtle.encrypt(
      { name: 'AES-GCM', iv, additionalData: this.aad(you, to) as BufferSource },
      p.key,
      enc.encode(JSON.stringify(value)) as BufferSource,
    );
    return { t: 'key', iv: toBase64(iv), ct: toBase64(new Uint8Array(ct)) };
  }

  async open(you: number, from: number, s: Sealed): Promise<unknown> {
    const p = this.pairs.get(from);
    if (!p || typeof s.iv !== 'string' || typeof s.ct !== 'string') return null;
    try {
      const pt = await crypto.subtle.decrypt(
        { name: 'AES-GCM', iv: fromBase64(s.iv) as BufferSource, additionalData: this.aad(from, you) as BufferSource },
        p.key,
        fromBase64(s.ct) as BufferSource,
      );
      return JSON.parse(dec.decode(pt));
    } catch {
      return null;
    }
  }
}
