/* The Solana USDC payer in src/wallet/solana.ts, checked without a browser.
 *
 * The pure checks pin it to what the reference tools print: the address
 * solana-keygen derives from the junk mnemonic at m/44'/501'/0'/0', and the
 * token account spl-token names for that owner and Circle's devnet USDC mint.
 * The live check needs a local validator holding that mint (cloned with
 * `solana-test-validator --clone`) with USDC on the buyer, and runs only when
 * LOSOS_SOLANA_RPC is set: it pays a seller and a treasury in one transaction
 * and reads the result back the way the edge's verifier will. With
 * LOSOS_SOLANA_SAMPLE set, the jsonParsed transaction is written there. With
 * LOSOS_ESCROW_PROGRAM set to the program the validator loaded at
 * LosEscrow111…, it also pays into the escrow and reads the deposit back.
 *
 *   bun tests/solana.unit.mjs
 *   LOSOS_SOLANA_RPC=http://127.0.0.1:8899 bun tests/solana.unit.mjs
 */

import assert from './assert.mjs';
import { ed25519 } from '@noble/curves/ed25519.js';
import { base58, base64 } from '@scure/base';

const JUNK = 'test test test test test test test test test test test junk';
// solana-keygen pubkey "prompt://?full-path=m/44'/501'/0'/0'" with JUNK, no passphrase.
const JUNK_ADDRESS = 'oeYf6KAJkLYhBuR8CiGc6L4D4Xtfepr85fuDgA9kq96';
const USDC_DEVNET = '4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU';
// spl-token address --verbose --owner JUNK_ADDRESS --token USDC_DEVNET
const JUNK_USDC_ATA = 'Acp8hQfvQgptyqCreekNzaEHXMv6o3L1JiSwXmRHaaDW';

const sol = await import('../src/wallet/solana.ts');

const failures = [];
async function check(name, fn) {
  try {
    await fn();
    console.log(`  ok   ${name}`);
  } catch (e) {
    failures.push(`${name}: ${e.message}`);
    console.log(`  FAIL ${name}\n       ${e.message}`);
  }
}

const freshOwner = () => base58.encode(ed25519.getPublicKey(ed25519.utils.randomSecretKey()));
const randomBytes = (n) => crypto.getRandomValues(new Uint8Array(n));
const memo = () => `0x${randomBytes(32).toHex()}`;
const buyer = sol.solanaKeyFromPhrase(JUNK);

await check('junk mnemonic derives the address solana-keygen prints', () => {
  assert.equal(buyer.address, JUNK_ADDRESS);
  assert.equal(buyer.secret.length, 32);
  assert.equal(sol.solanaKeyFromPhrase(`  ${JUNK.toUpperCase()}\n`).address, JUNK_ADDRESS);
});

await check('a phrase that fails its checksum is refused', () => {
  assert.throws(() => sol.solanaKeyFromPhrase(JUNK.replace('junk', 'test')), /phrase/);
});

await check('base58 round trip and address shape', () => {
  const bytes = randomBytes(32);
  const text = sol.formatBase58(bytes);
  assert.deepEqual(base58.decode(text), bytes);
  assert.equal(sol.formatBase58(new Uint8Array(32)), sol.SYSTEM_PROGRAM);
  assert.ok(sol.isSolanaAddress(JUNK_ADDRESS));
  assert.ok(sol.isSolanaAddress(sol.SYSTEM_PROGRAM));
  assert.ok(!sol.isSolanaAddress('0x0000000000000000000000000000000000000000'));
  assert.ok(!sol.isSolanaAddress(sol.formatBase58(new Uint8Array(31).fill(7))));
  assert.ok(!sol.isSolanaAddress('O0Il'));
  assert.ok(!sol.isSolanaAddress(''));
});

await check('associated token address matches spl-token', () => {
  assert.equal(sol.associatedTokenAddress(JUNK_ADDRESS, USDC_DEVNET), JUNK_USDC_ATA);
});

await check('signMessage verifies under the derived key', () => {
  const text = 'LosOS market payout v1\nchain: solana\n';
  const sig = base58.decode(sol.signMessage(buyer, text));
  assert.equal(sig.length, 64);
  assert.ok(ed25519.verify(sig, new TextEncoder().encode(text), base58.decode(buyer.address)));
});

