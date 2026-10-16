// A Solana USDC payer built from the same BIP-39 phrase as the EVM hot
// wallet, with no Solana SDK: only the noble/scure primitives viem already
// brings in. It derives the key Phantom, Solflare and Backpack derive, builds
// one legacy transaction (create both receiving token accounts if missing,
// two TransferChecked, a memo naming the order) and signs it. Sending and
// confirming are the caller's, over plain JSON-RPC.
import { ed25519 } from "@noble/curves/ed25519.js";
import { hmac } from "@noble/hashes/hmac.js";
import { sha256, sha512 } from "@noble/hashes/sha2.js";
import { base58, base64 } from "@scure/base";
import { mnemonicToSeedSync, validateMnemonic } from "@scure/bip39";
import { wordlist } from "@scure/bip39/wordlists/english.js";

export const TOKEN_PROGRAM = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";
export const ATA_PROGRAM = "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL";
export const MEMO_PROGRAM = "MemoSq4gqABAXKb96qnH8TysNcWxMyWCqXgDLGmfcHr";
export const SYSTEM_PROGRAM = "11111111111111111111111111111111";
export const SOLANA_PATH = "m/44'/501'/0'/0'"; // Phantom / Solflare / Backpack default

// USDC has 6 decimals on every cluster; TransferChecked makes the token
// program refuse the transfer if the mint says otherwise.
const USDC_DECIMALS = 6;

export interface SolanaKey {
  address: string;
  secret: Uint8Array;
}

export function solanaKeyFromPhrase(phrase: string): SolanaKey {
  const words = phrase.trim().toLowerCase().split(/\s+/).join(" ");
  // A typo still derives a valid key, just one nobody funded.
  if (!validateMnemonic(words, wordlist)) throw new Error("phrase");
  const secret = slip10(mnemonicToSeedSync(words), SOLANA_PATH);
  return { address: base58.encode(ed25519.getPublicKey(secret)), secret };
}

