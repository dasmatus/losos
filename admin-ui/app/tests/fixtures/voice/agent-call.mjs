/* The body of tests/voice-agent.browser.mjs.
 *
 * LosOS in a call, end to end: two Chromium tabs and the real SFU
 * (losos-calls), with the call's end-to-end encryption on.
 *
 * jan talks with Kokoro's voice into their microphone; eva's page adds the
 * agent, which joins as a second session of that page. The agent hears jan
 * through the SFU (every frame sealed), reads it with Whisper, asks the box
 * (a stand-in for LosOS Tag answers), and speaks the answer, which jan's tab
 * reads back with Whisper. Whisper and Kokoro run in the pages, from the
 * same files the box serves at /voice/.
 *
 *   cargo build --manifest-path ../../backend/Cargo.toml --bin losos-calls
 *   LOSOS_CALLS_BIN=../../backend/target/debug/losos-calls \
 *   LOSOS_VOICE_DIR=$(nix build --print-out-paths ../..#losos-voice) bun tests/voice-agent.browser.mjs
 *
 * The pages are served the way nginx serves /chat/ (modules/containers.nix):
 * cross-origin isolated, so the threaded Whisper build and ONNX Runtime's
 * thread pool are the ones under test. */

import assert from '../../assert.ts';
import { launch } from '../../harness.ts';
import { startFront } from '../calls/front.ts';

/* Paths as tests/ has them. */
const here = (p) => `${import.meta.dir}/../../${p}`;
const BIN = Bun.env.LOSOS_CALLS_BIN ?? here('../../../backend/target/debug/losos-calls');
/* The nix/voice.nix output: whisper/, whisper.bin, kokoro.onnx, voices/. */
const VOICE = Bun.env.LOSOS_VOICE_DIR;
const ADDRESS = Bun.env.LOSOS_CALLS_ADDRESS ?? '127.0.0.1';
const TMP = Bun.env.TMPDIR ?? '/tmp';
const QUESTION = 'Hey LosOS, what is the capital of Slovakia?';
const ANSWER = 'The capital of Slovakia is Bratislava. It sits on the Danube.';

const bundle = async (entry) => {
  const built = await Bun.build({
    entrypoints: [entry],
    target: 'browser',
    format: 'esm',
    define: { 'import.meta.env.BASE_URL': '"/"' },
  });
  if (!built.success) throw new AggregateError(built.logs, `bundling ${entry} failed`);
  return built.outputs[0].text();
};

const harness = await bundle(here('fixtures/voice/harness.ts'));
const files = {
  // Started by name from the harness, and by the agent's own session, which
  // takes the page's default: `new Worker(new URL('./e2ee-worker.ts', ...))`.
  '/e2ee-worker.js': await bundle(here('../src/calls/e2ee-worker.ts')),
  // The workers the page starts by `new Worker(new URL('./x.ts', import.meta.url))`.
  '/e2ee-worker.ts': await bundle(here('../src/calls/e2ee-worker.ts')),
  '/whisper-worker.ts': await bundle(here('../src/voice/whisper-worker.ts')),
  '/tts-worker.ts': await bundle(here('../src/voice/tts-worker.ts')),
};

/* What the box's assistant is asked, and how it answers. */
const asks = [];
const polls = new Map();

const ORT = here('../node_modules/onnxruntime-web/dist');
const isolated = { 'Cross-Origin-Opener-Policy': 'same-origin', 'Cross-Origin-Embedder-Policy': 'require-corp' };
const pages = Bun.serve({
  hostname: '127.0.0.1',
  port: 0,
  async fetch(req) {
    const url = new URL(req.url);
    const path = url.pathname;
    const js = { 'Content-Type': 'text/javascript', ...isolated };
    if (path === '/') return new Response('<!doctype html><title>voice</title><script type="module" src="/harness.js"></script>', { headers: { 'Content-Type': 'text/html', ...isolated } });
    if (path === '/harness.js') return new Response(harness, { headers: js });
    if (files[path]) return new Response(files[path], { headers: js });
    if (path === '/voice-worklet/pcm-tap.js') return new Response(Bun.file(here('../public/voice-worklet/pcm-tap.js')), { headers: js });
    if (path.startsWith('/ort/')) return new Response(Bun.file(`${ORT}/${path.slice(5)}`), { headers: isolated });
    if (path.startsWith('/voice/')) return new Response(Bun.file(`${VOICE}/${path.slice(7)}`), { headers: isolated });
    // LosOS Tag, as far as the agent uses it.
    if (path === '/api/tag/ask' && req.method === 'POST') {
      const body = await req.json();
      asks.push(body);
      const id = `ask${asks.length}`;
      polls.set(id, 0);
      return Response.json({ id, state: 'queued' });
    }
    const poll = /^\/api\/tag\/asks\/(.+)$/.exec(path);
    if (poll && req.method === 'GET') {
      const n = (polls.get(poll[1]) ?? 0) + 1;
      polls.set(poll[1], n);
      // The answer grows over a few reads, as a running turn's does.
      const words = ANSWER.split(' ');
      const shown = n < 3 ? words.slice(0, 3 * n) : words;
      return Response.json({ id: poll[1], state: n < 3 ? 'running' : 'done', answer: shown.join(' '), error: null });
    }
    return new Response(null, { status: 404 });
  },
});

const { sfu, nginx, front } = await startFront({ bin: BIN, address: ADDRESS, serverName: 'calls.test', tmp: TMP });
const api = (name) => `http://${front}/as/${name}/calls/api`;

