/* A call between Chromium tabs through the real SFU (losos-calls), with the
 * end-to-end encryption on: both see each other, the SFU's tap holds only
 * frames no decoder can read, a tab without the keys sees nothing, and a
 * tab that joins later gets in on a fresh key.
 *
 *   cargo build --manifest-path ../../backend/Cargo.toml --bin losos-calls
 *   LOSOS_CALLS_BIN=../../backend/target/debug/losos-calls bun tests/calls.browser.mjs
 *
 * LOSOS_CALLS_ADDRESS is the address the SFU offers media on (default: the
 * first non-loopback IPv4 address, else loopback, which Chromium also
 * takes); LOSOS_CHROMIUM a chromium binary.
 */

import assert from './assert.mjs';
import { launch } from './harness.mjs';

const here = (p) => `${import.meta.dir}/${p}`;
const BIN = Bun.env.LOSOS_CALLS_BIN ?? here('../../../backend/target/debug/losos-calls');
const SERVER = 'calls.test';
// Chromium takes a loopback candidate under --allow-loopback-in-peer-connection.
const ADDRESS = Bun.env.LOSOS_CALLS_ADDRESS ?? '127.0.0.1';
const TMP = Bun.env.TMPDIR ?? '/tmp';

const bundle = async (entry) => {
  const built = await Bun.build({
    entrypoints: [entry],
    target: 'browser',
    format: 'esm',
    // src/calls/background.ts reads Vite's base path.
    define: { 'import.meta.env.BASE_URL': '"/"' },
  });
  if (!built.success) throw new AggregateError(built.logs, `bundling ${entry} failed`);
  return built.outputs[0].text();
};

const harness = await bundle(here('fixtures/calls/harness.ts'));
const e2ee = await import('../src/calls/e2ee.ts');
const worker = await bundle(here('../src/calls/e2ee-worker.ts'));

const pages = Bun.serve({
  hostname: '127.0.0.1',
  port: 0,
  fetch(req) {
    const path = new URL(req.url).pathname;
    const body = { '/': '<!doctype html><title>call</title><script type="module" src="/harness.js"></script>', '/harness.js': harness, '/e2ee-worker.js': worker }[path];
    if (body === undefined) return new Response(null, { status: 404 });
    return new Response(body, { headers: { 'Content-Type': path === '/' ? 'text/html' : 'text/javascript' } });
  },
});
const pagePort = pages.port;

// An edge that makes guest links, as an official one does, and keeps what
// it was asked.
const asked = [];
const edge = Bun.serve({
  hostname: '127.0.0.1',
  port: 0,
  async fetch(req) {
    if (req.method !== 'POST' || new URL(req.url).pathname !== '/calls/guest-link') return new Response(null, { status: 404 });
    const b = await req.json();
    asked.push(b);
    return Response.json({ url: `https://box.calls.test/calls/guest/?call=${b.call}&invite=${b.invite}` });
  },
});
const edgePort = edge.port;

// The SFU, on a free port of its own.
const probe = Bun.serve({ hostname: '127.0.0.1', port: 0, fetch: () => new Response() });
const sfuPort = probe.port;
probe.stop(true);
const udpPort = 20000 + Math.floor(Math.random() * 20000);
const tap = `${TMP}/losos-calls-tap-${crypto.randomUUID()}`;
await Bun.$`mkdir -p ${tap}`;
const relayToken = `${TMP}/losos-calls-edge-${crypto.randomUUID()}/token`;
await Bun.write(relayToken, 'box-token\n');
const sfu = Bun.spawn(
  [BIN, '--listen', `127.0.0.1:${sfuPort}`, '--udp-port', String(udpPort), '--address', ADDRESS, '--server-name', SERVER, '--tap', tap, '--ffmpeg', Bun.env.LOSOS_FFMPEG ?? 'ffmpeg', '--registrar', `http://127.0.0.1:${edgePort}`, '--relay-id', 'box-1', '--relay-token-file', relayToken],
  { stdio: ['ignore', 'inherit', 'inherit'] },
);
for (let i = 0; ; i++) {
  try {
    if ((await fetch(`http://127.0.0.1:${sfuPort}/calls/api/health`)).ok) break;
  } catch {}
  if (i > 100) throw new Error('losos-calls did not start');
  await new Promise((r) => setTimeout(r, 100));
}

