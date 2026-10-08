#!/usr/bin/env node
// A byte relay between WebSockets and libvirt's unix socket, with no
// dependencies (Node 18+). It understands WebSocket framing and nothing of
// libvirt: every binary message from the page is written to the socket as
// is, every read from the socket goes back as one binary message. All of
// libvirt's protocol lives in the page (losos-lab-virt, compiled to wasm).
//
//   LOSOS_VIRT_SOCKET=/run/libvirt/virtqemud-sock \
//   LOSOS_VIRT_ORIGINS=https://lab.example,http://127.0.0.1:8095 \
//   LOSOS_VIRT_TOKEN=$(head -c16 /dev/urandom | xxd -p) \
//   node ws-relay.mjs --port 8095 [--static DIR]
//
// Whoever reaches /virt gets the libvirt rights of the user this relay runs
// as (libvirt sees the relay's uid on the socket, never the page's). For
// qemu:///system that is root on the host. Hence: loopback only, an Origin
// allowlist (a page on any other site could otherwise open
// ws://127.0.0.1:8095 from the owner's own browser), and a token.

import http from 'node:http';
import net from 'node:net';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';

const arg = (name, dflt) => {
  const i = process.argv.indexOf(name);
  return i > 0 ? process.argv[i + 1] : dflt;
};
const PORT = Number(arg('--port', '8095'));
const HOST = arg('--host', '127.0.0.1');
const STATIC = arg('--static', null) && path.resolve(arg('--static'));
const SOCKET = process.env.LOSOS_VIRT_SOCKET || '/run/libvirt/virtqemud-sock';
const TOKEN = process.env.LOSOS_VIRT_TOKEN || '';
const ORIGINS = new Set((process.env.LOSOS_VIRT_ORIGINS || `http://${HOST}:${PORT},http://localhost:${PORT}`).split(',').map((s) => s.trim()).filter(Boolean));
const GUID = '258EAFA5-E914-47DA-95CA-C5AB0DC85B11';
const MAX_FRAME = 33554432 + 4; // libvirt's VIR_NET_MESSAGE_MAX plus the length word

const TYPES = { '.html': 'text/html; charset=utf-8', '.js': 'text/javascript', '.mjs': 'text/javascript', '.wasm': 'application/wasm', '.json': 'application/json', '.css': 'text/css', '.png': 'image/png', '.webmanifest': 'application/manifest+json' };

const server = http.createServer((req, res) => {
  if (!STATIC) { res.writeHead(404).end(); return; }
  const url = new URL(req.url, 'http://x');
  const rel = path.normalize(decodeURIComponent(url.pathname)).replace(/^(\.\.[/\\])+/, '');
  let file = path.join(STATIC, rel);
  if (!file.startsWith(STATIC)) { res.writeHead(403).end(); return; }
  if (file.endsWith('/')) file += 'index.html';
  fs.readFile(file, (err, body) => {
    if (err) { res.writeHead(404).end(); return; }
    res.writeHead(200, {
      'content-type': TYPES[path.extname(file)] || 'application/octet-stream',
      // The hosted Lab is cross-origin isolated; serve the test page the same way.
      'cross-origin-opener-policy': 'same-origin',
      'cross-origin-embedder-policy': 'require-corp',
      'cross-origin-resource-policy': 'same-origin',
      'cache-control': 'no-store',
    });
    res.end(body);
  });
});

function frame(opcode, payload) {
  const n = payload.length;
  const head = n < 126 ? Buffer.from([0x80 | opcode, n])
    : n < 65536 ? Buffer.from([0x80 | opcode, 126, n >> 8, n & 255])
    : Buffer.concat([Buffer.from([0x80 | opcode, 127]), (() => { const b = Buffer.alloc(8); b.writeBigUInt64BE(BigInt(n)); return b; })()]);
  return Buffer.concat([head, payload]);
}

server.on('upgrade', (req, sock) => {
  const url = new URL(req.url, 'http://x');
  const refuse = (code, why) => { console.warn(`refused ${req.socket.remoteAddress} ${req.headers.origin}: ${why}`); sock.end(`HTTP/1.1 ${code} ${why}\r\n\r\n`); };
  if (url.pathname !== '/virt') return refuse(404, 'Not Found');
  if (!ORIGINS.has(req.headers.origin || '')) return refuse(403, 'Origin not allowed');
  if (TOKEN && url.searchParams.get('token') !== TOKEN) return refuse(403, 'Bad token');
  const key = req.headers['sec-websocket-key'];
  if (!key || req.headers['sec-websocket-version'] !== '13') return refuse(400, 'Bad Request');
  const accept = crypto.createHash('sha1').update(key + GUID).digest('base64');
  sock.write(`HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: ${accept}\r\n\r\n`);
  sock.setNoDelay(true);

  const daemon = net.connect(SOCKET);
  let buf = Buffer.alloc(0);
  let closed = false;
  const close = () => { if (closed) return; closed = true; daemon.destroy(); if (!sock.destroyed) sock.end(frame(8, Buffer.alloc(0))); };
  console.log(`relay ${req.headers.origin} -> ${SOCKET}`);
  daemon.on('data', (d) => { if (!sock.destroyed) sock.write(frame(2, d)); });
  daemon.on('error', (e) => { console.warn('daemon socket:', e.message); close(); });
  daemon.on('close', close);
  sock.on('error', close);
  sock.on('close', close);
  sock.on('data', (d) => {
    buf = buf.length ? Buffer.concat([buf, d]) : d;
    for (;;) {
      if (buf.length < 2) return;
      const op = buf[0] & 15, masked = buf[1] & 128;
      let len = buf[1] & 127, off = 2;
      if (len === 126) { if (buf.length < 4) return; len = buf.readUInt16BE(2); off = 4; }
      else if (len === 127) { if (buf.length < 10) return; len = Number(buf.readBigUInt64BE(2)); off = 10; }
      if (!masked || len > MAX_FRAME) return close(); // clients must mask (RFC 6455 5.1)
      if (buf.length < off + 4 + len) return;
      const mask = buf.subarray(off, off + 4);
      const data = Buffer.from(buf.subarray(off + 4, off + 4 + len));
      for (let i = 0; i < data.length; i++) data[i] ^= mask[i & 3];
      buf = buf.subarray(off + 4 + len);
      if (op === 2 || op === 0) daemon.write(data);
      else if (op === 9) sock.write(frame(10, data));
      else if (op === 8 || op === 1) return close(); // close, or text (not ours)
    }
  });
});

server.listen(PORT, HOST, () => console.log(`ws-relay on http://${HOST}:${PORT}/virt -> ${SOCKET}; origins ${[...ORIGINS].join(' ')}${TOKEN ? '; token required' : ''}`));