const browser = await launch({
  args: ['--use-fake-ui-for-media-stream', '--autoplay-policy=no-user-gesture-required', '--allow-loopback-in-peer-connection'],
});
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
const until = async (what, fn, ms = 60000) => {
  const end = Date.now() + ms;
  for (;;) {
    const v = await fn();
    if (v) return v;
    if (Date.now() > end) throw new Error(`timed out: ${what}`);
    await new Promise((r) => setTimeout(r, 250));
  }
};

const tab = async (user) => {
  const p = await ctx.newPage();
  p.on('pageerror', (e) => console.log(`[${user}] ${e}`));
  p.on('console', (m) => m.type() === 'error' && console.log(`[${user}] ${m.text()}`));
  await p.goto(`http://127.0.0.1:${pages.port}/`);
  await p.waitForFunction(() => typeof window.joinHost === 'function');
  return p;
};
const state = (p) => p.evaluate(() => JSON.parse(JSON.stringify(window.state)));

const created = await (
  await fetch(`${api('jan')}/calls`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ room: '!room:calls.test', invited: ['@eva:calls.test'], video: false }),
  })
).json();
const call = created.call;
const socket = (name) => `ws://${front}/as/${name}/calls/api/ws`;
const JAN = { user: '@jan:calls.test', device: 'JAN1' };
const EVA = { user: '@eva:calls.test', device: 'EVA1' };

const jan = await tab('jan');
const eva = await tab('eva');

await check('the page is cross-origin isolated, so the threaded engines are the ones in use', async () => {
  assert.equal(await eva.evaluate(() => self.crossOriginIsolated), true);
});

await check('jan joins with a voice, and eva adds LosOS to the call from her page', async () => {
  await jan.evaluate((o) => window.joinSpeaker(o), { socket: socket('jan'), call, me: JAN });
  await eva.evaluate((o) => window.joinHost(o), { socket: socket('eva'), call, me: EVA });
  const members = () => jan.evaluate(() => window.state.members);
  await until('three in the call', async () => (await members()).length === 3);
  const list = await members();
  assert.equal(list.filter((m) => m.agent === true).length, 1, JSON.stringify(list));
  const agent = list.find((m) => m.agent === true);
  // The agent is eva's, on her device, and another place in the call.
  assert.equal(agent.user, EVA.user);
  assert.equal(agent.device, EVA.device);
  assert.equal(list.filter((m) => m.user === EVA.user).length, 2);
});

await check('the agent is given the call keys: jan and the agent open each other', async () => {
  await until('the agent is listening', async () => (await state(eva)).phases.includes('listening'));
  await until('keys reached the other tab', async () => (await state(jan)).keysIn > 0);
});

await check('jan says the name and asks; the question reaches the box as text', async () => {
  // Let the greeting finish so the two voices do not overlap.
  await until('the greeting played', async () => (await state(eva)).phases.includes('speaking'));
  await until('and ended', async () => (await state(eva)).phases.at(-1) === 'listening');
  const spokeAt = Date.now();
  await jan.evaluate((q) => window.say(q), QUESTION);
  const spoken = Date.now() - spokeAt;
  await until('an ask on the box', () => asks.length > 0, 120000).catch(async (e) => {
    console.log('eva', JSON.stringify(await state(eva)), await eva.evaluate(() => window.stats()), 'agent', await eva.evaluate(() => window.agentStats()), 'jan', JSON.stringify(await state(jan)), await jan.evaluate(() => window.stats()));
    throw e;
  });
  const [ask] = asks;
  console.log(`# the question took ${spoken} ms to say; the ask reached the box ${Date.now() - spokeAt - spoken} ms after`);
  assert.equal(ask.voice, true);
  assert.ok(!ask.grounded, 'the host\'s files are not offered unless they allowed it');
  assert.match(ask.question, /capital of Slovakia/i);
  assert.match(ask.question, /read aloud/i);
});

await check('the answer is spoken into the call and jan hears it, sealed, as text', async () => {
  await until('jan heard Bratislava', async () => (await state(jan)).heard.some((t) => /bratislava/i.test(t)), 90000);
  const heard = (await state(jan)).heard;
  console.log(`# jan heard: ${JSON.stringify(heard)}`);
  // The words came from the agent's pid, not from jan.
  const agentPid = (await state(jan)).members.find((m) => m.agent === true).pid;
  assert.ok(heard.some((t) => t.startsWith(`p${agentPid}:`) && /bratislava/i.test(t)), JSON.stringify(heard));
  // Heard text arrives before the caption list. Wait, or the assertion races the paint.
  await until('captions name Bratislava', async () => (await state(eva)).captions.some((c) => c.agent && /Bratislava/.test(c.text)), 90000);
  const captions = (await state(eva)).captions;
  assert.ok(captions.some((c) => !c.agent && /capital of Slovakia/i.test(c.text)), JSON.stringify(captions));
  assert.ok(captions.some((c) => c.agent && /Bratislava/.test(c.text)), JSON.stringify(captions));
});

await check('the agent is not a person of the room: its page leaving ends it', async () => {
  await eva.evaluate(() => window.agent.stop());
  await until('the agent left', async () => (await state(jan)).members.every((m) => m.agent !== true));
  assert.equal((await state(jan)).members.length, 2);
});

await browser.close();
sfu.kill();
nginx.stop(true);
pages.stop(true);
if (failed) throw new Error('voice agent: FAILED');
console.log(`voice agent: ${checks.length} passed`);
