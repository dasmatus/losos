/* The first-run tour (src/tour/): a step per thing the box does, in a
 * dialog, each a looping picture drawn as SVG from a frame number, over one
 * sentence. Eleven steps on a box that shares nothing; a box that shares
 * its storage in a mesh gets a twelfth, for chat, mail and calls.
 *
 * What is checked: it opens over the app when this browser has it pending
 * (the wizard's Finish sets that; wizard.browser.mjs follows that path), the
 * buttons, the dots and the arrow keys move between steps, Skip, Escape and
 * Done close it and it stays closed on a reload, the About pane opens it
 * again, the picture moves and holds still under prefers-reduced-motion, and
 * none of it trips the appliance's Content-Security-Policy. The scenes are
 * SVG presentation attributes and Tailwind classes only, so a style
 * attribute slipping in would show up here as a violation. (Sonner's refused
 * copy of its stylesheet is logged at every load, tour or not; what counts is
 * that the tour adds nothing to it.)
 *
 * Same arrangement as the other files: the real dist/ bundle, the API
 * stubbed with page.route().
 */

import assert from 'node:assert';
import { launch, runner, serve } from './harness.mjs';

const TOKEN = 'a'.repeat(64);
const { origin, close: closeServer } = await serve({ csp: true });
const browser = await launch();
const { check, finish } = runner();

const SETTINGS = {
  sharingMyStorage: false,
  nextcloudMode: 'container',
  forgejoMode: 'container',
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

/* A claimed box, signed in. `tour` is what this browser remembers about the
 * tour: "pending" after the wizard, "seen" once closed, null for nothing. */
async function openApp({ path = '/', tour = 'pending', reducedMotion = 'no-preference', locale = 'en-US', settings = SETTINGS, settingsDelay = 0 } = {}) {
  const context = await browser.newContext({ viewport: { width: 1280, height: 900 }, locale, reducedMotion });
  const page = await context.newPage();
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));

  await page.route('**/api/**', (route) => json(route, 200, {}));
  await page.route('**/api/setup/claim', (route) => json(route, 200, { claimed: true }));
  await page.route('**/api/state', (route) => json(route, 200, { mode: 'local', sharing: false }));
  await page.route('**/api/settings', async (route) => {
    if (settingsDelay > 0) await new Promise((resolve) => setTimeout(resolve, settingsDelay));
    await json(route, 200, settings).catch(() => {});
  });
  await page.route('**/api/status', (route) => json(route, 200, { state: 'idle', progress: 0, message: '' }));
  await page.addInitScript(
    ({ token, tour }) => {
      window.sessionStorage.setItem('losos-token', token);
      // Only on the first load: a reload must see what the page itself wrote.
      if (tour !== null && window.sessionStorage.getItem('tour-seeded') === null) {
        window.localStorage.setItem('losos-tour', tour);
        window.sessionStorage.setItem('tour-seeded', '1');
      }
      window.__violations = [];
      document.addEventListener('securitypolicyviolation', (e) =>
        window.__violations.push(`${e.violatedDirective} ${e.blockedURI}`),
      );
    },
    { token: TOKEN, tour },
  );
  // A late answer would hold networkidle until it arrives, which is the moment under test.
  await page.goto(origin + path, { waitUntil: settingsDelay > 0 ? 'load' : 'networkidle' });
  return { page, context, errors };
}

const tour = (page) => page.getByTestId('tour');
const stored = (page) => page.evaluate(() => window.localStorage.getItem('losos-tour'));
const violations = (page) => page.evaluate(() => window.__violations);
/* The picture's markup: what the frame clock changes from one frame to the next. */
/* What a load logs with the tour closed: the baseline the tour must not add to. */
async function violationsAtLoad(path) {
  const { page, context } = await openApp({ path, tour: 'seen' });
  await page.waitForTimeout(300);
  const seen = await violations(page);
  await context.close();
  return seen;
}
const picture = (page) => tour(page).locator('svg').first().evaluate((svg) => svg.innerHTML);

console.log('admin-ui tour checks');

