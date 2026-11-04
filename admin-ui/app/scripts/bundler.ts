/* The bundling machinery the box's build (scripts/build.ts) and the dev
 * server (scripts/dev.ts) share. This is Bun.build doing the whole job the
 * old Vite configs did, minus Vite:
 *
 * - pages and guest page: one Bun.build each, splitting on, hashed chunk
 *   names (scripts/build.ts maps each HTML document onto its entry chunk);
 * - the design-system CSS: Tailwind v4 compiled once through its CLI
 *   (@tailwindcss/cli), handed to the bundle through a plugin, and every
 *   other stylesheet carried as a runtime-injected <link> so the admin CSP
 *   (`style-src 'self'`) still holds;
 * - theme-boot.js and the app frame runtime: classic, unhashed Bun builds;
 * - the source-quieting patches that keep MediaPipe's usage logger and
 *   Sonner's stylesheet injection from ever reaching the page.
 *
 * CSP is the load-bearing constraint: nothing may inject a <style> element
 * or a style attribute, so a stylesheet travels as a file the page links.
 */

import type { BunPlugin } from "bun";
import { compilePages } from "./pages";

export const ROOT = new URL("..", import.meta.url).pathname.replace(/\/$/, "");
export const DIST = `${ROOT}/dist`;
export const SRC = `${ROOT}/src`;

/* What the build sees of the world, at the call site. */
export interface BuildEnv {
  /* The URL prefix every asset of this build hangs off: "/" for the box,
   * "/calls/guest/" for the guest page. */
  base: string;
  /* VITE_LAB_HOSTED=1: the hosted Diagrams copy (admin-ui/lab/engine/build.sh). */
  hosted: boolean;
  /* LOSOS_LAB_MEME: a folder holding no.jpg and yes.jpg for the Lab's splash. */
  memeDir: string;
  /* LOSOS_SEGMENTER_MODEL / LOSOS_NUDITY_MODEL, pinned by nix/packages.nix. */
  segmenterModel?: string;
  nudityModel?: string;
}

/* ── What the build reads from the environment ─────────────────────────── */
export function buildEnv(bunEnv: Record<string, string | undefined>): BuildEnv {
  return {
    base: "/",
    hosted: bunEnv["VITE_LAB_HOSTED"] === "1",
    memeDir: bunEnv["LOSOS_LAB_MEME"] ?? "",
    segmenterModel: bunEnv["LOSOS_SEGMENTER_MODEL"] || undefined,
    nudityModel: bunEnv["LOSOS_NUDITY_MODEL"] || undefined,
  };
}

/* The two classic bundles (theme-boot, the app frame) see none of the page
 * knobs: one base, no hosted flag, no meme folder. */
const STATIC_ENV: BuildEnv = { base: "/", hosted: false, memeDir: "" };

/* ── Defines ─────────────────────────────────────────────────────────────
 * The build-time constants src/ reads. `import.meta.env` is replaced
 * wholesale (Bun's define wants dotted identifiers, and a bracketed
 * `env["VITE_LAB_HOSTED"]` is not one, so this is the one key), and the
 * shape src/lab/shape.ts expects stays: it compares the hosted flag against
 * the string "1" exactly. */
export function defines(env: BuildEnv, minify = false): NonNullable<Bun.BuildConfig["define"]> {
  const clientEnv = {
    BASE_URL: env.base,
    ...(env.hosted ? { VITE_LAB_HOSTED: "1" } : {}),
  };
  return {
    "process.env.NODE_ENV": JSON.stringify(minify ? "production" : "development"),
    "import.meta.env": JSON.stringify(clientEnv),
    ...(env.memeDir ? { __LAB_MEME__: JSON.stringify(true) } : {}),
  };
}

/* Entry and shared chunks, hashed under assets/. `[dir]` is relative to the
 * entrypoints' common ancestor — src/ — so the seven pages' main.tsx files
 * land beside each other, each in its own folder. */
