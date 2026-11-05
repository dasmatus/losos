/* The agent's ears, from the page's side: a worker that reads clips of
 * speech with whisper.cpp and answers with text. One clip at a time; a clip
 * that waits behind several others is not worth reading any more, since the
 * conversation has moved on, and is dropped. */

import type { WhisperIn, WhisperOut } from "./whisper-worker";

/* Words the model should expect: the name, so it is not heard as "laws
 * across", and what the box's apps are called. */
const PROMPT = "LosOS, LosOS Cloud, LosOS Git, LosOS Chat, LosOS Mail. Hey LosOS, what time is it?";
/* Clips waiting for the model beyond this are stale. */
const BACKLOG = 3;

export type Listen = (loaded: number, total: number) => void;

export class Ears {
  private id = 0;
  private waiting = new Map<number, { resolve: (text: string | null) => void; reject: (e: Error) => void }>();
  private order: number[] = [];

  private constructor(private worker: Worker) {
    worker.onmessage = (ev: MessageEvent<WhisperOut>) => {
      const m = ev.data;
      if (m.type === "text" || (m.type === "error" && m.id !== undefined)) {
        const w = this.waiting.get(m.id ?? -1);
        this.waiting.delete(m.id ?? -1);
        this.order = this.order.filter((i) => i !== m.id);
        if (m.type === "text") w?.resolve(m.text);
        else w?.reject(new Error(m.message));
      }
    };
  }

  /* Open the ears: the engine and the model, fetched once and kept. */
  static open(progress?: Listen): Promise<Ears> {
    const worker = new Worker(new URL("./whisper-worker.ts", import.meta.url), { type: "module" });
    const threaded = self.crossOriginIsolated === true && typeof SharedArrayBuffer !== "undefined";
    const send = (m: WhisperIn): void => worker.postMessage(m);
    return new Promise((resolve, reject) => {
      worker.onmessage = (ev: MessageEvent<WhisperOut>) => {
        const m = ev.data;
        if (m.type === "progress") progress?.(m.loaded, m.total);
        else if (m.type === "ready") resolve(new Ears(worker));
        else if (m.type === "error") {
          worker.terminate();
          reject(new Error(m.message));
        }
      };
      worker.onerror = (ev) => {
        worker.terminate();
        reject(new Error(ev.message || "the speech worker failed"));
      };
      send({
        type: "init",
        variant: threaded ? "threads" : "single",
        threads: threaded ? Math.min(4, Math.max(1, (navigator.hardwareConcurrency || 2) - 1)) : 1,
        prompt: PROMPT,
      });
    });
  }

  /* What the clip says, or null if it was dropped as stale. */
  transcribe(pcm: Float32Array): Promise<string | null> {
    const stale = this.order.length >= BACKLOG ? this.order.shift() : undefined;
    if (stale !== undefined) {
      this.waiting.get(stale)?.resolve(null);
      this.waiting.delete(stale);
    }
    const id = this.id++;
    this.order.push(id);
    return new Promise((resolve, reject) => {
      this.waiting.set(id, { resolve, reject });
      const copy = new Float32Array(pcm);
      this.worker.postMessage({ type: "run", id, pcm: copy } satisfies WhisperIn, [copy.buffer]);
    });
  }

  close(): void {
    this.worker.postMessage({ type: "free" } satisfies WhisperIn);
    this.worker.terminate();
    for (const w of this.waiting.values()) w.resolve(null);
    this.waiting.clear();
  }
}