function transfer(feeAmount) {
  return {
    payer: buyer.address,
    mint: USDC_DEVNET,
    seller: freshOwner(),
    treasury: freshOwner(),
    sellerAmount: 5_760_000n,
    feeAmount,
    memo: memo(),
    recentBlockhash: sol.formatBase58(new Uint8Array(32).fill(9)),
  };
}

await check('message layout with a fee: header, keys, five instructions', () => {
  const t = transfer(240_000n);
  const msg = sol.usdcTransferMessage(t);
  // one signer (the buyer); writable: three token accounts; readonly: seller,
  // treasury, mint and the four programs.
  assert.deepEqual([...msg.subarray(0, 3)], [1, 0, 7]);
  assert.equal(msg[3], 11);
  const keys = Array.from({ length: 11 }, (_, i) => base58.encode(msg.subarray(4 + 32 * i, 36 + 32 * i)));
  assert.equal(keys[0], buyer.address);
  assert.deepEqual(
    new Set(keys.slice(1, 4)),
    new Set([t.seller, t.treasury, t.payer].map((o) => sol.associatedTokenAddress(o, t.mint))),
  );
  for (const k of [t.seller, t.treasury, t.mint, sol.SYSTEM_PROGRAM, sol.TOKEN_PROGRAM, sol.ATA_PROGRAM, sol.MEMO_PROGRAM])
    assert.ok(keys.slice(4).includes(k), `${k} readonly`);
  const at = 4 + 32 * 11;
  assert.equal(base58.encode(msg.subarray(at, at + 32)), t.recentBlockhash);
  assert.equal(msg[at + 32], 5);
  // last instruction: memo program, no accounts, the memo as data
  const tail = new TextEncoder().encode(t.memo);
  assert.deepEqual([...msg.subarray(msg.length - tail.length - 3)], [keys.indexOf(sol.MEMO_PROGRAM), 0, tail.length, ...tail]);
});

await check('message layout without a fee: no treasury, three instructions', () => {
  const t = transfer(0n);
  const msg = sol.usdcTransferMessage(t);
  assert.deepEqual([...msg.subarray(0, 4)], [1, 0, 6, 9]);
  const keys = Array.from({ length: 9 }, (_, i) => base58.encode(msg.subarray(4 + 32 * i, 36 + 32 * i)));
  assert.ok(!keys.includes(t.treasury));
  assert.equal(msg[4 + 32 * 9 + 32], 3);
});

await check('signTransaction frames one signature over the message', () => {
  const msg = sol.usdcTransferMessage(transfer(240_000n));
  const { wire, signature } = sol.signTransaction(buyer, msg);
  const tx = base64.decode(wire);
  assert.equal(tx[0], 1);
  assert.deepEqual(tx.subarray(65), msg);
  assert.equal(base58.encode(tx.subarray(1, 65)), signature);
  assert.ok(ed25519.verify(tx.subarray(1, 65), msg, base58.decode(buyer.address)));
});

// The program's address in the edge's tests; any address serves.
const ESCROW = 'LosEscrow1111111111111111111111111111111111';

function escrowPay(seller = freshOwner(), treasury = freshOwner()) {
  return {
    payer: buyer.address,
    program: ESCROW,
    mint: USDC_DEVNET,
    treasury,
    seller,
    orderIdHash: memo(),
    gross: 6_000_000n,
    fee: 240_000n,
    deadline: BigInt(Math.floor(Date.now() / 1000) + 33 * 86_400),
    nonce: randomBytes(32),
    recentBlockhash: sol.formatBase58(new Uint8Array(32).fill(9)),
  };
}

await check('escrow addresses match the edge', () => {
  // edge/registrar/src/solana_escrow.rs, `program_addresses_match_the_runtime`.
  assert.equal(
    sol.escrowVaultAddress(ESCROW, '9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM'),
    '92UMAvkSCU1ATw3VWw8b1kXV16DHQS7G3hxjeCSzyjHL',
  );
});