export function hashedNaming(): NonNullable<Bun.BuildConfig["naming"]> {
  return {
    entry: "assets/[dir]/[name]-[hash].[ext]",
    chunk: "assets/[dir]/[name]-[hash].[ext]",
    asset: "assets/[dir]/[name]-[hash].[ext]",
  };
}

/* ── Basic classes ─────────────────────────────────────────────────────── */
/* The classes the pages without JavaScript use are scanned from the Rust
 * plugin sources in a checkout (nix copies the text out of the cached
 * database instead, so this is a no-op in the sandbox: there
 * backend/plugins/basic does not exist). Runs before Tailwind compiles
 * src/styles/index.css. */
export async function ensureBasicClasses(): Promise<void> {
  const repo = `${ROOT}/../../..`;
  const styles = `${ROOT}/src/styles`;
  const sources = `${repo}/backend/plugins/basic/src`;
  try {
    new Bun.Glob("*").scanSync({ cwd: sources }).next();
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code === "ENOENT") return;
    throw error;
  }
  const { writeBasicClasses } = await import("./basic-classes");
  await writeBasicClasses(repo, styles);
}

/* ── Tailwind ──────────────────────────────────────────────────────────── */
/* Compiles src/styles/index.css (tokens, the theme, the basic classes'
 * `@source`, everything the pages without a script draw too) into the one
 * stylesheet every built page links. The CLI comes from @tailwindcss/cli, a
 * devDependency Bun's runtime runs. Returns the compiled text. */
export async function compileAppCss(): Promise<string> {
  const out = `${ROOT}/.cache/app.css`;
  await Bun.write(out, "");
  const proc = Bun.spawnSync({
    // The CLI's own bin, from the workspace's node_modules: the Nix sandbox
    // that runs this build has no registry to ask for it (scripts/build.ts
    // runs after `bun install --frozen-lockfile` in the package's setup).
    cmd: ["bun", "node_modules/@tailwindcss/cli/dist/index.mjs", "--input", `${SRC}/styles/index.css`, "--output", out],
    cwd: ROOT,
    stdout: "inherit",
    stderr: "inherit",
  });
  if (proc.exitCode !== 0) throw new Error(`tailwindcss failed with exit code ${proc.exitCode}`);
  return await Bun.file(out).text();
}

/* The `?url` suffix a couple of src files attach to WebAssembly imports
 * (src/mdx/wasm.ts, src/lab/{core,render}.ts, src/lab/engine/virt-rpc.ts).
 * Bun has no asset-query syntax, so the suffix comes off here and the file
 * loader does the rest: the module copies to a hashed file under assets/
 * and imports as its URL string, exactly what `?url` promised. */
export function wasmUrl(): BunPlugin {
  return {
    name: "losos:wasm-url",
    setup(build) {
      build.onResolve({ filter: /\?url$/ }, (args) => {
        const target = args.path.replace(/\?url$/, "");
        // Bun resolves within the importer's own directory; hand it the
        // absolute, `..`-free path and the default loaders take over.
        const dir = args.importer.replace(/\/[^/]*$/, "");
        const joined = new URL(target, `file://${dir}/`).pathname;
        return { path: joined };
      });
    },
  };
}

/* ── The stylesheet registry ───────────────────────────────────────────── */
/* Every stylesheet the bundle imports becomes, in the page's JavaScript,
 * one line:
 *
 *   inject("9a4b…");
 *
 * and the registry appends a <link rel="stylesheet"> for that file under
 * assets/css/. The CSS files themselves are written straight from the
 * sources, hashed by content, so identical sheets share a file — never a
 * bundler-emitted copy whose name no one could map back. This is the CSP's
 * answer to a stylesheet: a file the page links, not a <style> element. */
export interface CssRegistry {
  /* key → css text, collected while the bundle transforms. */
  readonly files: Map<string, string>;
  plugin(): BunPlugin;
  /* Writes every collected file under `${dist}/assets/css/` and returns
   * the app sheet's key, for the <link> the HTML pipeline adds. */
  write(dist: string): Promise<string>;
}

