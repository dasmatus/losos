/* OpenPGP in LosOS Mail, the private half: everything that needs a
 * person's private key happens here, in their browser, with OpenPGP.js.
 *
 * The key is made or imported here and locked with the person's passphrase
 * before it leaves the page; the box keeps the locked key (sealed again in
 * its credential store, backend/src/mail_pgp.rs) and the public key, and
 * never sees the passphrase. Unlocked, the key lives in this module's
 * memory for as long as the tab is open or until the person locks it.
 *
 * Mail goes out as PGP/MIME (RFC 3156) by default: this module writes the
 * MIME part, encrypts or signs it, and hands the armour to lososd, which
 * puts the headers and the multipart around it. Inline PGP (the armour as
 * the text of the message) is there for mail programs that only read that.
 * Mail comes in as the raw message (GET /api/mail/messages/<id>/raw); this
 * module takes it apart, decrypts and checks it, and reads what was inside
 * with postal-mime. OpenPGP.js is a chunk of its own, loaded the first time
 * Mail needs it. */

import type * as OpenPGP from "openpgp";

type Lib = typeof OpenPGP;
let lib: Promise<Lib> | null = null;

/* OpenPGP.js, loaded once. */
export function openpgp(): Promise<Lib> {
  lib ??= import("openpgp");
  return lib;
}

/* ── The person's own key ──────────────────────────────────────────────── */

let unlocked: OpenPGP.PrivateKey | null = null;
const listeners = new Set<() => void>();

function changed() {
  for (const l of listeners) l();
}

