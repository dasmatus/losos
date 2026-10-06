/* The toast stack: every status update in the admin UI and the wizard is a
 * notification sliding in at the top right (src/components/ui/toast.tsx over
 * shadcn's Sonner, src/components/ui/sonner.tsx).
 *
 * What is checked here is the behaviour the other two files only meet in
 * passing: a toast lands in the top-right corner and not in the page, a new
 * one stacks above the old, a success goes on its own after a few seconds
 * and an error stays longer, the close button is immediate, the pointer
 * pauses the countdown, prefers-reduced-motion switches the slide off, and
 * the stack is still styled under the appliance's Content-Security-Policy
 * (Sonner injects its stylesheet as a <style> element, which that policy
 * refuses; the bundle has to carry it as a file). Each is driven through a real flow (an apply, a sign-in, a claim), never
 * by calling toast() from the test: the point is that the flows raise them.
 *
 * Same arrangement as the other files: the real dist/ bundle, the API
 * stubbed with page.route().
 */

import assert from 'node:assert';
import { launch, runner, serve } from './harness.mjs';

const TOKEN = 'a'.repeat(64);
const PASSWORD = 'Correct-horse battery staple 1';
const { origin, close: closeServer } = await serve();
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

/* A claimed box, signed in, with a rebuild that `statuses` scripts: idle
 * until the page POSTs /api/apply, then each poll of /api/status pops the
 * next answer and the last one repeats. (Not from the first request: the
 * screen asks /api/status once on load to join a rebuild already running,
 * and React may ask twice, so a script that started there would hand the
 * "building" meant for after the apply to the load.) Every script passes
 * through `building` first, as lososd does: the form ignores a terminal
 * state that arrives before any `building` for a while, since it could be
 * the previous job's (SETTLE_MS in use-settings-form.ts). */
async function openApp({
  path = '/settings/network',
  statuses = [
    { state: 'building', progress: 0, message: '', job: 'job-1' },
    { state: 'done', progress: 100, message: '', job: 'job-1' },
  ],
  applyStatus = 200,
  reducedMotion = 'no-preference',
  viewport = { width: 1280, height: 900 },
  colorScheme = 'light',
} = {}) {
  const page = await browser.newPage({ viewport, locale: 'en-US', reducedMotion, colorScheme });
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  const queue = [...statuses];
  const applies = [];

  await page.route('**/api/**', (route) => json(route, 200, {}));
  await page.route('**/api/setup/claim', (route) => json(route, 200, { claimed: true }));
  await page.route('**/api/state', (route) => json(route, 200, { mode: 'local', sharing: false }));
  await page.route('**/api/settings', (route) => json(route, 200, SETTINGS));
  await page.route('**/api/status', (route) => {
    if (applies.length === 0) return json(route, 200, { state: 'idle', progress: 0, message: '' });
    const next = queue.length > 1 ? queue.shift() : queue[0];
    return json(route, 200, next);
  });
  await page.route('**/api/apply', (route) => {
    applies.push(route.request().postData());
    return applyStatus === 200
      ? json(route, 200, { job: 'job-1' })
      : json(route, applyStatus, { error: 'nixos-rebuild is already running' });
  });
  await page.addInitScript((t) => window.sessionStorage.setItem('losos-token', t), TOKEN);
  await page.goto(origin + path, { waitUntil: 'networkidle' });
  return { page, errors, applies };
}

/* Rename the box and press Apply: the one change every pane shares. */
async function applyARename(page) {
  await page.getByLabel('This box is called').fill('fishbox');
  await page.getByRole('button', { name: 'Apply', exact: true }).click();
}

/* Sonner's toast element: data-type carries the tone. */
const toasts = (page) => page.locator('[data-sonner-toast]');
const toastWith = (page, text) => page.locator('[data-sonner-toast]').filter({ hasText: text });

console.log('admin-ui toast checks');

