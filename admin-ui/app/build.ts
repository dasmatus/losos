/* The production build, on Bun's bundler: `bun run build` writes dist/,
 * which is the front nginx vhost's document root.
 *
 *   bun build.ts [--outdir DIR]
 *
 * Eight pages share one assets/ folder (their chunks are split between
 * them), the guest call page is built again on its own under calls/guest/,
 * and a handful of files are written at fixed names beside them. */
import tailwind from "bun-plugin-tailwind";
import { losos, settings, workers } from "./bundler";
import { basicClasses } from "./scripts/basic-classes.ts";
import { compilePages } from "./pages";

const root = import.meta.dirname;
/* `--outdir DIR` writes the build there instead of dist/ (the hosted copy
 * of Diagrams, admin-ui/lab/engine/build.ts). */
const at = Bun.argv.indexOf("--outdir");
const dist = at > 0 && Bun.argv[at + 1] ? Bun.argv[at + 1]! : `${root}/dist`;

/* Bun's bundler for the classic scripts below: one file, no imports left.
 * A failed build throws with Bun's messages rather than emitting nothing. */
async function bundle(config: Bun.BuildConfig): Promise<Bun.BuildOutput> {
  const result = await Bun.build({ target: "browser", throw: false, ...config });
  if (!result.success) {
    for (const log of result.logs) console.error(log);
    throw new Error(`bundling ${config.entrypoints.join(", ")} failed`);
  }
  return result;
}

const naming = {
  entry: "[dir]/[name].[ext]",
  chunk: "assets/[name]-[hash].[ext]",
  asset: "assets/[name]-[hash].[ext]",
};

/* The theme has to be on the <html> element BEFORE the first paint, or a
 * box whose owner picked Dark while the browser reports Light flashes white
 * for a frame. The usual fix is an inline <script> in <head>; the
 * appliance's CSP (`script-src 'self'`, see modules/containers.nix) forbids
 * inline script, so that door is shut.
 *
 * So the boot snippet is a real file: src/theme-boot.ts, compiled here to a
 * standalone IIFE and served at /theme-boot.js, which each page loads as a
 * CLASSIC script, no `type="module"`, no `defer`, no `async`: the one
 * remaining way to block the parser and run before anything is painted.
 * The bundler turns every <script src> it sees into the page's module
 * bundle, so the page carries a placeholder,
 * <meta name="losos:classic-script" content="/theme-boot.js">, which
 * classicScripts() below (and dev.ts) turns into the tag.
 *
 * Deliberately NOT content-hashed: the tag names it, so the name has to be
 * stable. It is ~200 bytes of logic that has no reason to change, and nginx
 * serves it with an ETag off a per-build store path. */
export async function themeBoot(minify: boolean): Promise<string> {
  const result = await bundle({ entrypoints: [`${root}/src/theme-boot.ts`], format: "iife", minify });
  return result.outputs[0]!.text();
}

/* The app frame's runtime (src/app-frame/runtime.tsx): React, the LosOS
 * components and the bridge, bundled into one classic script and its
 * stylesheet at fixed names, /app-frame/runtime.js and runtime.css, which
 * public/app-frame/index.html loads. Classic and unhashed for the reasons
 * runtime.tsx gives: the frame is an opaque origin, where a module script
 * would be fetched in CORS mode, and the page that loads it is a static
 * file the bundler does not rewrite. */
export async function appFrame(minify: boolean): Promise<{ js: string; css: string }> {
  const result = await bundle({
    entrypoints: [`${root}/src/app-frame/runtime.tsx`],
    format: "iife",
    minify,
    plugins: [tailwind],
    define: { "process.env.NODE_ENV": JSON.stringify(minify ? "production" : "development") },
  });
  const pick = async (ext: string) => (await result.outputs.find((f) => f.path.endsWith(ext))?.text()) ?? "";
  return { js: await pick(".js"), css: await pick(".css") };
}

