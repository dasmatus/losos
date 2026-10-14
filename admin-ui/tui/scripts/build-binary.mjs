// One file with its own runtime, dist/losos, from the bundle dist/losos.mjs:
// `bun build --compile`. scripts/verify-binary.ts drives the result through a
// pseudo-terminal (login screen, full-screen view, hidden prompt) and the
// plain CLI, which is how this route was chosen over a Node single
// executable: those need a CommonJS entry, and Ink's layout engine loads
// with a top-level await that CommonJS cannot hold.
//
// bun is looked for in $BUN, on PATH, then in ~/.bun/bin.

import { spawnSync } from "node:child_process";
import { chmodSync, existsSync, statSync } from "node:fs";
import { homedir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const bundle = join(root, "dist", "losos.mjs");
const outfile = join(root, "dist", "losos");

if (!existsSync(bundle)) {
  console.error("build:binary: dist/losos.mjs is missing. npm run build makes it.");
  process.exit(1);
}

function findBun() {
  const candidates = [process.env.BUN, "bun", join(homedir(), ".bun", "bin", "bun")].filter(Boolean);
  for (const candidate of candidates) {
    const probe = spawnSync(candidate, ["--version"], { encoding: "utf8" });
    if (probe.status === 0) return { path: candidate, version: probe.stdout.trim() };
  }
  return null;
}

const bun = findBun();
if (bun === null) {
  console.error("build:binary: bun is not installed. Install it from bun.sh, or set BUN to its path, and run this again.");
  process.exit(1);
}

const run = spawnSync(
  bun.path,
  ["build", "--compile", "--target=bun-linux-x64", "--minify-whitespace", bundle, "--outfile", outfile],
  { stdio: "inherit" },
);
if (run.status !== 0) process.exit(run.status ?? 1);
chmodSync(outfile, 0o755);

const check = spawnSync(outfile, ["--version"], { encoding: "utf8" });
if (check.status !== 0) {
  console.error(`build:binary: dist/losos does not start: ${check.stderr.trim()}`);
  process.exit(1);
}
const mib = (statSync(outfile).size / 1024 / 1024).toFixed(1);
console.log(`build:binary: dist/losos (${mib} MiB, bun ${bun.version}) says ${check.stdout.trim()}`);
