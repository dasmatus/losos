// A BIP-39 hot wallet on the path MetaMask, Rabby, Rainbow and Trust Wallet
// use (m/44'/60'/0'/0/0). The phrase is the key. It is encrypted in this
// browser and never sent to the box. The same phrase works on every EVM
// chain; this market asks for Base, or Base Sepolia while the edge is in
// test mode.
import { english, generateMnemonic, mnemonicToAccount, type HDAccount } from "viem/accounts";

const DB = "losos-wallet";
const STORE = "key";
const PATH = "m/44'/60'/0'/0/0";

export const ANVIL_MNEMONIC =
  "test test test test test test test test test test test junk";

export function createPhrase(): string {
  return generateMnemonic(english);
}

export function accountFromPhrase(phrase: string): HDAccount {
  return mnemonicToAccount(normalizePhrase(phrase));
}

export function normalizePhrase(phrase: string): string {
  return phrase.trim().toLowerCase().split(/\s+/).join(" ");
}

export function payoutMessage(chainId: number, address: string): string {
  return `LosOS market payout v1\nchain: ${chainId}\naddress: ${address.toLowerCase()}\n`;
}

interface Sealed {
  v: 1;
  salt: string;
  iv: string;
  ct: string;
}

export async function sealPhrase(password: string, phrase: string): Promise<void> {
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
  await idbPut({ v: 1, salt: b64(salt), iv: b64(iv), ct: b64(ct) });
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

export const derivationPath = PATH;

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