/* The placeholder → the classic <script> it stands for. */
export function classicScripts(html: Response | string): Response {
  return new HTMLRewriter()
    .on('meta[name="losos:classic-script"]', {
      element(el) {
        const src = el.getAttribute("content") ?? "";
        el.replace(`<script src="${src}"></script>`, { html: true });
      },
    })
    .transform(typeof html === "string" ? new Response(html) : html);
}

/* The black background behind a person on a call (src/calls/background.ts)
 * runs MediaPipe's selfie segmenter on the device. Its WebAssembly comes
 * from the @mediapipe/tasks-vision package and its model from
 * LOSOS_SEGMENTER_MODEL, which nix/packages.nix points at a fixed download;
 * both land beside the chat page under chat/calls-ml/, so the page loads
 * them from the box itself. A build without the model still builds, and a
 * call then sends no picture with the black background on rather than the
 * room.
 *
 * The nudity warning (src/lib/nudity.ts) runs ONNX Runtime's WebAssembly
 * build on a ViT-Tiny from LOSOS_NUDITY_MODEL, which nix/packages.nix pins
 * to one download. The runtime lands under ort/ — Kokoro (src/voice/mouth.ts)
 * loads the same files — and the model under nudity/, both outside assets/,
 * so the 14 MB runtime and the 22 MB model load only when a page asks.
 *
 * LosOS Lab's splash can carry a two-panel meme: LOSOS_LAB_MEME names a
 * folder with no.jpg and yes.jpg, copied beside dist/diagrams/index.html as
 * meme-no.jpg and meme-yes.jpg; `losos:build`'s LAB_MEME tells the page they
 * are there. The repository carries no such folder.
 *
 * Each is [served path, source file]; dev.ts serves the same list. */
export function copiedFiles(): [string, string][] {
  const wasm = `${root}/node_modules/@mediapipe/tasks-vision/wasm`;
  const ort = `${root}/node_modules/onnxruntime-web/dist`;
  const list: [string, string][] = [
    ["chat/calls-ml/vision_wasm_internal.js", `${wasm}/vision_wasm_internal.js`],
    ["chat/calls-ml/vision_wasm_internal.wasm", `${wasm}/vision_wasm_internal.wasm`],
    ["ort/ort-wasm-simd-threaded.mjs", `${ort}/ort-wasm-simd-threaded.mjs`],
    ["ort/ort-wasm-simd-threaded.wasm", `${ort}/ort-wasm-simd-threaded.wasm`],
  ];
  const segmenter = Bun.env["LOSOS_SEGMENTER_MODEL"];
  if (segmenter) list.push(["chat/calls-ml/selfie_segmenter.tflite", segmenter]);
  const nudity = Bun.env["LOSOS_NUDITY_MODEL"];
  if (nudity) list.push(["nudity/model.onnx", nudity]);
  const meme = Bun.env["LOSOS_LAB_MEME"];
  if (meme) list.push(["diagrams/meme-no.jpg", `${meme}/no.jpg`], ["diagrams/meme-yes.jpg", `${meme}/yes.jpg`]);
  return list;
}

/* The pages for a browser with JavaScript off (/basic/, the `basic` plugin
 * in backend/plugins/basic/) are drawn by lososd in Rust, in this bundle's
 * stylesheet, and say what the script pages say. So the catalogue in
 * src/i18n/ is written out once per language as plain JSON,
 * basic/messages.<locale>.json, a key to its text or its plural forms, and
 * the plugin reads the copy in this package. nginx sends /basic/ to lososd,
 * so the files are never served. */
async function basicMessages() {
  // Bun runs the TypeScript catalogue as it is.
  const { MESSAGES } = (await import(`${root}/src/i18n/index.ts`)) as {
    MESSAGES: Record<string, Record<"en" | "sk" | "de", unknown>>;
  };
  for (const locale of ["en", "sk", "de"] as const) {
    const out: Record<string, unknown> = {};
    for (const [key, entry] of Object.entries(MESSAGES)) out[key] = entry[locale];
    await Bun.write(`${dist}/basic/messages.${locale}.json`, JSON.stringify(out));
  }
}

/* The default pages' sources and their table, beside the bundle under
 * pages/, for the `pages` plugin (pages.ts says why). */
