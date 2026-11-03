#!/usr/bin/env bun
/* The dev server: what `bun run dev` runs.
 *
 *   bun scripts/dev.ts          # http://localhost:5173
 *
 * It builds the site exactly as scripts/build.ts does (the box serves that
 * same bundle, so a dev page runs under the same latent constraints) and
 * serves dist/ on Bun.serve as localhost's stand-in for the admin Nginx
 * vhost: the same SPA and per-page fallbacks, plus the proxies a box gets
 * from nginx —
 *
 *   LOSOS_API_ORIGIN      /api/* and /setup/* → lososd's loopback API
 *                         (Bearer-authed; the wizard's step 1 reads
 *                         /setup/state.json on a box off disk — here the
 *                         proxy answers it, which is what keeps the dev
 *                         origin from finding no setup route at all (a
 *                         route-less 404 would parse as the SPA doc and
 *                         claim the box is too old to have one))
 *   LOSOS_CLOUD_ORIGIN    /cloud/api/* and /kb/api/* → a LosOS Cloud dev
 *                         run (`losos-cloud serve --listen 127.0.0.1:8095
 *                         --trust-header`); the service reads the person
 *                         nginx would set, so the proxy writes
 *                         X-Losos-Cloud-User on /cloud/api alone
 *
 * After every change under src/ (and the HTML pages) it rebuilds with the
 * production script's own commands — Bun.build is quick enough for one full
 * build per save — and tells the tabs over Server-Sent Events: they
 * reload. That is not module-level hot reload, and it does not want
 * to be: the dev page runs the built bundle, not a transpiled sibling, so
 * it cannot drift from what ships.
 *
 * Watching is manual and mtime-based on purpose: it keeps dev.ts free of
 * any `node:` import while doing the whole job on Bun's APIs.
 */

import { buildSite } from "./build";
import { buildEnv, DIST, ROOT, type BuildEnv } from "./bundler";

/* The static types that fit dist exactly (they drive which cache header a
 * file gets; the SPA fallback documents get no-store). */
const TYPES: Record<string, string> = {
  ".html": "text/html; charset=utf-8",
  ".js": "text/javascript; charset=utf-8",
  ".mjs": "text/javascript; charset=utf-8",
  ".css": "text/css; charset=utf-8",
  ".json": "application/json",
  ".svg": "image/svg+xml",
  ".png": "image/png",
  ".jpg": "image/jpeg",
  ".webp": "image/webp",
  ".wasm": "application/wasm",
  ".tflite": "application/octet-stream",
  ".onnx": "application/octet-stream",
};

/* Pages of their own with their own fallbacks, like their nginx locations
 * (/diagrams is the SPA's screen, so it falls back to /index.html). */
const PAGE_PREFIXES = ["manage", "cloud", "kb", "git", "wiki", "chat"];

/* Dev-only version patcher: it gets the tagged reload client wedged into
 * every HTML on its way out (before '</head>'), with THE SCRIPT INLINE
 * (safe only here, where nothing runs on the box: the built documents it
 * replaces have the CSP and no inline script at all). Server-Sent Events
 * need no `EventSource` polyfill and no extra port.
 *
 *   <script>new EventSource("/__dev/reload").onmessage = () =>
 *     location.reload();</script>
 */
function withReload(html: string, epoch: number): string {
  const client = `\n  <script>new EventSource("/__dev/reload?v=${epoch}").onmessage = (m) => m.data === "reload" && location.reload();</script>`;
  return html.includes("</head>") ? html.replace("</head>", `${client}\n  </head>`) : html + client;
}

/* One relative file stamp: mtimes string in a tuple, indexed per path with
 * an explicit 0-default (some platforms hand out no mtime at all). */
