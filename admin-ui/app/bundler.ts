/* What Bun's bundler needs to know about this app, for build.ts and for the
 * dev server (dev.ts, through bunfig.toml), so a page is bundled the same
 * way in both:
 *
 *   - `losos:build`, a module of the build's settings read from the
 *     environment (the hosted Diagrams copy, the Lab's splash meme);
 *   - `losos:pages`, the default pages' HTML (pages.ts);
 *   - `<file>?url`, which the wasm-bindgen packages are imported as: the
 *     file is copied into assets/ and the import is its URL;
 *   - `new URL("<file>", import.meta.url)` in our source and in the Matrix
 *     crypto package, which Bun leaves to the browser: the file is copied
 *     the same way, and a .ts file (the call's e2ee worker) is bundled on
 *     its own first (workers());
 *   - two packages that reach outside the page, quieted at the source
 *     (sonner's <style>, MediaPipe's usage logger).
 *
 * A package release that changes what is patched fails the build here
 * rather than shipping the old behaviour again. */
import type { BunPlugin, PluginBuilder } from "bun";
import { compilePages } from "./pages";

const root = import.meta.dirname;

/* The settings `losos:build` exports, besides BASE, the public path the
 * page is built for. LOSOS_LAB_HOSTED=1 builds the hosted
 * copy of Diagrams (admin-ui/lab/engine/build.ts); LOSOS_LAB_MEME names a
 * folder with no.jpg and yes.jpg for the Lab's splash, which build.ts copies
 * beside dist/diagrams/index.html. */
export function settings() {
  return {
    LAB_HOSTED: Bun.env["LOSOS_LAB_HOSTED"] === "1",
    LAB_MEME: (Bun.env["LOSOS_LAB_MEME"] ?? "") !== "",
  };
}

/* Sonner writes its stylesheet into a <style> element when its module
 * loads. The admin CSP (`style-src 'self'`) refuses the element, so every
 * page logged two violations at load for a copy nothing used: main.tsx
 * imports `sonner/dist/styles.css` into the bundle, and that file is what
 * styles the stack. This drops the call, so nothing is injected and
 * nothing is refused. */
const SONNER = { file: /[\\/]node_modules[\\/]sonner[\\/]dist[\\/]index\.m?js$/, from: '__insertCSS("', to: '(function(){})("' };

/* MediaPipe's tasks-vision posts usage counts to Google every minute, from
 * a logger nothing in its API turns off. The page's CSP refuses the
 * request anyway, but each refusal is a console error on a call. This
 * empties the logger's queue before it is sent, so nothing leaves the
 * page and nothing is refused. */
const MEDIAPIPE = {
  file: /[\\/]node_modules[\\/]@mediapipe[\\/]tasks-vision[\\/]vision_bundle\.m?js$/,
  from: "flush(t,e){if(this.error)",
  to: "flush(t,e){this.i=[];if(this.error)",
};

/* The files whose `new URL("…", import.meta.url)` are rewritten: our own
 * source and the Matrix crypto package's wasm. Others (ONNX Runtime's, whose
 * files nudity.ts and mouth.ts point at /ort/ instead) are left as they are. */