await check("escrow pay layout: the program's nine accounts and 89 bytes", () => {
  const p = escrowPay();
  const msg = sol.escrowPayMessage(p);
  // one signer; writable: the buyer's token account, the deposit, its
  // vault; readonly: config, seller, mint, token and system programs, the
  // escrow program.
  assert.deepEqual([...msg.subarray(0, 4)], [1, 0, 6, 10]);
  const keys = Array.from({ length: 10 }, (_, i) => base58.encode(msg.subarray(4 + 32 * i, 36 + 32 * i)));
  const config = sol.escrowConfigAddress(ESCROW, p.treasury, p.mint);
  const deposit = sol.escrowDepositAddress(ESCROW, config, p.nonce);
  const order = [
    buyer.address,
    sol.associatedTokenAddress(buyer.address, p.mint),
    config,
    deposit,
    sol.escrowVaultAddress(ESCROW, deposit),
    p.seller,
    p.mint,
    sol.TOKEN_PROGRAM,
    sol.SYSTEM_PROGRAM,
  ];
  const at = 4 + 32 * 10 + 32;
  assert.equal(msg[at], 1, 'one instruction');
  assert.equal(keys[msg[at + 1]], ESCROW);
  assert.equal(msg[at + 2], 9);
  assert.deepEqual([...msg.subarray(at + 3, at + 12)].map((i) => keys[i]), order);
  assert.equal(msg[at + 12], 89);
  const data = msg.subarray(at + 13);
  assert.equal(data.length, 89);
  assert.equal(data[0], 1);
  assert.deepEqual(data.subarray(1, 33), p.nonce);
  assert.equal(`0x${data.subarray(33, 65).toHex()}`, p.orderIdHash);
  const view = new DataView(data.buffer, data.byteOffset);
  assert.equal(view.getBigUint64(65, true), p.gross);
  assert.equal(view.getBigUint64(73, true), p.fee);
  assert.equal(view.getBigInt64(81, true), p.deadline);
});

await check('escrow pay refuses a fee over the gross or a short nonce', () => {
  assert.throws(() => sol.escrowPayMessage({ ...escrowPay(), fee: 7_000_000n }), /amount/);
  assert.throws(() => sol.escrowPayMessage({ ...escrowPay(), nonce: new Uint8Array(31) }), /nonce/);
  assert.throws(() => sol.escrowPayMessage({ ...escrowPay(), orderIdHash: '0x12' }), /order id hash/);
});

/* A compiled message read back: keys and instructions with their account
 * keys and data. Every count here is under 128, one byte of compact-u16. */
function parseMessage(msg) {
  const header = [...msg.subarray(0, 3)];
  const n = msg[3];
  const keys = Array.from({ length: n }, (_, i) => base58.encode(msg.subarray(4 + 32 * i, 36 + 32 * i)));
  let at = 4 + 32 * n + 32;
  const count = msg[at++];
  const ixs = [];
  for (let k = 0; k < count; k++) {
    const program = keys[msg[at++]];
    const m = msg[at++];
    const accounts = [...msg.subarray(at, at + m)].map((x) => keys[x]);
    at += m;
    const len = msg[at++];
    ixs.push({ program, accounts, data: msg.subarray(at, at + len) });
    at += len;
  }
  assert.equal(at, msg.length, 'trailing bytes');
  return { header, keys, ixs };
}

await check('balance and tap addresses match the edge', () => {
  // edge/registrar/src/solana_escrow.rs, `tap_addresses_match_the_runtime`.
  const config = '9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM';
  const owner = 'EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v';
  assert.equal(sol.escrowTapAddress(ESCROW, config, owner, new Uint8Array(32).fill(4)), '5Gbz1iYxjC5fegQqLZ7y5XxoR49y17gDhNQGjqzXWkWY');
  // Another wallet's tap of the same id is another account.
  assert.notEqual(sol.escrowTapAddress(ESCROW, config, config, new Uint8Array(32).fill(4)), '5Gbz1iYxjC5fegQqLZ7y5XxoR49y17gDhNQGjqzXWkWY');
  assert.equal(sol.escrowBalanceAddress(ESCROW, config, owner), '4p1yaXiRqaKiAUnnaeZStCXtG4krcWCnTnddBvDdXTHx');
});