await check('a pending tour opens over the app at step 1, and every step reads in order', async () => {
  const baseline = await violationsAtLoad('/');
  const { page, context, errors } = await openApp();
  await tour(page).waitFor({ timeout: 5000 });
  const titles = [
    'Your apps, on your box',
    'Issues land on the Board',
    'Write together, box to box',
    'Add more apps',
    'Describe an app, get an app',
    'See where the space goes',
    'Lend it while you sleep',
    'Get paid for what you lend',
    'Put a face to your name',
    'It updates itself at night',
    'Find this tour again',
  ];
  for (const [i, title] of titles.entries()) {
    await tour(page).getByRole('heading', { name: title }).waitFor({ timeout: 3000 });
    assert.ok(await tour(page).getByText(`${i + 1} of 11`).isVisible(), `no "${i + 1} of 11" on ${title}`);
    // Let the scene play a little, so every scene's drawing runs under the policy.
    await page.waitForTimeout(400);
    if (i < titles.length - 1) await tour(page).getByRole('button', { name: 'Next' }).click();
  }
  // The last step has Done and no Skip.
  assert.ok(await tour(page).getByRole('button', { name: 'Done' }).isVisible(), 'no Done on the last step');
  assert.equal(await tour(page).getByRole('button', { name: 'Skip the tour' }).count(), 0, 'Skip on the last step');
  await tour(page).getByRole('button', { name: 'Done' }).click();
  await tour(page).waitFor({ state: 'detached', timeout: 3000 });
  assert.equal(await stored(page), 'seen');
  assert.deepEqual(await violations(page), baseline, 'the tour added CSP violations');
  assert.deepEqual(errors, []);
  await context.close();
});

await check('Next holds the focus, so Enter walks the tour', async () => {
  const { page, context } = await openApp();
  await tour(page).waitFor({ timeout: 5000 });
  await page.waitForTimeout(200);
  assert.equal(await page.evaluate(() => document.activeElement?.textContent), 'Next');
  await page.keyboard.press('Enter');
  await tour(page).getByRole('heading', { name: 'Issues land on the Board' }).waitFor({ timeout: 3000 });
  await context.close();
});

const SHARED = { ...SETTINGS, sharingMyStorage: true, clusterEnable: true };
/* The miniature's sidebar labels: the text inside the picture. */
const pictureText = (page) => tour(page).locator('svg').first().evaluate((svg) => svg.textContent);

await check('chat, mail and calls are a step only on a box that shares its storage in a mesh', async () => {
  for (const [settings, expected] of [
    [SETTINGS, false],
    [{ ...SETTINGS, sharingMyStorage: true }, false],
    [{ ...SETTINGS, clusterEnable: true }, false],
    [SHARED, true],
  ]) {
    const { page, context, errors } = await openApp({ settings });
    await tour(page).waitFor({ timeout: 5000 });
    await page.waitForTimeout(400);
    const total = expected ? 12 : 11;
    assert.ok(await tour(page).getByText(`1 of ${total}`).isVisible(), `not "1 of ${total}" for ${JSON.stringify(settings)}`);
    assert.equal(/Chat/.test(await pictureText(page)), expected, 'Chat in the picture\'s sidebar');
    if (expected) {
      await tour(page).getByRole('button', { name: 'Go to step 9', exact: true }).click();
      await tour(page).getByRole('heading', { name: 'Chat, mail and calls' }).waitFor({ timeout: 3000 });
      await page.waitForTimeout(400);
      await tour(page).getByRole('button', { name: 'Next' }).click();
      await tour(page).getByRole('heading', { name: 'Put a face to your name' }).waitFor({ timeout: 3000 });
    } else {
      assert.equal(await tour(page).getByRole('heading', { name: 'Chat, mail and calls' }).count(), 0);
    }
    assert.deepEqual(errors, []);
    await context.close();
  }
});

await check('settings that answer late add the chat step without moving the owner', async () => {
  const { page, context } = await openApp({ settings: SHARED, settingsDelay: 2500 });
  await tour(page).waitFor({ timeout: 5000 });
  await tour(page).getByRole('button', { name: 'Go to step 9', exact: true }).click();
  await tour(page).getByRole('heading', { name: 'Put a face to your name' }).waitFor({ timeout: 3000 });
  await tour(page).getByText('9 of 11').waitFor({ timeout: 1000 });
  // The settings arrive: the chat step goes in before this one, and this one stays on screen.
  await tour(page).getByText('10 of 12').waitFor({ timeout: 5000 });
  assert.ok(await tour(page).getByRole('heading', { name: 'Put a face to your name' }).isVisible());
  await tour(page).getByRole('button', { name: 'Back' }).click();
  await tour(page).getByRole('heading', { name: 'Chat, mail and calls' }).waitFor({ timeout: 3000 });
  await context.close();
});