await check('an applied change is reported as a toast in the top-right corner, and the bar leaves', async () => {
  const { page, errors, applies } = await openApp({
    statuses: [
      { state: 'building', progress: 0, message: 'switching to the new generation', job: 'job-1' },
      { state: 'done', progress: 100, message: 'activation finished', job: 'job-1' },
    ],
  });
  await applyARename(page);
  assert.equal(applies.length, 1, 'expected one POST /api/apply');

  // Starting: an info toast, and the sticky bar as the progress report.
  const starting = toastWith(page, 'Applying your changes');
  await starting.waitFor();
  assert.equal(await starting.getAttribute('data-type'), 'info');
  await page.getByRole('progressbar').waitFor();

  // Finished: a success toast with the box's own last line.
  const done = toastWith(page, 'Changes applied');
  await done.waitFor({ timeout: 10000 });
  assert.equal(await done.getAttribute('data-type'), 'success');
  assert.match(await done.innerText(), /activation finished/, "the box's status message is not on the toast");

  // Top right: the toast's right edge sits near the viewport's, its top near the top.
  const box = await done.boundingBox();
  const width = await page.evaluate(() => window.innerWidth);
  assert.ok(box !== null && width - (box.x + box.width) < 40, `toast is not at the right edge: ${JSON.stringify(box)}`);
  assert.ok(box.y < 120, `toast is not at the top: ${JSON.stringify(box)}`);

  // The bar is gone: nothing inline repeats the outcome.
  await page.getByRole('progressbar').waitFor({ state: 'hidden' });
  const main = await page.locator('main').innerText();
  assert.ok(!/Changes applied/.test(main), 'the outcome is doubled up inline in the pane');
  assert.deepEqual(errors, []);
  await page.close();
});

await check('a new toast stacks above the older one, and a success leaves on its own after a few seconds', async () => {
  const { page } = await openApp({
    statuses: [
      { state: 'building', progress: 0, message: '', job: 'job-1' },
      { state: 'done', progress: 100, message: '', job: 'job-1' },
    ],
  });
  await applyARename(page);
  const starting = toastWith(page, 'Applying your changes');
  const done = toastWith(page, 'Changes applied');
  await done.waitFor({ timeout: 10000 });
  await starting.waitFor();
  const [a, b] = await Promise.all([starting.boundingBox(), done.boundingBox()]);
  assert.ok(b.y < a.y, `the newer toast should sit above the older: new y=${b.y}, old y=${a.y}`);

  // Both are gone within their 5 s, with no click.
  await starting.waitFor({ state: 'hidden', timeout: 8000 });
  await done.waitFor({ state: 'hidden', timeout: 8000 });
  assert.equal(await toasts(page).count(), 0);
  await page.close();
});

await check('a failed apply is an error toast carrying the box\'s last log line, and it stays longer', async () => {
  const { page } = await openApp({
    statuses: [
      { state: 'building', progress: 0, message: '', job: 'job-1' },
      { state: 'failed', progress: 0, message: 'error: builder for /nix/store/…-nextcloud.drv failed', job: 'job-1' },
    ],
  });
  await applyARename(page);
  const failed = toastWith(page, 'could not be applied');
  await failed.waitFor({ timeout: 10000 });
  assert.equal(await failed.getAttribute('data-type'), 'error');
  assert.match(await failed.innerText(), /nextcloud\.drv failed/);
  // Still there when a success would long have gone.
  await page.waitForTimeout(6500);
  assert.equal(await failed.count(), 1, 'the error left as fast as a success would');
  // And gone of its own accord afterwards.
  await failed.waitFor({ state: 'hidden', timeout: 6000 });
  await page.close();
});

await check('an apply that does not start is an error toast, not a stuck bar', async () => {
  const { page } = await openApp({ applyStatus: 409 });
  await applyARename(page);
  const toast = toastWith(page, 'did not start');
  await toast.waitFor();
  assert.equal(await toast.getAttribute('data-type'), 'error');
  assert.match(await toast.innerText(), /already running/);
  assert.equal(await page.getByRole('progressbar').count(), 0, 'the progress bar stayed up after the refusal');
  // Apply is live again: the change is still pending.
  assert.ok(await page.getByRole('button', { name: 'Apply', exact: true }).isEnabled());
  await page.close();
});

