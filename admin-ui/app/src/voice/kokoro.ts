/* The agent's mouth: Kokoro-82M (Apache-2.0, 24 kHz, one of the few open
 * voices that sounds like a person rather than a screen reader), run by
 * ONNX Runtime in WebAssembly in this page's worker. The model is the 8-bit
 * export, 92 MB; a voice is a 510 × 256 table of styles, one row for each
 * length of utterance.
 *
 * Text becomes phonemes with eSpeak NG (phonemizer, also WebAssembly, the
 * data inside it), phonemes become ids through the model's own table
 * (kokoro-vocab.json), and one forward pass makes the waveform. Text goes
 * in a sentence at a time and audio comes out a sentence at a time, so the
 * first words play while the rest is still being made. */

import type { InferenceSession, Tensor } from "onnxruntime-web/wasm";
import vocab from "./kokoro-vocab.json";
import { fetchBytes, type Progress, VOICE_BASE } from "./files";

export const SAMPLE_RATE = 24_000;
const STYLE_DIM = 256;
const MAX_TOKENS = 509;
/* One chunk of text; longer sentences are cut at their commas. */
const MAX_CHARS = 240;

export interface SpeakOptions {
  voice?: string;
  /* 1 is the voice's own pace. */
  speed?: number;
}

type Ort = typeof import("onnxruntime-web/wasm");

const ids = vocab as Record<string, number>;

/* What a model that reads phonemes can do with the words an assistant
 * writes: no markdown, no links, no emoji, abbreviations spelled out. */