/* nginx in front of the SFU, as modules/calls.nix has it: it names the
 * person LosOS Login signed in as Remote-User and drops any the client sent.
 * Here a path `/as/<name>/` stands in for that sign-in; anything else, a
 * guest's socket among it, reaches the SFU with nobody named. A socket is
 * passed on message by message, the way nginx passes its frames. */
const forward = (req) => {
  const url = new URL(req.url);
  const m = /^\/as\/([^/]+)(\/.*)$/.exec(url.pathname);
  const headers = new Headers(req.headers);
  headers.delete('remote-user');
  if (m) headers.set('remote-user', decodeURIComponent(m[1]));
  return { path: (m ? m[2] : url.pathname) + url.search, headers };
};
const nginx = Bun.serve({
  hostname: '127.0.0.1',
  port: 0,
  async fetch(req, server) {
    const { path, headers } = forward(req);
    if (req.headers.get('upgrade')?.toLowerCase() === 'websocket') {
      for (const h of ['upgrade', 'connection', 'sec-websocket-key', 'sec-websocket-version', 'sec-websocket-extensions', 'host']) headers.delete(h);
      const protocols = req.headers.get('sec-websocket-protocol')?.split(',').map((p) => p.trim());
      headers.delete('sec-websocket-protocol');
      return server.upgrade(req, { data: { url: `ws://127.0.0.1:${sfuPort}${path}`, headers: Object.fromEntries(headers), protocols } })
        ? undefined
        : new Response('upgrade failed', { status: 400 });
    }
    headers.delete('host');
    try {
      return await fetch(`http://127.0.0.1:${sfuPort}${path}`, {
        method: req.method,
        headers,
        body: ['GET', 'HEAD'].includes(req.method) ? undefined : await req.arrayBuffer(),
        redirect: 'manual',
      });
    } catch {
      return new Response('bad gateway', { status: 502 });
    }
  },
  websocket: {
    open(ws) {
      const { url, headers, protocols } = ws.data;
      const up = new WebSocket(url, { headers, protocols });
      up.binaryType = 'arraybuffer';
      ws.data.up = up;
      ws.data.queue = [];
      up.onopen = () => {
        for (const m of ws.data.queue.splice(0)) up.send(m);
      };
      up.onmessage = (e) => ws.send(e.data);
      up.onclose = (e) => ws.close(e.code === 1005 ? 1000 : e.code, e.reason);
      up.onerror = () => ws.close(1011);
    },
    message(ws, m) {
      if (ws.data.up.readyState === WebSocket.OPEN) ws.data.up.send(m);
      else ws.data.queue.push(m);
    },
    close(ws) {
      ws.data.up?.close();
    },
  },
});
const front = `127.0.0.1:${nginx.port}`;
const api = (name) => `http://${front}${name ? `/as/${name}` : ''}/calls/api`;

const browser = await launch({
  args: ['--use-fake-ui-for-media-stream', '--use-fake-device-for-media-stream', '--autoplay-policy=no-user-gesture-required', '--allow-loopback-in-peer-connection'],
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

const create = async (name, invited, room = '!room:calls.test') =>
  fetch(`${api(name)}/calls`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ room, invited, video: true }),
  });

/* Chromium lists the machine's interfaces to WebRTC only for a page whose
 * camera or microphone permission reads granted; otherwise it gathers on
 * the default route alone. The fake-UI flag answers getUserMedia's prompt
 * but leaves that status unasked in the headless shell (what Playwright
 * launches, and the nix check runs), and the build sandbox has loopback
 * and no route: without the grant no candidate is gathered, no ICE check
 * reaches the SFU and every DTLS handshake times out. */
