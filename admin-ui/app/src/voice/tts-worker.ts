/// <reference lib="webworker" />
/* The agent's mouth runs here: Kokoro (kokoro.ts) in a worker of its own, so
 * making a sentence of speech never holds up the call page. */

import { Kokoro, SAMPLE_RATE, chunks } from "./kokoro";

export type TtsIn =
  | { type: "init"; runtime: string }
  | { type: "speak"; id: number; text: string; voice: string; speed: number }
  | { type: "cancel"; id: number };

export type TtsOut =
  | { type: "progress"; loaded: number; total: number }
  | { type: "ready" }
  /* One piece of audio of speech `id`; `last` closes it. */
  | { type: "audio"; id: number; pcm: Float32Array<ArrayBuffer>; rate: number; last: boolean }
  | { type: "error"; id?: number; message: string };

const post = (m: TtsOut, transfer: Transferable[] = []): void => (self as unknown as Worker).postMessage(m, transfer);

let kokoro: Kokoro | null = null;
const cancelled = new Set<number>();
/* Speech is made one request at a time, in the order asked. */
let queue: Promise<void> = Promise.resolve();

async function speak(m: Extract<TtsIn, { type: "speak" }>): Promise<void> {
  const k = kokoro;
  if (k === null) return post({ type: "error", id: m.id, message: "the voice is not open" });
  try {
    const parts = chunks(m.text);
    for (const [i, part] of parts.entries()) {
      if (cancelled.has(m.id)) break;
      const pcm = await k.say(part, { voice: m.voice, speed: m.speed });
      if (cancelled.has(m.id)) break;
      const buffer = new Float32Array(pcm);
      post({ type: "audio", id: m.id, pcm: buffer, rate: SAMPLE_RATE, last: i === parts.length - 1 }, [buffer.buffer]);
    }
    // Nothing to say, or stopped: still close it, so the page stops waiting.
    if (parts.length === 0 || cancelled.has(m.id)) {
      post({ type: "audio", id: m.id, pcm: new Float32Array(0), rate: SAMPLE_RATE, last: true });
    }
  } catch (e) {
    post({ type: "error", id: m.id, message: String(e) });
  } finally {
    cancelled.delete(m.id);
  }
}

self.onmessage = (ev: MessageEvent<TtsIn>) => {
  const m = ev.data;
  if (m.type === "init") {
    void Kokoro.open(m.runtime, (loaded, total) => post({ type: "progress", loaded, total }))
      .then((k) => {
        kokoro = k;
        post({ type: "ready" });
      })
      .catch((e: unknown) => post({ type: "error", message: String(e) }));
  } else if (m.type === "speak") {
    queue = queue.then(() => speak(m));
  } else {
    cancelled.add(m.id);
  }
};
