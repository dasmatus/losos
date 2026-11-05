/* A voice changer for the microphone, in the audio thread. Two effects:
 *
 *   pitch   a delay-line pitch shifter: two taps of a delay line whose delay
 *           slides at the rate that moves the pitch, crossfaded with a Hann
 *           window so the jump of one tap is hidden by the other. Formants
 *           move with the pitch, which is the point here: a deeper voice
 *           sounds like a bigger throat.
 *   ring    ring modulation, a carrier multiplied in, for a robot.
 *
 * It takes {semitones, ring (Hz, 0 = off), mix} by message. Plain JS with no
 * imports: it is loaded by audioWorklet.addModule, and tests/voice-changer.unit.mjs
 * loads this same file with a stand-in for the worklet globals. */

const GRAIN = 0.05; // seconds

class VoiceChanger extends AudioWorkletProcessor {
  constructor() {
    super();
    this.size = Math.ceil(sampleRate * GRAIN);
    this.line = new Float32Array(this.size * 4);
    this.at = 0;
    this.phase = 0;
    this.ratio = 1;
    this.ring = 0;
    this.carrier = 0;
    this.port.onmessage = (ev) => {
      const m = ev.data;
      if (typeof m.semitones === "number") this.ratio = Math.pow(2, Math.max(-24, Math.min(24, m.semitones)) / 12);
      if (typeof m.ring === "number") this.ring = Math.max(0, Math.min(2000, m.ring));
    };
  }

  tap(delay) {
    const n = this.line.length;
    let p = this.at - delay;
    while (p < 0) p += n;
    const i = Math.floor(p);
    const f = p - i;
    return this.line[i % n] * (1 - f) + this.line[(i + 1) % n] * f;
  }

  process(inputs, outputs) {
    const input = inputs[0] && inputs[0][0];
    const output = outputs[0] && outputs[0][0];
    if (!output) return true;
    const n = this.line.length;
    const step = (1 - this.ratio) / this.size;
    for (let k = 0; k < output.length; k++) {
      this.line[this.at] = input ? input[k] : 0;
      let y;
      if (this.ratio === 1) {
        y = this.line[this.at];
      } else {
        const a = this.phase;
        const b = (this.phase + 0.5) % 1;
        const wa = 0.5 - 0.5 * Math.cos(2 * Math.PI * a);
        const wb = 0.5 - 0.5 * Math.cos(2 * Math.PI * b);
        y = this.tap(1 + a * this.size) * wa + this.tap(1 + b * this.size) * wb;
        this.phase += step;
        this.phase -= Math.floor(this.phase);
      }
      if (this.ring > 0) {
        this.carrier += (2 * Math.PI * this.ring) / sampleRate;
        if (this.carrier > 2 * Math.PI) this.carrier -= 2 * Math.PI;
        y *= Math.sin(this.carrier);
      }
      output[k] = y;
      this.at = (this.at + 1) % n;
    }
    return true;
  }
}
registerProcessor("voice-changer", VoiceChanger);
