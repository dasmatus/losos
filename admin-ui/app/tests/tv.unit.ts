/* The television's remote: which tile an arrow lands on, and when the
 * layout flag is on. No browser — the geometry is the whole question.
 *
 *   bun tests/tv.unit.ts
 */

import assert from './assert.ts';
import { nextFocus, type TvRect } from '../src/tv/nav.ts';
import { tvLayoutActive, userAgentLooksTv } from '../src/lib/tv.ts';

const tile = (id: string, x: number, y: number, width = 100, height = 80): TvRect => ({
  id,
  x,
  y,
  width,
  height,
});

/* Two rows, the way the home screen sits: glance, then the apps. */
const grid = [
  tile('health', 0, 0),
  tile('storage', 220, 0),
  tile('files', 0, 200),
  tile('photos', 220, 200),
  tile('code', 440, 200),
];

await (async () => {
  assert.equal(nextFocus(grid, 'health', 'right'), 'storage');
  assert.equal(nextFocus(grid, 'health', 'down'), 'files');
  assert.equal(nextFocus(grid, 'health', 'left'), null);
  assert.equal(nextFocus(grid, 'health', 'up'), null);
  assert.equal(nextFocus(grid, 'storage', 'down'), 'photos');
  assert.equal(nextFocus(grid, 'storage', 'left'), 'health');
  assert.equal(nextFocus(grid, 'photos', 'up'), 'storage');
  assert.equal(nextFocus(grid, 'photos', 'right'), 'code');
  assert.equal(nextFocus(grid, 'code', 'right'), null);
  assert.equal(nextFocus(grid, 'missing', 'right'), 'health');
  assert.equal(nextFocus([], 'health', 'down'), null);
  console.log('  ok   arrows stay in the row and the column');
})();

await (async () => {
  assert.equal(userAgentLooksTv('Mozilla/5.0 (SMART-TV; Linux; Tizen 6.5)'), true);
  assert.equal(userAgentLooksTv('Mozilla/5.0 (CrKey) Chrome'), true);
  assert.equal(userAgentLooksTv('AppleTV'), true);
  assert.equal(userAgentLooksTv('Mozilla/5.0 (X11; Linux x86_64) Chrome/120.0.0.0'), false);
  assert.equal(tvLayoutActive('off'), false);
  assert.equal(tvLayoutActive('on'), true);
  assert.equal(tvLayoutActive('auto'), false);
  (globalThis as { __LOSOS_FORCE_PLATFORM__?: string }).__LOSOS_FORCE_PLATFORM__ = 'tv';
  try {
    assert.equal(tvLayoutActive('off'), true, 'a forced set wins even Off');
    assert.equal(tvLayoutActive('auto'), true);
  } finally {
    delete (globalThis as { __LOSOS_FORCE_PLATFORM__?: string }).__LOSOS_FORCE_PLATFORM__;
  }
  console.log('  ok   the layout flag');
})();

console.log('\nall television checks passed');
