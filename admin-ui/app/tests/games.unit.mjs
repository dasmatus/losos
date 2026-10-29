/* The rules of the five wait games (src/games/), checked without a browser:
 * each game is driven through the same contract the arcade uses, with a
 * seeded random and fixed steps, so a rule that breaks fails here before
 * anyone has to play it.
 *
 *   bun tests/games.unit.mjs
 */

import assert from './assert.mjs';

const load = (file) => import(`../src/games/${file}`);

const { slideRow, slide, merge } = await load('merge.ts');
const { upstream } = await load('upstream.ts');
const { eel } = await load('eel.ts');
const { bricks } = await load('bricks.ts');
const { pairs } = await load('pairs.ts');

/* A small seeded generator, so a run is the same every time. */
function seeded(seed) {
  let s = seed >>> 0;
  return () => {
    s = (s * 1664525 + 1013904223) >>> 0;
    return s / 2 ** 32;
  };
}

function host(seed = 1) {
  const log = { scores: [], over: null, redraws: 0 };
  return {
    log,
    host: {
      score: (v) => log.scores.push(v),
      over: (v) => (log.over = v),
      redraw: () => (log.redraws += 1),
      random: seeded(seed),
    },
  };
}

let failures = 0;
const check = (name, fn) => {
  try {
    fn();
    console.log(`  ok   ${name}`);
  } catch (e) {
    failures += 1;
    console.log(`  FAIL ${name}\n       ${e.message}`);
  }
};

console.log('wait game rules');

check('Merge slides a row towards its start and merges each pair once', () => {
  assert.deepEqual(slideRow([2, 2, 2, 2]), { row: [4, 4, 0, 0], points: 8 });
  assert.deepEqual(slideRow([0, 2, 0, 2]), { row: [4, 0, 0, 0], points: 4 });
  assert.deepEqual(slideRow([4, 4, 8, 0]), { row: [8, 8, 0, 0], points: 8 });
  assert.deepEqual(slideRow([2, 4, 2, 4]), { row: [2, 4, 2, 4], points: 0 });
});

check('Merge reads the board the right way for each direction', () => {
  const board = [2, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0];
  assert.deepEqual(slide(board, 'left').board.slice(0, 4), [4, 0, 0, 0]);
  assert.deepEqual(slide(board, 'right').board.slice(0, 4), [0, 0, 0, 4]);
  const up = slide(board, 'up').board;
  assert.equal(up[0], 4, 'the first column merges upwards');
  assert.equal(up[12], 0);
  const still = [2, 4, 2, 4, 4, 2, 4, 2, 2, 4, 2, 4, 4, 2, 4, 2];
  for (const d of ['up', 'down', 'left', 'right']) assert.equal(slide(still, d).moved, false, d);
});

check('Merge drops a tile after a slide that moved, and ends when nothing can', () => {
  const { host: h, log } = host(7);
  const game = merge.create(h);
  assert.equal(game.phase, 'playing', 'a turn-based game has no idle start');
  for (let i = 0; i < 2000 && game.phase === 'playing'; i++) game.key(['ArrowLeft', 'ArrowUp', 'ArrowRight', 'ArrowDown'][i % 4]);
  assert.equal(game.phase, 'over', 'the board never filled up');
  assert.ok(log.over > 0, 'a full board scored nothing');
  assert.ok(log.redraws > 0, 'a move did not ask for a frame');
});

check('Upstream waits for the first input, then a fish that never jumps hits a rock', () => {
  const { host: h, log } = host(3);
  const game = upstream.create(h);
  for (let i = 0; i < 600; i++) game.step(1 / 120);
  assert.equal(game.phase, 'idle', 'the round started by itself');
  assert.equal(game.key(' '), true);
  game.release();
  for (let i = 0; i < 120 * 30 && game.phase === 'playing'; i++) game.step(1 / 120);
  assert.equal(game.phase, 'over');
  assert.ok(log.over > 0, 'no distance was scored');
});

check('Upstream: jumping every rock keeps the fish going', () => {
  const { host: h } = host(5);
  const game = upstream.create(h);
  game.key(' ');
  game.release();
  // Jump whenever a rock is close: the rule the player follows.
  for (let i = 0; i < 120 * 40; i++) {
    const near = game.rocks.some((r) => r.x > 60 && r.x < 60 + game.speed * 0.3);
    if (near) {
      game.key(' ');
    } else game.keyUp(' ');
    game.step(1 / 120);
    if (game.phase !== 'playing') break;
  }
  assert.equal(game.phase, 'playing', 'a well-timed jump still crashed: the gaps are too tight');
});

check('Eel grows on a shrimp, never reverses into itself, and ends on a wall', () => {
  const { host: h, log } = host(11);
  const game = eel.create(h);
  assert.equal(game.phase, 'idle');
  game.key('ArrowLeft'); // straight back: refused, but starts the round
  assert.equal(game.phase, 'playing');
  for (let i = 0; i < 60 && game.phase === 'playing'; i++) game.step(0.15);
  assert.equal(game.phase, 'over', 'the eel swam through the wall');
  assert.equal(log.over, 0);
  // Steer straight at a shrimp placed in its row.
  const second = eel.create(host(1).host);
  second.shrimp = { x: 9, y: 6 };
  second.key('ArrowRight');
  for (let i = 0; i < 3; i++) second.step(0.151);
  assert.equal(second.body.length, 4, 'eating did not grow the eel');
});

check('Bricks serves on Space, scores on a brick and ends after three balls', () => {
  const { host: h, log } = host(2);
  const game = bricks.create(h);
  game.key(' ');
  assert.equal(game.phase, 'playing');
  // Keep the paddle away so every ball is lost.
  for (let i = 0; i < 120 * 120 && game.phase === 'playing'; i++) {
    game.point(game.ball.x < 160 ? 300 : 20);
    game.step(1 / 120);
    if (!game.served) game.key(' ');
  }
  assert.equal(game.phase, 'over', 'the round never ended');
  assert.ok(log.scores.some((s) => s > 0), 'the first serve hit no brick');
});

check('Pairs counts tries, keeps a pair up and finishes when every card is', () => {
  const { host: h, log } = host(9);
  const game = pairs.create(h);
  // Know where each shape is, then turn the pairs over in order.
  const where = new Map();
  game.cards.forEach((shape, i) => where.set(shape, [...(where.get(shape) ?? []), i]));
  const centre = (i) => [8 + (i % 4) * 72 + 32, 8 + Math.floor(i / 4) * 72 + 32];
  for (const [a, b] of where.values()) {
    game.tap(...centre(a));
    game.tap(...centre(b));
  }
  assert.equal(game.phase, 'over');
  assert.equal(log.over, 8, 'eight pairs in eight tries');
  game.destroy();
});

if (failures > 0) {
  // Thrown, not printed: an uncaught error is what makes Bun exit non-zero.
  throw new Error(`${failures} failed`);
}
console.log('\nall rule checks passed');
