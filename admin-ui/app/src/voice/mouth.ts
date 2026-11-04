/* The agent's voice, from the page's side: text goes to the voice worker,
 * audio comes back a sentence at a time and is played into a stream that the
 * call sends as the agent's microphone.
 *
 * Playing is scheduled on the audio clock, one sentence right after the
 * last, so the joins are seamless; stopping drops what has not played yet. */

import type { TtsIn, TtsOut } from "./tts-worker";

export const VOICE = "af_heart";

export interface Mouth {
  /* The track the call sends; silent when nothing is being said. */
  readonly stream: MediaStream;
  /* Say `text`; resolves when it has all been played, or stopped. */
  say(text: string): Promise<void>;
  /* Stop mid-sentence and forget what was queued. */
  stop(): void;
  /* Whether anything is being said or waiting to be. */
  readonly speaking: boolean;
  close(): void;
}

export function openMouth(progress?: (loaded: number, total: number) => void): Promise<Mouth> {
  const worker = new Worker(new URL("./tts-worker.ts", import.meta.url), { type: "module" });
  const context = new AudioContext({ latencyHint: "interactive" });
  const destination = context.createMediaStreamDestination();
  const gain = context.createGain();
  gain.connect(destination);
  const runtime = `${import.meta.env.BASE_URL}ort/`;

  return new Promise((resolve, reject) => {
    let ids = 0;
    const speeches = new Map<number, { done: () => void; fail: (e: Error) => void; pending: number; closed: boolean }>();
    let playing = new Set<AudioBufferSourceNode>();
    let next = 0;

    const mouth: Mouth = {
      stream: destination.stream,
      get speaking() {
        return speeches.size > 0 || playing.size > 0;
      },
      say(text) {
        const id = ids++;
        return new Promise<void>((done, fail) => {
          speeches.set(id, { done, fail, pending: 0, closed: false });
          void context.resume();
          worker.postMessage({ type: "speak", id, text, voice: VOICE, speed: 1 } satisfies TtsIn);
        });
      },
      stop() {
        for (const id of speeches.keys()) worker.postMessage({ type: "cancel", id } satisfies TtsIn);
        for (const s of playing) {
          s.onended = null;
          try {
            s.stop();
          } catch {
            // Already ended.
          }
        }
        playing = new Set();
        next = 0;
        for (const s of speeches.values()) s.done();
        speeches.clear();
      },
      close() {
        mouth.stop();
        worker.terminate();
        void context.close();
      },
    };

    worker.onmessage = (ev: MessageEvent<TtsOut>) => {
      const m = ev.data;
      if (m.type === "progress") progress?.(m.loaded, m.total);
      else if (m.type === "ready") resolve(mouth);
      else if (m.type === "error") {
        if (m.id === undefined) {
          worker.terminate();
          reject(new Error(m.message));
        } else {
          speeches.get(m.id)?.fail(new Error(m.message));
          speeches.delete(m.id);
        }
      } else if (m.type === "audio") {
        const speech = speeches.get(m.id);
        if (speech === undefined) return;
        const finish = (): void => {
          if (speech.closed && speech.pending === 0 && speeches.get(m.id) === speech) {
            speeches.delete(m.id);
            speech.done();
          }
        };
        if (m.last) speech.closed = true;
        if (m.pcm.length > 0) {
          const buffer = context.createBuffer(1, m.pcm.length, m.rate);
          buffer.copyToChannel(m.pcm, 0);
          const source = context.createBufferSource();
          source.buffer = buffer;
          source.connect(gain);
          const at = Math.max(context.currentTime + 0.05, next);
          source.start(at);
          next = at + buffer.duration;
          speech.pending += 1;
          playing.add(source);
          source.onended = () => {
            playing.delete(source);
            speech.pending -= 1;
            finish();
          };
        }
        finish();
      }
    };
    worker.onerror = (ev) => {
      worker.terminate();
      reject(new Error(ev.message || "the voice worker failed"));
    };
    worker.postMessage({ type: "init", runtime } satisfies TtsIn);
  });
}
