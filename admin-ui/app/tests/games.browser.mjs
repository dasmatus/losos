/* "Play while you wait" (src/components/play-while-waiting.tsx, src/games/):
 * the game a long wait offers.
 *
 * What is checked: a rebuild offers the game on the apply bar and the
 * game's code is not fetched until it is wanted; each of the five games
 * waits for its first input and then plays, by keyboard and by pointer;
 * the score and the best are kept; a hidden tab and a lost focus pause an
 * action game; reduced motion offers only the two turn-based games; the end
 * of the wait stops the game and says so, and Close takes the line away;
 * the loop keeps time on a screen faster than its step; a lost focus lets
 * go of held keys; blocked storage keeps the visit's best; Merge and Pairs
 * read their boards out;
 * the wizard's wait offers it too; a phone gets the icon
 * button and a game that fits; and none of it trips the appliance's
 * Content-Security-Policy.
 *
 * Same arrangement as the other files: the real dist/ bundle, the API
 * stubbed with page.route().
 */

import assert from './assert.mjs';
import { launch, runner, serve } from './harness.mjs';

const TOKEN = 'a'.repeat(64);
const { origin, close: closeServer } = await serve({ csp: true });
const browser = await launch();
const { check, finish } = runner();

const SETTINGS = {
  sharingMyStorage: false,
  hostName: 'mattbox',
  https: false,
  gpuEnable: false,
  apachePort: 11000,
  proxyEnable: false,
  clusterEnable: false,
  shareCompute: false,
  computeWindowStart: '23:00',
  computeWindowEnd: '07:00',
  hardeningApparmor: false,
  hardeningMalloc: false,
  hardeningNosmt: false,
  hardeningUsbguard: false,
};

const json = (route, status, body) =>
  route.fulfill({ status, contentType: 'application/json', body: JSON.stringify(body) });

/* A signed-in box whose rebuild runs until `box.finish()`. */
async function openRebuild({
  reducedMotion = 'no-preference',
  viewport = { width: 1280, height: 900 },
  colorScheme = 'light',
  hasTouch = false,
  init = null,
} = {}) {
  const context = await browser.newContext({ viewport, locale: 'en-US', reducedMotion, colorScheme, hasTouch });
  const page = await context.newPage();
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  /* `finish` waits for the page to have seen the rebuild running: a
   * terminal state before any `building` is read as the last job's
   * (SETTLE_MS in use-settings-form.ts). */
  const box = {
    applied: false,
    seen: false,
    done: false,
    async finish() {
      while (!this.seen) await page.waitForTimeout(100);
      this.done = true;
    },
  };
  const chunks = [];
  page.on('request', (r) => {
    const path = new URL(r.url()).pathname;
    if (/\/assets\/arcade-[^/]+\.js$/.test(path)) chunks.push(path);
  });

  await page.route('**/api/**', (route) => json(route, 200, {}));
  await page.route('**/api/setup/claim', (route) => json(route, 200, { claimed: true }));
  await page.route('**/api/state', (route) => json(route, 200, { mode: 'local', sharing: false }));
  await page.route('**/api/settings', (route) => json(route, 200, SETTINGS));
  await page.route('**/api/status', (route) => {
    if (!box.applied) return json(route, 200, { state: 'idle', progress: 0, message: '' });
    if (!box.done) box.seen = true;
    return json(route, 200, box.done
      ? { state: 'done', progress: 100, message: 'activation finished', job: 'job-1' }
      : { state: 'building', progress: 0, message: 'building the new system', job: 'job-1' });
  });
  await page.route('**/api/apply', (route) => {
    box.applied = true;
    return json(route, 200, { job: 'job-1' });
  });
  await page.addInitScript((t) => {
    window.sessionStorage.setItem('losos-token', t);
    window.__violations = [];
    document.addEventListener('securitypolicyviolation', (e) =>
      window.__violations.push(`${e.violatedDirective} ${e.blockedURI} ${e.sample}`),
    );
  }, TOKEN);
  if (init !== null) await page.addInitScript(init);
  await page.goto(origin + '/settings/network', { waitUntil: 'networkidle' });
  await page.getByLabel('This box is called').fill('fishbox');
  /* By keyboard: the offer takes the place of Apply, and a pointer left
   * resting there would preload the game the way a hover is meant to. */
  await page.getByRole('button', { name: 'Apply', exact: true }).focus();
  await page.keyboard.press('Enter');
  return { page, context, errors, box, chunks };
}

