#!/usr/bin/env node
/* Builds one of LosOS Lab's Rust crates to WebAssembly and writes the
 * wasm-bindgen package the Lab page imports:
 *
 *   npm run lab:core    admin-ui/lab/core      -> src/lab/core-pkg/
 *   npm run lab:virt    admin-ui/lab/virt-rpc  -> src/lab/virt-pkg/
 *   npm run lab:render  admin-ui/lab/render    -> src/lab/render-pkg/{webgpu,webgl2}/
 *
 * Needs cargo with the wasm32-unknown-unknown target and wasm-bindgen-cli at
 * exactly the version the crates pin (0.2.127): on PATH, or named by
 * $WASM_BINDGEN. The folders it writes are gitignored; `npm run typecheck`
 * and `npm run build` need all three to exist, so run these first.
 *
 * The canvas (render) is built twice, once per backend, because Bevy picks
 * its WebGPU or WebGL2 code paths at compile time. Each build needs about
 * 3.5 GB of target directory and the two share no Bevy crate, so the first
 * one's target goes before the second starts ($LAB_KEEP_TARGET=1 keeps it;
 * $LAB_RENDER_BACKENDS="webgl2" builds one). wasm-opt (binaryen, on PATH
 * or $WASM_OPT) shrinks each module after wasm-bindgen, never before; without
 * it the modules are about 15% larger and otherwise the same. */

import { spawnSync } from 'node:child_process';
import { mkdirSync, rmSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const CRATES = {
  core: { dir: '../lab/core', wasm: 'losos_lab_core', out: 'src/lab/core-pkg' },
  virt: { dir: '../lab/virt-rpc', wasm: 'losos_lab_virt', out: 'src/lab/virt-pkg' },
  render: { dir: '../lab/render', wasm: 'losos_lab_render', out: 'src/lab/render-pkg', backends: ['webgpu', 'webgl2'] },
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
const target = process.env.CARGO_TARGET_DIR ? resolve(process.env.CARGO_TARGET_DIR) : join(crate, 'target');
const wasm = join(target, `wasm32-unknown-unknown/release/${spec.wasm}.wasm`);

function run(cmd, args, cwd, { optional = false } = {}) {
  console.log(`$ ${cmd} ${args.join(' ')}`);
  const r = spawnSync(cmd, args, { cwd, stdio: 'inherit' });
  if (r.error) {
    if (optional) return false;
    console.error(`${cmd}: ${r.error.message}`);
    process.exit(1);
  }
  if (r.status !== 0) process.exit(r.status ?? 1);
  return true;
}

const bindgen = process.env.WASM_BINDGEN || 'wasm-bindgen';

if (!spec.backends) {
  run('cargo', ['build', '--release', '--target', 'wasm32-unknown-unknown'], crate);
  rmSync(out, { recursive: true, force: true });
  mkdirSync(out, { recursive: true });
  run(bindgen, ['--target', 'web', '--out-dir', out, wasm], app);
  console.log(`wrote ${out}`);
} else {
  const wanted = (process.env.LAB_RENDER_BACKENDS || spec.backends.join(' ')).split(/\s+/).filter(Boolean);
  for (const backend of wanted) {
    if (!spec.backends.includes(backend)) {
      console.error(`unknown backend ${backend}; one of ${spec.backends.join(', ')}`);
      process.exit(2);
    }
    const dir = join(out, backend);
    run('cargo', ['build', '--release', '--target', 'wasm32-unknown-unknown', '--no-default-features', '--features', backend], crate);
    rmSync(dir, { recursive: true, force: true });
    mkdirSync(dir, { recursive: true });
    run(bindgen, ['--target', 'web', '--out-dir', dir, '--out-name', spec.wasm, wasm], app);
    const module = join(dir, `${spec.wasm}_bg.wasm`);
    const features = ['bulk-memory', 'nontrapping-float-to-int', 'sign-ext', 'mutable-globals', 'reference-types', 'multivalue'];
    const opt = run(
      process.env.WASM_OPT || 'wasm-opt',
      ['-Oz', ...features.map((f) => `--enable-${f}`), module, '-o', module],
      app,
      { optional: true },
    );
    if (!opt) console.warn('wasm-opt not found: the module is left as wasm-bindgen wrote it');
    if (process.env.LAB_KEEP_TARGET !== '1') rmSync(target, { recursive: true, force: true });
    console.log(`wrote ${dir}`);
  }
}