const URL_SOURCES = /[\\/](src[\\/].*\.(ts|tsx)|@matrix-org[\\/]matrix-sdk-crypto-wasm[\\/]index\.mjs)$/;
const NEW_URL = /new URL\(\s*(["'])(\.{1,2}\/[^"']+)\1\s*,\s*import\.meta\.url\s*\)/g;

function loaderOf(path: string): "ts" | "tsx" | "js" {
  if (path.endsWith(".tsx")) return "tsx";
  if (path.endsWith(".ts")) return "ts";
  return "js";
}

function patch(code: string, rule: { from: string; to: string }, what: string): string {
  if (code.split(rule.from).length !== 2) throw new Error(`${what} changed: look at bundler.ts again`);
  return code.replace(rule.from, rule.to);
}

export type Options = {
  publicPath: string;
  /* The URL of each worker, by its source file (workers() below). */
  workers: Map<string, string>;
};

/* The files `new URL("./w.ts", import.meta.url)` names in our source: the
 * call's e2ee worker. Each is a page of its own to the browser, so it is
 * bundled by itself, before the pages (Bun.build cannot run inside one of
 * its own plugins), at assets/<name>-<hash>.js. Without an outdir (the dev
 * server) the outputs stay in memory, keyed by the path they are served
 * at. */
export async function workers(options: { minify: boolean; publicPath: string; outdir?: string }) {
  const urls = new Map<string, string>();
  const files = new Map<string, Blob>();
  for await (const rel of new Bun.Glob("src/**/*.{ts,tsx}").scan(root)) {
    const path = `${root}/${rel}`;
    const dir = path.replace(/[\\/][^\\/]*$/, "");
    for (const m of (await Bun.file(path).text()).matchAll(NEW_URL)) {
      const target = m[2]!;
      if (!/\.(ts|tsx)$/.test(target)) continue;
      const entry = Bun.resolveSync(target, dir);
      if (urls.has(entry)) continue;
      const result = await Bun.build({
        entrypoints: [entry],
        target: "browser",
        format: "esm",
        minify: options.minify,
        naming: "assets/[name]-[hash].[ext]",
        publicPath: options.publicPath,
        throw: false,
        ...(options.outdir ? { outdir: options.outdir } : {}),
      });
      if (!result.success) throw new AggregateError(result.logs, `bundling the worker ${entry} failed`);
      const out = result.outputs[0]!;
      const name = out.path.slice(out.path.lastIndexOf("assets/"));
      urls.set(entry, `${options.publicPath}${name}`);
      files.set(`/${name}`, out);
    }
  }
  return { urls, files };
}

export function losos(options: Options): BunPlugin {
  return {
    name: "losos",
    setup(build: PluginBuilder) {
      build.onResolve({ filter: /^losos:build$/ }, () => ({ path: "losos:build", namespace: "losos-build" }));
      build.onLoad({ filter: /.*/, namespace: "losos-build" }, () => ({
        contents: Object.entries({ ...settings(), BASE: options.publicPath })
          .map(([k, v]) => `export const ${k} = ${JSON.stringify(v)};`)
          .join("\n"),
        loader: "js",
      }));

      build.onResolve({ filter: /^losos:pages$/ }, () => ({ path: "losos:pages", namespace: "losos-pages" }));
      build.onLoad({ filter: /.*/, namespace: "losos-pages" }, async () => ({
        contents: `export const DEFAULTS = ${JSON.stringify((await compilePages(root)).html)};`,
        loader: "js",
      }));

      // `x.wasm?url`: the file, copied and hashed, as a URL.
      build.onResolve({ filter: /\?url$/ }, (args) => ({
        path: Bun.resolveSync(args.path.slice(0, -"?url".length), args.resolveDir),
        namespace: "losos-url",
      }));
      build.onLoad({ filter: /.*/, namespace: "losos-url" }, async (args) => ({
        contents: await Bun.file(args.path).bytes(),
        loader: "file",
      }));

      build.onLoad({ filter: SONNER.file }, async (args) => ({
        contents: patch(await Bun.file(args.path).text(), SONNER, "Sonner's stylesheet injection"),
        loader: "js",
      }));
      build.onLoad({ filter: MEDIAPIPE.file }, async (args) => ({
        contents: patch(await Bun.file(args.path).text(), MEDIAPIPE, "MediaPipe's usage logger"),
        loader: "js",
      }));

      build.onLoad({ filter: URL_SOURCES }, async (args) => {
        const code = await Bun.file(args.path).text();
        const loader = loaderOf(args.path);
        if (!code.includes("import.meta.url")) return { contents: code, loader };
        const dir = args.path.replace(/[\\/][^\\/]*$/, "");
        const imports: string[] = [];
        const marked = code.replace(NEW_URL, (_all, _q, rel: string) => {
          if (/\.(ts|tsx)$/.test(rel)) {
            const url = options.workers.get(Bun.resolveSync(rel, dir));
            if (!url) throw new Error(`${args.path}: no worker built for ${rel}`);
            return `new URL(${JSON.stringify(url)}, import.meta.url)`;
          }
          imports.push(`import __losos_url_${imports.length} from ${JSON.stringify(rel)} with { type: "file" };`);
          return `new URL(__losos_url_${imports.length - 1}, import.meta.url)`;
        });
        return { contents: [...imports, marked].join("\n"), loader };
      });
    },
  };
}


/* The dev server's copy (bunfig.toml's [serve.static] plugins), at the
 * root path. dev.ts fills in its workers before it serves a page. */
export const devWorkers = new Map<string, string>();
export default losos({ publicPath: "/", workers: devWorkers });
