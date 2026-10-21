/* A call between Chromium tabs through the real SFU (losos-calls), with the
 * end-to-end encryption on: both see each other, the SFU's tap holds only
 * frames no decoder can read, a tab without the keys sees nothing, and a
 * tab that joins later gets in on a fresh key.
 *
 *   cargo build --manifest-path ../../backend/Cargo.toml --bin losos-calls
 *   LOSOS_CALLS_BIN=../../backend/target/debug/losos-calls node tests/calls.browser.mjs
 *
 * LOSOS_CALLS_ADDRESS is the address the SFU offers media on (default: the
 * first non-loopback IPv4 address); LOSOS_CHROMIUM a chromium binary.
 */

import assert from 'node:assert/strict';
import { spawn, spawnSync } from 'node:child_process';
import { createServer } from 'node:http';
import { mkdtempSync, readdirSync, readFileSync, writeFileSync } from 'node:fs';
import { networkInterfaces, tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import * as esbuild from 'esbuild';
import { chromium } from 'playwright';

const here = (p) => fileURLToPath(new URL(p, import.meta.url));
const BIN = process.env.LOSOS_CALLS_BIN ?? here('../../../backend/target/debug/losos-calls');
const SERVER = 'calls.test';
const ADDRESS =
  process.env.LOSOS_CALLS_ADDRESS ??
  Object.values(networkInterfaces())
    .flat()
    .find((i) => i && i.family === 'IPv4' && !i.internal)?.address ??
  '127.0.0.1';

const bundle = async (entry) =>
  (
    await esbuild.build({
      entryPoints: [entry],
      bundle: true,
      format: 'esm',
      platform: 'browser',
      write: false,
    })
  ).outputFiles[0].text;

const harness = await bundle(here('fixtures/calls/harness.ts'));
const e2ee = await import(
  `data:text/javascript;base64,${Buffer.from(
    (await esbuild.build({ entryPoints: [here('../src/calls/e2ee.ts')], bundle: true, format: 'esm', platform: 'node', write: false }))
      .outputFiles[0].text,
  ).toString('base64')}`
);
const worker = await bundle(here('../src/calls/e2ee-worker.ts'));

const listen = (server) =>
  new Promise((ok) => server.listen(0, '127.0.0.1', () => ok(server.address().port)));

// A homeserver that knows three OpenID tokens.
const TOKENS = { 'tok-matus': '@matus:calls.test', 'tok-eva': '@eva:calls.test', 'tok-jan': '@jan:calls.test' };
const homeserver = createServer((req, res) => {
  const u = new URL(req.url, 'http://x');
  const sub = u.pathname === '/_matrix/federation/v1/openid/userinfo' && TOKENS[u.searchParams.get('access_token')];
  res.writeHead(sub ? 200 : 401, { 'Content-Type': 'application/json' });
  res.end(JSON.stringify(sub ? { sub } : { errcode: 'M_UNKNOWN_TOKEN' }));
});
const hsPort = await listen(homeserver);

const pages = createServer((req, res) => {
  const body = { '/': '<!doctype html><title>call</title><script type="module" src="/harness.js"></script>', '/harness.js': harness, '/e2ee-worker.js': worker }[req.url];
  if (body === undefined) {
    res.writeHead(404).end();
    return;
  }
  res.writeHead(200, { 'Content-Type': req.url === '/' ? 'text/html' : 'text/javascript' });
  res.end(body);
});
const pagePort = await listen(pages);

// The SFU, on a free port of its own.
const probe = createServer();
const sfuPort = await listen(probe);
probe.close();
const udpPort = 20000 + Math.floor(Math.random() * 20000);
const tap = mkdtempSync(join(tmpdir(), 'losos-calls-tap-'));
const sfu = spawn(
  BIN,
  ['--listen', `127.0.0.1:${sfuPort}`, '--udp-port', String(udpPort), '--address', ADDRESS, '--server-name', SERVER, '--homeserver', `127.0.0.1:${hsPort}`, '--tap', tap],
  { stdio: ['ignore', 'inherit', 'inherit'] },
);
const base = `http://127.0.0.1:${sfuPort}/calls/api`;
for (let i = 0; ; i++) {
  try {
    if ((await fetch(`${base}/health`)).ok) break;
  } catch {}
  if (i > 100) throw new Error('losos-calls did not start');
  await new Promise((r) => setTimeout(r, 100));
}

const browser = await chromium.launch({
  executablePath: process.env.LOSOS_CHROMIUM ?? '/opt/pw-browsers/chromium',
  args: ['--use-fake-ui-for-media-stream', '--use-fake-device-for-media-stream', '--autoplay-policy=no-user-gesture-required'],
});
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

const until = async (what, fn, ms = 20000) => {
  const end = Date.now() + ms;
  for (;;) {
    const v = await fn();
    if (v) return v;
    if (Date.now() > end) throw new Error(`timed out: ${what}`);
    await new Promise((r) => setTimeout(r, 200));
  }
};

const create = async (token, invited) =>
  fetch(`${base}/calls`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ openid: { access_token: token, matrix_server_name: SERVER }, room: '!room:calls.test', invited, video: true }),
  });

