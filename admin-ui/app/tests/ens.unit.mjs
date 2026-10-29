/* Names a buyer types into the box's wallet, resolved the way the page
 * resolves them (src/wallet/chains.ts): every chain read through the box's
 * relay, and the offchain step of a DNS name (EIP-3668) through the edge's
 * `ens_ccipRead`, which asks ENS's DNSSEC gateway.
 *
 * The answers are Ethereum mainnet's and the gateway's, recorded in
 * tests/fixtures/ens.json and replayed here, so the check runs offline.
 * taytems.xyz carries `ENS1 dnsname.ens.eth <address>`, the record the edge
 * publishes at a box's names, so it walks the path a buyer's lookup of
 * mattbox.<zone> walks.
 *
 *   bun tests/ens.unit.mjs
 *   LOSOS_ENS_RECORD=https://ethereum-rpc.publicnode.com bun tests/ens.unit.mjs
 *
 * The second form asks mainnet and the gateway live and rewrites the fixture.
 */

import assert from './assert.mjs';

const chains = await import('../src/wallet/chains.ts');

const fixture = `${import.meta.dir}/fixtures/ens.json`;
const live = Bun.env.LOSOS_ENS_RECORD;
const answers = live ? {} : await Bun.file(fixture).json();
const seen = [];

// What the relay would ask upstream, by the call the page made.
async function upstream({ network, method, params }) {
  if (method === 'ens_ccipRead') {
    const [sender, data] = params;
    const reply = await fetch(chains.ENS_DNS_GATEWAY, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ sender: sender.toLowerCase(), data }),
    });
    return { result: (await reply.json()).data };
  }
  const reply = await fetch(live, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ jsonrpc: '2.0', id: 1, method, params }),
  });
  const { result, error } = await reply.json();
  return error ? { error } : { result };
}

const realFetch = globalThis.fetch;
globalThis.window = { sessionStorage: { getItem: () => null } };
globalThis.fetch = async (path, init) => {
  if (path !== '/api/market/rpc') return realFetch(path, init);
  const call = JSON.parse(init.body);
  // Only the ENS endpoint: a name lookup never touches a payment network.
  assert.equal(call.network, 'ens', `${call.method} went to ${call.network}`);
  seen.push(call.method);
  const key = JSON.stringify(call);
  if (live) answers[key] = await upstream(call);
  const answer = answers[key];
  assert.ok(answer, `no recorded answer for ${key.slice(0, 160)}`);
  return new Response(JSON.stringify(answer));
};

// A DNS name with the edge's record: an eth_call that asks for an offchain
// answer, the gateway's DNSSEC proof through the relay, then the address.
assert.equal(
  (await chains.ensAddress(false, 'taytems.xyz'))?.toLowerCase(),
  '0x8e8db5ccef88cca9d624701db544989c996e3216',
);
assert.ok(seen.includes('ens_ccipRead'), 'the DNS name took the offchain step through the relay');

// A name held on chain still resolves, now asked by Ethereum's coin type.
seen.length = 0;
assert.equal(
  (await chains.ensAddress(false, 'vitalik.eth'))?.toLowerCase(),
  '0xd8da6bf26964af9d7eed9e03e53415d37aa96045',
);

// The relay is asked only for ENS's gateway, whatever a resolver names.
seen.length = 0;
await assert.rejects(
  chains.ensGateway({ sender: '0x0000000000000000000000000000000000000001', data: '0x00', urls: ['https://gateway.example/{sender}/{data}'] }),
  /does not reach/,
);
assert.deepEqual(seen, []);

if (live) await Bun.write(fixture, `${JSON.stringify(answers, null, 2)}\n`);
console.log('ens: ok');