const offer = (page) => page.getByRole('button', { name: 'Play while you wait' });
const game = (page) => page.getByTestId('game');
const canvas = (page) => game(page).locator('canvas');
const attr = (page, name) => game(page).getAttribute(name);
const scoreText = (page) => page.getByTestId('game-score').innerText();
const number = (text) => Number(/(\d+)/.exec(text)?.[1] ?? NaN);

/* Press "Another game" until `id` is on screen. The pick is random and
 * never the same game twice, so every game comes up within a few tries. */
async function switchTo(page, id) {
  for (let i = 0; i < 40; i++) {
    if ((await attr(page, 'data-game')) === id) return;
    await page.getByRole('button', { name: 'Another game' }).click();
  }
  throw new Error(`never offered ${id}`);
}

/* The violations the page logs on every load (Sonner's stylesheet), so a
 * check can ask for new ones only. */
const violations = (page) => page.evaluate(() => window.__violations.slice());

console.log('admin-ui wait game checks');

await check('a rebuild offers the game, and its code is fetched only when it is wanted', async () => {
  const { page, context, errors, chunks } = await openRebuild();
  await offer(page).waitFor();
  assert.equal(await game(page).count(), 0, 'the game opened by itself');
  assert.deepEqual(chunks, [], 'the game chunk was fetched before anyone asked for it');
  const before = await violations(page);
  await offer(page).click();
  await game(page).waitFor();
  assert.equal(chunks.length, 1, 'the game chunk was not fetched exactly once');
  // The rebuild's own report is still there, above the game.
  const bar = page.getByRole('progressbar');
  assert.ok(await bar.isVisible(), 'the progress bar went when the game opened');
  const barBox = await bar.boundingBox();
  const gameBox = await game(page).boundingBox();
  assert.ok(gameBox.y > barBox.y, 'the game is not under the progress report');
  assert.equal(await offer(page).count(), 0, 'the offer stayed after it was taken');
  assert.ok(await canvas(page).evaluate((c) => document.activeElement === c), 'the canvas does not have the keyboard');
  assert.deepEqual(await violations(page), before, 'the game tripped the Content-Security-Policy');
  assert.deepEqual(errors, []);
  await context.close();
});

await check('Upstream waits for a key, plays, ends on a rock and starts again on Space', async () => {
  const { page, context, errors } = await openRebuild();
  await offer(page).click();
  await switchTo(page, 'upstream');
  assert.equal(await attr(page, 'data-phase'), 'idle');
  await page.waitForTimeout(500);
  assert.equal(await attr(page, 'data-phase'), 'idle', 'the round started without an input');
  assert.match(await game(page).innerText(), /Press Space or tap to start/);
  await canvas(page).press('Space');
  assert.equal(await attr(page, 'data-phase'), 'playing');
  // A fish that never jumps reaches the first rock in a few seconds.
  await page.locator('[data-testid=game][data-phase=over]').waitFor({ timeout: 15000 });
  const final = number(await scoreText(page));
  assert.ok(final > 0, 'no distance was scored');
  assert.match(await game(page).innerText(), /Round over with \d+ points?/);
  assert.match(await scoreText(page), /Best \d+/, 'the best was not kept');
  await canvas(page).press('Space');
  assert.equal(await attr(page, 'data-phase'), 'playing', 'Space did not start a new round');
  assert.deepEqual(errors, []);
  await context.close();
});

await check('Eel turns on the arrow keys and ends on a wall', async () => {
  const { page, context, errors } = await openRebuild();
  await offer(page).click();
  await switchTo(page, 'eel');
  await canvas(page).press('ArrowDown');
  assert.equal(await attr(page, 'data-phase'), 'playing');
  await page.locator('[data-testid=game][data-phase=over]').waitFor({ timeout: 15000 });
  assert.deepEqual(errors, []);
  await context.close();
});

