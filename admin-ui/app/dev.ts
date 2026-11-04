/* The dev server, on Bun.serve: `bun run dev` serves every page with hot
 * reloading on http://localhost:5173 and proxies the APIs to a box.
 *
 *   LOSOS_API_ORIGIN=https://box.local bun run dev
 *
 * Bun bundles each page on request, with the plugins bunfig.toml names
 * (Tailwind and bundler.ts's devPlugin), so a page is bundled the way
 * build.ts bundles it. The fixed-name files build.ts writes beside the
 * pages are made here on request instead. */
import index from "./index.html";
import chat from "./chat/index.html";
import manage from "./manage/index.html";
import cloud from "./cloud/index.html";
import kb from "./kb/index.html";
import git from "./git/index.html";
import wiki from "./wiki/index.html";
import diagrams from "./diagrams/index.html";
import guest from "./guest.html";
import { basicClasses } from "./scripts/basic-classes.mjs";
import { appFrame, classicScripts, copiedFiles, themeBoot } from "./build";
import { devWorkers, settings, workers } from "./bundler";

const root = import.meta.dirname;
const api = Bun.env["LOSOS_API_ORIGIN"] ?? "http://127.0.0.1:8082";
// LosOS Cloud's API: `losos-cloud serve --listen 127.0.0.1:8095
// --trust-header` and a header-setting proxy, or a box.
const cloudApi = Bun.env["LOSOS_CLOUD_ORIGIN"] ?? "http://127.0.0.1:8095";

await basicClasses();
const built = await workers({ minify: false, publicPath: "/" });
for (const [path, url] of built.urls) devWorkers.set(path, url);

/* A request passed on to `origin`. With `keepHost`, Host stays the dev
 * origin's: LosOS Cloud refuses a write whose Origin names another host
 * than Host, and the browser's Origin is this dev server. */
function proxy(origin: string, opts: { keepHost?: boolean; headers?: Record<string, string> } = {}) {
  return async (req: Request): Promise<Response> => {
    const url = new URL(req.url);
    const headers = new Headers(req.headers);
    if (!opts.keepHost) headers.set("Host", new URL(origin).host);
    for (const [k, v] of Object.entries(opts.headers ?? {})) headers.set(k, v);
    return fetch(`${origin}${url.pathname}${url.search}`, {
      method: req.method,
      headers,
      body: req.body,
      redirect: "manual",
      // @ts-expect-error Bun's fetch takes a streamed body this way.
      duplex: "half",
    });
  };
}

const js = { "Content-Type": "text/javascript", "Cache-Control": "no-store" };
const css = { "Content-Type": "text/css", "Cache-Control": "no-store" };

/* Each page's own route, under /_page/, is Bun's bundled HTML; the paths
 * a browser asks for answer with that HTML, its placeholder made into the
 * classic tag (build.ts's classicScripts). */
const PAGES = { index, chat, manage, cloud, kb, git, wiki, diagrams, guest };
type Page = keyof typeof PAGES;
const pageRoutes = Object.fromEntries(Object.entries(PAGES).map(([name, html]) => [`/_page/${name}`, html]));

function page(name: Page) {
  return async (req: Request) => {
    const html = await fetch(new URL(`/_page/${name}`, req.url));
    return classicScripts(html);
  };
}

/* A file from public/, as build.ts copies it into dist/, or the page. */
function publicOr(name: Page) {
  const fallback = page(name);
  return async (req: Request) => {
    const path = decodeURIComponent(new URL(req.url).pathname);
    if (!path.includes("..") && !path.endsWith("/")) {
      const file = Bun.file(`${root}/public${path}`);
      if (await file.exists()) return new Response(file);
    }
    return fallback(req);
  };
}

const server = Bun.serve({
  port: Number(Bun.env["PORT"] ?? 5173),
  development: { hmr: true, console: true },
  routes: {
    ...pageRoutes,
    "/theme-boot.js": async () => new Response(await themeBoot(false), { headers: js }),
    "/app-frame/runtime.js": async () => new Response((await appFrame(false)).js, { headers: js }),
    "/app-frame/runtime.css": async () => new Response((await appFrame(false)).css, { headers: css }),
    ...Object.fromEntries(copiedFiles().map(([path, src]) => [`/${path}`, () => new Response(Bun.file(src))])),
    ...Object.fromEntries([...built.files].map(([path, blob]) => [path, () => new Response(blob, { headers: js })])),
    "/api/*": proxy(api),
    // The first-run wizard's step 1 reads /setup/state.json, the box's name
    // and its certificate fingerprint, which nginx serves from disk on the
    // box (modules/setup.nix). Without this the dev origin answers with
    // the SPA's HTML, and step 1 reports the box as too old to have a
    // setup route at all.
    "/setup/*": proxy(api),
    "/cloud/api/*": proxy(cloudApi, {
      keepHost: true,
      headers: { "X-Losos-Cloud-User": Bun.env["LOSOS_CLOUD_USER"] ?? "owner" },
    }),
    // A knowledge base's data, read as nobody.
    "/kb/api/*": proxy(cloudApi, { keepHost: true }),
    "/chat/*": page("chat"),
    "/manage/*": page("manage"),
    "/cloud/*": page("cloud"),
    "/kb/*": page("kb"),
    "/git/*": page("git"),
    "/wiki/*": page("wiki"),
    "/calls/guest/*": page("guest"),
    // The hosted copy's own page; on a box, the SPA's screen.
    "/diagrams/*": settings().LAB_HOSTED ? page("diagrams") : publicOr("index"),
    "/*": publicOr("index"),
  },
});

console.log(`LosOS admin UI on ${server.url}, the API at ${api}`);
