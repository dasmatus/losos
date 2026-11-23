/// <reference lib="webworker" />
/* The agent's ears run here: whisper.cpp as WebAssembly (nix/whisper-wasm.nix),
 * in a worker of its own so a sentence being read never holds up the call
 * page. With cross-origin isolation the module brings its own threads; the
 * page picks the build that suits (`variant`). */

import { fetchBytes, VOICE_BASE } from "./files";

import { loadWhisperFactory, type WhisperModule } from "./whisper-load";

export type WhisperIn =
  | { type: "init"; variant: "threads" | "single"; threads: number; prompt: string }
  | { type: "run"; id: number; pcm: Float32Array<ArrayBuffer> }
  | { type: "free" };

export type WhisperOut =
  | { type: "progress"; loaded: number; total: number }
  | { type: "ready" }
  | { type: "text"; id: number; text: string; ms: number }
  | { type: "error"; id?: number; message: string };

type Module = WhisperModule;

const post = (m: WhisperOut): void => (self as unknown as Worker).postMessage(m);

let wasm: Module | null = null;
let threads = 1;

async function init(msg: Extract<WhisperIn, { type: "init" }>): Promise<void> {
  threads = msg.threads;
  const url = `${VOICE_BASE}/whisper/${msg.variant}/losos-whisper.js`;
  const factory = await loadWhisperFactory(url);
  const [module, model] = await Promise.all([
    factory(),
    fetchBytes(`${VOICE_BASE}/whisper.bin`, (loaded, total) => post({ type: "progress", loaded, total })),
  ]);
  module.FS.writeFile("/model.bin", model);
  const ok = module.ccall("losos_whisper_init", "number", ["string"], ["/model.bin"]);
  // The weights are in the engine's own buffers now; the file copy is not needed.
  module.FS.unlink("/model.bin");
  if (ok !== 1) throw new Error("the speech model did not load");
  module.ccall("losos_whisper_prompt", null, ["string"], [msg.prompt]);
  wasm = module;
  post({ type: "ready" });
}

function run(id: number, pcm: Float32Array<ArrayBuffer>): void {
  const w = wasm;
  if (w === null) return post({ type: "error", id, message: "the ears are not open" });
  const started = performance.now();
  const at = w.ccall("losos_whisper_input", "number", ["number"], [pcm.length]);
  // The heap may have grown while the buffer was made; read it afresh.
  w.HEAPF32.set(pcm, at / 4);
  const length = w.ccall("losos_whisper_run", "number", ["number"], [threads]);
  if (length < 0) return post({ type: "error", id, message: "the speech model failed" });
  const text = w.UTF8ToString(w.ccall("losos_whisper_text", "number", [], []));
  post({ type: "text", id, text: text.trim(), ms: Math.round(performance.now() - started) });
}

self.onmessage = (ev: MessageEvent<WhisperIn>) => {
  const m = ev.data;
  try {
    if (m.type === "init") void init(m).catch((e: unknown) => post({ type: "error", message: String(e) }));
    else if (m.type === "run") run(m.id, m.pcm);
    else if (m.type === "free") {
      wasm?.ccall("losos_whisper_free", null, [], []);
      wasm = null;
    }
  } catch (e) {
    post({ type: "error", ...(m.type === "run" ? { id: m.id } : {}), message: String(e) });
  }
};