await check('Merge moves on the arrow keys and on a swipe, and scores', async () => {
  const { page, context, errors } = await openRebuild();
  await offer(page).click();
  await switchTo(page, 'merge');
  assert.equal(await attr(page, 'data-phase'), 'playing', 'a turn-based game waits for nothing');
  const keys = ['ArrowLeft', 'ArrowUp', 'ArrowRight', 'ArrowDown'];
  for (let i = 0; i < 40 && number(await scoreText(page)) === 0; i++) await canvas(page).press(keys[i % 4]);
  assert.ok(number(await scoreText(page)) > 0, 'forty moves merged nothing');
  // A drag across the board is a swipe.
  const box = await canvas(page).boundingBox();
  const shot = await canvas(page).screenshot();
  for (const [dx, dy] of [[-1, 0], [0, -1], [1, 0], [0, 1]]) {
    await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
    await page.mouse.down();
    await page.mouse.move(box.x + box.width / 2 + dx * 80, box.y + box.height / 2 + dy * 80, { steps: 4 });
    await page.mouse.up();
  }
  assert.ok(!shot.equals(await canvas(page).screenshot()), 'four swipes left the board as it was');
  assert.deepEqual(errors, []);
  await context.close();
});

await check('Bricks follows the pointer and serves on a click', async () => {
  const { page, context, errors } = await openRebuild();
  await offer(page).click();
  await switchTo(page, 'bricks');
  const box = await canvas(page).boundingBox();
  await page.mouse.move(box.x + box.width * 0.2, box.y + box.height * 0.9);
  const left = await canvas(page).screenshot();
  await page.mouse.move(box.x + box.width * 0.8, box.y + box.height * 0.9);
  assert.ok(!left.equals(await canvas(page).screenshot()), 'the paddle did not follow the pointer');
  await page.mouse.click(box.x + box.width * 0.5, box.y + box.height * 0.9);
  assert.equal(await attr(page, 'data-phase'), 'playing');
  assert.deepEqual(errors, []);
  await context.close();
});

await check('Pairs turns cards on a tap and on Enter, and counts the tries', async () => {
  const { page, context, errors } = await openRebuild();
  await offer(page).click();
  await switchTo(page, 'pairs');
  const box = await canvas(page).boundingBox();
  const cell = box.width / 4;
  await page.mouse.click(box.x + cell * 0.5, box.y + cell * 0.5);
  await page.mouse.click(box.x + cell * 1.5, box.y + cell * 0.5);
  assert.match(await scoreText(page), /Tries 1/);
  await canvas(page).press('ArrowDown');
  await canvas(page).press('Enter');
  await canvas(page).press('ArrowRight');
  await canvas(page).press('Enter');
  assert.match(await scoreText(page), /Tries 2/);
  assert.deepEqual(errors, []);
  await context.close();
});

await check('a hidden tab and a lost focus pause an action game; a key goes on', async () => {
  const { page, context, errors } = await openRebuild();
  await offer(page).click();
  await switchTo(page, 'upstream');
  await canvas(page).press('Space');
  assert.equal(await attr(page, 'data-phase'), 'playing', `Space did not start the round (paused: ${await attr(page, 'data-paused')})`);
  await page.evaluate(() => {
    Object.defineProperty(document, 'hidden', { configurable: true, get: () => true });
    document.dispatchEvent(new Event('visibilitychange'));
  });
  // React draws the pause on its next render, not inside the event.
  await page
    .locator('[data-testid=game][data-paused=true]')
    .waitFor({ timeout: 2000 })
    .catch(async () => assert.fail(`a hidden tab kept the game running (phase ${await attr(page, 'data-phase')})`));
  const score = await scoreText(page);
  await page.waitForTimeout(600);
  assert.equal(await scoreText(page), score, 'the score moved while paused');
  assert.match(await game(page).innerText(), /Paused/);
  await page.evaluate(() => {
    Object.defineProperty(document, 'hidden', { configurable: true, get: () => false });
  });
  await canvas(page).press('ArrowLeft');
  assert.equal(await attr(page, 'data-paused'), 'false', 'a key did not wake the game');
  await canvas(page).evaluate((c) => c.blur());
  await page
    .locator('[data-testid=game][data-paused=true]')
    .waitFor({ timeout: 2000 })
    .catch(() => assert.fail('losing the focus kept the game running'));
  assert.deepEqual(errors, []);
  await context.close();
});