const ctx = await browser.newContext({ permissions: ['camera', 'microphone'] });
const tab = async (name, user, device, extra = {}) => {
  const p = await ctx.newPage();
  p.on('pageerror', (e) => console.log(`[${user}] ${e}`));
  await p.goto(`http://127.0.0.1:${pagePort}/`);
  await p.waitForFunction(() => typeof window.startCall === 'function');
  await p.evaluate(
    (o) => window.startCall(o),
    { socket: `ws://${front}/as/${name}/calls/api/ws`, call, me: { user, device }, ...extra },
  );
  return p;
};
const guestTab = async (invite, name, secret) => {
  const p = await ctx.newPage();
  p.on('pageerror', (e) => console.log(`[${name}] ${e}`));
  await p.goto(`http://127.0.0.1:${pagePort}/`);
  await p.waitForFunction(() => typeof window.startCall === 'function');
  await p.evaluate((o) => window.startCall(o), {
    socket: `ws://${front}/calls/guest/ws`,
    call,
    guest: { invite, name, secret },
  });
  return p;
};
const secret = () => crypto.getRandomValues(new Uint8Array(32)).toBase64({ alphabet: 'base64url', omitPadding: true });
const makeInvite = async (name, p) => {
  const r = await fetch(`${api(name)}/invites`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
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
  assert.equal((await create(null, [])).status, 401);
  const r = await create('matus', ['@eva:calls.test', '@jan:calls.test']);
  assert.equal(r.status, 200);
  const made = await r.json();
  call = made.call;
  assert.match(call, /^[0-9a-f]{32}$/);
  assert.equal(made.created, true);
  // The room has one call at a time: Eva starting one too lands in it.
  const again = await (await create('eva', ['@matus:calls.test'])).json();
  assert.deepEqual(again, { call, created: false });
});

// A tab that only listens to the key channel.
const ear = await ctx.newPage();
await ear.goto(`http://127.0.0.1:${pagePort}/`);
await ear.waitForFunction(() => typeof window.overhear === 'function');
await ear.evaluate(() => window.overhear());

