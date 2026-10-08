#!/usr/bin/env node
/* Builds one of LosOS Lab's Rust crates to WebAssembly and writes the
 * wasm-bindgen package the Lab page imports:
 *
 *   npm run lab:core    admin-ui/lab/core      -> src/lab/core-pkg/
 *   npm run lab:virt    admin-ui/lab/virt-rpc  -> src/lab/virt-pkg/
 *
 * Needs cargo with the wasm32-unknown-unknown target and wasm-bindgen-cli at
 * exactly the version the crates pin (0.2.127): on PATH, or named by
 * $WASM_BINDGEN. The folders it writes are gitignored; `npm run typecheck`
 * and `npm run build` need both to exist, so run these first. */

import { spawnSync } from 'node:child_process';
import { mkdirSync, rmSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const CRATES = {
  core: { dir: '../lab/core', wasm: 'losos_lab_core', out: 'src/lab/core-pkg' },
  virt: { dir: '../lab/virt-rpc', wasm: 'losos_lab_virt', out: 'src/lab/virt-pkg' },
};
const which = process.argv[2] ?? 'core';
const spec = CRATES[which];
if (!spec) {
  console.error(`usage: lab-core.mjs [${Object.keys(CRATES).join('|')}]`);
  process.exit(2);
}

const app = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const crate = resolve(app, spec.dir);
const out = join(app, spec.out);
const wasm = join(crate, `target/wasm32-unknown-unknown/release/${spec.wasm}.wasm`);

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
