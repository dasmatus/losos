#!/usr/bin/env bun
/* Builds one of LosOS Lab's Rust crates to WebAssembly and writes the
 * wasm-bindgen package the Lab page imports:
 *
 *   bun run lab:core    admin-ui/lab/core      -> src/lab/core-pkg/
 *   bun run lab:virt    admin-ui/lab/virt-rpc  -> src/lab/virt-pkg/
 *   bun run lab:render  admin-ui/lab/render    -> src/lab/render-pkg/{webgpu,webgl2}/
 *   bun run mdx         backend/mdx            -> src/mdx/pkg/ (LosOS Office, Notes)
 *
 * Needs cargo with the wasm32-unknown-unknown target and wasm-bindgen-cli at
 * exactly the version the crates pin (0.2.127): on PATH, or named by
 * $WASM_BINDGEN. The folders it writes are gitignored; `bun run typecheck`
 * and `bun run build` need all four to exist, so run these first.
 *
 * The canvas (render) is built twice, once per backend, because Bevy picks
 * its WebGPU or WebGL2 code paths at compile time. Each build needs about
 * 3.5 GB of target directory and the two share no Bevy crate, so the first
 * one's target goes before the second starts ($LAB_KEEP_TARGET=1 keeps it;
 * $LAB_RENDER_BACKENDS="webgl2" builds one). wasm-opt (binaryen, on PATH
 * or $WASM_OPT) shrinks each module after wasm-bindgen, never before; without
 * it the modules are about 15% larger and otherwise the same. */

const CRATES: Record<string, { dir: string; wasm: string; out: string; features?: string; backends?: string[] }> = {
  core: { dir: '../lab/core', wasm: 'losos_lab_core', out: 'src/lab/core-pkg' },
  virt: { dir: '../lab/virt-rpc', wasm: 'losos_lab_virt', out: 'src/lab/virt-pkg' },
  mdx: { dir: '../../backend/mdx', wasm: 'losos_mdx', out: 'src/mdx/pkg', features: 'wasm' },
  render: { dir: '../lab/render', wasm: 'losos_lab_render', out: 'src/lab/render-pkg', backends: ['webgpu', 'webgl2'] },
};

/* Beside the glue, the public URL story: Bun's bundler turns an import of
 * `<glue>.wasm?url` into the asset's hashed URL inside the bundle, but
 * Metro (Expo's web bundler, src/app/index.web.tsx) has no suffix
 * protocol, and an asset-module import would hand the callers a runtime
 * handle, not a URL. So both builds ship the wasm itself at a fixed
 * plain path under public/wasm/ and carry a generated twin module
 * `<glue>.wasm.url.ts` exporting the URL; metro.config.js points the
 * `.wasm?url` imports at the twin, and nix's losos-admin-ui (nix/
 * packages.nix preBuild) repeats these two write for the copies its
 * sandbox gets from the labWasm packages. */
async function wasmUrlTwin(dir: string, glue: string, slug: string) {
  await Bun.$`mkdir -p public/wasm`;
  const module = `${dir}/${glue}`;
  await Bun.write(`public/wasm/${slug}.wasm`, Bun.file(module));
  await Bun.write(
    `${module}.url.ts`,
    `/* GENERATED FILE (scripts/lab-core.ts): the public URL metro.config.js resolves the ?url import of this module to. Bun's bundler never reads this file. */\nexport default "/wasm/${slug}.wasm";\n`,
  );
}

const which = Bun.argv[2] ?? 'core';
const spec = CRATES[which];
if (!spec) throw new Error(`usage: lab-core.ts [${Object.keys(CRATES).join('|')}]`);

const app = `${import.meta.dir}/..`;
const crate = `${app}/${spec.dir}`;
const out = `${app}/${spec.out}`;
// A relative $CARGO_TARGET_DIR is the caller's, not the crate's.
const given = Bun.env.CARGO_TARGET_DIR;
const target = given ? (given.startsWith('/') ? given : `${Bun.env.PWD}/${given}`) : `${crate}/target`;
const wasm = `${target}/wasm32-unknown-unknown/release/${spec.wasm}.wasm`;

function run(cmd: string, args: string[], cwd: string, { optional = false }: { optional?: boolean } = {}): boolean {
  console.log(`$ ${cmd} ${args.join(' ')}`);
  const found = Bun.which(cmd);
  if (found === null) {
    if (optional) return false;
    throw new Error(`${cmd}: not found`);
  }
  const r = Bun.spawnSync([found, ...args], { cwd, stdio: ['inherit', 'inherit', 'inherit'], env: { ...Bun.env, CARGO_TARGET_DIR: target } });
  if (r.exitCode !== 0) throw new Error(`${cmd} exited with ${r.exitCode}`);
  return true;
}
const fresh = async (dir: string) => {
  await Bun.$`rm -rf ${dir}`;
  await Bun.$`mkdir -p ${dir}`;
};

const bindgen = Bun.env.WASM_BINDGEN || 'wasm-bindgen';

if (!spec.backends) {
  run('cargo', ['build', '--release', '--target', 'wasm32-unknown-unknown', ...(spec.features ? ['--features', spec.features] : [])], crate);
  await fresh(out);
  run(bindgen, ['--target', 'web', '--out-dir', out, wasm], app);
  await wasmUrlTwin(out, `${spec.wasm}_bg.wasm`, which);
  console.log(`wrote ${out}`);
} else {
  const wanted = (Bun.env.LAB_RENDER_BACKENDS || spec.backends.join(' ')).split(/\s+/).filter(Boolean);
  for (const backend of wanted) {
    if (!spec.backends.includes(backend)) throw new Error(`unknown backend ${backend}; one of ${spec.backends.join(', ')}`);
    const dir = `${out}/${backend}`;
    run('cargo', ['build', '--release', '--target', 'wasm32-unknown-unknown', '--no-default-features', '--features', backend], crate);
    await fresh(dir);
    run(bindgen, ['--target', 'web', '--out-dir', dir, '--out-name', spec.wasm, wasm], app);
    await wasmUrlTwin(dir, `${spec.wasm}_bg.wasm`, `render-${backend}`);
    const module = `${dir}/${spec.wasm}_bg.wasm`;
    const features = ['bulk-memory', 'nontrapping-float-to-int', 'sign-ext', 'mutable-globals', 'reference-types', 'multivalue'];
    const opt = run(
      Bun.env.WASM_OPT || 'wasm-opt',
      ['-Oz', ...features.map((f) => `--enable-${f}`), module, '-o', module],
      app,
      { optional: true },
    );
    if (!opt) console.warn('wasm-opt not found: the module is left as wasm-bindgen wrote it');
    if (Bun.env.LAB_KEEP_TARGET !== '1') await Bun.$`rm -rf ${target}`;
    console.log(`wrote ${dir}`);
  }
}