function escrowTaps(taps, topUp = 4_000_000n) {
  return {
    payer: buyer.address,
    program: ESCROW,
    mint: USDC_DEVNET,
    treasury: freshOwner(),
    topUp,
    feeBps: 400,
    taps: Array.from({ length: taps }, (_, i) => ({ key: memo(), seller: freshOwner(), rate: BigInt(100_000 + i) })),
    recentBlockhash: sol.formatBase58(new Uint8Array(32).fill(9)),
  };
}

await check('taps layout: top_up, then one open_tap per box with its id, rate and fee, in one packet', () => {
  const p = escrowTaps(sol.TAPS_PER_TRANSACTION);
  const msg = sol.escrowTapsMessage(p);
  const { header, keys, ixs } = parseMessage(msg);
  assert.deepEqual(header.slice(0, 2), [1, 0], 'the buyer signs alone');
  assert.equal(keys[0], buyer.address);
  const config = sol.escrowConfigAddress(ESCROW, p.treasury, p.mint);
  const balance = sol.escrowBalanceAddress(ESCROW, config, p.payer);
  assert.equal(ixs.length, 1 + p.taps.length);
  const [top, ...opens] = ixs;
  assert.equal(top.program, ESCROW);
  assert.deepEqual(top.accounts, [
    buyer.address,
    sol.associatedTokenAddress(buyer.address, p.mint),
    config,
    balance,
    sol.escrowVaultAddress(ESCROW, balance),
    p.mint,
    sol.TOKEN_PROGRAM,
    sol.SYSTEM_PROGRAM,
  ]);
  assert.equal(top.data.length, 9);
  assert.equal(top.data[0], 6);
  assert.equal(new DataView(top.data.buffer, top.data.byteOffset).getBigUint64(1, true), p.topUp);
  opens.forEach((ix, i) => {
    const tap = p.taps[i];
    const id = Uint8Array.fromHex(tap.key.slice(2));
    assert.equal(ix.program, ESCROW);
    assert.deepEqual(ix.accounts, [buyer.address, config, balance, sol.escrowTapAddress(ESCROW, config, buyer.address, new Uint8Array(id)), tap.seller, sol.SYSTEM_PROGRAM]);
    assert.equal(ix.data.length, 43);
    assert.equal(ix.data[0], 7);
    assert.deepEqual(ix.data.slice(1, 33), id);
    const view = new DataView(ix.data.buffer, ix.data.byteOffset);
    assert.equal(view.getBigUint64(33, true), tap.rate);
    assert.equal(view.getUint16(41, true), 400);
  });
  // The balance is only read by open_tap, and written by top_up.
  const writable = keys.length - header[2];
  assert.ok(keys.indexOf(balance) < writable, 'the balance is not writable');
  const { wire } = sol.signTransaction(buyer, msg);
  assert.ok(base64.decode(wire).length <= 1232, `${base64.decode(wire).length} bytes is over a packet`);
});

await check('taps without a top-up open the taps alone; too many, none or a zero rate is refused', () => {
  const { ixs } = parseMessage(sol.escrowTapsMessage(escrowTaps(2, 0n)));
  assert.deepEqual(ixs.map((ix) => ix.data[0]), [7, 7]);
  assert.throws(() => sol.escrowTapsMessage(escrowTaps(sol.TAPS_PER_TRANSACTION + 1)), /taps/);
  assert.throws(() => sol.escrowTapsMessage(escrowTaps(0)), /taps/);
  const zero = escrowTaps(1);
  zero.taps[0].rate = 0n;
  assert.throws(() => sol.escrowTapsMessage(zero), /rate/);
});

