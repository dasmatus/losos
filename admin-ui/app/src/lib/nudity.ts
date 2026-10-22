/* The nudity warning: a small picture classifier that runs in this page.
 *
 * The model is Marqo's nsfw-image-detection-384 (Apache 2.0), a ViT-Tiny
 * that looks at a 384 × 384 copy of the picture and answers two numbers,
 * "nsfw" and "safe". The box serves the file at /nudity/model.onnx
 * (`modules/pictures.nix`) and ONNX Runtime runs it here, in WebAssembly, on
 * one thread. Neither the picture nor the score leaves the page.
 *
 * It is a warning, not a filter: it errs both ways, so a picture it flags
 * can still be shown with one more press, and a picture it cannot read is
 * shown as if it were clear. The blur every picture from a stranger gets
 * (lib/pictures.ts) does not depend on it. */

import * as React from "react";
import type { InferenceSession } from "onnxruntime-web/wasm";

const MODEL_URL = "/nudity/model.onnx";
const SIZE = 384;
/** At or above this, the picture is held back. Marqo's own evaluation puts
 *  the balance of misses and false alarms near the middle. */
export const THRESHOLD = 0.5;

export type Verdict = "checking" | "flagged" | "clear" | "failed";

let session: Promise<{ run: InferenceSession; tensor: typeof import("onnxruntime-web/wasm").Tensor }> | null = null;

function open() {
  session ??= import("onnxruntime-web/wasm").then(async (ort) => {
    ort.env.wasm.numThreads = 1;
    ort.env.wasm.proxy = false;
    const run = await ort.InferenceSession.create(MODEL_URL, { executionProviders: ["wasm"] });
    return { run, tensor: ort.Tensor };
  });
  session.catch(() => {
    session = null;
  });
  return session;
}

function picture(src: string): Promise<HTMLImageElement> {
  return new Promise((resolve, reject) => {
    const img = new Image();
    img.decoding = "async";
    img.onload = () => resolve(img);
    img.onerror = () => reject(new Error("the picture did not load"));
    img.src = src;
  });
}

/** The model's input: RGB in [-1, 1], channels first. */
async function pixels(source: ImageBitmapSource): Promise<Float32Array<ArrayBuffer>> {
  const bitmap = await createImageBitmap(source, {
    resizeWidth: SIZE,
    resizeHeight: SIZE,
    resizeQuality: "high",
  });
  const canvas = new OffscreenCanvas(SIZE, SIZE);
  const context = canvas.getContext("2d");
  if (context === null) throw new Error("no canvas");
  context.drawImage(bitmap, 0, 0);
  bitmap.close();
  const rgba = context.getImageData(0, 0, SIZE, SIZE).data;
  const plane = SIZE * SIZE;
  const out = new Float32Array(new ArrayBuffer(3 * plane * 4));
  for (let i = 0; i < plane; i += 1) {
    out[i] = (rgba[i * 4] ?? 0) / 127.5 - 1;
    out[plane + i] = (rgba[i * 4 + 1] ?? 0) / 127.5 - 1;
    out[2 * plane + i] = (rgba[i * 4 + 2] ?? 0) / 127.5 - 1;
  }
  return out;
}

/* One picture at a time: a conversation full of pictures would otherwise
 * start a dozen runs at once on a phone. */
let queue: Promise<unknown> = Promise.resolve();

/** How likely the model thinks `source` shows nudity, 0 to 1. */
export function nudityScore(source: string | Blob): Promise<number> {
  const work = queue.then(async () => {
    const { run, tensor } = await open();
    const input = await pixels(typeof source === "string" ? await picture(source) : source);
    const feeds = { [run.inputNames[0] ?? "input"]: new tensor("float32", input, [1, 3, SIZE, SIZE]) };
    const result = await run.run(feeds);
    const logits = result[run.outputNames[0] ?? "logits"]?.data as Float32Array | undefined;
    if (logits === undefined || logits.length < 2) throw new Error("no answer");
    const [nsfw = 0, safe = 0] = logits;
    const top = Math.max(nsfw, safe);
    const a = Math.exp(nsfw - top);
    return a / (a + Math.exp(safe - top));
  });
  queue = work.catch(() => undefined);
  return work;
}

const verdicts = new Map<string, Promise<Verdict>>();

/** The warning's verdict on a picture already in the page (a blob: or
 *  data: URL), worked out once per picture. Null when there is none. */
export function useNudity(src: string | null): Verdict | null {
  const [verdict, setVerdict] = React.useState<Verdict | null>(src === null ? null : "checking");
  React.useEffect(() => {
    if (src === null) {
      setVerdict(null);
      return;
    }
    let live = true;
    setVerdict("checking");
    let found = verdicts.get(src);
    if (found === undefined) {
      found = nudityScore(src).then(
        (score): Verdict => (score >= THRESHOLD ? "flagged" : "clear"),
        (): Verdict => "failed",
      );
      verdicts.set(src, found);
    }
    void found.then((v) => live && setVerdict(v));
    return () => {
      live = false;
    };
  }, [src]);
  return verdict;
}

/** Whether a picture about to be sent should be asked about first. A
 *  picture the model cannot read is not held up. */
export async function flaggedBeforeSending(file: Blob): Promise<boolean> {
  try {
    return (await nudityScore(file)) >= THRESHOLD;
  } catch {
    return false;
  }
}
