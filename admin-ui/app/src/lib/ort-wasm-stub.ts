/* The metro web build's stand-in for onnxruntime-web's wasm backend: the
 * library's register file (dist/ort.wasm.bundle.min.mjs) is one
 * self-contained ESM output whose loader opens its worker through a
 * computed `import(url)` — Bun's bundler and the browser both run it fine
 * (webpack's and vite's ignore comments are the other bundlers' escape
 * hatches), and Metro's own rewrite refuses the computed form outright,
 * so the resolver (metro.config.cjs) points the
 * `onnxruntime-web/wasm` imports at this shim instead.
 *
 * It answers the API shape I need out of the box (env's settings, the
 * InferenceSession.create/store/call surface, the Tensor constructor) and
 * the classes throw with the flow's own wording when used, which the
 * pages' loaders (src/lib/nudity.ts, src/voice/kokoro.ts) already catch
 * and report. The substitution being visible means the splice's follow-up
 * is to make the wasm backend land on a static module the export ships —
 * the loader's own notes carry it. */

import type {
  InferenceSession as InferenceSessionShape,
} from "onnxruntime-web/wasm";

export type { InferenceSession as InferenceSessionShape, Tensor as TensorShape };

export type InferenceSession = InferenceSessionShape;

/* env's shape the loaders set; a plain runtime object so the settings are
 * written and read across both backends' calls. */
export const env = {
  wasm: {
    numThreads: 1,
    proxy: false,
    wasmPaths: "",
  },
};

class StubError extends Error {
  constructor() {
    super(
      "the onnxruntime-web wasm backend loads beside the Bun-bundled build only; the Metro web export's resolver ships this sync signalling shim until the static backend lands",
    );
    this.name = "OrtWebStubError";
  }
}

/* InferenceSession.create returnsWH a genuine promise rather than a throw
 * in the circular shape the loaders store (the then-chain reads .run). */
export const InferenceSession = {
  create: (): Promise<InferenceSession> =>
    new Promise((_resolve, reject) => {
      void _resolve;
      reject(new StubError());
    }),
} as unknown as {
  create: (...args: unknown[]) => Promise<InferenceSession>;
};

export class Tensor {
  constructor(..._args: unknown[]) {
    throw new StubError();
  }
}
