/* The Solana USDC payer in src/wallet/solana.ts, checked without a browser.
 *
 * The pure checks pin it to what the reference tools print: the address
 * solana-keygen derives from the junk mnemonic at m/44'/501'/0'/0', and the
 * token account spl-token names for that owner and Circle's devnet USDC mint.
 * The live check needs a local validator holding that mint (cloned with
 * `solana-test-validator --clone`) with USDC on the buyer, and runs only when
 * LOSOS_SOLANA_RPC is set: it pays a seller and a treasury in one transaction
 * and reads the result back the way the edge's verifier will. With
 * LOSOS_SOLANA_SAMPLE set, the jsonParsed transaction is written there.
 *
 *   node tests/solana.node.mjs
 *   LOSOS_SOLANA_RPC=http://127.0.0.1:8899 node tests/solana.node.mjs
 */

import assert from 'node:assert/strict';
import { randomBytes } from 'node:crypto';
import { writeFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { ed25519 } from '@noble/curves/ed25519.js';
import { base58, base64 } from '@scure/base';
import * as esbuild from 'esbuild';

const JUNK = 'test test test test test test test test test test test junk';
// solana-keygen pubkey "prompt://?full-path=m/44'/501'/0'/0'" with JUNK, no passphrase.
const JUNK_ADDRESS = 'oeYf6KAJkLYhBuR8CiGc6L4D4Xtfepr85fuDgA9kq96';
const USDC_DEVNET = '4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU';
// spl-token address --verbose --owner JUNK_ADDRESS --token USDC_DEVNET
const JUNK_USDC_ATA = 'Acp8hQfvQgptyqCreekNzaEHXMv6o3L1JiSwXmRHaaDW';

const built = await esbuild.build({
  entryPoints: [fileURLToPath(new URL('../src/wallet/solana.ts', import.meta.url))],
  bundle: true,
  format: 'esm',
  platform: 'node',
  write: false,
});
const sol = await import(
  `data:text/javascript;base64,${Buffer.from(built.outputFiles[0].text).toString('base64')}`
);

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
const memo = () => `0x${randomBytes(32).toString('hex')}`;
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
  const bytes = new Uint8Array(randomBytes(32));
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

const RPC = process.env.LOSOS_SOLANA_RPC;
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
    if (process.env.LOSOS_SOLANA_SAMPLE) {
      await writeFile(process.env.LOSOS_SOLANA_SAMPLE, `${JSON.stringify(tx, null, 2)}\n`);
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

if (failures.length > 0) {
  console.error(`\n${failures.length} failed:\n${failures.map((f) => `  - ${f}`).join('\n')}`);
  process.exit(1);
}
console.log('\nall solana checks passed');