await check('the tour is wider than the other dialogs, and still fits a phone', async () => {
  for (const [width, expected] of [[1280, 736], [390, 358]]) {
    const { page, context } = await openApp();
    await page.setViewportSize({ width, height: 900 });
    await tour(page).waitFor({ timeout: 5000 });
    const box = await page.locator('dialog[open]').boundingBox();
    assert.ok(Math.abs(box.width - expected) <= 2, `at ${width} px the tour is ${box.width} px wide, not ${expected}`);
    await context.close();
  }
});

await check('Back, the step dots and the arrow keys move between steps', async () => {
  const { page, context } = await openApp();
  await tour(page).waitFor({ timeout: 5000 });
  await tour(page).getByRole('button', { name: 'Go to step 7', exact: true }).click();
  await tour(page).getByRole('heading', { name: 'Lend it while you sleep' }).waitFor();
  assert.equal(await tour(page).getByRole('button', { name: 'Go to step 7', exact: true }).getAttribute('aria-current'), 'step');
  await tour(page).getByRole('button', { name: 'Back' }).click();
  await tour(page).getByRole('heading', { name: 'See where the space goes' }).waitFor();
  await page.keyboard.press('ArrowRight');
  await tour(page).getByRole('heading', { name: 'Lend it while you sleep' }).waitFor();
  await page.keyboard.press('ArrowLeft');
  await page.keyboard.press('ArrowLeft');
  await tour(page).getByRole('heading', { name: 'Describe an app, get an app' }).waitFor();
  // Step 1 has no Back.
  await tour(page).getByRole('button', { name: 'Go to step 1', exact: true }).click();
  assert.equal(await tour(page).getByRole('button', { name: 'Back' }).count(), 0, 'Back on step 1');
  await context.close();
});

await check('Skip closes the tour and a reload does not bring it back', async () => {
  const { page, context } = await openApp();
  await tour(page).waitFor({ timeout: 5000 });
  await tour(page).getByRole('button', { name: 'Skip the tour' }).click();
  await tour(page).waitFor({ state: 'detached', timeout: 3000 });
  assert.equal(await stored(page), 'seen');
  await page.reload({ waitUntil: 'networkidle' });
  await page.waitForTimeout(500);
  assert.equal(await tour(page).count(), 0, 'the tour came back after a reload');
  await context.close();
});

await check('Escape closes the tour the same way', async () => {
  const { page, context } = await openApp();
  await tour(page).waitFor({ timeout: 5000 });
  await page.keyboard.press('Escape');
  await tour(page).waitFor({ state: 'detached', timeout: 3000 });
  assert.equal(await stored(page), 'seen');
  await context.close();
});

await check('a box whose owner has seen the tour does not open it, and About opens it again', async () => {
  const { page, context, errors } = await openApp({ path: '/settings/about', tour: 'seen' });
  await page.waitForTimeout(500);
  assert.equal(await tour(page).count(), 0, 'the tour opened for an owner who had seen it');
  const atLoad = (await violations(page)).length;
  await page.getByRole('button', { name: 'Take the tour' }).click();
  await tour(page).getByRole('heading', { name: 'Your apps, on your box' }).waitFor({ timeout: 5000 });
  await tour(page).getByRole('button', { name: 'Skip the tour' }).click();
  await tour(page).waitFor({ state: 'detached', timeout: 3000 });
  const after = await violations(page);
  assert.equal(after.length, atLoad, `the tour added CSP violations: ${after.slice(atLoad).join(', ')}`);
  assert.deepEqual(errors, []);
  await context.close();
});

await check('the picture moves, and holds still under prefers-reduced-motion', async () => {
  const moving = await openApp();
  await tour(moving.page).waitFor({ timeout: 5000 });
  const a = await picture(moving.page);
  await moving.page.waitForTimeout(600);
  assert.notEqual(await picture(moving.page), a, 'the scene did not move');
  await moving.context.close();

  const still = await openApp({ reducedMotion: 'reduce' });
  await tour(still.page).waitFor({ timeout: 5000 });
  const b = await picture(still.page);
  await still.page.waitForTimeout(600);
  assert.equal(await picture(still.page), b, 'the scene moved under reduced motion');
  await still.context.close();
});

await check('the tour follows the language on screen', async () => {
  const { page, context } = await openApp({ locale: 'sk-SK' });
  await tour(page).waitFor({ timeout: 5000 });
  const text = await tour(page).innerText();
  assert.ok(/1 z 11/.test(text), `the count is not in Slovak:\n${text}`);
  assert.ok(!/Your apps, on your box/.test(text), `the title stayed in English:\n${text}`);
  await context.close();
});

await browser.close();
await closeServer();
finish();
