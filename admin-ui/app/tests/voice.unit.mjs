/* The call agent's ears and manners without a browser: cutting speech into
 * utterances (src/voice/vad.ts) and deciding when it is being spoken to
 * (src/voice/dialog.ts).
 *
 *   bun tests/voice.unit.mjs
 */

import assert from './assert.mjs';

const { Segmenter, RATE, END_QUIET_MS } = await import('../src/voice/vad.ts');
const d = await import('../src/voice/dialog.ts');
const { SentenceStream, plain } = await import('../src/voice/sentences.ts');
const { KeyHub } = await import('../src/voice/keys.ts');
const { voiceFor, VOICES, VOICE } = await import('../src/voice/mouth.ts');
const { blinking } = await import('../src/voice/avatar.ts');

let passed = 0;
const pending = [];
function test(name, fn) {
  const r = fn();
  if (r instanceof Promise) pending.push(r.then(() => (passed++, console.log(`ok ${name}`))));
  else passed++;
  if (r instanceof Promise) return;
  console.log(`ok ${name}`);
}

/* `ms` of a 220 Hz tone at `level`, or of a whisper of noise. */
const tone = (ms, level = 0.2) =>
  Float32Array.from({ length: (RATE * ms) / 1000 }, (_, i) => level * Math.sin((2 * Math.PI * 220 * i) / RATE));
const hiss = (ms, level = 0.002) =>
  Float32Array.from({ length: (RATE * ms) / 1000 }, (_, i) => level * Math.sin(i * 12.9898) * Math.cos(i * 4.1414));

function run(parts) {
  const out = [];
  const seg = new Segmenter((u) => out.push(u));
  for (const p of parts) {
    // Fed in the 128-sample blocks the audio worklet sends.
    for (let i = 0; i < p.length; i += 128) seg.push(p.subarray(i, i + 128));
  }
  seg.flush();
  return out;
}

test('a sentence between two silences is one utterance with a lead-in', () => {
  const [u, ...rest] = run([hiss(1000), tone(1500), hiss(1500)]);
  assert.equal(rest.length, 0);
  assert.ok(u.speechMs >= 1400 && u.speechMs <= 1700, `speech ${u.speechMs}`);
  // The clip holds the lead-in and the quiet before the cut.
  assert.ok(u.pcm.length / RATE > 1.5 && u.pcm.length / RATE < 1.5 + 0.3 + (END_QUIET_MS + 40) / 1000);
});

test('a short pause inside a sentence does not split it', () => {
  const out = run([hiss(500), tone(900), hiss(300), tone(900), hiss(1500)]);
  assert.equal(out.length, 1);
});

test('a long pause does', () => {
  const out = run([hiss(500), tone(900), hiss(1200), tone(900), hiss(1500)]);
  assert.equal(out.length, 2);
});

test('a click is no utterance', () => {
  assert.equal(run([hiss(800), tone(100), hiss(1500)]).length, 0);
});

test('a quiet room never starts one, however long', () => {
  assert.equal(run([hiss(20_000)]).length, 0);
});

test('a monologue is cut and the rest follows', () => {
  const out = run([hiss(300), tone(20_000), hiss(1500)]);
  assert.ok(out.length >= 2);
  assert.equal(out[0].cut, true);
});

test('the person still talking when the stream ends gets their clip', () => {
  assert.equal(run([hiss(300), tone(1200)]).length, 1);
});

test('the name opens a request, however Whisper spells it', () => {
  for (const [said, asked] of [
    ['LosOS, what is on my calendar tomorrow?', 'what is on my calendar tomorrow?'],
    ['Hey LosOS what time is it', 'what time is it'],
    ['hey Los OS, summarise this', 'summarise this'],
    ['Okay, Losos: who is in the call?', 'who is in the call?'],
    ['Hi Laws O.S. can you help', 'can you help'],
    ['What do you think, LosOS?', 'What do you think'],
    ['LosOS.', ''],
  ]) {
    assert.equal(d.addressed(said), asked, said);
  }
});

test('talk about the product is not a request to it', () => {
  for (const said of [
    'LosOS Cloud is slow today',
    'I installed LosOS on a mini PC last week',
    'Have you tried the LosOS chat?',
    'losos is a salmon in Slovak',
    'I really love LosOS.',
    'Nothing here.',
  ]) {
    assert.equal(d.addressed(said), null, said);
  }
});

