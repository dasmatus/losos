/* The frame encryption of calls (src/calls/e2ee.ts), without a browser:
 * what a frame looks like on the wire, that only the right key opens it,
 * that a changed byte anywhere is refused, and that the key schedule gives
 * the forward secrecy the call page promises.
 *
 *   bun tests/calls-e2ee.unit.mjs
 */

import assert from './assert.mjs';

const e = await import('../src/calls/e2ee.ts');

const bytes = (...xs) => new Uint8Array(xs);
const fill = (n, f) => Uint8Array.from({ length: n }, (_, i) => f(i));
// A VP8 keyframe starts with bit 0 clear; a delta frame with it set.
const keyframe = fill(400, (i) => (i === 0 ? 0x10 : (i * 7) & 0xff));
const delta = fill(120, (i) => (i === 0 ? 0x11 : (i * 13) & 0xff));
const opus = fill(80, (i) => (i * 3) & 0xff);

let passed = 0;
async function test(name, fn) {
  await fn();
  passed++;
  console.log(`ok ${name}`);
}

await test('the clear prefix is VP8 10/3 bytes and one Opus byte', () => {
  assert.equal(e.clearBytes('video', keyframe), 10);
  assert.equal(e.clearBytes('video', delta), 3);
  assert.equal(e.clearBytes('video', delta, true), 10);
  assert.equal(e.clearBytes('audio', opus), 1);
  assert.equal(e.clearBytes('video', bytes(0)), 1);
});

await test('a sealed frame opens with the same key and keeps its prefix clear', async () => {
  const send = new e.SendKey();
  const ring = new e.ReceiveRing();
  ring.set(send.index, send.key);
  for (const [kind, frame] of [
    ['video', keyframe],
    ['video', delta],
    ['audio', opus],
  ]) {
    const sealed = await send.seal(kind, frame);
    assert.equal(sealed.length, frame.length + e.TAG_BYTES + 6);
    const clear = e.clearBytes(kind, frame);
    assert.deepEqual(sealed.subarray(0, clear), frame.subarray(0, clear));
    // Nothing of the body survives in the clear.
    assert.notDeepEqual(sealed.subarray(clear, frame.length), frame.subarray(clear));
    assert.equal(sealed.at(-1), 0x4c);
    const keys = await ring.get(e.readTrailer(sealed).index);
    const opened = await e.decryptFrame(keys[kind], sealed, e.receivedClearBytes(kind, sealed));
    assert.deepEqual(opened, frame);
  }
});

await test('audio and video of one sender never share a key and IV', async () => {
  const send = new e.SendKey(fill(32, () => 7));
  const a = await send.seal('audio', fill(50, () => 0));
  const v = await send.seal('video', fill(50, (i) => (i === 0 ? 1 : 0)));
  // Same counter (0), same plaintext body, different kind: different bytes.
  assert.notDeepEqual(a.subarray(3, 40), v.subarray(3, 40));
  const keys = await e.frameKeys(fill(32, () => 7));
  assert.equal(await e.decryptFrame(keys.audio, v, 3), null, 'the audio key does not open video');
});

await test('another key, or any changed byte, is refused', async () => {
  const send = new e.SendKey();
  const sealed = await send.seal('video', keyframe);
  const right = await e.frameKeys(send.key);
  const wrong = await e.frameKeys(e.randomKey());
  assert.equal(await e.decryptFrame(wrong.video, sealed, 10), null);
  for (const at of [0, 5, 9, 10, 200, sealed.length - 7, sealed.length - 6, sealed.length - 2]) {
    const bent = sealed.slice();
    bent[at] ^= 1;
    assert.equal(await e.decryptFrame(right.video, bent, 10), null, `byte ${at}`);
  }
  assert.equal(e.readTrailer(sealed.subarray(0, 10)), null, 'too short to be a frame');
  const nomagic = sealed.slice();
  nomagic[nomagic.length - 1] = 0;
  assert.equal(e.readTrailer(nomagic), null);
});

await test('counters advance, so no two frames share an IV', async () => {
  const send = new e.SendKey();
  const counters = [];
  for (let i = 0; i < 5; i++) counters.push(e.readTrailer(await send.seal('audio', opus)).counter);
  assert.deepEqual(counters, [0, 1, 2, 3, 4]);
  const same = await Promise.all([1, 2, 3].map(() => send.seal('video', delta)));
  assert.equal(new Set(same.map((f) => e.readTrailer(f).counter)).size, 3);
});

