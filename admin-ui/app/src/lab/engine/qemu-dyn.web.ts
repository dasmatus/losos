/* The qemu engine's module load, the Metro web build's twin, native one in
 * qemu-dyn.ts. Metro's babel-transform refuses a computed dynamic import
 * (SyntaxError: `Invalid call at line …: import(here(...))`), and a static
 * import cannot name the engine's module either — it is a build artefact
 * the hosted copy writes beside its page (admin-ui/lab/engine/build.ts),
 * not a module of this tree. So this build's loader answers the ask with
 * the same unavailable story the engine's own probe prints when the
 * module is simply not there, and the loader falls back to no backend.
 *
 * A browser `import(url)` needs no bundler rewrite where the bundler
 * understood the path; Metro does not offer an escape hatch for a
 * computed import the compile knows nothing about, and the no-eval CSP
 * (modules/containers.nix, script-src 'self') keeps the `new
 * Function('u', 'return import(u)')` shim nobody runs anyway. The
 * follow-up is an Expo-side glide: either the engine ships as a static
 * module the export carries (asset copy of out.js beside the bundle) or a
 * metro resolver alias commits the twin path with the URL resolved game.
 */

type EmModule = Record<string, unknown>;
type EngineFactory = (mod: EmModule) => Promise<unknown>;

export async function loadQemuFactory(url: string): Promise<EngineFactory> {
  void url;
  throw new Error(
    "the qemu-wasm engine module loads beside the Bun-bundled build only; the Metro web export has no dynamic-import shim for it yet",
  );
}