async function pageSources() {
  const { registry, sources } = await compilePages(root);
  await Bun.write(`${dist}/pages/blocks.json`, registry);
  for (const [id, source] of Object.entries(sources)) await Bun.write(`${dist}/pages/${id}.mdx`, source);
}

/* Every page's HTML as the bundler wrote it, with its placeholders made
 * into the classic tags. */
async function finishPages(outputs: Bun.BuildArtifact[]) {
  for (const out of outputs) {
    if (!out.path.endsWith(".html")) continue;
    await Bun.write(out.path, await classicScripts(await out.text()).text());
  }
}

/* Seven pages: the admin SPA at / (LosOS Diagrams is its screen at
 * /diagrams), LosOS Chat at /chat/ (the admin SPA's own document under
 * chat's CSP, which adds blob: media for decrypted voice messages and
 * videos; modules/chat.nix), a company edge's manage page, which the edge
 * serves at /manage/ from this same dist/
 * (edge/registrar/src/server/manage.rs), LosOS Cloud at /cloud/ and the
 * knowledge bases published from it at /kb/ (modules/cloud.nix), and LosOS
 * Git at /git/ with its spaces published as sites at /wiki/
 * (modules/containers.nix). They share chunks. The hosted copy of Diagrams
 * (LOSOS_LAB_HOSTED=1, admin-ui/lab/engine/build.ts) adds
 * diagrams/index.html, the same screen on a page of its own; a box never
 * gets that page, so nothing shadows the SPA's /diagrams. */
export function pages(): string[] {
  const list = ["index.html", "chat/index.html", "manage/index.html", "cloud/index.html", "kb/index.html", "git/index.html", "wiki/index.html"];
  if (settings().LAB_HOSTED) list.push("diagrams/index.html");
  return list.map((p) => `${root}/${p}`);
}

const production = { "process.env.NODE_ENV": '"production"' };

async function main() {
  // Tailwind scans the class list this writes (src/styles/index.css).
  await basicClasses();
  await Bun.$`rm -rf ${dist}`;
  await Bun.$`mkdir -p ${dist}`;
  // public/ as it is: the widget frame, the app frame's page, the
  // backgrounds.
  await Bun.$`cp -r ${root}/public/. ${dist}/`;

  const siteWorkers = await workers({ minify: true, publicPath: "/", outdir: dist });
  const site = await bundle({
    entrypoints: pages(),
    outdir: dist,
    root,
    splitting: true,
    minify: true,
    publicPath: "/",
    naming,
    define: production,
    plugins: [tailwind, losos({ publicPath: "/", workers: siteWorkers.urls })],
  });
  await finishPages(site.outputs);

  /* The guest call page (guest.html, src/calls/guest-main.tsx), built on
   * its own under /calls/guest/. nginx serves this folder to anyone holding
   * a guest link, so it carries its own copy of every chunk it needs rather
   * than the LAN-only /assets/. The segmenter's files are not copied
   * again: nginx serves dist/chat/calls-ml/ under the guest path too
   * (modules/calls.nix), which is where background.ts looks. */
  const guestDir = `${dist}/calls/guest`;
  const guestWorkers = await workers({ minify: true, publicPath: "/calls/guest/", outdir: guestDir });
  const guest = await bundle({
    entrypoints: [`${root}/guest.html`],
    outdir: guestDir,
    root,
    splitting: true,
    minify: true,
    publicPath: "/calls/guest/",
    naming,
    define: production,
    plugins: [tailwind, losos({ publicPath: "/calls/guest/", workers: guestWorkers.urls })],
  });
  await finishPages(guest.outputs);

  await Bun.write(`${dist}/theme-boot.js`, await themeBoot(true));
  const frame = await appFrame(true);
  await Bun.write(`${dist}/app-frame/runtime.js`, frame.js);
  await Bun.write(`${dist}/app-frame/runtime.css`, frame.css);
  await basicMessages();
  await pageSources();
  for (const [path, src] of copiedFiles()) await Bun.write(`${dist}/${path}`, Bun.file(src));
}

if (import.meta.main) await main();
