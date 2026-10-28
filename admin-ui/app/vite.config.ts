import { defineConfig, type Plugin, type ResolvedConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import { build as esbuild } from "esbuild";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { segmenterQuiet } from "./vite.segmenter-quiet";

/* The theme has to be on the <html> element BEFORE the first paint, or a box
 * whose owner picked Dark while the browser reports Light flashes white for a
 * frame. The usual fix is an inline <script> in <head>; the appliance's CSP
 * (`script-src 'self'`, see modules/containers.nix) forbids inline script, so
 * that door is shut.
 *
 * So the boot snippet is a real file: src/theme-boot.ts, compiled here to a
 * standalone IIFE and served at /theme-boot.js. index.html loads it as a
 * CLASSIC script — no `type="module"`, no `defer`, no `async` — which is the
 * one remaining way to block the parser and run before anything is painted.
 * A module script would not do: module scripts are deferred by definition and
 * run after the document is parsed, which is after the first paint.
 *
 * Deliberately NOT content-hashed. The tag in index.html is written by hand
 * (it cannot be rewritten by Vite's HTML pipeline, which only touches module
 * scripts), so the name has to be stable. It is ~200 bytes of logic that has
 * no reason to change, nginx serves it with an ETag off a per-build store
 * path, and it is the only unhashed file in dist/.
 */
const THEME_BOOT_FILE = "theme-boot.js";
const THEME_BOOT_ENTRY = "src/theme-boot.ts";

function themeBoot(): Plugin {
  let config: ResolvedConfig;

  const compile = async (minify: boolean): Promise<string> => {
    const result = await esbuild({
      absWorkingDir: config.root,
      entryPoints: [THEME_BOOT_ENTRY],
      bundle: true,
      format: "iife",
      target: "es2020",
      minify,
      write: false,
      legalComments: "none",
    });
    return result.outputFiles[0]?.text ?? "";
  };

  return {
    name: "losos:theme-boot",
    configResolved(resolved) {
      config = resolved;
    },
    // Dev: serve the same URL from memory so the dev page and the built page
    // load the identical tag. No CSP header in dev, but keeping the two in
    // step is how a CSP regression stays visible in dev at all.
    configureServer(server) {
      server.middlewares.use(`/${THEME_BOOT_FILE}`, (_req, res) => {
        void compile(false).then((code) => {
          res.setHeader("Content-Type", "text/javascript");
          res.setHeader("Cache-Control", "no-store");
          res.end(code);
        });
      });
    },
    async buildStart() {
      if (config.command !== "build") return;
      this.emitFile({
        type: "asset",
        fileName: THEME_BOOT_FILE,
        source: await compile(true),
      });
    },
  };
}

/* The app frame's runtime (src/app-frame/runtime.tsx): React, the LosOS
 * components and the bridge, bundled by esbuild into one classic script and
 * its stylesheet at fixed names, /app-frame/runtime.js and runtime.css,
 * which public/app-frame/index.html loads. Classic and unhashed for the
 * reasons runtime.tsx gives: the frame is an opaque origin, where a module
 * script would be fetched in CORS mode, and the page that loads it is a
 * static file Vite's HTML pipeline does not rewrite. */
const APP_FRAME_ENTRY = "src/app-frame/runtime.tsx";

function appFrame(): Plugin {
  let config: ResolvedConfig;

  const compile = async (minify: boolean): Promise<{ js: string; css: string }> => {
    const result = await esbuild({
      absWorkingDir: config.root,
      entryPoints: [APP_FRAME_ENTRY],
      bundle: true,
      format: "iife",
      target: "es2020",
      jsx: "automatic",
      minify,
      write: false,
      outfile: "app-frame/runtime.js",
      legalComments: "none",
      define: { "process.env.NODE_ENV": JSON.stringify(minify ? "production" : "development") },
    });
    const pick = (ext: string) => result.outputFiles.find((f) => f.path.endsWith(ext))?.text ?? "";
    return { js: pick(".js"), css: pick(".css") };
  };

  return {
    name: "losos:app-frame",
    configResolved(resolved) {
      config = resolved;
    },
    configureServer(server) {
      for (const [file, type] of [
        ["runtime.js", "text/javascript"],
        ["runtime.css", "text/css"],
      ] as const) {
        server.middlewares.use(`/app-frame/${file}`, (_req, res) => {
          void compile(false).then((out) => {
            res.setHeader("Content-Type", type);
            res.setHeader("Cache-Control", "no-store");
            res.end(file.endsWith(".js") ? out.js : out.css);
          });
        });
      }
    },
    async buildStart() {
      if (config.command !== "build") return;
      const out = await compile(true);
      this.emitFile({ type: "asset", fileName: "app-frame/runtime.js", source: out.js });
      this.emitFile({ type: "asset", fileName: "app-frame/runtime.css", source: out.css });
    },
  };
}

/* LosOS Lab's splash can carry a two-panel meme: LOSOS_LAB_MEME names a
 * folder with no.jpg and yes.jpg, which are copied beside dist/diagrams/index.html
 * as meme-no.jpg and meme-yes.jpg, and __LAB_MEME__ tells the page they are
 * there. The repository carries no such folder (admin-ui/lab/build.py reads
 * the same variable for the old page). */
/* The black background behind a person on a call (src/calls/background.ts)
 * runs MediaPipe's selfie segmenter on the device. Its WebAssembly comes
 * from the npm package and its model from LOSOS_SEGMENTER_MODEL, which
 * nix/packages.nix points at a fixed download; both land beside the chat
 * page under chat/calls-ml/, so the page loads them from the box itself. A
 * build without the model still builds, and a call then sends no picture
 * with the black background on rather than the room. */
function segmenterFiles(): Plugin {
  const wasm = "node_modules/@mediapipe/tasks-vision/wasm";
  const files = (): [string, string][] => {
    const list: [string, string][] = [
      ["vision_wasm_internal.js", join(wasm, "vision_wasm_internal.js")],
      ["vision_wasm_internal.wasm", join(wasm, "vision_wasm_internal.wasm")],
    ];
    const model = process.env["LOSOS_SEGMENTER_MODEL"];
    if (model) list.push(["selfie_segmenter.tflite", model]);
    return list;
  };
  return {
    name: "losos:segmenter-files",
    configureServer(server) {
      for (const [name, src] of files()) {
        server.middlewares.use(`/chat/calls-ml/${name}`, (_req, res) => {
          res.end(readFileSync(src));
        });
      }
    },
    buildStart() {
      for (const [name, src] of files()) {
        this.emitFile({ type: "asset", fileName: `chat/calls-ml/${name}`, source: readFileSync(src) });
      }
    },
  };
}

/* The nudity warning (src/lib/nudity.ts) runs ONNX Runtime's WebAssembly
 * build on a ViT-Tiny from LOSOS_NUDITY_MODEL, which nix/packages.nix pins
 * to one download. Both land under nudity/, outside assets/, so the 14 MB
 * runtime and the 22 MB model load only on a page whose owner turned the
 * warning on. A build without the model still builds; the warning then
 * finds no model and lets every picture through as it was before. */
function nudityFiles(): Plugin {
  const ort = "node_modules/onnxruntime-web/dist";
  const files = (): [string, string][] => {
    const list: [string, string][] = [
      ["ort-wasm-simd-threaded.mjs", join(ort, "ort-wasm-simd-threaded.mjs")],
      ["ort-wasm-simd-threaded.wasm", join(ort, "ort-wasm-simd-threaded.wasm")],
    ];
    const model = process.env["LOSOS_NUDITY_MODEL"];
    if (model) list.push(["model.onnx", model]);
    return list;
  };
  return {
    name: "losos:nudity-files",
    configureServer(server) {
      for (const [name, src] of files()) {
        server.middlewares.use(`/nudity/${name}`, (_req, res) => {
          res.end(readFileSync(src));
        });
      }
    },
    buildStart() {
      for (const [name, src] of files()) {
        this.emitFile({ type: "asset", fileName: `nudity/${name}`, source: readFileSync(src) });
      }
    },
  };
}

function labMeme(): Plugin {
  const dir = process.env["LOSOS_LAB_MEME"] || "";
  const files = { "meme-no.jpg": "no.jpg", "meme-yes.jpg": "yes.jpg" } as const;
  return {
    name: "losos:lab-meme",
    config: () => ({ define: { __LAB_MEME__: JSON.stringify(dir !== "") } }),
    configureServer(server) {
      if (!dir) return;
      for (const [name, src] of Object.entries(files)) {
        server.middlewares.use(`/diagrams/${name}`, (_req, res) => {
          res.setHeader("Content-Type", "image/jpeg");
          res.end(readFileSync(join(dir, src)));
        });
      }
    },
    buildStart() {
      if (!dir) return;
      for (const [name, src] of Object.entries(files)) {
        this.emitFile({ type: "asset", fileName: `diagrams/${name}`, source: readFileSync(join(dir, src)) });
      }
    },
  };
}

const root = fileURLToPath(new URL(".", import.meta.url));

export default defineConfig({
  plugins: [react(), tailwindcss(), themeBoot(), appFrame(), labMeme(), segmenterFiles(), nudityFiles(), segmenterQuiet()],
  // Served from the vhost root by nginx.
  base: "/",
  resolve: {
    alias: [{ find: /^@\//, replacement: "/src/" }],
  },
  build: {
    target: "es2022",
    // Hashed filenames for everything the HTML pipeline owns; theme-boot.js is
    // emitted separately at a fixed name (see above).
    assetsDir: "assets",
    sourcemap: false,
    // The appliance has no external hosts and no CDN: one origin, one bundle.
    modulePreload: { polyfill: false },
    // Seven pages: the admin SPA at / (LosOS Diagrams is its screen at
    // /diagrams), LosOS Chat at /chat/ (the admin SPA's own document under
    // chat's CSP, which adds blob: media for decrypted voice messages and
    // videos; modules/chat.nix), a company edge's manage page, which the
    // edge serves at /manage/ from this same dist/
    // (edge/registrar/src/server/manage.rs), LosOS Cloud at /cloud/ and the
    // knowledge bases published from it at /kb/ (modules/cloud.nix), and
    // LosOS Git at /git/ with its spaces published as sites at /wiki/
    // (modules/containers.nix). They share chunks. The
    // hosted copy of Diagrams (VITE_LAB_HOSTED=1,
    // admin-ui/lab/engine/build.sh) adds diagrams/index.html, the same
    // screen on a page of its own; a box never gets that page, so nothing
    // shadows the SPA's /diagrams.
    rolldownOptions: {
      input: {
        main: join(root, "index.html"),
        chat: join(root, "chat/index.html"),
        ...(process.env["VITE_LAB_HOSTED"] === "1" ? { diagrams: join(root, "diagrams/index.html") } : {}),
        manage: join(root, "manage/index.html"),
        cloud: join(root, "cloud/index.html"),
        kb: join(root, "kb/index.html"),
        git: join(root, "git/index.html"),
        wiki: join(root, "wiki/index.html"),
      },
    },
  },
  server: {
    port: 5173,
    proxy: {
      // `npm run dev` against a real box: point this at the appliance and the
      // Bearer-authed API answers on the dev origin too.
      "/api": {
        target: process.env["LOSOS_API_ORIGIN"] ?? "http://127.0.0.1:8082",
        changeOrigin: true,
      },
      // LosOS Cloud's API: `losos-cloud serve --listen 127.0.0.1:8095
      // --trust-header` and a header-setting proxy, or a box. Host stays the
      // dev origin's: the service refuses a write whose Origin names another
      // host than Host, and the browser's Origin is this dev server.
      "/cloud/api": {
        target: process.env["LOSOS_CLOUD_ORIGIN"] ?? "http://127.0.0.1:8095",
        changeOrigin: false,
        headers: { "X-Losos-Cloud-User": process.env["LOSOS_CLOUD_USER"] ?? "owner" },
      },
      // A knowledge base's data, read as nobody.
      "/kb/api": {
        target: process.env["LOSOS_CLOUD_ORIGIN"] ?? "http://127.0.0.1:8095",
        changeOrigin: false,
      },
      // The first-run wizard's step 1 reads /setup/state.json — the box's name
      // and its certificate fingerprint — and on the appliance that file is
      // served by nginx from disk rather than by lososd (modules/setup.nix),
      // because the wizard has to name the box and show the fingerprint
      // *before* the owner has trusted the connection or holds a token. Without
      // this entry the dev origin falls through to the SPA's index.html, the
      // document parses as HTML, and step 1 reports the box as too old to have
      // a setup route at all.
      "/setup": {
        target: process.env["LOSOS_API_ORIGIN"] ?? "http://127.0.0.1:8082",
        changeOrigin: true,
      },
    },
  },
});