export function cssRegistry(env: BuildEnv, appCss: string): CssRegistry {
  const files = new Map<string, string>();
  const keyOf = (text: string): string =>
    new Bun.CryptoHasher("sha256").update(text).digest("hex").slice(0, 16);
  const appKey = keyOf(appCss);
  files.set(appKey, appCss);

  /* The registry itself, compiled into the page: linking is all it does,
   * and it is idempotent, so five imports of one sheet make one <link>. */
  const REGISTRY = `export function inject(key) {
  if (typeof document === "undefined") return;
  const href = new URL(\`assets/css/\${key}.css\`, ${JSON.stringify(env.base)}).href;
  for (const link of document.querySelectorAll("link[rel=stylesheet]")) {
    if (link.href === href) return;
  }
  const link = document.createElement("link");
  link.rel = "stylesheet";
  link.href = href;
  document.head.appendChild(link);
}
`;

  /* Tailwind compiled app.css already inlined its own imports; the other
   * sheets are plain pages of CSS, so inline any relative @import one
   * might carry, and refuse anything else. */
  const IMPORTS = /@import\s+(?:url\()?["']([^"']+)["']\)?\s*;/g;
  const load = async (path: string): Promise<string> => {
    const css = await Bun.file(path).text();
    const inlined: string[] = [];
    for (const target of css.matchAll(IMPORTS)) {
      const name = target[1];
      if (name === undefined) continue;
      if (!name.startsWith(".")) {
        throw new Error(`@import "${name}" in ${path}: the registry carries plain CSS only`);
      }
      inlined.push(await Bun.file(`${path}/../${name}`).text());
    }
    if (inlined.length === 0) return css;
    return `${inlined.join("\n")}\n${css.replaceAll(IMPORTS, "").trimStart()}`;
  };

  return {
    files,
    plugin() {
      return {
        name: `losos:css-registry`,
        setup(build) {
          build.onResolve({ filter: /^virtual:losos-css-registry$/ }, () => ({
            path: "virtual:losos-css-registry",
            namespace: "losos-css",
          }));
          build.onLoad({ filter: /^virtual:losos-css-registry$/, namespace: "losos-css" }, () => ({
            loader: "js",
            contents: REGISTRY,
          }));
          build.onLoad({ filter: /\.css$/ }, async (args) => {
            // index.css never loads as source: the registry carries the
            // sheet Tailwind compiled from it, bytes for bytes. The @/
            // tsconfig alias can arrive resolved or not; match the tail.
            const isApp = args.path.endsWith("/src/styles/index.css") || args.path === `${SRC}/styles/index.css`;
            const css = isApp ? appCss : await load(args.path);
            const key = keyOf(css);
            if (!files.has(key)) files.set(key, css);
            return {
              loader: "js",
              contents: `import { inject } from "virtual:losos-css-registry";\ninject(${JSON.stringify(key)});\n`,
            };
          });
        },
      };
    },
    async write(dist: string): Promise<string> {
      for (const [key, css] of files) {
        await Bun.write(`${dist}/assets/css/${key}.css`, css);
      }
      return appKey;
    },
  };
}

/* ── The pages module ──────────────────────────────────────────────────── */
export function pagesVirtual(root = ROOT): BunPlugin {
  return {
    name: "losos:pages",
    setup(build) {
      build.onResolve({ filter: /^virtual:losos-pages$/ }, () => ({
        path: "virtual:losos-pages",
        namespace: "losos-pages",
      }));
      build.onLoad({ filter: /virtual:losos-pages/, namespace: "losos-pages" }, async () => {
        const { html } = await compilePages(root);
        return { loader: "js", contents: `export const DEFAULTS = ${JSON.stringify(html)};\n` };
      });
    },
  };
}

/* blocks.json and the default MDX files the pages plugin reads under
 * dist/pages/ (they are never served; nginx sends /pages/ nowhere). */
export async function emitPageSources(dist: string, root = ROOT): Promise<void> {
  const { registry, sources } = await compilePages(root);
  await Bun.write(`${dist}/pages/blocks.json`, registry);
  for (const [id, source] of Object.entries(sources)) {
    await Bun.write(`${dist}/pages/${id}.mdx`, source);
  }
}