await check('the close button dismisses a toast at once', async () => {
  const { page } = await openApp({
    statuses: [
      { state: 'building', progress: 0, message: '', job: 'job-1' },
      { state: 'failed', progress: 0, message: 'something', job: 'job-1' },
    ],
  });
  await applyARename(page);
  const failed = toastWith(page, 'could not be applied');
  await failed.waitFor({ timeout: 10000 });
  await failed.getByRole('button', { name: 'Dismiss' }).click();
  await failed.waitFor({ state: 'hidden', timeout: 2000 });
  await page.close();
});

await check('the pointer over a toast pauses its countdown', async () => {
  const { page } = await openApp({
    statuses: [
      { state: 'building', progress: 0, message: '', job: 'job-1' },
      { state: 'done', progress: 100, message: '', job: 'job-1' },
    ],
  });
  await applyARename(page);
  const done = toastWith(page, 'Changes applied');
  await done.waitFor({ timeout: 10000 });
  await done.hover();
  // Past its 5 s and still there while the pointer rests on it.
  await page.waitForTimeout(6000);
  assert.equal(await done.count(), 1, 'the toast left while hovered');
  await page.mouse.move(10, 10);
  await done.waitFor({ state: 'hidden', timeout: 8000 });
  await page.close();
});

await check('prefers-reduced-motion switches the slide off', async () => {
  const { page } = await openApp({
    reducedMotion: 'reduce',
    statuses: [
      { state: 'building', progress: 0, message: '', job: 'job-1' },
      { state: 'done', progress: 100, message: '', job: 'job-1' },
    ],
  });
  await applyARename(page);
  const done = toastWith(page, 'Changes applied');
  await done.waitFor({ timeout: 10000 });
  // Sonner moves a toast with a transition on transform, not a keyframe
  // animation; the global reduced-motion rule in index.css (and Sonner's
  // own) must have zeroed it.
  const motion = await done.evaluate((el) => {
    const s = getComputedStyle(el);
    return { transition: s.transitionDuration, animation: s.animationName };
  });
  assert.ok(/^(0s)(, 0s)*$/.test(motion.transition), `the toast still slides under reduced motion: ${motion.transition}`);
  assert.equal(motion.animation, 'none', `the toast still animates under reduced motion: ${motion.animation}`);
  await page.close();
});

await check('unlocking the admin pages raises a toast, and a wrong password does not', async () => {
  const page = await browser.newPage({ viewport: { width: 1280, height: 900 }, locale: 'en-US' });
  await page.route('**/api/**', (route) => json(route, 200, {}));
  await page.route('**/api/setup/claim', (route) => json(route, 200, { claimed: true }));
  await page.route('**/api/settings', (route) => json(route, 200, SETTINGS));
  await page.route('**/api/status', (route) => json(route, 200, { state: 'idle', progress: 0, message: '' }));
  await page.route('**/api/sign-in', (route) =>
    route.request().postDataJSON()?.password === PASSWORD
      ? json(route, 200, { user: 'notshared', token: TOKEN })
      : json(route, 401, { error: 'that password was not accepted' }),
  );
  await page.goto(origin + '/', { waitUntil: 'networkidle' });

  await page.locator('#owner-password').fill('wrong password 1A!');
  await page.getByRole('button', { name: 'Unlock' }).click();
  await page.getByText('That password was not accepted').waitFor();
  assert.equal(await toasts(page).count(), 0, 'a refused password must stay under the field, not become a toast');

  await page.locator('#owner-password').fill(PASSWORD);
  await page.getByRole('button', { name: 'Unlock' }).click();
  const unlocked = toastWith(page, 'Unlocked');
  await unlocked.waitFor();
  assert.equal(await unlocked.getAttribute('data-type'), 'success');
  await page.close();
});

/* The wizard has its own shell and so its own stack: the claim on step 2
 * raises "Password set" there. */
