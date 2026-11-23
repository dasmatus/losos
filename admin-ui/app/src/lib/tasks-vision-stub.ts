/* The metro web build's stand-in for @mediapipe/tasks-vision, the same
 * shape src/lib/ort-wasm-stub.ts carries for onnxruntime-web: the library
 * bundle carries a computed dynamic `import(url)` (its wasm loader's
 * glue), whose computed form Metro's transform refuses; the resolver
 * (metro.config.cjs) points the import at this shim, whose classes throw
 * with the load's own wording — the calls background's loader catches it
 * and reports the segmenter as unavailable for this build ("No visible
 * calls background" wording, follow-up note in its own comments: the
 * backend's wasm segmenter lands as a static module the export carries).
 */

import type { ImageSegmenter as ImageSegmenterShape } from "@mediapipe/tasks-vision";

export type { ImageSegmenter as ImageSegmenterShape };

export type ImageSegmenter = ImageSegmenterShape;

class StubError extends Error {
  constructor() {
    super(
      "the tasks-vision wasm files load beside the Bun-bundled build only; the Metro web export's resolver ships this signalling shim until the static backend lands",
    );
    this.name = "TasksVisionStubError";
  }
}

export const FilesetResolver = {
  forVisionTasks: () => Promise.reject(new StubError()),
} as unknown as {
  forVisionTasks: (path: string) => Promise<unknown>;
};

export const ImageSegmenter = {
  createFromOptions: () => Promise.reject(new StubError()),
} as unknown as {
  createFromOptions: (...args: unknown[]) => Promise<ImageSegmenter>;
};