await check('ask_withdrawal and withdraw: the buyer, the config and the balance, then the vault back to the buyer', () => {
  const w = { payer: buyer.address, program: ESCROW, mint: USDC_DEVNET, treasury: freshOwner(), recentBlockhash: sol.formatBase58(new Uint8Array(32).fill(9)) };
  const config = sol.escrowConfigAddress(ESCROW, w.treasury, w.mint);
  const balance = sol.escrowBalanceAddress(ESCROW, config, w.payer);
  const ask = parseMessage(sol.escrowAskMessage({ ...w, amount: 2_500_000n }));
  assert.equal(ask.ixs.length, 1);
  assert.deepEqual(ask.ixs[0].accounts, [buyer.address, config, balance]);
  assert.equal(ask.ixs[0].data[0], 10);
  assert.equal(new DataView(ask.ixs[0].data.buffer, ask.ixs[0].data.byteOffset).getBigUint64(1, true), 2_500_000n);
  assert.throws(() => sol.escrowAskMessage({ ...w, amount: 0n }), /amount/);
  const take = parseMessage(sol.escrowWithdrawMessage(w));
  assert.equal(take.ixs.length, 2, 'the token account is opened first if it is gone');
  assert.equal(take.ixs[0].program, sol.ATA_PROGRAM);
  assert.equal(take.ixs[1].program, ESCROW);
  assert.deepEqual(take.ixs[1].accounts, [
    buyer.address,
    config,
    balance,
    sol.escrowVaultAddress(ESCROW, balance),
    sol.associatedTokenAddress(buyer.address, w.mint),
    w.mint,
    sol.TOKEN_PROGRAM,
  ]);
  assert.deepEqual([...take.ixs[1].data], [11]);
});

const RPC = Bun.env.LOSOS_SOLANA_RPC;
if (RPC === undefined || RPC === '') {
  console.log('  skip live payment (LOSOS_SOLANA_RPC is not set)');
} else {
  await check(`live payment against ${RPC}`, async () => {
    let id = 0;
    const rpc = async (method, params = []) => {
      const res = await fetch(RPC, {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ jsonrpc: '2.0', id: ++id, method, params }),
      });
      const body = await res.json();
      if (body.error) throw new Error(`${method}: ${JSON.stringify(body.error)}`);
      return body.result;
    };
    const { value: latest } = await rpc('getLatestBlockhash', [{ commitment: 'confirmed' }]);
    const t = { ...transfer(240_000n), recentBlockhash: latest.blockhash };
    const { wire, signature } = sol.signTransaction(buyer, sol.usdcTransferMessage(t));
    const sent = await rpc('sendTransaction', [wire, { encoding: 'base64', preflightCommitment: 'confirmed' }]);
    assert.equal(sent, signature);

    for (let i = 0; ; i++) {
      const { value } = await rpc('getSignatureStatuses', [[signature]]);
      const s = value[0];
      if (s?.err) throw new Error(`failed on chain: ${JSON.stringify(s.err)}`);
      if (s && ['confirmed', 'finalized'].includes(s.confirmationStatus)) break;
      if (i > 120) throw new Error('not confirmed within 60 s');
      await new Promise((r) => setTimeout(r, 500));
    }
    const tx = await rpc('getTransaction', [
      signature,
      { encoding: 'jsonParsed', commitment: 'confirmed', maxSupportedTransactionVersion: 0 },
    ]);
    if (Bun.env.LOSOS_SOLANA_SAMPLE) {
      await Bun.write(Bun.env.LOSOS_SOLANA_SAMPLE, `${JSON.stringify(tx, null, 2)}\n`);
    }

    assert.equal(tx.meta.err, null);
    assert.equal(tx.transaction.signatures[0], signature);
    const memoIx = tx.transaction.message.instructions.find((ix) => ix.programId === sol.MEMO_PROGRAM);
    assert.equal(memoIx?.program, 'spl-memo');
    assert.equal(memoIx.parsed, t.memo);

    const byOwner = (list) => {
      const out = new Map();
      for (const b of list ?? []) {
        if (b.mint !== t.mint) continue;
        out.set(b.owner, (out.get(b.owner) ?? 0n) + BigInt(b.uiTokenAmount.amount));
      }
      return out;
    };
    const pre = byOwner(tx.meta.preTokenBalances);
    const post = byOwner(tx.meta.postTokenBalances);
    const delta = (o) => (post.get(o) ?? 0n) - (pre.get(o) ?? 0n);
    assert.equal(delta(t.seller), 5_760_000n);
    assert.equal(delta(t.treasury), 240_000n);
    assert.equal(delta(t.payer), -6_000_000n);
  });
}

