/* A call between Chromium tabs through the real SFU (losos-calls), with the
 * end-to-end encryption on: both see each other, the SFU's tap holds only
 * frames no decoder can read, a tab without the keys sees nothing, and a
 * tab that joins later gets in on a fresh key.
 *
 *   cargo build --manifest-path ../../backend/Cargo.toml --bin losos-calls
 *   LOSOS_CALLS_BIN=../../backend/target/debug/losos-calls node tests/calls.browser.mjs
 *
 * LOSOS_CALLS_ADDRESS is the address the SFU offers media on (default: the
 * first non-loopback IPv4 address, else loopback, which Chromium also
 * takes); LOSOS_CHROMIUM a chromium binary.
 */

import assert from 'node:assert/strict';
import { spawn, spawnSync } from 'node:child_process';
import { createServer } from 'node:http';
import { randomBytes } from 'node:crypto';
import { mkdtempSync, readdirSync, readFileSync, writeFileSync } from 'node:fs';
import { networkInterfaces, tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import * as esbuild from 'esbuild';
import { launch } from './harness.mjs';

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

// An edge that makes guest links, as an official one does, and keeps what
// it was asked.
const asked = [];
const edge = createServer((req, res) => {
  let body = '';
  req.on('data', (c) => (body += c));
  req.on('end', () => {
    if (req.method !== 'POST' || req.url !== '/calls/guest-link') {
      res.writeHead(404).end();
      return;
    }
    const b = JSON.parse(body);
    asked.push(b);
    res.writeHead(200, { 'Content-Type': 'application/json' });
    res.end(JSON.stringify({ url: `https://box.calls.test/calls/guest/?call=${b.call}&invite=${b.invite}` }));
  });
});
const edgePort = await listen(edge);

// The SFU, on a free port of its own.
const probe = createServer();
const sfuPort = await listen(probe);
probe.close();
const udpPort = 20000 + Math.floor(Math.random() * 20000);
const tap = mkdtempSync(join(tmpdir(), 'losos-calls-tap-'));
const relayToken = join(mkdtempSync(join(tmpdir(), 'losos-calls-edge-')), 'token');
writeFileSync(relayToken, 'box-token\n');
const sfu = spawn(
  BIN,
  ['--listen', `127.0.0.1:${sfuPort}`, '--udp-port', String(udpPort), '--address', ADDRESS, '--server-name', SERVER, '--homeserver', `127.0.0.1:${hsPort}`, '--tap', tap, '--ffmpeg', process.env.LOSOS_FFMPEG ?? 'ffmpeg', '--registrar', `http://127.0.0.1:${edgePort}`, '--relay-id', 'box-1', '--relay-token-file', relayToken],
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

const browser = await launch({
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
const guestTab = async (invite, name, secret) => {
  const p = await ctx.newPage();
  p.on('pageerror', (e) => console.log(`[${name}] ${e}`));
  await p.goto(`http://127.0.0.1:${pagePort}/`);
  await p.waitForFunction(() => typeof window.startCall === 'function');
  await p.evaluate((o) => window.startCall(o), {
    socket: `ws://127.0.0.1:${sfuPort}/calls/guest/ws`,
    call,
    guest: { invite, name, secret },
  });
  return p;
};
const secret = () => randomBytes(32).toString('base64url');
const makeInvite = async (token, p) => {
  const r = await fetch(`${base}/invites`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      openid: { access_token: token, matrix_server_name: SERVER },
      call,
      pid: await p.evaluate(() => window.session.pid),
    }),
  });
  return { status: r.status, body: await r.json().catch(() => ({})) };
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

await check('a shared screen reaches the others as a track of its own, beside the camera', async () => {
  const matusPid = await pidOf(eva, '@matus:calls.test');
  await matus.evaluate(() => window.share('#c0187a'));
  const screenAt = async () => {
    const [a, b] = [await picture(matus, 'local-screen'), await picture(eva, `p${matusPid}-screen`)];
    if (!a || !b) return 255;
    return a.reduce((s, x, i) => s + Math.abs(x - b[i]), 0) / a.length;
  };
  await until("matus's screen at eva", async () => (await screenAt()) < SAME, 30000);
  // The camera keeps coming while the screen is shared.
  await until("matus's camera still at eva", async () => (await distance(matus, eva, matusPid)) < SAME, 15000);
  assert.equal((await stats(eva)).refused, 0, 'the screen opens with the same keys');
});

const rtmpPort = 20000 + Math.floor(Math.random() * 20000);
await check('podcast mode streams the call to an RTMP address, and every member sees it is live', async () => {
  const evaPid = await pidOf(matus, '@eva:calls.test');
  const flv = join(tap, 'podcast.flv');
  // An RTMP server that keeps six seconds of what it gets.
  const server = spawn('ffmpeg', ['-v', 'error', '-listen', '1', '-i', `rtmp://127.0.0.1:${rtmpPort}/live/key-123`, '-t', '6', '-c', 'copy', '-y', flv], {
    stdio: ['ignore', 'inherit', 'inherit'],
  });
  const exited = new Promise((ok) => server.on('exit', ok));
  await new Promise((r) => setTimeout(r, 1000));
  await eva.evaluate((url) => window.goLive(url, 'The LosOS podcast'), `rtmp://127.0.0.1:${rtmpPort}/live/key-123`);
  await until('eva live', () => eva.evaluate(() => window.callState.podcast.includes('live')));
  await until('matus sees eva live', () => matus.evaluate((pid) => window.callState.live === pid, evaPid));
  // One stream per call.
  await matus.evaluate((url) => window.goLive(url, 'second'), `rtmp://127.0.0.1:${rtmpPort + 1}/live/x`);
  await until('a second stream is refused', () => matus.evaluate(() => window.callState.podcast.includes('ended:alreadyLive')));
  await Promise.race([exited, new Promise((_, no) => setTimeout(() => no(new Error('the RTMP server got no six seconds')), 60000))]);
  const probe = JSON.parse(
    spawnSync('ffprobe', ['-v', 'error', '-show_entries', 'stream=codec_name,width,height:format=duration', '-of', 'json', flv]).stdout.toString(),
  );
  const codecs = probe.streams.map((x) => x.codec_name).sort();
  assert.deepEqual(codecs, ['aac', 'h264'], JSON.stringify(probe));
  assert.equal(probe.streams.find((x) => x.codec_name === 'h264').width, 1280);
  assert.ok(Number(probe.format.duration) >= 5, JSON.stringify(probe.format));
  await eva.evaluate(() => window.podcast.stop());
  await until('live no more', () => matus.evaluate(() => window.callState.live === null));
});

await check('an address that is not RTMP is refused before ffmpeg runs', async () => {
  await matus.evaluate(() => {
    window.callState.podcast = [];
    window.goLive('file:///etc/passwd', 'x');
  });
  await until('refused', () => matus.evaluate(() => window.callState.podcast.includes('ended:badAddress')));
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

let zoe;
let zoePid;
const matusAt = await pidOf(eva, '@matus:calls.test');
const evaAt = await pidOf(matus, '@eva:calls.test');
await check('a guest link comes from the edge, and a guest holding its secret sees and is seen', async () => {
  const made = await makeInvite('tok-matus', matus);
  assert.equal(made.status, 200, JSON.stringify(made.body));
  const url = new URL(made.body.url);
  const invite = url.searchParams.get('invite');
  assert.match(invite, /^[0-9a-f]{32}$/);
  assert.equal(url.searchParams.get('call'), call);
  const ask = asked.at(-1);
  assert.deepEqual(ask, { appliance_id: 'box-1', token: 'box-token', call, invite }, 'the edge is asked for this call only');
  // A stranger's token makes no link.
  assert.equal((await makeInvite('tok-nobody', matus)).status, 401);
  const s = secret();
  // Matus keeps the secret and hands it to Eva over the key channel.
  await matus.evaluate((s) => window.addGuestSecret(s), s);
  zoe = await guestTab(invite, 'Zoe', s);
  await until('zoe in the call', async () => (await members(zoe)).length === 3 && (await members(matus)).length === 3);
  const listed = (await members(matus)).find((m) => m.guest === 'Zoe');
  assert.ok(listed, 'the members see the guest by the name she gave');
  zoePid = listed.pid;
  const evaPid = await pidOf(zoe, '@eva:calls.test');
  const matusPid = await pidOf(zoe, '@matus:calls.test');
  await until(
    'both cameras at zoe',
    async () => (await distance(eva, zoe, evaPid)) < SAME && (await distance(matus, zoe, matusPid)) < SAME,
    30000,
  );
  await until(
    "zoe's camera at both",
    async () => (await distance(zoe, eva, zoePid)) < SAME && (await distance(zoe, matus, zoePid)) < SAME,
    30000,
  );
  assert.equal(await zoe.evaluate(() => window.callState.keysIn), 0, 'no key reached the guest over Olm');
  // The same invite lets in one guest at a time.
  const twin = await guestTab(invite, 'Zoe again', s);
  await until('the twin refused', async () => (await twin.evaluate(() => window.callState.states)).some((x) => x.startsWith('failed:')));
  assert.deepEqual((await twin.evaluate(() => window.callState.states)).at(-1), 'failed:inviteInUse');
  await twin.close();
});

await check('a guest without the secret gets in through the box but sees and shows nothing', async () => {
  const made = await makeInvite('tok-eva', eva);
  assert.equal(made.status, 200);
  const invite = new URL(made.body.url).searchParams.get('invite');
  // A link with someone else's secret after the `#`: what the box and the
  // edge could make on their own.
  const mallory = await guestTab(invite, 'Mallory', secret());
  await until('mallory in the call', async () => (await members(mallory)).length === 4);
  const malloryPid = (await members(matus)).find((m) => m.guest === 'Mallory').pid;
  const matusPid = await pidOf(mallory, '@matus:calls.test');
  await new Promise((r) => setTimeout(r, 6000));
  assert.ok((await distance(matus, mallory, matusPid)) >= SAME, "matus's picture stays sealed at mallory");
  const st = await stats(mallory);
  assert.equal(st?.opened ?? 0, 0, `mallory opened nothing: ${JSON.stringify(st)}`);
  assert.ok((await distance(mallory, matus, malloryPid)) >= SAME, "mallory's picture does not open at matus");
  await mallory.close();
  await until('mallory gone', async () => (await members(matus)).length === 3);
});

await check('guests are sent away once the last of the room has left', async () => {
  await eva.evaluate(() => window.session.leave());
  await until('eva gone', async () => (await members(zoe)).length === 2);
  // Zoe and Matus still see each other on the key rotated after Eva left.
  const matusPid = await pidOf(zoe, '@matus:calls.test');
  await until('matus at zoe after the rotation', async () => (await distance(matus, zoe, matusPid)) < SAME, 20000);
  await matus.evaluate(() => window.session.leave());
  await until('zoe sent away', async () => (await zoe.evaluate(() => window.callState.states)).some((x) => x.startsWith('failed:')));
  assert.equal((await zoe.evaluate(() => window.callState.states)).at(-1), 'failed:ended');
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
    // Two frames the tap stamped with the same millisecond are a muxer
    // warning, not a frame the decoder could not read.
    .filter((l) => l.trim() && !l.includes('non monotonically increasing dts')).length;

let pids;
await check("the SFU's tap holds frames no decoder can read", async () => {
  pids = { matus: matusAt, eva: evaAt };
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
    // Once Eva left, Matus's next key went to the guest alone, over their
    // pair key: the key channel never carried it.
    let guestsOnly = 0;
    const lastHeard = Math.max(...keys.map((k) => k.index));
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
      } else if (t.index > lastHeard) {
        guestsOnly++;
      }
    }
    assert.ok(
      opened > 0.85 * (frames.length - beforeAnyone - guestsOnly),
      `${who}: ${opened}/${frames.length} opened, ${beforeAnyone} before anyone, ${guestsOnly} to guests only`,
    );
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
edge.close();
homeserver.close();
pages.close();
console.log(failed ? 'calls: FAILED' : `calls: ${checks.length} passed`);
process.exit(failed ? 1 : 0);
