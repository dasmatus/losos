/* The body of tests/voice-assistant.browser.mjs.
 *
 * A video call with one of the person's assistants, end to end: one Chromium
 * tab, the real SFU (losos-calls), the call's end-to-end encryption on.
 *
 * The person's page is the one src/chat/calls.tsx makes in a chat with an
 * assistant (`@tag-gardener`): their own session, with a Kokoro voice for a
 * microphone, and the assistant as a second session of the same page, with a
 * face for a camera. The person asks without saying a name; the question
 * reaches the box as text addressed to that assistant; the answer comes
 * back as speech, and the face that arrives through the SFU, sealed and
 * opened again, moves its mouth with it.
 *
 *   cargo build --manifest-path ../../backend/Cargo.toml --bin losos-calls
 *   LOSOS_CALLS_BIN=../../backend/target/debug/losos-calls \
 *   LOSOS_VOICE_DIR=$(nix build --print-out-paths ../..#losos-voice) bun tests/voice-assistant.browser.mjs
 *
 * The pages are served the way nginx serves /chat/ (modules/containers.nix):
 * cross-origin isolated, so the threaded Whisper build and ONNX Runtime's
 * thread pool are the ones under test. */

import assert from '../../assert.mjs';
import { launch } from '../../harness.ts';
import { startFront } from '../calls/front.ts';

/* Paths as tests/ has them. */
const here = (p) => `${import.meta.dir}/../../${p}`;
const BIN = Bun.env.LOSOS_CALLS_BIN ?? here('../../../backend/target/debug/losos-calls');
/* The nix/voice.nix output: whisper/, whisper.bin, kokoro.onnx, voices/. */
const VOICE = Bun.env.LOSOS_VOICE_DIR;
const ADDRESS = Bun.env.LOSOS_CALLS_ADDRESS ?? '127.0.0.1';
const TMP = Bun.env.TMPDIR ?? '/tmp';
const QUESTION = 'What is the capital of Slovakia?';
const ANSWER = 'The capital of Slovakia is Bratislava. It sits on the Danube, close to Austria and Hungary. Many people call it the small big city.';

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
    body: JSON.stringify({ room: '!gardener:calls.test', invited: ['@tag-gardener:calls.test'], video: true }),
  })
).json();
const call = created.call;
const socket = (name) => `ws://${front}/as/${name}/calls/api/ws`;
const JAN = { user: '@jan:calls.test', device: 'JAN1' };

const jan = await tab('jan');

await check('the page is cross-origin isolated, so the threaded engines are the ones in use', async () => {
  assert.equal(await jan.evaluate(() => self.crossOriginIsolated), true);
});

await check('the assistant joins the person\'s call by itself, as an agent on their device', async () => {
  await jan.evaluate((o) => window.joinPerson(o), { socket: socket('jan'), call, me: JAN, assistant: 'gardener', name: 'Gardener', video: true });
  const members = () => jan.evaluate(() => window.state.members);
  await until('two in the call', async () => (await members()).length === 2);
  const list = await members();
  const agent = list.find((m) => m.agent === true);
  assert.ok(agent, JSON.stringify(list));
  assert.equal(agent.user, JAN.user);
  assert.equal(agent.device, JAN.device);
  // It has a voice of its own, the same every time for that name.
  assert.ok(/^(af|am|bf|bm)_/.test((await state(jan)).voice), JSON.stringify(await state(jan)));
});

await check('its face arrives through the SFU, sealed and opened again', async () => {
  await until('frames decoded', async () => (await jan.evaluate(() => window.faces)).frames > 10, 60000);
  const faces = await jan.evaluate(() => window.faces);
  console.log(`# the face: ${faces.width}x${faces.height}, ${faces.frames} frames so far`);
  assert.ok(faces.width >= 320 && faces.height >= 240, JSON.stringify(faces));
  const look = await jan.evaluate(() => window.faceLook());
  // Skin over a good part of the picture, not a black or a grey frame.
  assert.ok(look.all.skin > 0.1, JSON.stringify(look));
});

await check('it greets the person by its name and listens', async () => {
  await until('the greeting', async () => (await state(jan)).phases.includes('speaking'), 90000);
  await until('and ended', async () => (await state(jan)).phases.at(-1) === 'listening');
});

await check('the person asks with no name; the question reaches that assistant as text', async () => {
  await jan.evaluate((q) => window.say(q), QUESTION);
  await until('an ask on the box', () => asks.length > 0, 120000).catch(async (e) => {
    console.log('jan', JSON.stringify(await state(jan)), await jan.evaluate(() => window.stats()), 'agent', await jan.evaluate(() => window.agentStats()));
    throw e;
  });
  const [ask] = asks;
  assert.equal(ask.assistant, 'gardener');
  assert.equal(ask.voice, true);
  // A call of one is private to its person, so their material may help.
  assert.equal(ask.grounded, true);
  assert.match(ask.question, /capital of Slovakia/i);
  assert.match(ask.question, /You are Gardener, in a video call with jan/);
  assert.match(ask.question, /read aloud to them/);
});

await check('while it speaks its mouth opens; at rest it is a line', async () => {
  // The answer is read a sentence at a time; sample the face all through.
  let open = 0;
  let calm = 1;
  const end = Date.now() + 40000;
  while (Date.now() < end) {
    const phases = (await state(jan)).phases;
    const look = await jan.evaluate(() => window.faceLook());
    if (look !== null) {
      if (phases.at(-1) === 'speaking') open = Math.max(open, look.mouth.ink);
      if (phases.at(-1) === 'listening' && phases.includes('thinking')) calm = Math.min(calm, look.mouth.ink);
    }
    if (open > 0 && phases.at(-1) === 'listening' && phases.filter((p) => p === 'speaking').length >= 2 && calm < 1) break;
    await new Promise((r) => setTimeout(r, 120));
  }
  console.log(`# mouth ink: at rest ${calm.toFixed(3)}, speaking (largest) ${open.toFixed(3)}`);
  assert.ok(open > calm * 1.8 && open > 0.08, `speaking ${open}, at rest ${calm}`);
});

await check('its voice reaches the person: audio energy, and its own caption', async () => {
  const stats = await jan.evaluate(() => window.stats());
  const inbound = stats.find((s) => s.type === 'inbound-rtp');
  assert.ok(inbound && inbound.packets > 50, JSON.stringify(stats));
  const captions = (await state(jan)).captions;
  assert.ok(captions.some((c) => c.agent && /Bratislava/.test(c.text) && c.who === 'Gardener'), JSON.stringify(captions));
});

await check('hanging up takes the assistant with it', async () => {
  await jan.evaluate(() => window.agent.stop());
  await until('the agent left', async () => (await state(jan)).members.every((m) => m.agent !== true));
});

await browser.close();
sfu.kill();
nginx.stop(true);
pages.stop(true);
if (failed) throw new Error('voice assistant: FAILED');
console.log(`voice assistant: ${checks.length} passed`);
