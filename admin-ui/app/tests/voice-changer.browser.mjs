/* The voice changer in a real call: one tab sends a steady 300 Hz tone as its
 * microphone through src/calls/voice-changer.ts, and another, across the
 * SFU with the end-to-end encryption on, measures the pitch it receives.
 * Natural stays 300 Hz; deeper and higher move it by the semitones they ask.
 *
 *   cargo build --manifest-path ../../backend/Cargo.toml --bin losos-calls
 *   LOSOS_CALLS_BIN=../../backend/target/debug/losos-calls bun tests/voice-changer.browser.mjs
 */

import assert from './assert.mjs';
import { launch } from './harness.mjs';
import { startFront } from './fixtures/calls/front.mjs';

const here = (p) => `${import.meta.dir}/${p}`;
const BIN = Bun.env.LOSOS_CALLS_BIN ?? here('../../../backend/target/debug/losos-calls');
const ADDRESS = Bun.env.LOSOS_CALLS_ADDRESS ?? '127.0.0.1';
const TMP = Bun.env.TMPDIR ?? '/tmp';
const HZ = 300;

const bundle = async (entry) => {
  const built = await Bun.build({ entrypoints: [entry], target: 'browser', format: 'esm', define: { 'import.meta.env.BASE_URL': '"/"' } });
  if (!built.success) throw new AggregateError(built.logs, `bundling ${entry} failed`);
  return built.outputs[0].text();
};
const harness = await bundle(here('fixtures/voice/harness.ts'));
const worker = await bundle(here('../src/calls/e2ee-worker.ts'));
const js = { 'Content-Type': 'text/javascript' };
const pages = Bun.serve({
  hostname: '127.0.0.1',
  port: 0,
  fetch(req) {
    const path = new URL(req.url).pathname;
    if (path === '/') return new Response('<!doctype html><title>voice</title><script type="module" src="/harness.js"></script>', { headers: { 'Content-Type': 'text/html' } });
    if (path === '/harness.js') return new Response(harness, { headers: js });
    if (path === '/e2ee-worker.js' || path === '/e2ee-worker.ts') return new Response(worker, { headers: js });
    if (path === '/voice-worklet/voice-changer.js') return new Response(Bun.file(here('../public/voice-worklet/voice-changer.js')), { headers: js });
    return new Response(null, { status: 404 });
  },
});

const { sfu, nginx, front } = await startFront({ bin: BIN, address: ADDRESS, serverName: 'calls.test', tmp: TMP });
const api = (name) => `http://${front}/as/${name}/calls/api`;
const socket = (name) => `ws://${front}/as/${name}/calls/api/ws`;
const browser = await launch({ args: ['--autoplay-policy=no-user-gesture-required', '--allow-loopback-in-peer-connection'] });
const ctx = await browser.newContext({ permissions: ['microphone'] });

let failed = false;
const checks = [];
const check = async (name, fn) => {
  try {
    await fn();
    console.log(`ok ${name}`);
  } catch (e) {
    failed = true;
    console.log(`not ok ${name}\n  ${e.stack ?? e}`);
  }
  checks.push(name);
};
const until = async (what, fn, ms = 30000) => {
  const end = Date.now() + ms;
  for (;;) {
    const v = await fn();
    if (v) return v;
    if (Date.now() > end) throw new Error(`timed out: ${what}`);
    await new Promise((r) => setTimeout(r, 200));
  }
};
const tab = async (user) => {
  const p = await ctx.newPage();
  p.on('pageerror', (e) => console.log(`[${user}] ${e}`));
  await p.goto(`http://127.0.0.1:${pages.port}/`);
  await p.waitForFunction(() => typeof window.joinTone === 'function');
  return p;
};

const { call } = await (
  await fetch(`${api('jan')}/calls`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ room: '!room:calls.test', invited: ['@eva:calls.test'], video: false }),
  })
).json();
const JAN = { user: '@jan:calls.test', device: 'JAN1' };
const EVA = { user: '@eva:calls.test', device: 'EVA1' };
const jan = await tab('jan');
const eva = await tab('eva');
await jan.evaluate((o) => window.joinTone(o), { socket: socket('jan'), call, me: JAN, hz: HZ });
await eva.evaluate((o) => window.listenPitch(o), { socket: socket('eva'), call, me: EVA });

/* The pitch eva hears, once it has settled on what jan sends. */
const heard = async () => {
  await until('a stream from jan', () => eva.evaluate(() => window.analyser !== undefined));
  await new Promise((r) => setTimeout(r, 1500));
  return eva.evaluate(() => window.pitch(2000));
};
const within = (got, want, pct = 5) => assert.ok(Math.abs(got - want) / want < pct / 100, `${got.toFixed(1)} Hz is not ${want.toFixed(1)} Hz within ${pct}%`);

await check('natural: eva hears the 300 Hz tone, sealed and opened', async () => {
  const r = await heard();
  assert.ok(r.peak > 0.1, JSON.stringify(r));
  within(r.hz, HZ);
});

for (const [preset, semitones] of [['deep', -4], ['high', 4], ['giant', -8]]) {
  await check(`${preset}: eva hears it ${Math.abs(semitones)} semitones ${semitones < 0 ? 'lower' : 'higher'}`, async () => {
    await jan.evaluate((p) => window.changeVoice(p), preset);
    const r = await heard();
    assert.ok(r.peak > 0.05, JSON.stringify(r));
    within(r.hz, HZ * 2 ** (semitones / 12));
  });
}

await check('natural again: the changer is taken out and the tone is back', async () => {
  await jan.evaluate(() => window.changeVoice('off'));
  within((await heard()).hz, HZ);
});

await browser.close();
sfu.kill();
nginx.stop(true);
pages.stop(true);
if (failed) throw new Error('voice changer: FAILED');
console.log(`voice changer: ${checks.length} passed`);
