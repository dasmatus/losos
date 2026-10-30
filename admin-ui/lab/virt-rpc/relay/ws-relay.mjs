#!/usr/bin/env bun
// A byte relay between WebSockets and libvirt's unix socket, with no
// dependencies (Bun's own server does the WebSocket framing). It knows
// nothing of libvirt: every binary message from the page is written to the socket as
// is, every read from the socket goes back as one binary message. All of
// libvirt's protocol lives in the page (losos-lab-virt, compiled to wasm).
//
//   LOSOS_VIRT_SOCKET=/run/libvirt/virtqemud-sock \
//   LOSOS_VIRT_ORIGINS=https://lab.example,http://127.0.0.1:8095 \
//   LOSOS_VIRT_TOKEN=$(head -c16 /dev/urandom | xxd -p) \
//   bun ws-relay.mjs --port 8095 [--static DIR]
//
// Whoever reaches /virt gets the libvirt rights of the user this relay runs
// as (libvirt sees the relay's uid on the socket, never the page's). For
// qemu:///system that is root on the host. Hence: loopback only, an Origin
// allowlist (a page on any other site could otherwise open
// ws://127.0.0.1:8095 from the owner's own browser), and a token.

const arg = (name, dflt) => {
  const i = Bun.argv.indexOf(name);
  return i > 0 ? Bun.argv[i + 1] : dflt;
};
const PORT = Number(arg('--port', '8095'));
const HOST = arg('--host', '127.0.0.1');
const STATIC = arg('--static', null) && new URL(arg('--static'), `file://${Bun.env.PWD}/`).pathname.replace(/\/$/, '');
const SOCKET = Bun.env.LOSOS_VIRT_SOCKET || '/run/libvirt/virtqemud-sock';
const TOKEN = Bun.env.LOSOS_VIRT_TOKEN || '';
const ORIGINS = new Set((Bun.env.LOSOS_VIRT_ORIGINS || `http://${HOST}:${PORT},http://localhost:${PORT}`).split(',').map((s) => s.trim()).filter(Boolean));
const MAX_FRAME = 33554432 + 4; // libvirt's VIR_NET_MESSAGE_MAX plus the length word

const TYPES = { '.html': 'text/html; charset=utf-8', '.js': 'text/javascript', '.mjs': 'text/javascript', '.wasm': 'application/wasm', '.json': 'application/json', '.css': 'text/css', '.png': 'image/png', '.webmanifest': 'application/manifest+json' };

async function serveStatic(url) {
  if (!STATIC) return new Response(null, { status: 404 });
  // URL parsing has already folded every `..`, so the path stays under STATIC.
  let file = `${STATIC}${decodeURIComponent(url.pathname)}`;
  if (file.endsWith('/')) file += 'index.html';
  const body = Bun.file(file);
  if (!(await body.exists())) return new Response(null, { status: 404 });
  return new Response(body, {
    headers: {
      'content-type': TYPES[/\.[^./]*$/.exec(file)?.[0]] || 'application/octet-stream',
      // The hosted Lab is cross-origin isolated; serve the test page the same way.
      'cross-origin-opener-policy': 'same-origin',
      'cross-origin-embedder-policy': 'require-corp',
      'cross-origin-resource-policy': 'same-origin',
      'cache-control': 'no-store',
    },
  });
}

/* Writes what the page sent, in order, as far as the daemon's socket takes it; the rest waits for its drain. */
function flush(ws, socket) {
  const queue = ws.data.queue;
  while (queue.length > 0) {
    const wrote = socket.write(queue[0]);
    if (wrote < queue[0].length) {
      queue[0] = queue[0].subarray(Math.max(wrote, 0));
      return;
    }
    queue.shift();
  }
}

Bun.serve({
  hostname: HOST,
  port: PORT,
  fetch(req, server) {
    const url = new URL(req.url);
    if (req.headers.get('upgrade')?.toLowerCase() !== 'websocket') return serveStatic(url);
    const origin = req.headers.get('origin') || '';
    const refuse = (status, why) => {
      console.warn(`refused ${server.requestIP(req)?.address} ${origin}: ${why}`);
      return new Response(why, { status });
    };
    if (url.pathname !== '/virt') return refuse(404, 'Not Found');
    if (!ORIGINS.has(origin)) return refuse(403, 'Origin not allowed');
    if (TOKEN && url.searchParams.get('token') !== TOKEN) return refuse(403, 'Bad token');
    if (!server.upgrade(req, { data: { origin } })) return refuse(400, 'Bad Request');
  },
  websocket: {
    maxPayloadLength: MAX_FRAME,
    async open(ws) {
      console.log(`relay ${ws.data.origin} -> ${SOCKET}`);
      ws.data.queue = [];
      try {
        ws.data.daemon = await Bun.connect({
          unix: SOCKET,
          socket: {
            data(_, bytes) { ws.sendBinary(bytes); },
            drain(socket) { flush(ws, socket); },
            error(_, e) { console.warn('daemon socket:', e.message); ws.close(); },
            close() { ws.close(); },
          },
        });
      } catch (e) {
        console.warn('daemon socket:', e.message);
        ws.close();
        return;
      }
      flush(ws, ws.data.daemon);
    },
    message(ws, message) {
      // Text is not ours; close.
      if (typeof message === 'string') return ws.close();
      ws.data.queue.push(message);
      if (ws.data.daemon) flush(ws, ws.data.daemon);
    },
    close(ws) { ws.data.daemon?.end(); },
  },
});
console.log(`ws-relay on http://${HOST}:${PORT}/virt -> ${SOCKET}; origins ${[...ORIGINS].join(' ')}${TOKEN ? '; token required' : ''}`);