interface Mtimes {
  stamp: Map<string, number>;
}
function scan(dir: string, glob: Bun.Glob, stamp: Map<string, number>): Promise<void> {
  return new Promise((resolve) => {
    try {
      for (const rel of glob.scanSync({ cwd: dir, onlyFiles: true })) {
        const abs = `${dir}/${rel}`;
        const stat = Bun.file(abs).lastModified ?? 0;
        stamp.set(rel, stat);
      }
      resolve();
    } catch {
      // A missing directory is a quiet no-change scan.
      resolve();
    }
  });
}

/* The watcher: every tick it rescans src/ (sources, the plugin packages and
 * the styles under src/styles) and each page's HTML document, deep-compares
 * the stamps, and resolves after the first change. rescan() has no fs
 * watchers: polling stays on Bun APIs whichever kernel neither git nor
 * inotify can speak — and one pinned tab is enough signal for a page in a
 * dev tab that is watching its own src/. */
async function waitSrcChange(stamp: Mtimes, interval = 500): Promise<void> {
  const sources = [
    new Bun.Glob("**/*.{ts,tsx,css,json,html,md,mdx,yml,yaml}"),
  ];
  const page = new Bun.Glob("*.html");
  for (;;) {
    await new Promise((r) => setTimeout(r, interval));
    const fresh = new Map<string, number>();
    await scan(`${ROOT}/src`, sources[0]!, fresh);
    await scan(ROOT, page, fresh);
    await scan(`${ROOT}/../lab`, new Bun.Glob("assets/**"), fresh);
    const changed = fresh.size !== stamp.stamp.size
      ? true
      : [...fresh].some(([k, m]) => stamp.stamp.get(k) !== m);
    stamp.stamp.clear();
    for (const [k, v] of fresh) stamp.stamp.set(k, v);
    if (changed) return;
  }
}

/* ── The proxied origins ───────────────────────────────────────────────── */
interface Proxies {
  api: string;
  cloud: string;
  cloudUser: string;
}
function proxyTargets(envv: Record<string, string | undefined>): Proxies {
  return {
    api: envv["LOSOS_API_ORIGIN"] ?? "http://127.0.0.1:8082",
    cloud: envv["LOSOS_CLOUD_ORIGIN"] ?? "http://127.0.0.1:8095",
    cloudUser: envv["LOSOS_CLOUD_USER"] ?? "owner",
  };
}

/* /api/* and /setup/* go to lososd, Host rewritten to the target (the
 * changeOrigin box nginx gives a box a box); /cloud/api/* keeps the dev
 * origin's Host (the cloud service refuses a write whose Origin names
 * another host than Host, and the browser's Origin is this dev server —
 * the same as the old dev config, `changeOrigin: false`) and gains the
 * person header. */
function isApi(rel: string): boolean {
  return rel.startsWith("/api") || rel.startsWith("/setup");
}
function isCloud(rel: string): boolean {
  return rel.startsWith("/cloud/api") || rel.startsWith("/kb/api");
}

/* ── The server ────────────────────────────────────────────────────────── */
const env: BuildEnv = buildEnv(Bun.env as unknown as Record<string, string | undefined>);
const proxies = proxyTargets(Bun.env as unknown as Record<string, string | undefined>);

/* The epoch the tabs reload on: every build that succeeds bumps it. */
let epoch = 0;
/* Tabs waiting on the reload signal; SSE writers push on signal. */
const tabs = new Set<(line: string) => void>();
const notify = () => {
  for (const write of tabs) write("data: reload\n\n");
};

/* The first build runs before the server appears on the port, so the first
 * paint already serves the built site. Failures abort: a half-built dist
 * would 404 the SPA fallbacks. */
console.log("[dev] first build");
await buildSite(Bun.env as unknown as Record<string, string | undefined>, false);

