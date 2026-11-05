/* The SFU (losos-calls) on free ports with nginx's part in front of it, for
 * the browser checks that run a real call: tests/calls.browser.mjs and
 * tests/voice-agent.browser.mjs.
 *
 * nginx in front of the SFU is as modules/calls.nix has it: it names the
 * person LosOS Login signed in as Remote-User and drops any the client sent.
 * Here a path `/as/<name>/` stands in for that sign-in. */

export async function startFront({ bin: BIN, address: ADDRESS, serverName: SERVER, tmp: TMP, registrar }) {
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
    [BIN, '--listen', `127.0.0.1:${sfuPort}`, '--udp-port', String(udpPort), '--address', ADDRESS, '--server-name', SERVER, '--tap', tap, '--ffmpeg', Bun.env.LOSOS_FFMPEG ?? 'ffmpeg', ...(registrar ? ['--registrar', registrar, '--relay-id', 'box-1', '--relay-token-file', relayToken] : [])],
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
  return {
    sfu,
    tap,
    nginx,
    front,
    api,
    stop() {
      sfu.kill();
      nginx.stop(true);
    },
  };
}