await check('an action game runs on a screen faster than its physics step', async () => {
  // Frames every 6 ms, a 165 Hz screen: each one is shorter than 1/120 s.
  const { page, context, errors } = await openRebuild({
    init: () => {
      window.requestAnimationFrame = (cb) => setTimeout(() => cb(performance.now()), 6);
      window.cancelAnimationFrame = (id) => clearTimeout(id);
    },
  });
  await offer(page).click();
  await switchTo(page, 'upstream');
  await canvas(page).press('Space');
  await page.waitForTimeout(1200);
  assert.ok(number(await scoreText(page)) > 0, 'the game stood still at 165 frames a second');
  assert.deepEqual(errors, []);
  await context.close();
});

await check('a key let go while the game had lost the focus is let go in the game too', async () => {
  const { page, context, errors } = await openRebuild();
  await offer(page).click();
  await switchTo(page, 'bricks');
  await canvas(page).focus();
  await page.keyboard.down('ArrowRight');
  await page.waitForTimeout(60);
  await canvas(page).evaluate((c) => c.blur());
  assert.equal(await attr(page, 'data-paused'), 'true');
  await page.keyboard.up('ArrowRight');
  await canvas(page).focus();
  await page.keyboard.press('Space');
  assert.equal(await attr(page, 'data-paused'), 'false', 'a key did not wake the game');
  await page.waitForTimeout(60);
  const still = await canvas(page).screenshot();
  await page.waitForTimeout(250);
  assert.ok(still.equals(await canvas(page).screenshot()), 'the paddle kept moving on a key nobody held');
  assert.deepEqual(errors, []);
  await context.close();
});

await check('with the best scores refused by storage, a worse round keeps the best', async () => {
  const { page, context, errors } = await openRebuild({
    init: () => {
      for (const name of ['getItem', 'setItem']) {
        const real = Storage.prototype[name];
        Storage.prototype[name] = function (key, ...rest) {
          if (key === 'losos-games-best') throw new DOMException('blocked', 'SecurityError');
          return real.call(this, key, ...rest);
        };
      }
    },
  });
  await offer(page).click();
  await switchTo(page, 'upstream');
  const finals = [];
  for (let round = 0; round < 3; round++) {
    await canvas(page).press('Space');
    await page.locator('[data-testid=game][data-phase=over]').waitFor({ timeout: 15000 });
    finals.push(number(await scoreText(page)));
  }
  const best = Number(/Best (\d+)/.exec(await scoreText(page))?.[1]);
  assert.equal(best, Math.max(...finals), `rounds ${finals.join(', ')} showed a best of ${best}`);
  assert.deepEqual(errors, []);
  await context.close();
});

await check('Merge and Pairs read their board out after each move', async () => {
  const { page, context, errors } = await openRebuild();
  await offer(page).click();
  await switchTo(page, 'merge');
  const board = page.getByTestId('game-board');
  assert.match(await board.textContent(), /^Row 1: .+\. Row 2: .+\. Row 3: .+\. Row 4: .+\.$/);
  assert.match(await board.textContent(), /\b2\b|\b4\b/, 'the starting tiles were not read out');
  assert.equal(await board.getAttribute('aria-live'), 'polite');
  await switchTo(page, 'pairs');
  await canvas(page).press('ArrowRight');
  assert.match(await board.textContent(), /^Row 1, card 2: face down\. Pairs found: 0 of 8\.$/);
  await canvas(page).press('Enter');
  assert.match(await board.textContent(), /^Row 1, card 2: [a-z]+\. Face up: [a-z]+\. Pairs found: 0 of 8\.$/);
  assert.deepEqual(errors, []);
  await context.close();
});

