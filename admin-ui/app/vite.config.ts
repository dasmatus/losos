import { defineConfig, type Plugin, type ResolvedConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import { build as esbuild } from "esbuild";

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

export default defineConfig({
  plugins: [react(), tailwindcss(), themeBoot()],
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
    },
  },
});
