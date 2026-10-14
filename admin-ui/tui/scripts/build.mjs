// Bundles src/main.ts and everything it imports (Ink, React, openpgp.js and
// the web UI's message catalogue) into one file, dist/losos.mjs, for node 20
// and later.

import { build } from "esbuild";
import { chmodSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const pkg = JSON.parse(readFileSync(join(root, "package.json"), "utf8"));
const outfile = join(root, "dist", "losos.mjs");

await build({
  entryPoints: [join(root, "src", "main.ts")],
  outfile,
  bundle: true,
  platform: "node",
  format: "esm",
  target: "node20",
  jsx: "automatic",
  minify: false,
  legalComments: "eof",
  tsconfig: join(root, "tsconfig.json"),
  define: {
    __LOSOS_VERSION__: JSON.stringify(pkg.version),
    "process.env.NODE_ENV": JSON.stringify("production"),
    "process.env.DEV": JSON.stringify("false"),
  },
  // Ink connects to React's devtools only when DEV=true. The package is an
  // optional peer that is not installed; an empty module stands in for it.
  plugins: [
    {
      name: "no-devtools",
      setup(b) {
        b.onResolve({ filter: /^react-devtools-core$/ }, () => ({ path: "react-devtools-core", namespace: "stub" }));
        b.onLoad({ filter: /.*/, namespace: "stub" }, () => ({
          contents: "export default { initialize() {}, connectToDevTools() {} };",
          loader: "js",
        }));
      },
    },
  ],
  // CommonJS dependencies inside an ES module bundle call require().
  banner: {
    js: "import { createRequire as __losos_createRequire } from 'node:module';\nconst require = __losos_createRequire(import.meta.url);",
  },
  logLevel: "warning",
});
chmodSync(outfile, 0o755);
console.log(`build: ${outfile}`);