await check('under reduced motion only the turn-based games come up', async () => {
  const { page, context, errors } = await openRebuild({ reducedMotion: 'reduce' });
  await offer(page).click();
  await game(page).waitFor();
  const seen = new Set();
  for (let i = 0; i < 12; i++) {
    seen.add(await attr(page, 'data-game'));
    await page.getByRole('button', { name: 'Another game' }).click();
  }
  assert.deepEqual([...seen].sort(), ['merge', 'pairs']);
  assert.deepEqual(errors, []);
  await context.close();
});

await check('the end of the rebuild stops the game, says so, and Close takes the line away', async () => {
  const { page, context, errors, box } = await openRebuild();
  await offer(page).click();
  await switchTo(page, 'upstream');
  await canvas(page).press('Space');
  await box.finish();
  const ready = page.getByTestId('game-ready');
  await ready.waitFor({ timeout: 10000 });
  assert.match(await ready.innerText(), /The wait is over, so the game has stopped/);
  assert.equal(await page.locator('canvas').count(), 0, 'the canvas is still there after the wait');
  await page.getByRole('progressbar').waitFor({ state: 'detached' });
  await ready.getByRole('button', { name: 'Close game' }).click();
  await ready.waitFor({ state: 'detached' });
  assert.equal(await offer(page).count(), 0, 'the offer came back with nothing to wait for');
  assert.deepEqual(errors, []);
  await context.close();
});

await check('a rebuild that ends with the game never opened leaves nothing behind', async () => {
  const { page, context, errors, box } = await openRebuild();
  await offer(page).waitFor();
  await box.finish();
  await offer(page).waitFor({ state: 'detached', timeout: 10000 });
  assert.equal(await page.getByTestId('game-ready').count(), 0);
  assert.deepEqual(errors, []);
  await context.close();
});

await check('on a phone the offer is an icon with its name, and the game fits the width', async () => {
  const { page, context, errors } = await openRebuild({ viewport: { width: 390, height: 844 }, hasTouch: true, colorScheme: 'dark' });
  await offer(page).waitFor();
  assert.equal((await offer(page).innerText()).trim(), '', 'the label shows at phone width');
  await offer(page).tap();
  await switchTo(page, 'pairs');
  const box = await canvas(page).boundingBox();
  await page.touchscreen.tap(box.x + box.width / 8, box.y + box.height / 8);
  await page.touchscreen.tap(box.x + (box.width * 3) / 8, box.y + box.height / 8);
  assert.match(await scoreText(page), /Tries 1/, 'a tap did not turn a card');
  const wide = await page.evaluate(() => document.documentElement.scrollWidth > window.innerWidth);
  assert.ok(!wide, 'the page scrolls sideways with the game open');
  assert.deepEqual(errors, []);
  await context.close();
});

await check("the wizard's wait offers the game, and it gives way when the box is ready", async () => {
  const context = await browser.newContext({ viewport: { width: 1280, height: 900 }, locale: 'en-US' });
  const page = await context.newPage();
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  let installed = false;
  await page.route('**/api/**', (route) => json(route, 200, {}));
  await page.route('**/api/setup/claim', (route) =>
    json(route, 200, { claimed: false, ready: installed, waitingFor: installed ? null : 'LosOS Git is still starting.' }),
  );
  await page.route('**/setup/state.json', (route) =>
    json(route, 200, { hostName: 'mattbox', fqdn: 'mattbox.local', address: '192.168.122.56', tls: false, certificate: null }),
  );
  await page.goto(origin + '/', { waitUntil: 'networkidle' });
  await page.getByRole('button', { name: /^Continue$/ }).click();
  await offer(page).click();
  await game(page).waitFor();
  assert.ok(await page.locator('input[name="new-password"]').isDisabled(), 'the form opened while the box was not ready');
  installed = true;
  await page.getByTestId('game-ready').waitFor({ timeout: 10000 });
  await page.locator('input[name="new-password"]:not([disabled])').waitFor();
  await page.getByRole('button', { name: 'Close game' }).click();
  await page.getByTestId('game-ready').waitFor({ state: 'detached' });
  assert.deepEqual(errors, []);
  await context.close();
});

await browser.close();
await closeServer();
finish();