/** Be told when the key is unlocked or locked. */
export function onKeyChange(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

/** The unlocked private key, if it is the one with `fingerprint`. */
export function unlockedKey(fingerprint: string | null | undefined): OpenPGP.PrivateKey | null {
  if (unlocked === null || fingerprint == null) return null;
  return unlocked.getFingerprint().toUpperCase() === fingerprint.toUpperCase() ? unlocked : null;
}

/** Forget the unlocked key. */
export function lockKey(): void {
  unlocked = null;
  changed();
}

export class WrongPassphrase extends Error {
  constructor() {
    super("wrong passphrase");
  }
}

/** Unlock the person's locked key with their passphrase, for this tab. */
export async function unlockKey(lockedKey: string, passphrase: string): Promise<void> {
  const pgp = await openpgp();
  const key = await pgp.readPrivateKey({ armoredKey: lockedKey });
  try {
    unlocked = await pgp.decryptKey({ privateKey: key, passphrase });
  } catch {
    throw new WrongPassphrase();
  }
  changed();
}

/** The address in a user ID, lower case. */
export function uidEmail(uid: string): string | null {
  const m = /<([^<>\s]+@[^<>\s]+)>\s*$/.exec(uid) ?? /^\s*([^<>\s]+@[^<>\s]+)\s*$/.exec(uid);
  return m?.[1] !== undefined ? m[1].toLowerCase() : null;
}

export interface KeyPair {
  publicKey: string;
  lockedKey: string;
  fingerprint: string;
}

/** The shortest passphrase the page takes. */
export const MIN_PASSPHRASE = 10;

/** A new key for `email`: Ed25519 to sign, Curve25519 to encrypt, the v4
 *  format every mail program reads, locked with `passphrase`. The unlocked
 *  key stays in this tab. */
export async function makeKey(name: string, email: string, passphrase: string): Promise<KeyPair> {
  const pgp = await openpgp();
  const userIDs = name.trim() !== "" ? [{ name: name.trim(), email }] : [{ email }];
  const { privateKey, publicKey } = await pgp.generateKey({
    type: "ecc",
    curve: "curve25519Legacy",
    userIDs,
    passphrase,
    format: "armored",
  });
  const key = await pgp.readPrivateKey({ armoredKey: privateKey });
  unlocked = await pgp.decryptKey({ privateKey: key, passphrase });
  changed();
  return { publicKey, lockedKey: privateKey, fingerprint: key.getFingerprint().toUpperCase() };
}

export class ImportProblem extends Error {
  constructor(
    readonly kind: "notPrivate" | "wrongPassphrase" | "otherAddress" | "noEncryption",
    readonly detail = "",
  ) {
    super(kind);
  }
}

/** A private key the person already has, armoured: unlocked with
 *  `current` when it is locked, then locked with `passphrase` (the same
 *  one when that is empty). It must have their address and a key mail can
 *  be encrypted to. */
export async function importKey(
  armored: string,
  current: string,
  passphrase: string,
  email: string,
): Promise<KeyPair> {
  const pgp = await openpgp();
  let key: OpenPGP.PrivateKey;
  try {
    key = await pgp.readPrivateKey({ armoredKey: armored.trim() });
  } catch {
    throw new ImportProblem("notPrivate");
  }
  const emails = key.getUserIDs().map(uidEmail);
  if (!emails.includes(email.toLowerCase())) {
    throw new ImportProblem("otherAddress", emails.filter((e) => e !== null).join(", "));
  }
  try {
    await key.getEncryptionKey();
  } catch {
    throw new ImportProblem("noEncryption");
  }
  let open = key;
  if (!key.isDecrypted()) {
    try {
      open = await pgp.decryptKey({ privateKey: key, passphrase: current });
    } catch {
      throw new ImportProblem("wrongPassphrase");
    }
  }
  const lockWith = passphrase !== "" ? passphrase : current;
  const locked = await pgp.encryptKey({ privateKey: open, passphrase: lockWith });
  unlocked = open;
  changed();
  return {
    publicKey: open.toPublic().armor(),
    lockedKey: locked.armor(),
    fingerprint: open.getFingerprint().toUpperCase(),
  };
}

/** What a public key says about itself, and whether mail can be encrypted
 *  to it today. */
export interface KeyFacts {
  fingerprint: string;
  emails: string[];
  canEncrypt: boolean;
}

export async function keyFacts(armored: string): Promise<KeyFacts | null> {
  const pgp = await openpgp();
  try {
    const key = await pgp.readKey({ armoredKey: armored });
    let canEncrypt = true;
    try {
      await key.getEncryptionKey();
    } catch {
      canEncrypt = false;
    }
    return {
      fingerprint: key.getFingerprint().toUpperCase(),
      emails: key.getUserIDs().flatMap((u) => uidEmail(u) ?? []),
      canEncrypt,
    };
  } catch {
    return null;
  }
}

/** A fingerprint in groups of four, as people read them out. */
export function grouped(fingerprint: string): string {
  return fingerprint.toUpperCase().replace(/(.{4})(?=.)/g, "$1 ");
}

/* ── Writing ───────────────────────────────────────────────────────────── */

const encoder = new TextEncoder();

/** Quoted-printable (RFC 2045 6.7) of UTF-8 text, lines ending in CRLF: a
 *  signed part must survive every server on the way unchanged, so it is
 *  7-bit, no line is long and no line ends in a space. */
export function quotedPrintable(text: string): string {
  const lines = text.replace(/\r\n?/g, "\n").split("\n");
  return lines
    .map((line) => {
      const bytes = encoder.encode(line);
      let out = "";
      let width = 0;
      const put = (piece: string) => {
        if (width + piece.length > 75) {
          out += "=\r\n";
          width = 0;
        }
        out += piece;
        width += piece.length;
      };
      bytes.forEach((b, i) => {
        const last = i === bytes.length - 1;
        const plain =
          (b >= 33 && b <= 126 && b !== 61) || ((b === 32 || b === 9) && !last);
        // "From " at the start of a line is changed by mbox; a dot alone
        // by SMTP. Both are encoded.
        const risky = width === 0 && (b === 70 || b === 46);
        put(plain && !risky ? String.fromCharCode(b) : `=${b.toString(16).toUpperCase().padStart(2, "0")}`);
      });
      return out;
    })
    .join("\r\n");
}

/** The MIME part a message's text goes in, CRLF throughout. */
export function textPart(text: string): string {
  return (
    "Content-Type: text/plain; charset=utf-8\r\n" +
    "Content-Transfer-Encoding: quoted-printable\r\n" +
    "\r\n" +
    quotedPrintable(text)
  );
}

const MICALG: Record<string, string> = {
  sha224: "pgp-sha224",
  sha256: "pgp-sha256",
  sha384: "pgp-sha384",
  sha512: "pgp-sha512",
};

/** A file that travels inside the encrypted or signed part. */
export interface OutgoingFile {
  name: string;
  type: string;
  bytes: Uint8Array;
}

function base64Lines(bytes: Uint8Array): string {
  let binary = "";
  for (let i = 0; i < bytes.length; i += 0x8000) binary += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
  return (btoa(binary).match(/.{1,76}/g) ?? []).join("\r\n");
}

/** A header parameter: quoted when it is plain ASCII, RFC 2231 otherwise. */
function parameter(key: string, value: string): string {
  return /^[\x20-\x7e]*$/.test(value) && !/["\\]/.test(value)
    ? `${key}="${value}"`
    : `${key}*=utf-8''${encodeURIComponent(value).replace(/['()*]/g, (c) => `%${c.charCodeAt(0).toString(16).toUpperCase()}`)}`;
}

/** The part that is signed or encrypted: the text alone, or the text and
 *  the files as multipart/mixed (RFC 3156 puts them inside, so nothing of
 *  them shows on the way). */
export function bodyPart(text: string, files: readonly OutgoingFile[]): string {
  if (files.length === 0) return textPart(text);
  const random = crypto.getRandomValues(new Uint8Array(12));
  const boundary = `losos-${Array.from(random, (b) => b.toString(16).padStart(2, "0")).join("")}`;
  const parts = [textPart(text)];
  for (const file of files) {
    const type = /^[\w.+-]+\/[\w.+-]+$/.test(file.type) ? file.type : "application/octet-stream";
    parts.push(
      `Content-Type: ${type}; ${parameter("name", file.name)}\r\n` +
        `Content-Disposition: attachment; ${parameter("filename", file.name)}\r\n` +
        "Content-Transfer-Encoding: base64\r\n" +
        "\r\n" +
        base64Lines(file.bytes),
    );
  }
  return (
    `Content-Type: multipart/mixed; boundary="${boundary}"\r\n` +
    "\r\n" +
    parts.map((part) => `--${boundary}\r\n${part}\r\n`).join("") +
    `--${boundary}--\r\n`
  );
}

export interface Outgoing {
  text: string;
  /** Files, carried inside the PGP/MIME part; inline PGP cannot carry them. */
  files?: readonly OutgoingFile[];
  /** Recipients' public keys, armoured, and the sender's own, so the
   *  message in Sent can be read again. */
  encryptTo: string[];
  /** The unlocked key to sign with, or null not to sign. */
  signWith: OpenPGP.PrivateKey | null;
  inline: boolean;
}

/** What goes to POST /api/mail/send in place of the plain text. */
export type Made = { text: string; pgp?: undefined } | { text: ""; pgp: import("@/lib/api").MailPgpBody };

export async function seal(out: Outgoing): Promise<Made> {
  const pgp = await openpgp();
  const signingKeys = out.signWith ?? undefined;
  const files = out.files ?? [];
  const inline = out.inline && files.length === 0;
  if (out.encryptTo.length > 0) {
    const encryptionKeys = await Promise.all(out.encryptTo.map((armoredKey) => pgp.readKey({ armoredKey })));
    const message = inline
      ? await pgp.createMessage({ text: out.text })
      : await pgp.createMessage({ binary: encoder.encode(bodyPart(out.text, files)) });
    const armored = await pgp.encrypt({ message, encryptionKeys, signingKeys, format: "armored" });
    return inline ? { text: armored } : { text: "", pgp: { mime: "encrypted", armored } };
  }
  if (out.signWith !== null) {
    if (inline) {
      const message = await pgp.createCleartextMessage({ text: out.text });
      return { text: await pgp.sign({ message, signingKeys: out.signWith, format: "armored" }) };
    }
    const part = bodyPart(out.text, files);
    const message = await pgp.createMessage({ binary: encoder.encode(part) });
    const signature = await pgp.sign({ message, signingKeys: out.signWith, detached: true, format: "armored" });
    const parsed = await pgp.readSignature({ armoredSignature: signature });
    const hash = pgp.enums.read(pgp.enums.hash, parsed.packets[0]?.hashAlgorithm ?? pgp.enums.hash.sha256);
    return { text: "", pgp: { mime: "signed", part, signature, micalg: MICALG[hash] ?? "pgp-sha256" } };
  }
  return { text: out.text };
}

/* ── Reading ───────────────────────────────────────────────────────────── */

export interface Opened {
  /** How the message came. */
  kind: "encrypted" | "signed" | "plain";
  /** Inline PGP rather than PGP/MIME. */
  inline: boolean;
  text: string;
  html: string | null;
  /** A subject the sender protected inside the encryption, if any. */
  subject: string | null;
  attachments: { name: string; type: string; bytes: Uint8Array }[];
  signature: Check;
}

export type Check =
  | { state: "none" }
  | { state: "good"; fingerprint: string; emails: string[] }
  | { state: "bad" }
  | { state: "unknown"; keyId: string };

export class CannotOpen extends Error {
  constructor(readonly kind: "locked" | "notForMe" | "broken") {
    super(kind);
  }
}

/* Bytes as a "binary string", one char per byte, so offsets found in text
 * are offsets in the bytes. */
function binary(bytes: Uint8Array): string {
  let out = "";
  for (let i = 0; i < bytes.length; i += 0x8000) {
    out += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
  }
  return out;
}

function bytesOf(text: string): Uint8Array {
  const out = new Uint8Array(text.length);
  for (let i = 0; i < text.length; i++) out[i] = text.charCodeAt(i) & 0xff;
  return out;
}

interface Entity {
  type: string;
  params: Record<string, string>;
  head: string;
  body: string;
}

/* Header block and body of a MIME entity, with its Content-Type. */
export function entity(raw: string): Entity {
  const crlf = raw.indexOf("\r\n\r\n");
  const lf = raw.indexOf("\n\n");
  const [at, gap] = crlf !== -1 && (lf === -1 || crlf <= lf) ? [crlf, 4] : lf !== -1 ? [lf, 2] : [raw.length, 0];
  const head = raw.slice(0, at);
  const body = raw.slice(at + gap);
  const unfolded = head.replace(/\r?\n[ \t]+/g, " ");
  const line = unfolded.split(/\r?\n/).find((l) => /^content-type\s*:/i.test(l)) ?? "content-type: text/plain";
  const value = line.slice(line.indexOf(":") + 1);
  const [type = "text/plain", ...rest] = value.split(";");
  const params: Record<string, string> = {};
  for (const p of rest) {
    const eq = p.indexOf("=");
    if (eq === -1) continue;
    params[p.slice(0, eq).trim().toLowerCase()] = p.slice(eq + 1).trim().replace(/^"(.*)"$/, "$1");
  }
  return { type: type.trim().toLowerCase(), params, head, body };
}

/* The parts of a multipart body, byte for byte: what lies between a
 * delimiter line and the CRLF before the next one (RFC 2046 5.1.1). */
export function parts(body: string, boundary: string): string[] {
  const delimiter = `--${boundary}`;
  const out: string[] = [];
  let start = -1;
  let pos = 0;
  while (pos <= body.length) {
    const at = body.indexOf(delimiter, pos);
    if (at === -1) break;
    const lineStart = at === 0 || body[at - 1] === "\n";
    if (!lineStart) {
      pos = at + delimiter.length;
      continue;
    }
    if (start !== -1) {
      const end = body[at - 2] === "\r" ? at - 2 : at - 1;
      out.push(body.slice(start, end));
    }
    if (body.startsWith("--", at + delimiter.length)) break;
    const eol = body.indexOf("\n", at);
    if (eol === -1) break;
    start = eol + 1;
    pos = start;
  }
  return out;
}

async function check(
  pgp: Lib,
  results: OpenPGP.DecryptMessageResult["signatures"],
  keys: OpenPGP.Key[],
): Promise<Check> {
  const first = results[0];
  if (first === undefined) return { state: "none" };
  const signer = keys.find((k) => k.getKeys(first.keyID).length > 0);
  if (signer === undefined) return { state: "unknown", keyId: first.keyID.toHex().toUpperCase() };
  try {
    await first.verified;
    void pgp;
    return {
      state: "good",
      fingerprint: signer.getFingerprint().toUpperCase(),
      emails: signer.getUserIDs().flatMap((u) => uidEmail(u) ?? []),
    };
  } catch {
    return { state: "bad" };
  }
}

async function readEntity(raw: string): Promise<Omit<Opened, "kind" | "inline" | "signature">> {
  const { default: PostalMime } = await import("postal-mime");
  const mail = await PostalMime.parse(bytesOf(raw));
  const protectedSubject =
    /protected-headers\s*=\s*"?v1/i.test(entity(raw).head) && mail.subject !== undefined ? mail.subject : null;
  return {
    text: mail.text ?? "",
    html: mail.html ?? null,
    subject: protectedSubject,
    attachments: mail.attachments.map((a) => ({
      name: a.filename ?? "attachment",
      type: a.mimeType,
      bytes: typeof a.content === "string" ? encoder.encode(a.content) : new Uint8Array(a.content),
    })),
  };
}

/** Open a message: decrypt it with `key` and check its signature against
 *  `senderKeys` (the From address's keys, armoured). */
export async function openMail(
  raw: Uint8Array,
  key: OpenPGP.PrivateKey | null,
  senderKeys: string[],
): Promise<Opened> {
  const pgp = await openpgp();
  const verificationKeys = (
    await Promise.all(senderKeys.map((armoredKey) => pgp.readKey({ armoredKey }).catch(() => null)))
  ).filter((k): k is OpenPGP.Key => k !== null);

  const decrypt = async (armoredMessage: string, format: "binary" | "utf8") => {
    if (key === null) throw new CannotOpen("locked");
    let message: OpenPGP.Message<string>;
    try {
      message = await pgp.readMessage({ armoredMessage });
    } catch {
      throw new CannotOpen("broken");
    }
    const mine = new Set(key.getKeys().map((k) => k.getKeyID().toHex()));
    if (!message.getEncryptionKeyIDs().some((id) => mine.has(id.toHex()) || /^0+$/.test(id.toHex()))) {
      throw new CannotOpen("notForMe");
    }
    try {
      return format === "binary"
        ? await pgp.decrypt({ message, decryptionKeys: key, verificationKeys, format: "binary" })
        : await pgp.decrypt({ message, decryptionKeys: key, verificationKeys });
    } catch {
      throw new CannotOpen("broken");
    }
  };

  const verifyPart = async (signed: string, armoredSignature: string): Promise<Check> => {
    try {
      const signature = await pgp.readSignature({ armoredSignature });
      const message = await pgp.createMessage({ binary: bytesOf(signed) });
      const result = await pgp.verify({ message, signature, verificationKeys, format: "binary" });
      return await check(pgp, result.signatures, verificationKeys);
    } catch {
      return { state: "bad" };
    }
  };

  // A multipart/signed: the signature covers the first part as it is.
  const signedEntity = async (top: Entity): Promise<{ content: string; signature: Check } | null> => {
    if (top.type !== "multipart/signed" || !/pgp-signature/i.test(top.params.protocol ?? "")) return null;
    const [content, sig] = parts(top.body, top.params.boundary ?? "");
    if (content === undefined || sig === undefined) return null;
    return { content, signature: await verifyPart(content, entity(sig).body.trim()) };
  };

  const text = binary(raw);
  const top = entity(text);

  if (top.type === "multipart/encrypted" && /pgp-encrypted/i.test(top.params.protocol ?? "")) {
    const armored = parts(top.body, top.params.boundary ?? "")
      .map((p) => entity(p).body)
      .find((b) => b.includes("-----BEGIN PGP MESSAGE-----"));
    if (armored === undefined) throw new CannotOpen("broken");
    const result = await decrypt(armored, "binary");
    let inner = binary(result.data);
    let signature = await check(pgp, result.signatures, verificationKeys);
    // Signed, then encrypted as a whole (RFC 3156 6.1).
    const nested = await signedEntity(entity(inner));
    if (nested !== null) {
      inner = nested.content;
      signature = nested.signature;
    }
    return { kind: "encrypted", inline: false, signature, ...(await readEntity(inner)) };
  }

  const signed = await signedEntity(top);
  if (signed !== null) {
    return { kind: "signed", inline: false, signature: signed.signature, ...(await readEntity(signed.content)) };
  }

  // Inline: an armoured block in the text.
  const plain = await readEntity(text);
  const block = /-----BEGIN PGP MESSAGE-----[\s\S]*?-----END PGP MESSAGE-----/.exec(plain.text)?.[0];
  if (block !== undefined) {
    const result = await decrypt(block, "utf8");
    return {
      ...plain,
      kind: "encrypted",
      inline: true,
      text: plain.text.replace(block, String(result.data)),
      html: null,
      signature: await check(pgp, result.signatures, verificationKeys),
    };
  }
  const cleartext = /-----BEGIN PGP SIGNED MESSAGE-----[\s\S]*?-----END PGP SIGNATURE-----/.exec(plain.text)?.[0];
  if (cleartext !== undefined) {
    try {
      const message = await pgp.readCleartextMessage({ cleartextMessage: cleartext });
      const result = await pgp.verify({ message, verificationKeys });
      return {
        ...plain,
        kind: "signed",
        inline: true,
        text: plain.text.replace(cleartext, message.getText()),
        html: null,
        signature: await check(pgp, result.signatures, verificationKeys),
      };
    } catch {
      return { ...plain, kind: "signed", inline: true, signature: { state: "bad" } };
    }
  }
  return { ...plain, kind: "plain", inline: false, signature: { state: "none" } };
}
