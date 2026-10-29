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

/** One payment into the escrow program (`edge/contracts/solana`): the order
 *  is held in a deposit of its own until the edge releases it to the seller
 *  and treasury, refunds it, or splits it. `nonce` names the deposit; a
 *  fresh random one per payment. */
export interface EscrowPay {
  payer: string;
  program: string;
  mint: string;
  treasury: string;
  seller: string;
  /** The invoice's `order_id_hash`, 0x and 64 hex digits. */
  orderIdHash: string;
  gross: bigint;
  fee: bigint;
  /** Unix seconds: after it the payer may take an unreleased deposit back. */
  deadline: bigint;
  /** The two boxes' escrow keys from the invoice: with the edge's, the
   *  program moves the deposit only when two of the three agree. */
  buyerBox: string;
  sellerBox: string;
  nonce: Uint8Array;
  recentBlockhash: string;
}

const enc = (s: string) => new TextEncoder().encode(s);

/** The program's config for `treasury`: who it pays the fee and who releases. */
export function escrowConfigAddress(program: string, treasury: string, mint: string): string {
  return findProgramAddress([enc("config"), pubkey(treasury), pubkey(mint)], program);
}

/** The deposit a payment with `nonce` makes. */
export function escrowDepositAddress(program: string, config: string, nonce: Uint8Array): string {
  return findProgramAddress([enc("deposit"), pubkey(config), nonce], program);
}

/** The token account the deposit holds the USDC in. */
export function escrowVaultAddress(program: string, deposit: string): string {
  return findProgramAddress([enc("vault"), pubkey(deposit)], program);
}

function hex32(raw: string): Uint8Array {
  const h = raw.startsWith("0x") ? raw.slice(2) : raw;
  if (!/^[0-9a-fA-F]{64}$/.test(h)) throw new Error("order id hash");
  return Uint8Array.from(h.match(/../g)!.map((b) => parseInt(b, 16)));
}

/** The escrow's `pay`: the program opens the deposit and its vault, paid for
 *  by the payer and given back when the deposit closes, and moves the gross
 *  in. The seller's and treasury's token accounts are the edge's to open. */
export function escrowPayMessage(p: EscrowPay): Uint8Array {
  if (p.gross <= 0n || p.fee < 0n || p.fee > p.gross) throw new Error("amount");
  if (p.nonce.length !== 32) throw new Error("nonce");
  const config = escrowConfigAddress(p.program, p.treasury, p.mint);
  const deposit = escrowDepositAddress(p.program, config, p.nonce);
  const data = new Uint8Array(153);
  const view = new DataView(data.buffer);
  data[0] = 1;
  data.set(p.nonce, 1);
  data.set(hex32(p.orderIdHash), 33);
  view.setBigUint64(65, p.gross, true);
  view.setBigUint64(73, p.fee, true);
  view.setBigInt64(81, p.deadline, true);
  data.set(pubkey(p.buyerBox), 89);
  data.set(pubkey(p.sellerBox), 121);
  const ix: Instruction = {
    program: p.program,
    accounts: [
      { key: p.payer, signer: true, writable: true },
      w(associatedTokenAddress(p.payer, p.mint)),
      r(config),
      w(deposit),
      w(escrowVaultAddress(p.program, deposit)),
      r(p.seller),
      r(p.mint),
      r(TOKEN_PROGRAM),
      r(SYSTEM_PROGRAM),
    ],
    data,
  };
  return compile(p.payer, [ix], p.recentBlockhash);
}

// ── Prepaid balances and taps (pay as you go for AI models) ──────────────
// The program's tags 6, 7, 10 and 11 (edge/contracts/solana/README.md): a
// buyer tops up a balance of their own, opens one tap per lending box
// against it, and can take what is left out after a 48-hour notice. The edge
// bills the taps and closes them; the wallet never has to.

/** `["balance", config, buyer]`: the buyer's prepaid balance. */
export function escrowBalanceAddress(program: string, config: string, buyer: string): string {
  return findProgramAddress([enc("balance"), pubkey(config), pubkey(buyer)], program);
}

/** `["tap", config, buyer, tap id]`: the buyer is in the seeds, so the same
 *  id from another wallet is another tap. */
export function escrowTapAddress(program: string, config: string, buyer: string, tapId: Uint8Array): string {
  if (tapId.length !== 32) throw new Error("tap id");
  return findProgramAddress([enc("tap"), pubkey(config), pubkey(buyer), tapId], program);
}