await check('the wizard raises its toasts too: the claim on step 2', async () => {
  const page = await browser.newPage({ viewport: { width: 1280, height: 900 }, locale: 'en-US' });
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  await page.route('**/api/**', (route) => json(route, 200, {}));
  await page.route('**/api/setup/claim', (route) =>
    route.request().method() === 'POST'
      ? json(route, 200, { claimed: true, user: 'notshared', token: '0'.repeat(64) })
      : json(route, 200, { claimed: false, ready: true, waitingFor: null }),
  );
  await page.route('**/setup/state.json', (route) =>
    json(route, 200, {
      hostName: 'mattbox',
      fqdn: 'mattbox.local',
      address: '192.168.122.56',
      tls: true,
      certificate: {
        url: '/setup/losos-ca.crt',
        fingerprint: 'sha256:' + 'ab'.repeat(32),
        fingerprintDisplay: 'AB:'.repeat(31) + 'AB',
        expires: '2028-01-01T00:00:00Z',
        install: { sh: '/setup/trust.sh', ps1: '/setup/trust.ps1' },
      },
    }),
  );
  await page.goto(origin + '/', { waitUntil: 'networkidle' });

  await page.getByRole('button', { name: /^Continue$/ }).click();
  await page.locator('input[name="new-password"]').fill(PASSWORD);
  await page.locator('input[name="confirm-password"]').fill(PASSWORD);
  await page.getByRole('button', { name: /^Set the password$/ }).click();
  const set = toastWith(page, 'Password set');
  await set.waitFor();
  assert.equal(await set.getAttribute('data-type'), 'success');
  assert.match(await set.innerText(), /notshared/, 'the account name is not on the toast');

  // The copy button on the key: its outcome is a toast as well.
  await page.getByRole('button', { name: /^Copy/ }).first().click();
  await toastWith(page, /copied|Not copied/i).first().waitFor();
  assert.deepEqual(errors, []);
  await page.close();
});

/* The real policy: the bundle served with the admin vhost's CSP header. A
 * toast must still come out styled (position, width, the surface colour,
 * the house stripe), which it only does when Sonner's stylesheet reached
 * the page as a file and the palette came from a class, since the policy
 * refuses the <style> element Sonner injects and any style attribute. The
 * refused element is a console line, never a page error. */
await check('under the appliance CSP the stack is styled and the page raises no error', async () => {
  const { origin: strict, close } = await serve({ csp: true });
  const page = await browser.newPage({ viewport: { width: 1280, height: 900 }, locale: 'en-US' });
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  const applies = [];
  await page.route('**/api/**', (route) => json(route, 200, {}));
  await page.route('**/api/setup/claim', (route) => json(route, 200, { claimed: true }));
  await page.route('**/api/state', (route) => json(route, 200, { mode: 'local', sharing: false }));
  await page.route('**/api/settings', (route) => json(route, 200, SETTINGS));
  await page.route('**/api/status', (route) =>
    json(route, 200, applies.length === 0
      ? { state: 'idle', progress: 0, message: '' }
      : { state: 'building', progress: 0, message: '', job: 'job-1' }),
  );
  await page.route('**/api/apply', (route) => {
    applies.push(1);
    return json(route, 200, { job: 'job-1' });
  });
  await page.addInitScript((t) => window.sessionStorage.setItem('losos-token', t), TOKEN);
  await page.goto(strict + '/settings/network', { waitUntil: 'networkidle' });
  const header = await page.evaluate(async () => (await fetch('/')).headers.get('content-security-policy'));
  assert.match(header ?? '', /style-src 'self'/, 'the strict server is not sending the policy');

  await applyARename(page);
  const starting = toastWith(page, 'Applying your changes');
  await starting.waitFor();
  const style = await starting.evaluate((el) => {
    const s = getComputedStyle(el);
    return {
      position: s.position,
      radius: s.getPropertyValue('--border-radius').trim(),
      background: s.backgroundColor,
      shadow: s.boxShadow,
    };
  });
  assert.equal(style.position, 'absolute', "Sonner's own stylesheet did not reach the page");
  // 9px is --card-radius; Sonner's own default is 8px, so the two cannot be confused.
  assert.equal(style.radius, '9px', `the house radius is not applied: ${style.radius}`);
  assert.equal(style.background, 'rgb(255, 255, 255)', `the surface colour is not applied: ${style.background}`);
  assert.match(style.shadow, /inset/, 'the tone stripe is missing');
  const box = await starting.boundingBox();
  assert.ok(box !== null && 1280 - (box.x + box.width) < 40 && box.y < 120, `not in the corner: ${JSON.stringify(box)}`);
  assert.deepEqual(errors, []);
  await page.close();
  await close();
});

await browser.close();
await closeServer();
finish();