// With the escrow program loaded at ESCROW as well (the edge's test loads it
// the same way: `--bpf-program LosEscrow111… losos_escrow.so`), the page's
// payment goes into a deposit the program then holds.
if (RPC && Bun.env.LOSOS_ESCROW_PROGRAM === ESCROW) {
  await check(`live escrow payment against ${RPC}`, async () => {
    let id = 0;
    const rpc = async (method, params = []) => {
      const res = await fetch(RPC, {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ jsonrpc: '2.0', id: ++id, method, params }),
      });
      const body = await res.json();
      if (body.error) throw new Error(`${method}: ${JSON.stringify(body.error)}`);
      return body.result;
    };
    const landed = async (signature) => {
      for (let i = 0; ; i++) {
        const { value } = await rpc('getSignatureStatuses', [[signature]]);
        const s = value[0];
        if (s?.err) throw new Error(`failed on chain: ${JSON.stringify(s.err)}`);
        if (s && ['confirmed', 'finalized'].includes(s.confirmationStatus)) return;
        if (i > 120) throw new Error('not confirmed within 60 s');
        await new Promise((r) => setTimeout(r, 500));
      }
    };
    // A treasury of its own makes its config: `initialize`, which the
    // page never sends, framed here by hand.
    const secret = ed25519.utils.randomSecretKey();
    const treasury = base58.encode(ed25519.getPublicKey(secret));
    await landed(await rpc('requestAirdrop', [treasury, 1_000_000_000]));
    const config = sol.escrowConfigAddress(ESCROW, treasury, USDC_DEVNET);
    const { value: first } = await rpc('getLatestBlockhash', [{ commitment: 'confirmed' }]);
    const keys = [treasury, config, USDC_DEVNET, sol.SYSTEM_PROGRAM, ESCROW];
    const init = Uint8Array.from([
      1, 0, 3, keys.length,
      ...keys.flatMap((k) => [...base58.decode(k)]),
      ...base58.decode(first.blockhash),
      1, 4, 4, 0, 1, 2, 3, 33, 0,
      ...base58.decode(freshOwner()),
    ]);
    const initSig = ed25519.sign(init, secret);
    await rpc('sendTransaction', [
      base64.encode(Uint8Array.from([1, ...initSig, ...init])),
      { encoding: 'base64', preflightCommitment: 'confirmed' },
    ]);
    await landed(base58.encode(initSig));

    const seller = freshOwner();
    const { value: latest } = await rpc('getLatestBlockhash', [{ commitment: 'confirmed' }]);
    const p = { ...escrowPay(seller, treasury), recentBlockhash: latest.blockhash };
    const { wire, signature } = sol.signTransaction(buyer, sol.escrowPayMessage(p));
    assert.equal(await rpc('sendTransaction', [wire, { encoding: 'base64', preflightCommitment: 'confirmed' }]), signature);
    await landed(signature);

    const deposit = sol.escrowDepositAddress(ESCROW, config, p.nonce);
    const info = await rpc('getAccountInfo', [deposit, { encoding: 'base64', commitment: 'confirmed' }]);
    assert.equal(info.value.owner, ESCROW);
    const d = base64.decode(info.value.data[0]);
    assert.equal(d.length, 196);
    assert.equal(base58.encode(d.subarray(36, 68)), buyer.address);
    assert.equal(base58.encode(d.subarray(68, 100)), seller);
    assert.equal(`0x${d.subarray(100, 132).toHex()}`, p.orderIdHash);
    const view = new DataView(d.buffer, d.byteOffset);
    assert.equal(view.getBigUint64(164, true), p.gross);
    assert.equal(view.getBigUint64(172, true), p.fee);
    assert.equal(view.getBigInt64(180, true), p.deadline);
    const vault = await rpc('getTokenAccountBalance', [sol.escrowVaultAddress(ESCROW, deposit), { commitment: 'confirmed' }]);
    assert.equal(vault.value.amount, p.gross.toString());
  });
}

if (failures.length > 0) {
  // Thrown, not printed: an uncaught error is what makes Bun exit non-zero.
  throw new Error(`${failures.length} failed:\n${failures.map((f) => `  - ${f}`).join('\n')}`);
}
console.log('\nall solana checks passed');
