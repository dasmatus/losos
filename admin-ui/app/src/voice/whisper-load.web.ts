/* The speech engine's module load, the Metro web build's twin (native one
 * in whisper-load.ts). Metro's babel-transform refuses a computed dynamic
 * import (SyntaxError: `Invalid call at line …`), and the engine is a
 * build-artifact the served page carries beside itself
 * (`${VOICE_BASE}/whisper/<variant>/losos-whisper.js`), not a module of
 * this tree — so the web build answers the ask with the sharp message the
 * engine's own loader would have failed with at run time.
 *
 * The follow-up is the same as the qemu twin's (src/lab/engine/
 * qemu-dyn.web.ts): an export-side glide where the engine's module becomes
 * a static part of the web bundle, then a real loader behind it.
 */

import type { WhisperModule } from "./whisper-load";

export async function loadWhisperFactory(url: string): Promise<() => Promise<WhisperModule>> {
  void url;
  throw new Error(
    "the speech engine's module loads beside the Bun-bundled build only; the Metro web export has no dynamic-import shim for it yet",
  );
}
