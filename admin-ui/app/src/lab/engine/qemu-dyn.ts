/* The qemu engine's module load, the leg Bun's bundler runs (and the box
 * ships through build.ts): a dynamic `import()` of the engine's module,
 * where `here()` carries the URL the loader computed — the module lives
 * beside the page it serves (admin-ui/lab/engine/build.ts writes it), not
 * in this tree, so neither a static import nor a Metro tree path can name
 * it. Metro's babel-transform refuses a computed import call outright
 * (SyntaxError: `Invalid call at line …`), which is what the sibling
 * qemu-dyn.web.ts twin answers for the Expo web build. */

/** The emscripten module factory the engine module exports, in the shape
 * qemu.ts's loader keeps it. */
type EmModule = Record<string, unknown>;

type EngineFactory = (mod: EmModule) => Promise<unknown>;

/** The dynamic import this file is for. */
async function importArg(url: string): Promise<{ default: EngineFactory }> {
  return import(/* @vite-ignore */ url);
}

export async function loadQemuFactory(url: string): Promise<EngineFactory> {
  const mod = await importArg(url);
  return mod.default;
}