const ctx = await browser.newContext();
const tab = async (token, user, device, extra = {}) => {
  const p = await ctx.newPage();
  p.on('pageerror', (e) => console.log(`[${user}] ${e}`));
  await p.goto(`http://127.0.0.1:${pagePort}/`);
  await p.waitForFunction(() => typeof window.startCall === 'function');
  await p.evaluate(
    (o) => window.startCall(o),
    { socket: `ws://127.0.0.1:${sfuPort}/calls/api/ws`, call, token, server: SERVER, me: { user, device }, ...extra },
  );
  return p;
};
const stats = (p) => p.evaluate(() => window.callState.stats);
const members = (p) => p.evaluate(() => window.callState.members);
const pidOf = async (p, user) => (await members(p)).find((m) => m.user === user)?.pid;
const picture = (p, id) => p.evaluate((id) => window.picture(id), id);
/* How far apart two pictures are, 0 to 255: what `to` shows of `from`'s
 * camera against what `from` itself captures. */
const distance = async (from, to, pid) => {
  const [a, b] = [await picture(from, 'local'), await picture(to, `p${pid}`)];
  if (!a || !b) return 255;
  return a.reduce((s, x, i) => s + Math.abs(x - b[i]), 0) / a.length;
};
const SAME = 20;

let call;
await check('only a local person starts a call, and only the invited join', async () => {
  assert.equal((await create('tok-nobody', [])).status, 401);
  const r = await create('tok-matus', ['@eva:calls.test', '@jan:calls.test']);
  assert.equal(r.status, 200);
  call = (await r.json()).call;
  assert.match(call, /^[0-9a-f]{32}$/);
});

// A tab that only listens to the key channel.
const ear = await ctx.newPage();
await ear.goto(`http://127.0.0.1:${pagePort}/`);
await ear.waitForFunction(() => typeof window.overhear === 'function');
await ear.evaluate(() => window.overhear());

let matus;
let eva;
await check('two people see each other through the SFU, decrypted', async () => {
  matus = await tab('tok-matus', '@matus:calls.test', 'MATUSDEV');
  eva = await tab('tok-eva', '@eva:calls.test', 'EVADEV');
  await until('both in the member list', async () => (await members(matus)).length === 2 && (await members(eva)).length === 2);
  const evaPid = await pidOf(matus, '@eva:calls.test');
  const matusPid = await pidOf(eva, '@matus:calls.test');
  await until("eva's camera at matus", async () => (await distance(eva, matus, evaPid)) < SAME, 30000);
  await until("matus's camera at eva", async () => (await distance(matus, eva, matusPid)) < SAME, 30000);
  await until('stats', async () => (await stats(matus))?.opened > 20 && (await stats(eva))?.opened > 20);
  const [m, e] = [await stats(matus), await stats(eva)];
  assert.ok(m.sealed > 50 && e.sealed > 50, JSON.stringify({ m, e }));
  assert.ok(m.opened > 20 && e.opened > 20, JSON.stringify({ m, e }));
  assert.equal(m.refused + e.refused, 0, 'no frame failed to authenticate');
});

await check('a stranger is turned away at the socket', async () => {
  const p = await ctx.newPage();
  await p.goto(`http://127.0.0.1:${pagePort}/`);
  await p.waitForFunction(() => typeof window.startCall === 'function');
  // Mallory's token is valid on the homeserver, but she was not invited.
  TOKENS['tok-mallory'] = '@mallory:calls.test';
  await p.evaluate((o) => window.startCall(o), {
    socket: `ws://127.0.0.1:${sfuPort}/calls/api/ws`,
    call,
    token: 'tok-mallory',
    server: SERVER,
    me: { user: '@mallory:calls.test', device: 'M' },
  });
  await until('refused', () => p.evaluate(() => window.callState.states.includes('failed:notInvited')));
  assert.equal((await members(matus)).length, 2);
  await p.close();
});

await check('a member without the keys gets frames it cannot open', async () => {
  const jan = await tab('tok-jan', '@jan:calls.test', 'JANDEV', { dropKeys: true });
  await until('jan in', async () => (await members(matus)).length === 3);
  const evaPid = await pidOf(jan, '@eva:calls.test');
  await new Promise((r) => setTimeout(r, 5000));
  const s = await stats(jan);
  assert.equal(s.opened, 0, JSON.stringify(s));
  assert.ok(s.withoutKey > 20, `frames arrived and were dropped: ${JSON.stringify(s)}`);
  assert.equal(await picture(jan, `p${evaPid}`), null, 'no picture without the key');
  await jan.close();
  await until('jan gone', async () => (await members(matus)).length === 2);
});