/** Where the balance's USDC is: the token account `["vault", balance]`. */
export function escrowBalanceVault(program: string, treasury: string, mint: string, buyer: string): string {
  const config = escrowConfigAddress(program, treasury, mint);
  return escrowVaultAddress(program, escrowBalanceAddress(program, config, buyer));
}

/** One tap of a quote: the 32-byte id the edge picked (`0x` hex), the
 *  box's payout address, and base units an hour. */
export interface EscrowTap {
  key: string;
  seller: string;
  rate: bigint;
}

export interface EscrowTaps {
  payer: string;
  program: string;
  mint: string;
  treasury: string;
  /** Moved into the balance first, in the same transaction; 0 for none. */
  topUp: bigint;
  taps: EscrowTap[];
  feeBps: number;
  recentBlockhash: string;
}

/** The most taps one transaction carries with a top-up, under Solana's
 *  1232-byte packet. A quote of more is sent in several. */
export const TAPS_PER_TRANSACTION = 6;

function u64le(tag: number, value: bigint): Uint8Array {
  const data = new Uint8Array(9);
  data[0] = tag;
  new DataView(data.buffer).setBigUint64(1, value, true);
  return data;
}

/** `top_up` (if any) and an `open_tap` for each tap, in one transaction. */
export function escrowTapsMessage(p: EscrowTaps): Uint8Array {
  if (p.topUp < 0n) throw new Error("amount");
  if (p.taps.length === 0 || p.taps.length > TAPS_PER_TRANSACTION) throw new Error("taps");
  if (!Number.isInteger(p.feeBps) || p.feeBps < 0 || p.feeBps > 0xffff) throw new Error("fee");
  const config = escrowConfigAddress(p.program, p.treasury, p.mint);
  const balance = escrowBalanceAddress(p.program, config, p.payer);
  const buyer: Meta = { key: p.payer, signer: true, writable: true };
  const ixs: Instruction[] = [];
  if (p.topUp > 0n) {
    ixs.push({
      program: p.program,
      accounts: [
        buyer,
        w(associatedTokenAddress(p.payer, p.mint)),
        r(config),
        w(balance),
        w(escrowVaultAddress(p.program, balance)),
        r(p.mint),
        r(TOKEN_PROGRAM),
        r(SYSTEM_PROGRAM),
      ],
      data: u64le(6, p.topUp),
    });
  }
  for (const tap of p.taps) {
    if (tap.rate <= 0n) throw new Error("rate");
    const id = hex32(tap.key);
    const data = new Uint8Array(43);
    data[0] = 7;
    data.set(id, 1);
    const view = new DataView(data.buffer);
    view.setBigUint64(33, tap.rate, true);
    view.setUint16(41, p.feeBps, true);
    ixs.push({
      program: p.program,
      accounts: [buyer, r(config), r(balance), w(escrowTapAddress(p.program, config, p.payer, id)), r(tap.seller), r(SYSTEM_PROGRAM)],
      data,
    });
  }
  return compile(p.payer, ixs, p.recentBlockhash);
}

export interface EscrowWithdrawal {
  payer: string;
  program: string;
  mint: string;
  treasury: string;
  recentBlockhash: string;
}

/** `ask_withdrawal(amount)`: take `amount` out after the notice. */
export function escrowAskMessage(p: EscrowWithdrawal & { amount: bigint }): Uint8Array {
  if (p.amount <= 0n) throw new Error("amount");
  const config = escrowConfigAddress(p.program, p.treasury, p.mint);
  const ix: Instruction = {
    program: p.program,
    accounts: [
      { key: p.payer, signer: true, writable: false },
      r(config),
      w(escrowBalanceAddress(p.program, config, p.payer)),
    ],
    data: u64le(10, p.amount),
  };
  return compile(p.payer, [ix], p.recentBlockhash);
}

/** `withdraw`: after the notice, the ask (or what is left) back to the
 *  buyer's own token account, which is opened first if it is gone. */
export function escrowWithdrawMessage(p: EscrowWithdrawal): Uint8Array {
  const config = escrowConfigAddress(p.program, p.treasury, p.mint);
  const balance = escrowBalanceAddress(p.program, config, p.payer);
  const ix: Instruction = {
    program: p.program,
    accounts: [
      { key: p.payer, signer: true, writable: false },
      r(config),
      w(balance),
      w(escrowVaultAddress(p.program, balance)),
      w(associatedTokenAddress(p.payer, p.mint)),
      r(p.mint),
      r(TOKEN_PROGRAM),
    ],
    data: Uint8Array.of(11),
  };
  return compile(p.payer, [createAta(p.payer, p.payer, p.mint), ix], p.recentBlockhash);
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