test('Whisper\'s silence words are noise', () => {
  for (const said of ['[BLANK_AUDIO]', ' (music) ', '...', 'you', '♪', '*sigh*', '']) assert.ok(d.isNoise(said), said);
  assert.ok(!d.isNoise('Can you hear me?'));
});

test('only the name opens a conversation, then the same person may go on', () => {
  const dialog = new d.Dialog();
  const eva = { user: '@eva:b', name: 'Eva' };
  const jan = { user: '@jan:b', name: 'Jan' };
  assert.equal(dialog.hear({ ...eva, text: 'We should move the review to Thursday.', at: 1000 }), null);
  assert.equal(dialog.hear({ ...jan, text: 'Fine with me.', at: 3000 }), null);
  const q = dialog.hear({ ...eva, text: 'LosOS, is Thursday free for everyone?', at: 6000 });
  assert.equal(q.text, 'is Thursday free for everyone?');
  assert.deepEqual(q.context.map((l) => l.name), ['Eva', 'Jan']);

  dialog.answered(eva.user, 'Thursday is free for all four of you after two.', 9000);
  // Eva goes on without the name; Jan, who has not been answered, does not.
  assert.equal(dialog.hear({ ...eva, text: 'Then book it for three.', at: 12_000 }).text, 'Then book it for three.');
  assert.equal(dialog.hear({ ...jan, text: 'Book it for three.', at: 13_000 }), null);
  // Twenty seconds on, the name is needed again.
  assert.equal(dialog.hear({ ...eva, text: 'Thanks, see you then.', at: 40_000 }), null);
});

test('its own voice coming back through a speaker is not heard as a request', () => {
  const dialog = new d.Dialog();
  dialog.answered('@eva:b', 'Thursday is free for all four of you after two.', 1000);
  assert.equal(dialog.hear({ user: '@eva:b', name: 'Eva', text: 'Thursday is free for all four of you after two', at: 2500 }), null);
});

test('the prompt carries the conversation and the register of a voice', () => {
  const p = d.prompt({ user: '@eva:b', name: 'Eva', text: 'is Thursday free?', context: [{ user: '@jan:b', name: 'Jan', text: 'Fine with me.', at: 1 }] });
  assert.match(p, /read aloud/);
  assert.match(p, /Jan: Fine with me\./);
  assert.match(p, /Eva asks you: is Thursday free\?/);
});

test('in a call of one person with their assistant every sentence is for it', () => {
  const dialog = new d.Dialog({ direct: true, name: 'Gardener' });
  const jan = { user: '@jan:b', name: 'Jan' };
  const q = dialog.hear({ ...jan, text: 'What is on my calendar tomorrow?', at: 1000 });
  assert.equal(q.text, 'What is on my calendar tomorrow?');
  // The name is still taken off when it is said, and noise is still noise.
  assert.equal(dialog.hear({ ...jan, text: 'LosOS, and the day after?', at: 5000 }).text, 'and the day after?');
  assert.equal(dialog.hear({ ...jan, text: '[BLANK_AUDIO]', at: 9000 }), null);
  // Its own voice coming back is still not heard as a request.
  dialog.answered(jan.user, 'Tomorrow you have a review at ten and lunch with Eva.', 20_000);
  assert.equal(dialog.hear({ ...jan, text: 'a review at ten and lunch with Eva', at: 21_000 }), null);
  assert.equal(dialog.hear({ ...jan, text: 'Move the review to eleven.', at: 40_000 }).text, 'Move the review to eleven.');
  // Without the option nothing changes: the name is needed.
  assert.equal(new d.Dialog().hear({ ...jan, text: 'What is on my calendar tomorrow?', at: 1000 }), null);
});

test('the prompt tells an assistant it is talking to one person', () => {
  const q = { user: '@jan:b', name: 'Jan', text: 'is Thursday free?', context: [] };
  const p = d.prompt(q, { name: 'Gardener', direct: true });
  assert.match(p, /You are Gardener, in a video call with Jan/);
  assert.match(p, /read aloud to them/);
  assert.match(p, /Jan says: is Thursday free\?/);
  assert.doesNotMatch(p, /everyone in the call/);
});

