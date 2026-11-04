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
  /* How loud it is right now, 0 to 1: what a face moves its mouth by. */
  level(): number;
  close(): void;
}

/* The Kokoro voices the box ships (nix/voice.nix): women and men, American
 * and British. */
export const VOICES = ["af_heart", "af_bella", "am_michael", "am_adam", "bf_emma", "bm_george"] as const;

/* The voice an assistant always has: the built-in one the default, every
 * other one a fixed pick from its name, so it sounds like itself in every
 * call and on every device. */
export function voiceFor(slug: string): string {
  if (slug === "" || slug === "losos") return VOICE;
  let h = 0;
  for (const c of slug) h = (h * 31 + c.charCodeAt(0)) >>> 0;
  return VOICES[h % VOICES.length] as string;
}

export function openMouth(progress?: (loaded: number, total: number) => void, voice: string = VOICE): Promise<Mouth> {
  const worker = new Worker(new URL("./tts-worker.ts", import.meta.url), { type: "module" });
  const context = new AudioContext({ latencyHint: "interactive" });
  const destination = context.createMediaStreamDestination();
  const gain = context.createGain();
  gain.connect(destination);
  const meter = context.createAnalyser();
  meter.fftSize = 512;
  gain.connect(meter);
  const wave = new Float32Array(meter.fftSize);
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
      level() {
        meter.getFloatTimeDomainData(wave);
        let sum = 0;
        for (const x of wave) sum += x * x;
        // Speech sits around 0.05 to 0.2 RMS; a mouth should be wide open
        // well before that is clipped.
        return Math.min(1, Math.sqrt(sum / wave.length) * 5);
      },
      say(text) {
        const id = ids++;
        return new Promise<void>((done, fail) => {
          speeches.set(id, { done, fail, pending: 0, closed: false });
          void context.resume();
          worker.postMessage({ type: "speak", id, text, voice, speed: 1 } satisfies TtsIn);
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