await test('a receiver follows a sender that ratcheted on its own', async () => {
  const send = new e.SendKey();
  const ring = new e.ReceiveRing();
  ring.set(send.index, send.key);
  await send.ratchet();
  await send.ratchet();
  const sealed = await send.seal('audio', opus);
  assert.equal(e.readTrailer(sealed).index, 2);
  // Frames of the new index arrive together; the ring steps forward once.
  const [k1, k2] = await Promise.all([ring.get(2), ring.get(2)]);
  assert.ok(k1 && k2);
  assert.deepEqual(await e.decryptFrame(k1.audio, sealed, 1), opus);
  // A sealed frame from before the ratchet still opens for a moment.
  assert.ok(await ring.get(1));
});

await test('a receiver does not ratchet without bound, or backwards', async () => {
  const ring = new e.ReceiveRing();
  ring.set(10, e.randomKey());
  assert.equal(await ring.get(10 + e.MAX_RATCHET_AHEAD + 1), null);
  assert.equal(await ring.get(9), null);
  assert.ok(await ring.get(10 + e.MAX_RATCHET_AHEAD));
  assert.equal(await new e.ReceiveRing().get(0), null, 'no key at all');
});

await test('the index wraps at 256', async () => {
  const send = new e.SendKey(e.randomKey(), 255);
  const ring = new e.ReceiveRing();
  ring.set(255, send.key);
  await send.ratchet();
  assert.equal(send.index, 0);
  const sealed = await send.seal('audio', opus);
  assert.deepEqual(await e.decryptFrame((await ring.get(0)).audio, sealed, 1), opus);
});

await test('someone who joins later cannot open what was sent before', async () => {
  const send = new e.SendKey();
  const before = await send.seal('video', keyframe);
  // On a join every sender ratchets and the joiner gets only the new key.
  await send.ratchet();
  const joiner = new e.ReceiveRing();
  joiner.set(send.index, send.key);
  // The ring will not step backwards, and HKDF does not run backwards, so
  // index 0's key is out of reach.
  assert.equal(await joiner.get(e.readTrailer(before).index), null, 'no way back to index 0');
  const after = await send.seal('video', keyframe);
  assert.deepEqual(await e.decryptFrame((await joiner.get(send.index)).video, after, 10), keyframe);
});

await test('someone who left cannot open what comes after a rotation', async () => {
  const send = new e.SendKey();
  const leaver = new e.ReceiveRing();
  leaver.set(send.index, send.key);
  // The members who stay get a fresh random key; the leaver gets nothing.
  send.rotate();
  const after = await send.seal('audio', opus);
  const guessed = await leaver.get(e.readTrailer(after).index);
  assert.ok(guessed, 'the leaver can ratchet to the same index');
  assert.equal(await e.decryptFrame(guessed.audio, after, 1), null, 'but its key is unrelated');
});

await test('old keys are wiped as the index moves on', async () => {
  const ring = new e.ReceiveRing();
  const k = e.randomKey();
  ring.set(0, k);
  for (let i = 1; i <= 4; i++) ring.set(i, e.randomKey());
  assert.equal(ring.has(0), false);
  assert.equal(ring.has(1), false);
  assert.equal(ring.has(2), true);
  // A late rotation message for an older index does not move the ring back.
  ring.set(3, e.randomKey());
  assert.equal(ring.has(4), true);
  ring.clear();
  assert.equal(ring.has(4), false);
});

await test('a frame under a rotated key that has not arrived meets a guess, not a forgery', async () => {
  const ring = new e.ReceiveRing();
  ring.set(0, e.randomKey());
  // The sender rotated to a fresh key at index 1; the receiver ratchets.
  const fresh = new e.SendKey(e.randomKey(), 1);
  const frame = await fresh.seal('video', new Uint8Array(64).fill(7));
  const guess = await ring.get(1);
  assert.equal(ring.guessed(1), true);
  assert.equal(await e.decryptFrame(guess.video, frame, e.receivedClearBytes('video', frame)), null);
  // Once the key is handed over it is no guess, and opens the frame.
  ring.set(1, fresh.key);
  assert.equal(ring.guessed(1), false);
  assert.notEqual(await e.decryptFrame((await ring.get(1)).video, frame, e.receivedClearBytes('video', frame)), null);
});

await test('base64 round trip', () => {
  const k = e.randomKey();
  assert.deepEqual(e.fromBase64(e.toBase64(k)), k);
});

console.log(`calls-e2ee: ${passed} passed`);