test('an assistant sounds the same in every call, and the built-in one is af_heart', () => {
  assert.equal(voiceFor('losos'), VOICE);
  assert.equal(voiceFor('gardener'), voiceFor('gardener'));
  for (const slug of ['gardener', 'tax-helper', 'a', 'x'.repeat(40)]) assert.ok(VOICES.includes(voiceFor(slug)), slug);
  // Not everyone gets the same voice.
  assert.ok(new Set(['gardener', 'tax-helper', 'scribe', 'cook', 'planner', 'butler', 'coach'].map(voiceFor)).size >= 3);
});

test('the face blinks briefly and not all the time', () => {
  let shut = 0;
  for (let t = 0; t < 60; t += 0.04) if (blinking(t)) shut++;
  const share = shut / 1500;
  assert.ok(share > 0.01 && share < 0.08, String(share));
  assert.equal(blinking(0.05), true);
  assert.equal(blinking(1), false);
});

test('stop is a command only when it is the whole sentence', () => {
  assert.equal(d.isStop('Stop.'), true);
  assert.equal(d.isStop('Okay, be quiet please'), true);
  assert.equal(d.isStop('LosOS, stop'), true);
  assert.equal(d.isStop("Don't stop the build"), false);
  assert.equal(d.isStop('We should stop by on Friday'), false);
});

test('an answer is spoken a sentence at a time as it grows', () => {
  const s = new SentenceStream();
  assert.deepEqual(s.feed('Thursday is free'), []);
  assert.deepEqual(s.feed('Thursday is free. Fri'), ['Thursday is free.']);
  assert.deepEqual(s.feed('Thursday is free. Friday is not! And'), ['Friday is not!']);
  assert.deepEqual(s.feed('Thursday is free. Friday is not! And so on', true), ['And so on']);
});

test('a number with a point in it is not the end of a sentence', () => {
  const s = new SentenceStream();
  assert.deepEqual(s.feed('The box has 3.'), []);
  assert.deepEqual(s.feed('The box has 3.5 GiB free. Fine.', true), ['The box has 3.5 GiB free.', 'Fine.']);
});

test('abbreviations do not end a sentence', () => {
  const s = new SentenceStream();
  assert.deepEqual(s.feed('Ask Dr. Novak about it. Done.', true), ['Ask Dr. Novak about it.', 'Done.']);
});

test('markdown and links are not read aloud', () => {
  assert.equal(plain('**Bold** and `code` see [the docs](https://x.example/a) or https://y.example now'), 'Bold and code see the docs or now');
  assert.equal(plain('- one\n- two\n# Title'), 'one two Title');
  const s = new SentenceStream();
  assert.deepEqual(s.feed('```\nrm -rf\n```\n', true), []);
});

test('a sentence without an end is cut before it runs on for ever', () => {
  const s = new SentenceStream();
  const long = 'word, '.repeat(80);
  const out = s.feed(long, true);
  assert.ok(out.length >= 2, String(out.length));
  assert.ok(out.every((x) => x.length < 260));
});

test('the two sessions of a page trade keys in memory and everyone else over Olm', async () => {
  const sent = [];
  const inbound = new Set();
  const base = { send: async (to, msg) => void sent.push({ to, msg }), subscribe: (h) => (inbound.add(h), () => inbound.delete(h)) };
  const hub = new KeyHub(base, { user: '@eva:b', device: 'D1' });
  const host = hub.channel();
  const agent = hub.channel();
  const got = [];
  agent.subscribe((from, msg) => got.push({ from, msg }));
  const msg = { call: 'c', pid: 1, index: 0, key: 'AAAA' };
  await host.send(
    [
      { user: '@eva:b', device: 'D1' },
      { user: '@jan:b', device: 'D2' },
      { user: '@jan:b', device: 'D2' },
    ],
    msg,
  );
  assert.equal(got.length, 1);
  assert.deepEqual(got[0].from, { user: '@eva:b', device: 'D1' });
  assert.equal(sent.length, 1);
  assert.deepEqual(sent[0].to, [{ user: '@jan:b', device: 'D2' }]);
  // Someone else's key reaches both sessions.
  const hostGot = [];
  host.subscribe((from, m) => hostGot.push(m));
  for (const h of inbound) h({ user: '@jan:b', device: 'D2' }, msg);
  assert.equal(got.length, 2);
  assert.equal(hostGot.length, 1);
});

await Promise.all(pending);
console.log(`${passed} passed`);
