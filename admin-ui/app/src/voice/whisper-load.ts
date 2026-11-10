/* The speech engine's module load, the leg Bun's bundler runs: a dynamic
 * `import()` of the engine's emscripten factory by the URL the voice
 * files' base computes (`${VOICE_BASE}/whisper/<variant>/losos-whisper.js`
 * — a build-artifact page serves beside itself, not a module of this
 * tree). Metro's babel-transform refuses a computed import call outright
 * (SyntaxError: `Invalid call at line …`), which is what the sibling
 * whisper-load.web.ts twin answers for the Expo web build.
 *
 * The worker file compiles twice — build.ts's workers() makes it its own
 * bundle (bun), and Metro sees it as the asset ears.ts's `new
 * Worker(new URL(...))` names — so this file keeps the shape both answer.
 */

/** The emscripten factory module, in the shape whisper-worker.ts reads. */
export type WhisperModule = {
  FS: { writeFile(path: string, data: Uint8Array): void; unlink(path: string): void };
  HEAPF32: Float32Array;
  UTF8ToString(ptr: number): string;
  ccall(name: string, ret: string | null, types: string[], args: unknown[]): number;
};

async function importArg(url: string): Promise<{ default: () => Promise<WhisperModule> }> {
  return import(/* @vite-ignore */ url);
}

export async function loadWhisperFactory(url: string): Promise<() => Promise<WhisperModule>> {
  const mod = await importArg(url);
  return mod.default;
}