export function normalize(text: string): string {
  return text
    .replace(/```[\s\S]*?```/g, " ")
    .replace(/`([^`]*)`/g, "$1")
    .replace(/!?\[([^\]]*)\]\([^)]*\)/g, "$1")
    .replace(/https?:\/\/\S+/g, "")
    .replace(/[*_#>~|]+/g, " ")
    .replace(/^\s*[-+]\s+/gm, "")
    .replace(/\p{Extended_Pictographic}/gu, "")
    .replace(/[‘’]/g, "'")
    .replace(/[«»“”]/g, '"')
    .replace(/\bDr\.(?=\s+[A-Z])/g, "Doctor")
    .replace(/\bMr\.(?=\s+[A-Z])/g, "Mister")
    .replace(/\bMrs\.(?=\s+[A-Z])/g, "Missus")
    .replace(/\bMs\.(?=\s+[A-Z])/g, "Miss")
    .replace(/\be\.g\./gi, "for example")
    .replace(/\bi\.e\./gi, "that is")
    .replace(/\betc\./gi, "etcetera")
    .replace(/\bvs\.?\b/gi, "versus")
    .replace(/\bLosOS\b/g, "Losos")
    .replace(/\bGiB\b/g, "gigabytes")
    .replace(/\bMiB\b/g, "megabytes")
    .replace(/&/g, " and ")
    .replace(/%/g, " percent")
    .replace(/(?<=\d),(?=\d{3}\b)/g, "")
    .replace(/(?<=\d)-(?=\d)/g, " to ")
    .replace(/[ \t]+/g, " ")
    .trim();
}

/* Sentences, joined up to MAX_CHARS, in order. */
export function chunks(text: string): string[] {
  const sentences = normalize(text)
    .split(/(?<=[.!?…])\s+|\n+/)
    .map((s) => s.trim())
    .filter((s) => /[\p{L}\p{N}]/u.test(s));
  const out: string[] = [];
  for (const sentence of sentences) {
    let rest = sentence;
    while (rest.length > MAX_CHARS) {
      const at = rest.lastIndexOf(",", MAX_CHARS);
      const cut = at > 40 ? at + 1 : rest.lastIndexOf(" ", MAX_CHARS);
      out.push(rest.slice(0, cut > 0 ? cut : MAX_CHARS).trim());
      rest = rest.slice(cut > 0 ? cut : MAX_CHARS).trim();
    }
    const last = out[out.length - 1];
    // Short neighbours read better as one breath.
    if (last !== undefined && last.length + rest.length < 90 && !/[?!]$/.test(last)) out[out.length - 1] = `${last} ${rest}`;
    else out.push(rest);
  }
  return out;
}

/* eSpeak's phonemes, adjusted to the ones Kokoro was trained on
 * (misaki's espeak.py does the same). */
export function adjust(phonemes: string): string {
  return phonemes
    .replace(/kəkˈoːɹoʊ/g, "kˈoʊkəɹoʊ")
    .replace(/kəkˈɔːɹəʊ/g, "kˈəʊkəɹəʊ")
    .replace(/ʲ/g, "j")
    .replace(/r/g, "ɹ")
    .replace(/x/g, "k")
    .replace(/ɬ/g, "l")
    .replace(/(?<=[a-zɹː])(?=hˈʌndɹɪd)/g, " ")
    .replace(/ z(?=[;:,.!?¡¿—…"«»“” ]|$)/g, "z")
    .replace(/(?<=nˈaɪn)ti(?!ː)/g, "di")
    .trim();
}

export function tokens(phonemes: string): number[] {
  const out: number[] = [];
  for (const ch of phonemes) {
    const id = ids[ch];
    if (id !== undefined) out.push(id);
  }
  return out.slice(0, MAX_TOKENS);
}

export class Kokoro {
  private styles = new Map<string, Float32Array>();

  private constructor(
    private ort: Ort,
    private session: InferenceSession,
  ) {}

  /* `runtime` is where ONNX Runtime's own .wasm and .mjs are served. */
  static async open(runtime: string, progress?: Progress): Promise<Kokoro> {
    const ort = await import("onnxruntime-web/wasm");
    ort.env.wasm.wasmPaths = runtime;
    // This runs in a worker already; a thread pool needs the page to be
    // cross-origin isolated (modules/containers.nix sends the headers).
    ort.env.wasm.proxy = false;
    ort.env.wasm.numThreads = self.crossOriginIsolated ? Math.min(4, Math.max(1, navigator.hardwareConcurrency - 1)) : 1;
    const model = await fetchBytes(`${VOICE_BASE}/kokoro.onnx`, progress);
    const session = await ort.InferenceSession.create(model, { executionProviders: ["wasm"], graphOptimizationLevel: "all" });
    return new Kokoro(ort, session);
  }

  private async style(voice: string): Promise<Float32Array> {
    const known = this.styles.get(voice);
    if (known) return known;
    const bytes = await fetchBytes(`${VOICE_BASE}/voices/${encodeURIComponent(voice)}.bin`);
    const table = new Float32Array(bytes.buffer, bytes.byteOffset, bytes.byteLength / 4);
    this.styles.set(voice, table);
    return table;
  }

  /* One chunk of text as audio. */
  async say(text: string, { voice = "af_heart", speed = 1 }: SpeakOptions = {}): Promise<Float32Array> {
    const { phonemize } = await import("phonemizer");
    const phonemes = adjust((await phonemize(text, "en-us")).join(" "));
    const list = tokens(phonemes);
    if (list.length === 0) return new Float32Array(0);
    const table = await this.style(voice);
    const row = Math.min(list.length, MAX_TOKENS);
    const style = table.slice(row * STYLE_DIM, (row + 1) * STYLE_DIM);
    const input = new BigInt64Array(list.length + 2);
    list.forEach((id, i) => {
      input[i + 1] = BigInt(id);
    });
    const feeds: Record<string, Tensor> = {
      input_ids: new this.ort.Tensor("int64", input, [1, input.length]),
      style: new this.ort.Tensor("float32", style, [1, STYLE_DIM]),
      speed: new this.ort.Tensor("float32", new Float32Array([speed]), [1]),
    };
    const out = await this.session.run(feeds);
    const wave = out["waveform"];
    if (wave === undefined) throw new Error("the voice model returned no waveform");
    return new Float32Array(wave.data as Float32Array);
  }

  /* A sentence at a time, so the first plays before the last is made. */
  async *speak(text: string, options: SpeakOptions = {}): AsyncGenerator<Float32Array> {
    for (const part of chunks(text)) {
      const audio = await this.say(part, options);
      if (audio.length > 0) yield audio;
    }
  }
}