/* ── The two classic, unhashed bundles ─────────────────────────────────── */
/* theme-boot.ts compiles to a standalone IIFE: index.html loads it as a
 * CLASSIC script — no `type="module"`, without `defer` — the only way to
 * block the parser and stamp the theme before the first paint
 * (scripts/build.ts keeps the tag in index.html hand-written). It is
 * deliberately not content-hashed: the tag names it, so the name has to
 * stay. ~200 bytes of logic that has no reason to change. */
export async function compileThemeBoot(minify = false): Promise<string> {
  const result = await Bun.build({
    entrypoints: [`${SRC}/theme-boot.ts`],
    target: "browser",
    format: "iife",
    minify,
    define: defines(STATIC_ENV, minify),
  });
  if (!result.success) throw new AggregateError(result.logs, "bundling theme-boot failed");
  return (await result.outputs[0]?.text()) ?? "";
}

/* The app frame's runtime (src/app-frame/runtime.tsx): React, the LosOS
 * components and the bridge, in one classic script and its stylesheet at
 * fixed names, /app-frame/runtime.js and runtime.css, which
 * public/app-frame/index.html loads. Classic and unhashed for the reasons
 * runtime.tsx gives: the frame is an opaque origin, where a module script
 * would be fetched in CORS mode, and the page that loads it is a static
 * file no HTML rewriting passes over. */
export interface RuntimeBundle {
  js: string;
  css: string;
}
export async function compileAppFrame(minify = false): Promise<RuntimeBundle> {
  const result = await Bun.build({
    entrypoints: [`${SRC}/app-frame/runtime.tsx`],
    target: "browser",
    format: "iife",
    minify,
    define: defines(STATIC_ENV, minify),
  });
  if (!result.success) throw new AggregateError(result.logs, "bundling the app frame failed");
  const pick = (suffix: string) => result.outputs.find((f) => f.path.endsWith(suffix));
  const js = await pick(".js")?.text();
  const css = await pick(".css")?.text();
  if (js === undefined || css === undefined) {
    throw new Error(`the app frame build produced no ${js === undefined ? "script" : "stylesheet"}`);
  }
  return { js, css };
}

/* ── Messages for the pages without JavaScript ─────────────────────────── */
/* /basic/ is drawn in Rust, in this bundle's stylesheet, and says what the
 * script pages say: the catalogue in src/i18n/ is written out once per
 * language as plain JSON, basic/messages.<locale>.json, a key to its text
 * or its plural forms, which the `basic` plugin in backend/plugins/basic/
 * reads. nginx sends /basic/ to lososd, so the files are never served. */
export async function emitBasicMessages(dist: string): Promise<void> {
  const { MESSAGES } = (await import(`${SRC}/i18n/index.ts`)) as {
    MESSAGES: Record<string, Record<"en" | "sk" | "de", unknown>>;
  };
  for (const locale of ["en", "sk", "de"] as const) {
    const out: Record<string, unknown> = {};
    for (const [key, entry] of Object.entries(MESSAGES)) out[key] = entry[locale];
    await Bun.write(`${dist}/basic/messages.${locale}.json`, JSON.stringify(out));
  }
}

/* ── The two source patches ────────────────────────────────────────────── */
/* MediaPipe's tasks-vision posts usage counts to Google every minute, from
 * a logger nothing in its API turns off. The page's CSP refuses the
 * request anyway, but each refusal is a console error on a call. This
 * empties the logger's queue before it is sent, so nothing leaves the
 * page and nothing is refused. A MediaPipe release that changes the
 * logger, or drops it, fails the build here instead of calling out again. */