const server = Bun.serve({
  hostname: "127.0.0.1",
  port: 5173,
  async fetch(req) {
    const rel = decodeURIComponent(new URL(req.url).pathname);

    if (req.headers.get("accept")?.includes("text/event-stream") && rel.startsWith("/__dev/reload")) {
      let writeBack: ((line: string) => void) | null = null;
      const stream = new ReadableStream({
        start(controller) {
          writeBack = (line: string) => {
            try {
              controller.enqueue(new TextEncoder().encode(`${line}\n\n`));
            } catch {
              tabs.delete(writeBack!);
            }
          };
          tabs.add(writeBack);
          writeBack(`data: v${epoch}\n\n`);
        },
        cancel() {
          if (writeBack) tabs.delete(writeBack);
        },
      });
      return new Response(stream, {
        headers: { "Content-Type": "text/event-stream", "Cache-Control": "no-store" },
      });
    }

    /* The proxies, in front of dist: lososd owns /api, so a real box must
     * answer, never the fallback SPA. */
    if (isApi(rel)) {
      const target = new URL(req.url.replace(/^http:\/\/[^/]+/, proxies.api));
      return await proxy(req, target, { host: true });
    }
    if (isCloud(rel)) {
      const target = new URL(req.url.replace(/^http:\/\/[^/]+/, proxies.cloud));
      return await proxy(req, target, { user: proxies.cloudUser });
    }

    /* Static dist/ with a page fallback, byte for byte as the harness's
     * serve() does it; HTML gains the reload client. */
    const file = rel === "/" ? "index.html" : rel.endsWith("/") ? `${rel}index.html`.slice(1) : rel.slice(1);
    const page = PAGE_PREFIXES.find((p) => rel.startsWith(`/${p}/`) || rel === `/${p}`);
    const fallback = page === undefined ? "index.html" : `${page}/index.html`;
    const send = async (name: string) => {
      const f = Bun.file(`${DIST}/${name}`);
      if (!(await f.exists())) return null;
      const type = TYPES[name.match(/\.[^./]*$/)?.[0] ?? ""] ?? "application/octet-stream";
      const headers: Record<string, string> = { "Content-Type": type, "Cache-Control": "no-store" };
      return { body: f, type, headers };
    };
    const picked = (await send(file)) ?? (await send(fallback));
    if (picked === null) return new Response("not found", { status: 404 });
    if (picked.type === TYPES[".html"]) {
      const html = picked.body as Bun.BunFile;
      return new Response(withReload(await html.text(), epoch), { headers: picked.headers });
    }
    return new Response(picked.body, { headers: picked.headers });
  },
});

/* One HTTP hop, waiting on the whole body (a dev-page-sized proxy never
 * wants streaming). `host: true` rewrites Host, `user` writes the person. */
async function proxy(req: Request, target: URL, opts: { host?: boolean; user?: string }): Promise<Response> {
  const headers = new Headers(req.headers);
  if (opts.host) headers.set("Host", target.host);
  else headers.delete("Host");
  if (opts.user) headers.set("X-Losos-Cloud-User", opts.user);
  const body = req.method === "GET" || req.method === "HEAD" ? undefined : await req.arrayBuffer();
  const out = new Request(target, {
    method: req.method,
    headers,
    body,
    redirect: "manual",
  });
  try {
    const upstream = await fetch(out);
    const answer = new Headers(upstream.headers);
    answer.delete("content-length");
    answer.set("Cache-Control", "no-store");
    return new Response(upstream.body, { status: upstream.status, headers: answer });
  } catch (error) {
    return new Response(`dev proxy: ${(error as Error).message}`, { status: 502 });
  }
}

/* The loop: rebuild after every change the watcher reports, bump the epoch,
 * tell the tabs. A failed build keeps the last good dist/ and prints. */
const watcher: Mtimes = { stamp: new Map() };
console.log(`[dev] http://127.0.0.1:${server.port} — watching src/`);
for (;;) {
  try {
    await waitSrcChange(watcher);
    await buildSite(Bun.env as unknown as Record<string, string | undefined>, false);
    epoch++;
    notify();
    console.log(`[dev] rebuilt (v${epoch})`);
  } catch (error) {
    console.error("[dev] build failed:", (error as Error).message);
  }
}