// SLIP-10 for ed25519 has only hardened children, so every index gets the
// high bit whether or not the path marks it.
function slip10(seed: Uint8Array, path: string): Uint8Array {
  let I = hmac(sha512, new TextEncoder().encode("ed25519 seed"), seed);
  for (const part of path.split("/").slice(1)) {
    const index = (Number.parseInt(part.replace(/'/g, ""), 10) | 0x80000000) >>> 0;
    const data = new Uint8Array(37);
    data.set(I.slice(0, 32), 1);
    new DataView(data.buffer).setUint32(33, index, false);
    I = hmac(sha512, I.slice(32), data);
  }
  return I.slice(0, 32);
}

export function isSolanaAddress(raw: string): boolean {
  try {
    return base58.decode(raw.trim()).length === 32;
  } catch {
    return false;
  }
}

export function signMessage(key: SolanaKey, message: string): string {
  return base58.encode(ed25519.sign(new TextEncoder().encode(message), key.secret));
}

export function formatBase58(bytes: Uint8Array): string {
  return base58.encode(bytes);
}

function pubkey(address: string): Uint8Array {
  const bytes = base58.decode(address);
  if (bytes.length !== 32) throw new Error(`address: ${address}`);
  return bytes;
}

// The runtime's own test (curve25519-dalek decompress) accepts what ZIP-215
// accepts, so a hash RFC 8032 would refuse still counts as on the curve.
function onCurve(bytes: Uint8Array): boolean {
  try {
    ed25519.Point.fromBytes(bytes, true);
    return true;
  } catch {
    return false;
  }
}

function findProgramAddress(seeds: Uint8Array[], program: string): string {
  const tail = concat([pubkey(program), new TextEncoder().encode("ProgramDerivedAddress")]);
  for (let bump = 255; bump >= 0; bump--) {
    const hash = sha256(concat([...seeds, Uint8Array.of(bump), tail]));
    if (!onCurve(hash)) return base58.encode(hash);
  }
  throw new Error("no program address");
}

export function associatedTokenAddress(owner: string, mint: string): string {
  return findProgramAddress([pubkey(owner), pubkey(TOKEN_PROGRAM), pubkey(mint)], ATA_PROGRAM);
}

export interface UsdcTransfer {
  payer: string;
  mint: string;
  seller: string;
  treasury: string;
  sellerAmount: bigint;
  feeAmount: bigint;
  memo: string;
  recentBlockhash: string;
}

interface Meta {
  key: string;
  signer: boolean;
  writable: boolean;
}

interface Instruction {
  program: string;
  accounts: Meta[];
  data: Uint8Array;
}

const w = (key: string): Meta => ({ key, signer: false, writable: true });
const r = (key: string): Meta => ({ key, signer: false, writable: false });

function createAta(payer: string, owner: string, mint: string): Instruction {
  return {
    program: ATA_PROGRAM,
    accounts: [
      { key: payer, signer: true, writable: true },
      w(associatedTokenAddress(owner, mint)),
      r(owner),
      r(mint),
      r(SYSTEM_PROGRAM),
      r(TOKEN_PROGRAM),
    ],
    // CreateIdempotent: succeeds when the account already exists.
    data: Uint8Array.of(1),
  };
}

function transferChecked(t: UsdcTransfer, to: string, amount: bigint): Instruction {
  const data = new Uint8Array(10);
  data[0] = 12;
  new DataView(data.buffer).setBigUint64(1, amount, true);
  data[9] = USDC_DECIMALS;
  return {
    program: TOKEN_PROGRAM,
    accounts: [
      w(associatedTokenAddress(t.payer, t.mint)),
      r(t.mint),
      w(associatedTokenAddress(to, t.mint)),
      { key: t.payer, signer: true, writable: false },
    ],
    data,
  };
}

export function usdcTransferMessage(t: UsdcTransfer): Uint8Array {
  if (t.sellerAmount <= 0n || t.feeAmount < 0n) throw new Error("amount");
  const fee = t.feeAmount > 0n;
  const ixs: Instruction[] = [createAta(t.payer, t.seller, t.mint)];
  if (fee) ixs.push(createAta(t.payer, t.treasury, t.mint));
  ixs.push(transferChecked(t, t.seller, t.sellerAmount));
  if (fee) ixs.push(transferChecked(t, t.treasury, t.feeAmount));
  // A plain send has no order to name.
  if (t.memo !== "") ixs.push({ program: MEMO_PROGRAM, accounts: [], data: new TextEncoder().encode(t.memo) });
  return compile(t.payer, ixs, t.recentBlockhash);
}

// The fee payer must be key 0, and the header only works if keys are grouped
// signer-writable, signer-readonly, writable, readonly.
function compile(payer: string, ixs: Instruction[], blockhash: string): Uint8Array {
  const metas = new Map<string, Meta>([[payer, { key: payer, signer: true, writable: true }]]);
  const merge = (m: Meta) => {
    const seen = metas.get(m.key);
    if (seen === undefined) metas.set(m.key, { ...m });
    else {
      seen.signer ||= m.signer;
      seen.writable ||= m.writable;
    }
  };
  for (const ix of ixs) {
    ix.accounts.forEach(merge);
    merge(r(ix.program));
  }
  const all = [...metas.values()];
  const group = (s: boolean, wr: boolean) => all.filter((m) => m.signer === s && m.writable === wr);
  const keys = [...group(true, true), ...group(true, false), ...group(false, true), ...group(false, false)];
  const index = new Map(keys.map((m, i) => [m.key, i]));
  if (keys.length > 255) throw new Error("too many accounts");

  const header = Uint8Array.of(
    keys.filter((m) => m.signer).length,
    group(true, false).length,
    group(false, false).length,
  );
  const body = ixs.map((ix) =>
    concat([
      Uint8Array.of(index.get(ix.program)!),
      shortvec(ix.accounts.length),
      Uint8Array.from(ix.accounts.map((m) => index.get(m.key)!)),
      shortvec(ix.data.length),
      ix.data,
    ]),
  );
  return concat([
    header,
    shortvec(keys.length),
    ...keys.map((m) => pubkey(m.key)),
    pubkey(blockhash),
    shortvec(ixs.length),
    ...body,
  ]);
}

export function signTransaction(key: SolanaKey, message: Uint8Array): { wire: string; signature: string } {
  const signature = ed25519.sign(message, key.secret);
  return {
    wire: base64.encode(concat([shortvec(1), signature, message])),
    signature: base58.encode(signature),
  };
}

// compact-u16: seven bits per byte, low first, high bit means more follows.
function shortvec(n: number): Uint8Array {
  const out: number[] = [];
  do {
    let byte = n & 0x7f;
    n >>= 7;
    if (n > 0) byte |= 0x80;
    out.push(byte);
  } while (n > 0);
  return Uint8Array.from(out);
}

function concat(parts: Uint8Array[]): Uint8Array {
  const out = new Uint8Array(parts.reduce((n, p) => n + p.length, 0));
  let at = 0;
  for (const p of parts) {
    out.set(p, at);
    at += p.length;
  }
  return out;
}
