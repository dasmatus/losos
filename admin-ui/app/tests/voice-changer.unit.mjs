/* The voice changer's DSP (public/voice-worklet/voice-changer.js) without a
 * browser: the file is loaded with stand-ins for the worklet globals, a
 * sine goes in, and the pitch that comes out is counted.
 *
 *   bun tests/voice-changer.unit.mjs
 */

import assert from './assert.ts';
import { SETTINGS, VOICE_PRESETS } from '../src/calls/voice-changer.ts';

const RATE = 48000;
let Processor;
globalThis.sampleRate = RATE;
globalThis.AudioWorkletProcessor = class {
  constructor() {
    this.port = {};
  }
};
globalThis.registerProcessor = (name, cls) => {
  assert.equal(name, 'voice-changer');
  Processor = cls;
};
await import('../public/voice-worklet/voice-changer.js');

/* `seconds` of a sine through the effect with `msg`, 128 samples at a time
 * as the audio thread does. */
function run(freq, msg, seconds = 1) {
  const p = new Processor();
  p.port.onmessage({ data: msg });
  const out = [];
  const total = RATE * seconds;
  for (let i = 0; i < total; i += 128) {
    const input = Float32Array.from({ length: 128 }, (_, k) => 0.5 * Math.sin((2 * Math.PI * freq * (i + k)) / RATE));
    const output = new Float32Array(128);
    p.process([[input]], [[output]]);
    out.push(...output);
  }
  return out;
}

/* Rising zero crossings per second, on the second half. */
function pitch(samples) {
  const half = samples.slice(samples.length / 2);
  let crossings = 0;
  for (let i = 1; i < half.length; i++) if (half[i - 1] < 0 && half[i] >= 0) crossings++;
  return crossings / (half.length / RATE);
}

const within = (got, want, pct = 4) => assert.ok(Math.abs(got - want) / want < pct / 100, `${got} Hz is not ${want} Hz within ${pct}%`);

let passed = 0;
function test(name, fn) {
  fn();
  passed++;
  console.log(`ok ${name}`);
}

test('with nothing asked the voice passes through unchanged', () => {
  const out = run(300, {});
  within(pitch(out), 300, 1);
});

for (const [name, semitones] of [['deep', -4], ['high', 4], ['giant', -8], ['chipmunk', 8]]) {
  test(`${name} moves the pitch by ${semitones} semitones`, () => {
    assert.equal(SETTINGS[name].semitones, semitones);
    const out = run(300, SETTINGS[name]);
    within(pitch(out), 300 * 2 ** (semitones / 12));
    // A voice, not silence or a click: the level survives the crossfade.
    const tail = out.slice(out.length / 2);
    const rms = Math.sqrt(tail.reduce((s, x) => s + x * x, 0) / tail.length);
    assert.ok(rms > 0.2 && rms < 0.5, `rms ${rms}`);
  });
}

test('the robot multiplies a carrier in, so the sine becomes the sum and difference tones', () => {
  const out = run(300, SETTINGS.robot);
  const tail = out.slice(out.length / 2);
  const rms = Math.sqrt(tail.reduce((s, x) => s + x * x, 0) / tail.length);
  // A sine times a sine has half the power.
  assert.ok(rms > 0.15 && rms < 0.3, `rms ${rms}`);
});

test('every preset is described, and "off" asks for nothing', () => {
  for (const p of VOICE_PRESETS) assert.ok(SETTINGS[p], p);
  assert.deepEqual(SETTINGS.off, { semitones: 0, ring: 0 });
});

test('values outside the range are held to it', () => {
  const p = new Processor();
  p.port.onmessage({ data: { semitones: 99, ring: -5 } });
  assert.equal(p.ratio, 2 ** 2);
  assert.equal(p.ring, 0);
});

console.log(`${passed} passed`);