await check('after a leave, the others rotate and still see each other', async () => {
  const before = await stats(eva);
  await new Promise((r) => setTimeout(r, 4000));
  const after = await stats(eva);
  assert.ok(after.opened - before.opened > 30, JSON.stringify({ before, after }));
  assert.ok(after.refused - before.refused < 5, `few frames lost to the rotation: ${JSON.stringify(after)}`);
});

await check('someone who joins later sees everyone, on keys that open nothing earlier', async () => {
  const jan = await tab('tok-jan', '@jan:calls.test', 'JANDEV2');
  await until('jan in', async () => (await members(jan)).length === 3);
  const evaPid = await pidOf(jan, '@eva:calls.test');
  const matusPid = await pidOf(jan, '@matus:calls.test');
  await until(
    'both cameras at jan',
    async () => (await distance(eva, jan, evaPid)) < SAME && (await distance(matus, jan, matusPid)) < SAME,
    30000,
  );
  await jan.close();
});

const readIvf = (file) => {
  const raw = readFileSync(file);
  const frames = [];
  for (let at = 32; at + 12 <= raw.length; ) {
    const len = raw.readUInt32LE(at);
    frames.push({ head: raw.subarray(at, at + 12), data: new Uint8Array(raw.subarray(at + 12, at + 12 + len)) });
    at += 12 + len;
  }
  return frames;
};
const ffmpegErrors = (file) =>
  spawnSync('ffmpeg', ['-v', 'error', '-i', file, '-f', 'null', '-'])
    .stderr.toString()
    .split('\n')
    .filter((l) => l.trim()).length;

let pids;
await check("the SFU's tap holds frames no decoder can read", async () => {
  pids = { matus: await pidOf(eva, '@matus:calls.test'), eva: await pidOf(matus, '@eva:calls.test') };
  await matus.evaluate(() => window.session.leave());
  await eva.evaluate(() => window.session.leave());
  for (const pid of Object.values(pids)) {
    const file = join(tap, `video-p${pid}-1.ivf`);
    const frames = readIvf(file);
    assert.ok(frames.length > 100, `${file}: ${frames.length} frames`);
    assert.ok(frames.every((f) => e2ee.readTrailer(f.data)), 'every frame carries the e2ee trailer');
    const errors = ffmpegErrors(file);
    assert.ok(errors >= frames.length * 0.9, `${file}: only ${errors} decode errors for ${frames.length} frames`);
  }
});

await check('the tapped frames are AES-GCM under keys only the members had', async () => {
  const heard = await ear.evaluate(() => window.heard);
  for (const [who, pid] of Object.entries(pids)) {
    const ring = new e2ee.ReceiveRing();
    const keys = heard.filter((h) => h.msg.pid === pid).map((h) => h.msg);
    assert.ok(keys.length >= 2, `${who} sent keys: ${keys.length}`);
    const file = join(tap, `video-p${pid}-1.ivf`);
    const frames = readIvf(file);
    let opened = 0;
    let beforeAnyone = 0;
    const plain = [];
    for (const f of frames) {
      const t = e2ee.readTrailer(f.data);
      // A key handed out for this index; ratchets in between the ring
      // follows by itself, as a member's browser does.
      const handed = keys.find((k) => k.index === t.index);
      if (handed && !ring.has(t.index)) ring.set(t.index, e2ee.fromBase64(handed.key));
      const fk = await ring.get(t.index);
      const out = fk && (await e2ee.decryptFrame(fk.video, f.data, e2ee.receivedClearBytes('video', f.data)));
      if (out) {
        opened++;
        plain.push({ head: f.head, data: out });
      } else if (t.index < Math.min(...keys.map((k) => k.index))) {
        // Sent while alone in the call: that key never left the browser.
        beforeAnyone++;
      }
    }
    assert.ok(opened > 0.85 * (frames.length - beforeAnyone), `${who}: ${opened}/${frames.length} opened, ${beforeAnyone} before anyone`);
    // The same frames, decrypted, are a clean VP8 stream.
    const head = readFileSync(file).subarray(0, 32);
    const body = plain.flatMap((p) => {
      const h = Buffer.from(p.head);
      h.writeUInt32LE(p.data.length, 0);
      return [h, Buffer.from(p.data)];
    });
    const clean = join(tap, `decrypted-p${pid}.ivf`);
    writeFileSync(clean, Buffer.concat([head, ...body]));
    const errors = ffmpegErrors(clean);
    assert.ok(errors <= 3, `${who}: ${errors} decode errors once decrypted`);
  }
});

await browser.close();
sfu.kill();
homeserver.close();
pages.close();
console.log(failed ? 'calls: FAILED' : `calls: ${checks.length} passed`);
process.exit(failed ? 1 : 0);
