/* The network-fee estimates in src/wallet/fees.ts, checked without a browser
 * against a stand-in chain behind the market relay: the lowest EIP-1559 fee,
 * what a payment costs on an EVM network and on Solana, and which network
 * the wallet offers first.
 *
 *   bun tests/fees.unit.mjs
 */

import assert from './assert.mjs';

const fees = await import('../src/wallet/fees.ts');

const gwei = 1_000_000_000n;
const hex = (n) => `0x${n.toString(16)}`;

// The relay's answers, per network and method.
let chain = {};
globalThis.window = { sessionStorage: { getItem: () => null } };
globalThis.fetch = async (path, init) => {
  assert.equal(path, '/api/market/rpc');
  const { network, method, params } = JSON.parse(init.body);
  const answer = chain[network]?.[method];
  if (answer === undefined) {
    return new Response(JSON.stringify({ error: { code: -32601, message: `no ${method}` } }));
  }
  const result = typeof answer === 'function' ? answer(params) : answer;
  return new Response(JSON.stringify({ result }));
};

// Ten blocks: one empty block that tipped nothing, nine whose cheapest tenth
// tipped 1 to 9 gwei, and a base fee of 4 gwei for the next block.
const history = {
  oldestBlock: '0x10',
  baseFeePerGas: [...Array(10).fill(hex(3n * gwei)), hex(4n * gwei)],
  gasUsedRatio: Array(10).fill(0.5),
  reward: [[hex(0n)], ...[1n, 2n, 3n, 4n, 5n, 6n, 7n, 8n, 9n].map((n) => [hex(n * gwei)])],
};

chain = { 'base-sepolia': { eth_feeHistory: history, eth_gasPrice: hex(50n * gwei), eth_chainId: '0x14a34' } };
{
  const low = await fees.lowestFees('base-sepolia');
  // The median of the sorted low tips [0, 1, ..., 9] gwei is the sixth: 5 gwei.
  assert.equal(low.maxPriorityFeePerGas, 5n * gwei);
  assert.equal(low.perGas, 9n * gwei);
  // Two blocks of 12.5% headroom on the base fee, plus the tip.
  assert.equal(low.maxFeePerGas, (4n * gwei * 81n) / 64n + 5n * gwei);
  assert.ok(low.perGas < 50n * gwei, 'cheaper than the node\'s suggested gas price');
  assert.deepEqual(await fees.lowFeeFields('base-sepolia'), {
    maxFeePerGas: low.maxFeePerGas,
    maxPriorityFeePerGas: low.maxPriorityFeePerGas,
  });
}

// A network that will not say gives no fee fields; the wallet picks its own.
assert.deepEqual(await fees.lowFeeFields('arbitrum-sepolia'), {});

const evm = {
  network: 'base-sepolia',
  name: 'Base Sepolia',
  family: 'evm',
  usdc: '0x036CbD53842c5426634e7929541eC2318f3dCF7e',
  pay: '0x1111111111111111111111111111111111111111',
  treasury: '0x2222222222222222222222222222222222222222',
};
const buyer = '0x3333333333333333333333333333333333333333';

// A payment that still needs an approval: both transactions' gas at the low
// fee, and the L1 data fee the OP Stack oracle quotes (here 7 gwei worth).
chain['base-sepolia'].eth_call = ([call]) =>
  call.to.toLowerCase() === evm.usdc.toLowerCase()
    ? `0x${'0'.repeat(64)}` // allowance 0
    : `0x${(7n * gwei).toString(16).padStart(64, '0')}`;
{
  const est = await fees.estimateFee(evm, { kind: 'pay', amount: 1_000_000n, owner: buyer }, { eth: '3000000000' });
  assert.equal(est.symbol, 'ETH');
  assert.equal(est.native, (190_000n + 46_000n) * 9n * gwei + 7n * gwei);
  // 3000 dollars per ETH.
  assert.equal(est.usdMicros, (est.native * 3_000_000_000n) / 10n ** 18n);
}

const sol = {
  network: 'solana-devnet',
  name: 'Solana Devnet',
  family: 'solana',
  usdc: '4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU',
  treasury: 'oeYf6KAJkLYhBuR8CiGc6L4D4Xtfepr85fuDgA9kq96',
};
const seller = '11111111111111111111111111111112';
// The treasury's USDC account exists, the seller's does not.
let asked = 0;
chain['solana-devnet'] = {
  getAccountInfo: () => (asked++ === 0 ? { value: null } : { value: { lamports: 1 } }),
  getMinimumBalanceForRentExemption: 2_039_280,
};
{
  const est = await fees.estimateFee(sol, { kind: 'pay', amount: 1_000_000n, recipients: [seller] }, {});
  assert.equal(est.symbol, 'SOL');
  assert.equal(est.native, 5_000n + 2_039_280n);
  assert.equal(est.usdMicros, null);
}

// Through the escrow program the edge opens the token accounts; the buyer
// lends the rent of the deposit and its vault, which comes back, and pays
// only the signature.
chain['solana-devnet'] = {
  getAccountInfo: () => {
    throw new Error('the escrow estimate looks up no token account');
  },
  getMinimumBalanceForRentExemption: ([bytes]) => ({ 196: 2_255_040, 165: 2_039_280 })[bytes],
};
{
  const escrowed = { ...sol, pay: 'LosEscrow1111111111111111111111111111111111' };
  const est = await fees.estimateFee(escrowed, { kind: 'pay', amount: 1_000_000n, recipients: [seller] }, { sol: '150000000' });
  assert.equal(est.native, 5_000n + 2_255_040n + 2_039_280n);
  // 150 dollars per SOL: 5000 lamports is 750 micro-dollars.
  assert.equal(est.usdMicros, 750n);
}

// The ranking: the cheapest network the wallet can pay on, in dollars.
const e = (network, native, usdMicros, symbol = 'ETH') => ({ network, native, symbol, decimals: 18, usdMicros });
const held = (network, usdc, gas) => ({ network, usdc, gas });
assert.equal(
  fees.cheapest([e('base', 10n, 4_000n), e('arbitrum', 10n, 2_000n), e('optimism', 10n, 9_000n)], null, 1n),
  'arbitrum',
);
// Arbitrum is cheaper but holds too little USDC, Optimism too little ETH.
assert.equal(
  fees.cheapest(
    [e('base', 10n, 4_000n), e('arbitrum', 10n, 2_000n), e('optimism', 10n, 1_000n)],
    [held('base', 5n, 10n), held('arbitrum', 4n, 10n), held('optimism', 5n, 9n)],
    5n,
  ),
  'base',
);
// Without prices, ETH fees compare among themselves; no estimate comes last.
assert.equal(fees.cheapest([null, e('base', 30n, null), e('optimism', 20n, null)], null, 1n), 'optimism');
assert.equal(fees.cheapest([null], null, 1n), null);
assert.equal(fees.cheapest([e('base', 10n, 1n)], [held('base', 0n, 10n)], 1n), null);

assert.equal(fees.formatFeeUsd(500n), '<$0.001');
assert.equal(fees.formatFeeUsd(4_200n), '$0.004');
assert.equal(fees.formatFeeUsd(123_000n), '$0.12');

console.log('fees: ok');
