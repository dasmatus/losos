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
//   bun ws-relay.ts --port 8095 [--static DIR]
//
// Whoever reaches /virt gets the libvirt rights of the user this relay runs
// as (libvirt sees the relay's uid on the socket, never the page's). For
// qemu:///system that is root on the host. Hence: loopback only, an Origin
// allowlist (a page on any other site could otherwise open
// ws://127.0.0.1:8095 from the owner's own browser), and a token.

const arg = (name: string, dflt: string | null): string | null => {
  const i = Bun.argv.indexOf(name);
  return i > 0 ? Bun.argv[i + 1]! : dflt;
};
const PORT = Number(arg('--port', '8095'));
const HOST = arg('--host', '127.0.0.1')!;
const STATIC = arg('--static', null) && new URL(arg('--static', null)!, `file://${Bun.env.PWD}/`).pathname.replace(/\/$/, '');
const SOCKET = Bun.env.LOSOS_VIRT_SOCKET || '/run/libvirt/virtqemud-sock';
const TOKEN = Bun.env.LOSOS_VIRT_TOKEN || '';
const ORIGINS = new Set((Bun.env.LOSOS_VIRT_ORIGINS || `http://${HOST}:${PORT},http://localhost:${PORT}`).split(',').map((s) => s.trim()).filter(Boolean));
const MAX_FRAME = 33554432 + 4; // libvirt's VIR_NET_MESSAGE_MAX plus the length word

const TYPES: Record<string, string> = { '.html': 'text/html; charset=utf-8', '.js': 'text/javascript', '.mjs': 'text/javascript', '.wasm': 'application/wasm', '.json': 'application/json', '.css': 'text/css', '.png': 'image/png', '.webmanifest': 'application/manifest+json' };

async function serveStatic(url: URL): Promise<Response> {
  if (!STATIC) return new Response(null, { status: 404 });
  // URL parsing has already folded every `..`, so the path stays under STATIC.
  let file = `${STATIC}${decodeURIComponent(url.pathname)}`;
  if (file.endsWith('/')) file += 'index.html';
  const body = Bun.file(file);
  if (!(await body.exists())) return new Response(null, { status: 404 });
  return new Response(body, {
    headers: {
      'content-type': TYPES[/\.[^./]*$/.exec(file)?.[0] ?? ''] || 'application/octet-stream',
      // The hosted Lab is cross-origin isolated; serve the test page the same way.
      'cross-origin-opener-policy': 'same-origin',
      'cross-origin-embedder-policy': 'require-corp',
      'cross-origin-resource-policy': 'same-origin',
      'cache-control': 'no-store',
    },
  });
}

/* Writes what the page sent, in order, as far as the daemon's socket takes it; the rest waits for its drain. */
function flush(ws: Bun.ServerWebSocket<WsData>, socket: Bun.Socket): void {
  const queue = ws.data.queue;
  while (queue.length > 0) {
    const first = queue[0]!;
    const wrote = socket.write(first);
    if (wrote < first.length) {
      queue[0] = first.subarray(Math.max(wrote, 0));
      return;
    }
    queue.shift();
  }
}

type WsData = {
  origin: string;
  queue: Uint8Array[];
  daemon?: Bun.Socket;
};

Bun.serve({
  hostname: HOST,
  port: PORT,
  fetch(req: Request, server: Bun.Server<WsData>) {
    const url = new URL(req.url);
    if (req.headers.get('upgrade')?.toLowerCase() !== 'websocket') return serveStatic(url);
    const origin = req.headers.get('origin') || '';
    const refuse = (status: number, why: string): Response => {
      console.warn(`refused ${server.requestIP(req)?.address} ${origin}: ${why}`);
      return new Response(why, { status });
    };
    if (url.pathname !== '/virt') return refuse(404, 'Not Found');
    if (!ORIGINS.has(origin)) return refuse(403, 'Origin not allowed');
    if (TOKEN && url.searchParams.get('token') !== TOKEN) return refuse(403, 'Bad token');
    if (!server.upgrade(req, { data: { origin, queue: [] } })) return refuse(400, 'Bad Request');
  },
  websocket: {
    maxPayloadLength: MAX_FRAME,
    async open(ws: Bun.ServerWebSocket<WsData>) {
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
        console.warn('daemon socket:', (e as Error).message);
        ws.close();
        return;
      }
      flush(ws, ws.data.daemon);
    },
    message(ws: Bun.ServerWebSocket<WsData>, message: string | Uint8Array) {
      // Text is not ours; close.
      if (typeof message === 'string') return ws.close();
      ws.data.queue.push(message);
      if (ws.data.daemon) flush(ws, ws.data.daemon);
    },
    close(ws: Bun.ServerWebSocket<WsData>) { ws.data.daemon?.end(); },
  },
});
console.log(`ws-relay on http://${HOST}:${PORT}/virt -> ${SOCKET}; origins ${[...ORIGINS].join(' ')}${TOKEN ? '; token required' : ''}`);
