/* Cutting one person's audio into utterances.
 *
 * Each member of a call arrives as a stream of their own, so there is no
 * guessing who is speaking: a segmenter sits on each stream, listens for
 * speech by loudness against that stream's own noise floor, and hands over
 * a clip when the person stops. The ears (whisper.ts) read clips, not a
 * running stream: the model needs no context between them, and a clip that
 * ends when the speaker does is ready to answer right away.
 *
 * Plain arithmetic on 16 kHz mono floats, so it runs the same in the page
 * and in tests. */

export const RATE = 16_000;
/* One decision every 20 ms. */
const FRAME = RATE / 50;
/* Speech starts when 3 of the last 5 frames are loud; it ends after this
 * much quiet. A pause for breath is shorter than that. */
const START_VOTES = 3;
const START_WINDOW = 5;
export const END_QUIET_MS = 650;
/* What comes before the first loud frame is kept, so a word is not
 * clipped at its first letter. */
const PREROLL_MS = 300;
/* Shorter than this is a cough, a click or the end of someone else's
 * sentence. */
export const MIN_SPEECH_MS = 280;
/* A monologue is cut here and read in parts. */
export const MAX_UTTERANCE_MS = 14_000;
/* Below this RMS nothing counts as speech, however silent the room. */
const FLOOR_MIN = 0.008;
/* Speech is this many times louder than the room. */
const OVER_FLOOR = 3;

export interface Utterance {
  pcm: Float32Array;
  /* Milliseconds of speech in it, without the lead-in and tail. */
  speechMs: number;
  /* Whether the clip was cut at the maximum while the person went on. */
  cut: boolean;
}

export class Segmenter {
  private pending = new Float32Array(0);
  private floor = 0.01;
  private recent: boolean[] = [];
  private quiet = 0;
  private speech = 0;
  private clip: Float32Array[] = [];
  private lead: Float32Array[] = [];
  private speaking = false;

  constructor(private onUtterance: (u: Utterance) => void) {}

  /* Whether this stream is mid-sentence now. */
  get active(): boolean {
    return this.speaking;
  }

  push(samples: Float32Array): void {
    const joined = new Float32Array(this.pending.length + samples.length);
    joined.set(this.pending, 0);
    joined.set(samples, this.pending.length);
    let at = 0;
    for (; at + FRAME <= joined.length; at += FRAME) this.frame(joined.subarray(at, at + FRAME));
    this.pending = joined.slice(at);
  }

  /* The clip so far, as if the person had stopped; for a stream that ends. */
  flush(): void {
    if (this.speaking) this.finish(false);
  }

  private frame(samples: Float32Array): void {
    let sum = 0;
    for (const s of samples) sum += s * s;
    const rms = Math.sqrt(sum / samples.length);
    const loud = rms > Math.max(FLOOR_MIN, this.floor * OVER_FLOOR);
    // The room's level follows the quiet frames only, slowly up, quickly down.
    if (!loud) this.floor = rms < this.floor ? this.floor * 0.9 + rms * 0.1 : this.floor * 0.995 + rms * 0.005;
    const copy = samples.slice();
    this.recent.push(loud);
    if (this.recent.length > START_WINDOW) this.recent.shift();

    if (!this.speaking) {
      this.lead.push(copy);
      while (this.lead.length * 20 > PREROLL_MS) this.lead.shift();
      if (this.recent.filter(Boolean).length >= START_VOTES) {
        this.speaking = true;
        this.clip = [...this.lead];
        this.lead = [];
        this.quiet = 0;
        this.speech = this.recent.filter(Boolean).length * 20;
      }
      return;
    }
    this.clip.push(copy);
    if (loud) {
      this.quiet = 0;
      this.speech += 20;
    } else {
      this.quiet += 20;
    }
    if (this.quiet >= END_QUIET_MS) this.finish(false);
    else if (this.clip.length * 20 >= MAX_UTTERANCE_MS) this.finish(true);
  }

  private finish(cut: boolean): void {
    const frames = this.clip;
    const speechMs = this.speech;
    this.clip = [];
    this.speaking = false;
    this.quiet = 0;
    this.speech = 0;
    this.recent = [];
    if (speechMs < MIN_SPEECH_MS) return;
    const pcm = new Float32Array(frames.length * FRAME);
    frames.forEach((f, i) => pcm.set(f, i * FRAME));
    this.onUtterance({ pcm, speechMs, cut });
  }
}