let matus;
let eva;
await check('two people see each other through the SFU, decrypted', async () => {
  matus = await tab('matus', '@matus:calls.test', 'MATUSDEV');
  eva = await tab('eva', '@eva:calls.test', 'EVADEV');
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

await check('Opus sends at 128 kb/s, and the camera keeps its frame rate before its resolution', async () => {
  const sent = await matus.evaluate(async () => {
    const pc = window.connection;
    const [audio, camera, screen] = pc.getTransceivers().map((t) => t.sender.getParameters());
    let target = 0;
    let fps = 0;
    for (let i = 0; i < 50 && (target === 0 || fps === 0); i++) {
      for (const r of (await pc.getStats()).values()) {
        if (r.type === 'outbound-rtp' && r.kind === 'audio') target = r.targetBitrate ?? 0;
        if (r.type === 'outbound-rtp' && r.kind === 'video' && r.framesPerSecond) fps = Math.max(fps, r.framesPerSecond);
      }
      await new Promise((ok) => setTimeout(ok, 200));
    }
    return {
      audioMax: audio.encodings[0]?.maxBitrate,
      cameraMax: camera.encodings[0]?.maxBitrate,
      camera: camera.degradationPreference,
      screen: screen.degradationPreference,
      target,
      fps,
    };
  });
  assert.equal(sent.audioMax, 128000, JSON.stringify(sent));
  assert.equal(sent.cameraMax, 1000000, JSON.stringify(sent));
  assert.equal(sent.camera, 'maintain-framerate', JSON.stringify(sent));
  assert.equal(sent.screen, 'maintain-resolution', JSON.stringify(sent));
  // The encoder aims for the cap, not Opus's 32 kb/s default.
  assert.ok(sent.target >= 100000, JSON.stringify(sent));
  assert.ok(sent.fps >= 15, JSON.stringify(sent));
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
  const sharing = async () => (await members(eva)).find((m) => m.pid === matusPid)?.screen === true;
  assert.ok(await sharing(), 'the others are told matus is sharing');
  await matus.evaluate(() => window.unshare());
  await until('eva told the share stopped', async () => !(await sharing()), 15000);
});

const rtmpPort = 20000 + Math.floor(Math.random() * 20000);
await check('podcast mode streams the call to an RTMP address, and every member sees it is live', async () => {
  const evaPid = await pidOf(matus, '@eva:calls.test');
  const flv = `${tap}/podcast.flv`;
  // An RTMP server that keeps six seconds of what it gets.
  const server = Bun.spawn(['ffmpeg', '-v', 'error', '-listen', '1', '-i', `rtmp://127.0.0.1:${rtmpPort}/live/key-123`, '-t', '6', '-c', 'copy', '-y', flv], {
    stdio: ['ignore', 'inherit', 'inherit'],
  });
  const exited = server.exited;
  await new Promise((r) => setTimeout(r, 1000));
  await eva.evaluate((url) => window.goLive(url, 'The LosOS podcast'), `rtmp://127.0.0.1:${rtmpPort}/live/key-123`);
  await until('eva live', () => eva.evaluate(() => window.callState.podcast.includes('live')));
  await until('matus sees eva live', () => matus.evaluate((pid) => window.callState.live === pid, evaPid));
  // One stream per call.
  await matus.evaluate((url) => window.goLive(url, 'second'), `rtmp://127.0.0.1:${rtmpPort + 1}/live/x`);
  await until('a second stream is refused', () => matus.evaluate(() => window.callState.podcast.includes('ended:alreadyLive')));
  await Promise.race([exited, new Promise((_, no) => setTimeout(() => no(new Error('the RTMP server got no six seconds')), 60000))]);
  const probe = JSON.parse(
    new TextDecoder().decode(Bun.spawnSync(['ffprobe', '-v', 'error', '-show_entries', 'stream=codec_name,width,height:format=duration', '-of', 'json', flv]).stdout),
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
  // Mallory is signed in to the box, but she was not invited.
  await p.evaluate((o) => window.startCall(o), {
    socket: `ws://${front}/as/mallory/calls/api/ws`,
    call,
    me: { user: '@mallory:calls.test', device: 'M' },
  });
  await until('refused', () => p.evaluate(() => window.callState.states.includes('failed:notInvited')));
  assert.equal((await members(matus)).length, 2);
  await p.close();
});

await check('a member without the keys gets frames it cannot open', async () => {
  const jan = await tab('jan', '@jan:calls.test', 'JANDEV', { dropKeys: true });
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
  const jan = await tab('jan', '@jan:calls.test', 'JANDEV2');
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
  const made = await makeInvite('matus', matus);
  assert.equal(made.status, 200, JSON.stringify(made.body));
  const url = new URL(made.body.url);
  const invite = url.searchParams.get('invite');
  assert.match(invite, /^[0-9a-f]{32}$/);
  assert.equal(url.searchParams.get('call'), call);
  const ask = asked.at(-1);
  assert.deepEqual(ask, { appliance_id: 'box-1', token: 'box-token', call, invite }, 'the edge is asked for this call only');
  // Nobody signed in makes no link, and someone not in the call neither.
  assert.equal((await makeInvite(null, matus)).status, 401);
  assert.equal((await makeInvite('mallory', matus)).status, 403);
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
  const made = await makeInvite('eva', eva);
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
  // Matus is still there, so Eva's hang-up does not end the call.
  assert.equal(await eva.evaluate(() => window.session.leave()), false);
  await until('eva gone', async () => (await members(zoe)).length === 2);
  // Zoe and Matus still see each other on the key rotated after Eva left.
  const matusPid = await pidOf(zoe, '@matus:calls.test');
  await until('matus at zoe after the rotation', async () => (await distance(matus, zoe, matusPid)) < SAME, 20000);
  // Only a guest is left: Matus was the last of the room.
  assert.equal(await matus.evaluate(() => window.session.leave()), true);
  await until('zoe sent away', async () => (await zoe.evaluate(() => window.callState.states)).some((x) => x.startsWith('failed:')));
  assert.equal((await zoe.evaluate(() => window.callState.states)).at(-1), 'failed:ended');
});

const readIvf = async (file) => {
  const raw = await Bun.file(file).bytes();
  const view = new DataView(raw.buffer, raw.byteOffset, raw.byteLength);
  const frames = [];
  for (let at = 32; at + 12 <= raw.length; ) {
    const len = view.getUint32(at, true);
    frames.push({ head: raw.slice(at, at + 12), data: raw.slice(at + 12, at + 12 + len) });
    at += 12 + len;
  }
  return frames;
};
const ffmpegErrors = (file) =>
  new TextDecoder()
    .decode(Bun.spawnSync(['ffmpeg', '-v', 'error', '-i', file, '-f', 'null', '-']).stderr)
    .split('\n')
    // Two frames the tap stamped with the same millisecond are a muxer
    // warning, not a frame the decoder could not read.
    .filter((l) => l.trim() && !l.includes('non monotonically increasing dts')).length;

let pids;
await check("the SFU's tap holds frames no decoder can read", async () => {
  pids = { matus: matusAt, eva: evaAt };
  for (const pid of Object.values(pids)) {
    const file = `${tap}/video-p${pid}-1.ivf`;
    const frames = await readIvf(file);
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
    const file = `${tap}/video-p${pid}-1.ivf`;
    const frames = await readIvf(file);
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
    const head = (await Bun.file(file).bytes()).subarray(0, 32);
    const body = plain.flatMap((p) => {
      const h = p.head.slice();
      new DataView(h.buffer).setUint32(0, p.data.length, true);
      return [h, p.data];
    });
    const clean = `${tap}/decrypted-p${pid}.ivf`;
    await Bun.write(clean, new Blob([head, ...body]));
    const errors = ffmpegErrors(clean);
    assert.ok(errors <= 3, `${who}: ${errors} decode errors once decrypted`);
  }
});

await check('a call starts without a camera, or listening only without a microphone', async () => {
  const r = await create('matus', ['@eva:calls.test', '@jan:calls.test'], '!quiet:calls.test');
  call = (await r.json()).call;
  // Each tab's browser has no camera, or no device at all.
  const without = async (name, user, device, refuse) => {
    const p = await ctx.newPage();
    p.on('pageerror', (e) => console.log(`[${user}] ${e}`));
    await p.addInitScript((refuse) => {
      const open = navigator.mediaDevices.getUserMedia.bind(navigator.mediaDevices);
      navigator.mediaDevices.getUserMedia = (c) =>
        (refuse === 'all' || c?.video) ? Promise.reject(new DOMException('Requested device not found', 'NotFoundError')) : open(c);
    }, refuse);
    await p.goto(`http://127.0.0.1:${pagePort}/`);
    await p.waitForFunction(() => typeof window.startCall === 'function');
    await p.evaluate((o) => window.startCall(o), {
      socket: `ws://${front}/as/${name}/calls/api/ws`,
      call,
      me: { user, device },
      media: true,
      video: true,
    });
    return p;
  };
  const host = await tab('matus', '@matus:calls.test', 'MATUSDEV2', { media: true });
  const noCamera = await without('eva', '@eva:calls.test', 'EVADEV2', 'camera');
  const nothing = await without('jan', '@jan:calls.test', 'JANDEV2', 'all');
  const said = (p) => p.evaluate(() => ({ missing: window.media.missing, camera: window.media.cameraOn, mic: window.media.micOn }));
  assert.deepEqual(await said(noCamera), { missing: { mic: false, camera: true }, camera: false, mic: true });
  assert.deepEqual(await said(nothing), { missing: { mic: true, camera: true }, camera: false, mic: false });
  await until('all three in the call', async () => (await members(host)).length === 3);
  // The camera-less one is heard: their sound reaches the host, the only
  // sound it receives, as jan has no microphone.
  const heard = () =>
    host.evaluate(async () => {
      let packets = 0;
      for (const r of (await window.connection.getStats()).values()) {
        if (r.type === 'inbound-rtp' && r.kind === 'audio') packets += r.packetsReceived ?? 0;
      }
      return packets;
    });
  await until("eva's sound at matus", async () => (await heard()) > 100, 30000);
  for (const p of [host, noCamera, nothing]) await p.evaluate(() => window.session.leave());
});

await browser.close();
sfu.kill();
edge.stop(true);
nginx.stop(true);
pages.stop(true);
// Thrown, not printed: an uncaught error is what makes Bun exit non-zero.
if (failed) throw new Error('calls: FAILED');
console.log(`calls: ${checks.length} passed`);