export function segmenterQuiet(): BunPlugin {
  const SEND = "flush(t,e){if(this.error)";
  const QUIET = "flush(t,e){this.i=[];if(this.error)";
  return {
    name: "losos:segmenter-quiet",
    setup(build) {
      build.onLoad({ filter: /@mediapipe[\\/]tasks-vision[\\/]vision_bundle/ }, async (args) => {
        const code = await Bun.file(args.path).text();
        if (code.split(SEND).length !== 2) {
          throw new Error("MediaPipe's usage logger changed: look at scripts/bundler.ts again");
        }
        return { loader: "js", contents: code.replace(SEND, QUIET) };
      });
    },
  };
}

/* Sonner writes its stylesheet into a <style> element when its module
 * loads. The admin CSP (`style-src 'self'`) refuses the element, so every
 * page logged two violations at load for a copy nothing used: main.tsx
 * imports `sonner/dist/styles.css`, and that file is what styles the stack
 * (it reaches the page through the registry instead). This drops the call,
 * so nothing is injected and nothing is refused. A Sonner release that
 * changes the call fails the build here instead of injecting again. */
export function sonnerQuiet(): BunPlugin {
  const INSERT = '__insertCSS("';
  const SKIP = '(function(){})("';
  return {
    name: "losos:sonner-quiet",
    setup(build) {
      build.onLoad({ filter: /[\\/]node_modules[\\/]sonner[\\/]/ }, async (args) => {
        if (!/[\\/]sonner[\\/]dist[\\/]index\.m?js$/.test(args.path)) return undefined;
        const code = await Bun.file(args.path).text();
        if (code.split(INSERT).length !== 2) {
          throw new Error("Sonner's stylesheet injection changed: look at scripts/bundler.ts again");
        }
        return { loader: "js", contents: code.replace(INSERT, SKIP) };
      });
    },
  };
}

/* ── The builds ────────────────────────────────────────────────────────── */
export interface PageSpec {
  /* Where the document sits, from admin-ui/app/: index.html at the top (the
   * admin SPA) and chat/ beside manage, cloud, kb, git and wiki; Diagrams'
   * hosted copy (VITE_LAB_HOSTED=1, admin-ui/lab/engine/build.sh) builds
   * its page at diagrams/index.html too, which a box never gets, so
   * nothing shadows the SPA's /diagrams screen. */
  html: string;
  entry: string;
}

export function pageSpecs(env: Pick<BuildEnv, "hosted">): PageSpec[] {
  return [
    { html: "index.html", entry: "src/main.tsx" },
    { html: "chat/index.html", entry: "src/main.tsx" },
    ...(env.hosted ? [{ html: "diagrams/index.html", entry: "src/lab/main.tsx" }] : []),
    { html: "manage/index.html", entry: "src/manage/main.tsx" },
    { html: "cloud/index.html", entry: "src/cloud/main.tsx" },
    { html: "kb/index.html", entry: "src/kb/main.tsx" },
    { html: "git/index.html", entry: "src/git/main.tsx" },
    { html: "wiki/index.html", entry: "src/wiki/main.tsx" },
  ];
}

export interface PageBuild {
  outputs: Bun.BuildArtifact[];
  /* For each HTML page, its entry chunk's path under the build root —
   * "assets/main-<hash>.js" — as the rewriter names it. */
  chunks: Map<string, string>;
}

export interface BundleResult {
  chunks: Map<string, string>;
  /* The app stylesheet's key in each build's registry, for the HTML links. */
  appKeys: { box: string; guest: string };
}

export async function bundleAll(env: BuildEnv, appCss: string): Promise<BundleResult> {
  const specs = pageSpecs(env);
  const entrypoints = [...new Set(specs.map((p) => `${ROOT}/${p.entry}`))];
  const boxCss = cssRegistry(env, appCss);
  const outputs = await runBuild(env, boxCss.plugin(), entrypoints, DIST);
  const chunks = new Map<string, string>();
  for (const spec of specs) {
    chunks.set(spec.html, chunkFor(outputs, DIST, spec.entry, "src"));
  }
  const boxAppKey = await boxCss.write(DIST);
  /* The guest call page, built on its own under dist/calls/guest/ after the
   * main build, per the comment above. Since its base differs, its CSS
   * registry is another one, so its assets/css/ links hang off
   * /calls/guest/ too. */
  const guestEnv: BuildEnv = { ...env, base: "/calls/guest/" };
  const guestOut = `${DIST}/calls/guest`;
  const guestCss = cssRegistry(guestEnv, appCss);
  const guest = await runBuild(guestEnv, guestCss.plugin(), [`${SRC}/calls/guest-main.tsx`], guestOut);
  chunks.set("guest.html", chunkFor(guest, guestOut, "src/calls/guest-main.tsx", "src/calls"));
  const guestAppKey = await guestCss.write(guestOut);
  return { chunks, appKeys: { box: boxAppKey, guest: guestAppKey } };
}

