#!/usr/bin/env node
/* Builds LosOS Lab's core (admin-ui/lab/core, Rust) to WebAssembly and writes
 * the wasm-bindgen package the Lab page imports into src/lab/core-pkg/.
 *
 *   npm run lab:core
 *
 * Needs cargo with the wasm32-unknown-unknown target and wasm-bindgen-cli at
 * exactly the version Cargo.toml pins (0.2.127): on PATH, or named by
 * $WASM_BINDGEN. The folder it writes is gitignored; `npm run typecheck` and
 * `npm run build` need it to exist, so run this first. */

import { spawnSync } from 'node:child_process';
import { mkdirSync, rmSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const app = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const crate = resolve(app, '../lab/core');
const out = join(app, 'src/lab/core-pkg');
const wasm = join(crate, 'target/wasm32-unknown-unknown/release/losos_lab_core.wasm');

function run(cmd, args, cwd) {
  console.log(`$ ${cmd} ${args.join(' ')}`);
  const r = spawnSync(cmd, args, { cwd, stdio: 'inherit' });
  if (r.error) {
    console.error(`${cmd}: ${r.error.message}`);
    process.exit(1);
  }
  if (r.status !== 0) process.exit(r.status ?? 1);
}

run('cargo', ['build', '--release', '--target', 'wasm32-unknown-unknown'], crate);
rmSync(out, { recursive: true, force: true });
mkdirSync(out, { recursive: true });
run(process.env.WASM_BINDGEN || 'wasm-bindgen', ['--target', 'web', '--out-dir', out, wasm], app);
console.log(`wrote ${out}`);
