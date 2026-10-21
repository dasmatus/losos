// A BIP-39 hot wallet on the path MetaMask, Rabby, Rainbow and Trust Wallet
// use (m/44'/60'/0'/0/0), and on Solana the path Phantom, Solflare and
// Backpack use (see solana.ts). The phrase is the key. It is encrypted in
// this browser and never sent to the box. The same EVM address works on
// every EVM chain the edge settles on.
import { english, generateMnemonic, mnemonicToAccount, type HDAccount } from "viem/accounts";
import { solanaKeyFromPhrase } from "./solana";

const DB = "losos-wallet";
const STORE = "key";

export function createPhrase(): string {
  return generateMnemonic(english);
}

export function accountFromPhrase(phrase: string): HDAccount {
  return mnemonicToAccount(normalizePhrase(phrase));
}

export function normalizePhrase(phrase: string): string {
  return phrase.trim().toLowerCase().split(/\s+/).join(" ");
}

/** The message a payout address signs. `chain` is the EVM chain id, or the
 *  Solana network key; base58 is case-sensitive, so only a 0x address is
 *  lowercased. */
export function payoutMessage(chain: number | string, address: string): string {
  const shown = address.startsWith("0x") ? address.toLowerCase() : address;
  return `LosOS market payout v1\nchain: ${chain}\naddress: ${shown}\n`;
}

/** What the wallet signs to let the edge publish its address at the box's
 *  names (edge/registrar/src/domains.rs, `wallet_message`). */
export function walletMessage(zone: string, box: string, address: string): string {
  return `LosOS box name v1\nzone: ${zone}\nbox: ${box}\naddress: ${address.toLowerCase()}\n`;
}

interface Sealed {
  v: 1;
  salt: string;
  iv: string;
  ct: string;
  /** The two public addresses, so balances and names show before the
   *  password is typed. Absent on a wallet saved before they were kept. */
  evm?: string;
  sol?: string;
  /** The phrase was made on this box rather than imported. */
  made?: boolean;
}

/** The saved wallet's public addresses, or null when none is saved. */
export async function savedAddresses(): Promise<{ evm?: string; sol?: string; made: boolean } | null> {
  const sealed = await idbGet();
  return sealed === null ? null : { evm: sealed.evm, sol: sealed.sol, made: sealed.made === true };
}

export async function sealPhrase(password: string, phrase: string, made = false): Promise<void> {
  if (password.length < 8) throw new Error("password");
  const salt = crypto.getRandomValues(new Uint8Array(16));
  const iv = crypto.getRandomValues(new Uint8Array(12));
  const key = await derive(password, salt);
  const ct = new Uint8Array(
    await crypto.subtle.encrypt(
      { name: "AES-GCM", iv: iv as BufferSource },
      key,
      new TextEncoder().encode(normalizePhrase(phrase)),
    ),
  );
  const words = normalizePhrase(phrase);
  await idbPut({
    v: 1,
    salt: b64(salt),
    iv: b64(iv),
    ct: b64(ct),
    evm: mnemonicToAccount(words).address,
    sol: solanaKeyFromPhrase(words).address,
    made,
  });
}

export async function openPhrase(password: string): Promise<string> {
  const sealed = await idbGet();
  if (sealed === null) throw new Error("empty");
  const key = await derive(password, b64dec(sealed.salt));
  const plain = await crypto.subtle.decrypt(
    { name: "AES-GCM", iv: b64dec(sealed.iv) as BufferSource },
    key,
    b64dec(sealed.ct) as BufferSource,
  );
  return new TextDecoder().decode(plain);
}

export async function hasPhrase(): Promise<boolean> {
  return (await idbGet()) !== null;
}

export async function forgetPhrase(): Promise<void> {
  const db = await openDb();
  await new Promise<void>((resolve, reject) => {
    const tx = db.transaction(STORE, "readwrite");
    tx.objectStore(STORE).delete("phrase");
    tx.oncomplete = () => resolve();
    tx.onerror = () => reject(tx.error);
  });
  db.close();
}

async function derive(password: string, salt: Uint8Array): Promise<CryptoKey> {
  const base = await crypto.subtle.importKey(
    "raw",
    new TextEncoder().encode(password),
    "PBKDF2",
    false,
    ["deriveKey"],
  );
  return crypto.subtle.deriveKey(
    { name: "PBKDF2", salt: salt as BufferSource, iterations: 210_000, hash: "SHA-256" },
    base,
    { name: "AES-GCM", length: 256 },
    false,
    ["encrypt", "decrypt"],
  );
}

function b64(bytes: Uint8Array): string {
  let s = "";
  for (const b of bytes) s += String.fromCharCode(b);
  return btoa(s);
}

function b64dec(s: string): Uint8Array {
  const raw = atob(s);
  const out = new Uint8Array(raw.length);
  for (let i = 0; i < raw.length; i++) out[i] = raw.charCodeAt(i);
  return out;
}

function openDb(): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    const req = indexedDB.open(DB, 1);
    req.onupgradeneeded = () => {
      req.result.createObjectStore(STORE);
    };
    req.onsuccess = () => resolve(req.result);
    req.onerror = () => reject(req.error);
  });
}

function idbPut(value: Sealed): Promise<void> {
  return openDb().then(
    (db) =>
      new Promise((resolve, reject) => {
        const tx = db.transaction(STORE, "readwrite");
        tx.objectStore(STORE).put(value, "phrase");
        tx.oncomplete = () => {
          db.close();
          resolve();
        };
        tx.onerror = () => reject(tx.error);
      }),
  );
}

function idbGet(): Promise<Sealed | null> {
  return openDb().then(
    (db) =>
      new Promise((resolve, reject) => {
        const tx = db.transaction(STORE, "readonly");
        const req = tx.objectStore(STORE).get("phrase");
        req.onsuccess = () => {
          db.close();
          resolve((req.result as Sealed | undefined) ?? null);
        };
        req.onerror = () => reject(req.error);
      }),
  );
}