/* One Bun.build over a page set: hashed names, the stylesheet registry's
 * plugin, the pages module and the two source patches shared. */
async function runBuild(
  env: BuildEnv,
  cssPlugin: BunPlugin,
  entrypoints: string[],
  outDir: string,
): Promise<Bun.BuildArtifact[]> {
  const result = await Bun.build({
    entrypoints,
    outdir: outDir,
    target: "browser",
    format: "esm",
    splitting: true,
    minify: true,
    naming: hashedNaming(),
    define: defines(env, true),
    plugins: [wasmUrl(), cssPlugin, pagesVirtual(), sonnerQuiet(), segmenterQuiet()],
  });
  if (!result.success) throw new AggregateError(result.logs, `bundling ${entrypoints.join(", ")} failed`);
  return result.outputs;
}

/* Maps an entrypoint onto the hashed chunk the build emitted for it. Bun
 * names entry chunks `assets/<dir>/<name>-<hash>.js`, with `[dir]` relative
 * to the entrypoints' common ancestor — here, src/. */
function chunkFor(outputs: Bun.BuildArtifact[], outDir: string, entry: string, entryRoot: string): string {
  const relative = entry.replace(new RegExp(`^${entryRoot}/`), "");
  const dir = relative.includes("/") ? relative.slice(0, relative.lastIndexOf("/")) : "";
  const name = relative.slice(relative.lastIndexOf("/") + 1).replace(/\.[^.]+$/, "");
  const escape = (s: string): string => s.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const prefix = `${outDir}/assets/${dir === "" ? "" : `${dir}/`}`;
  const re = new RegExp(`^${escape(prefix)}${escape(name)}-[^/]+\\.js$`);
  const found = outputs.filter((o) => o.kind === "entry-point" && re.test(o.path));
  if (found.length !== 1) {
    throw new Error(
      `the bundle has no single entry chunk for ${entry}: ${found.map((o) => o.path).join(", ") || "none"}\nprefix: ${prefix}\noutputs: ${outputs.filter((o) => o.kind === "entry-point").map((o) => o.path).join(", ")}`,
    );
  }
  return found[0]!.path.slice(outDir.length).replace(/^\//, "");
}

/* ── The HTML pipeline ─────────────────────────────────────────────────── */
/* Writes a built HTML document: the module script's src becomes the hashed
 * chunk, the app stylesheet is linked in <head> before the page paints (the
 * chunked sheets a component's own import reaches for come from the
 * registry), and both the hand-written classic theme-boot tag and the
 * noscript meta stay exactly as the source carries them — the whole point
 * is that nothing else goes near the parser. */
export function rewriteHtml(html: string, entryTag: string, env: BuildEnv, chunkJs: string, appKey: string): string {
  const base = env.base;
  const script = `<script type="module" src="${base}${chunkJs}"></script>`;
  const out = html
    .replaceAll(`href="/src/assets/losos.png"`, `href="${base}assets/losos.png"`)
    .replaceAll(`src="/src/assets/losos.png"`, `src="${base}assets/losos.png"`)
    .replace(`<script type="module" src="${entryTag}"></script>`, script)
    .replace("</head>", `\n  <link rel="stylesheet" href="${base}assets/css/${appKey}.css">\n  </head>`);
  if (!out.includes(script)) {
    throw new Error(`an HTML page does not name ${entryTag} the way the bundler expects it to`);
  }
  return out;
}
