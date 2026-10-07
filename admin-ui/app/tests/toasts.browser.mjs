/* The notification corner: every status update and every outcome in the
 * admin UI and the wizard is a notification sliding in at the top right
 * (src/components/ui/toast.tsx). Two kinds share the corner: a status
 * ("applying", "not ready yet", "finish paying in the other tab") is
 * shadcn's Sonner (sonner.tsx, `[data-sonner-toast]`), and a confirmation
 * ("applied", "failed", "copied", "paid") is shadcn's Toast over Radix
 * (toaster.tsx, `[data-toast]`). Confirmations take the top of the corner,
 * the status stack sits under them, and a confirmation that answers a
 * status dismisses it.
 *
 * What is checked here is the behaviour the other two files only meet in
 * passing: each kind lands in the top-right corner and not in the page, the
 * status gives way to its outcome, the two never overlap, a success goes on
 * its own after a few seconds and an error stays longer, the close button is
 * immediate, the pointer pauses the countdown, prefers-reduced-motion
 * switches the slide off, and both are still styled under the appliance's
 * Content-Security-Policy (Sonner injects its stylesheet as a <style>
 * element, which that policy refuses; the bundle has to carry it as a file;
 * Radix sets its swipe offset through React's style prop, which the policy
 * allows). Each is driven through a real flow (an apply, a sign-in, a claim), never
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
  await page.route('**/api/storage', (route) => json(route, 200, STORAGE));
  await page.route('**/api/grow', (route) =>
    json(route, 200, { grew: true, beforeBytes: STORAGE.totalBytes, afterBytes: STORAGE.totalBytes + STORAGE.reserveBytes, claimedBytes: STORAGE.reserveBytes }),
  );
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

/* The two kinds. A status is Sonner's element; a confirmation is the Radix
 * toast, with the tone on data-tone. */
const statuses = (page) => page.locator('[data-sonner-toast]');
const statusWith = (page, text) => statuses(page).filter({ hasText: text });
const toasts = (page) => page.locator('[data-toast]');
const toastWith = (page, text) => toasts(page).filter({ hasText: text });

const STORAGE = { totalBytes: 480 * 1024 ** 3, usedBytes: 120 * 1024 ** 3, lentBytes: 0, reserveBytes: 110 * 1024 ** 3 };

/* Right edge near the viewport's, top near the top. */
function inTheCorner(box, width) {
  return box !== null && width - (box.x + box.width) < 40 && box.y < 160;
}
function overlap(a, b) {
  return a.x < b.x + b.width && b.x < a.x + a.width && a.y < b.y + b.height && b.y < a.y + a.height;
}

console.log('admin-ui toast checks');

await check('an apply is a status while it runs and a confirmation when it is done, which settles the status', async () => {
  const { page, errors, applies } = await openApp({
    statuses: [
      { state: 'building', progress: 0, message: 'switching to the new generation', job: 'job-1' },
      { state: 'done', progress: 100, message: 'activation finished', job: 'job-1' },
    ],
  });
  await applyARename(page);
  assert.equal(applies.length, 1, 'expected one POST /api/apply');

  // Running: a Sonner status in the corner, and the sticky bar as the progress report.
  const starting = statusWith(page, 'Applying your changes');
  await starting.waitFor();
  assert.equal(await starting.getAttribute('data-type'), 'info');
  await page.getByRole('progressbar').waitFor();
  const width = await page.evaluate(() => window.innerWidth);
  assert.ok(inTheCorner(await starting.boundingBox(), width), 'the status is not in the corner');
  assert.equal(await toasts(page).count(), 0, 'a confirmation showed before anything was done');

  // Done: a confirmation with the box's own last line, and the status has given way.
  const done = toastWith(page, 'Changes applied');
  await done.waitFor({ timeout: 10000 });
  assert.equal(await done.getAttribute('data-tone'), 'success');
  assert.match(await done.innerText(), /activation finished/, "the box's status message is not on the confirmation");
  await starting.waitFor({ state: 'hidden', timeout: 2000 });
  const box = await done.boundingBox();
  assert.ok(inTheCorner(box, width), `the confirmation is not in the corner: ${JSON.stringify(box)}`);

  // The bar is gone: nothing inline repeats the outcome.
  await page.getByRole('progressbar').waitFor({ state: 'hidden' });
  const main = await page.locator('main').innerText();
  assert.ok(!/Changes applied/.test(main), 'the outcome is doubled up inline in the pane');
  assert.deepEqual(errors, []);
  await page.close();
});

await check('a confirmation lands above a status that is still up, and the two never overlap', async () => {
  // A rebuild that never finishes keeps the status up; a disk grow on the
  // storage pane then produces a confirmation beside it.
  const { page, errors } = await openApp({
    statuses: [{ state: 'building', progress: 0, message: '', job: 'job-1' }],
  });
  await applyARename(page);
  const applying = statusWith(page, 'Applying your changes');
  await applying.waitFor();
  const alone = await applying.boundingBox();

  await page.getByRole('link', { name: 'Storage' }).first().click();
  await page.getByRole('button', { name: /^Use reserve/ }).click();
  await page.getByRole('button', { name: /^Use it/ }).click();
  const grew = toastWith(page, 'more room');
  await grew.waitFor({ timeout: 10000 });
  await page.waitForTimeout(500); // the status slides down under it
  const [top, under] = await Promise.all([grew.boundingBox(), applying.boundingBox()]);
  assert.ok(top !== null && under !== null, 'both should be visible');
  assert.ok(top.y < under.y, `the confirmation should sit above the status: confirmation y=${top.y}, status y=${under.y}`);
  assert.ok(!overlap(top, under), `the two overlap: ${JSON.stringify([top, under])}`);
  assert.ok(under.y > alone.y, 'the status did not move down to make room');
  assert.ok(await applying.isVisible(), 'an unrelated confirmation dismissed the status');
  assert.deepEqual(errors, []);
  await page.close();
});

await check('a success leaves on its own after a few seconds; a persistent status does not', async () => {
  const { page } = await openApp({
    statuses: [
      { state: 'building', progress: 0, message: '', job: 'job-1' },
      { state: 'done', progress: 100, message: '', job: 'job-1' },
    ],
  });
  await applyARename(page);
  const done = toastWith(page, 'Changes applied');
  await done.waitFor({ timeout: 10000 });
  // Gone within its 5 s, with no click.
  await done.waitFor({ state: 'hidden', timeout: 8000 });
  assert.equal(await toasts(page).count(), 0);
  await page.close();
});

await check('a failed apply is an error confirmation carrying the box\'s last log line, and it stays longer', async () => {
  const { page } = await openApp({
    statuses: [
      { state: 'building', progress: 0, message: '', job: 'job-1' },
      { state: 'failed', progress: 0, message: 'error: builder for /nix/store/…-nextcloud.drv failed', job: 'job-1' },
    ],
  });
  await applyARename(page);
  const failed = toastWith(page, 'could not be applied');
  await failed.waitFor({ timeout: 10000 });
  assert.equal(await failed.getAttribute('data-tone'), 'error');
  assert.match(await failed.innerText(), /nextcloud\.drv failed/);
  assert.equal(await failed.locator('a[data-help]').getAttribute('href'), '/handbook/troubleshooting/apply-fails/');
  assert.equal(await statuses(page).count(), 0, 'the "applying" status outlived its failure');
  // Still there when a success would long have gone.
  await page.waitForTimeout(6500);
  assert.equal(await failed.count(), 1, 'the error left as fast as a success would');
  // And gone of its own accord afterwards.
  await failed.waitFor({ state: 'hidden', timeout: 6000 });
  await page.close();
});

await check('an apply that does not start is an error confirmation, not a stuck bar', async () => {
  const { page } = await openApp({ applyStatus: 409 });
  await applyARename(page);
  const toast = toastWith(page, 'did not start');
  await toast.waitFor();
  assert.equal(await toast.getAttribute('data-tone'), 'error');
  assert.match(await toast.innerText(), /already running/);
  /* Every error leads to its handbook page on this box: a "What to do" link
   * to the apply page, opening in a new tab so the pending change stays. */
  const help = toast.locator('a[data-help]');
  assert.equal(await help.count(), 1, 'an error confirmation without a "What to do" link');
  assert.equal(await help.getAttribute('href'), '/handbook/troubleshooting/apply-fails/');
  assert.equal(await help.getAttribute('target'), '_blank');
  assert.equal(await help.innerText(), 'What to do');
  assert.equal(await page.getByRole('progressbar').count(), 0, 'the progress bar stayed up after the refusal');
  assert.equal(await statuses(page).count(), 0, 'an "applying" status is up for a rebuild that never started');
  // Apply is live again: the change is still pending.
  assert.ok(await page.getByRole('button', { name: 'Apply', exact: true }).isEnabled());
  await page.close();
});

await check('the close button dismisses a confirmation at once, and a status too', async () => {
  const { page } = await openApp({
    statuses: [
      { state: 'building', progress: 0, message: '', job: 'job-1' },
      { state: 'failed', progress: 0, message: 'something', job: 'job-1' },
    ],
  });
  await applyARename(page);
  const applying = statusWith(page, 'Applying your changes');
  await applying.waitFor();
  await applying.hover();
  await applying.getByRole('button', { name: 'Dismiss' }).click();
  await applying.waitFor({ state: 'hidden', timeout: 2000 });
  const failed = toastWith(page, 'could not be applied');
  await failed.waitFor({ timeout: 10000 });
  await failed.getByRole('button', { name: 'Dismiss' }).click();
  await failed.waitFor({ state: 'hidden', timeout: 2000 });
  await page.close();
});

await check('the pointer over a confirmation pauses its countdown', async () => {
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
  assert.equal(await done.count(), 1, 'the confirmation left while hovered');
  await page.mouse.move(10, 10);
  await done.waitFor({ state: 'hidden', timeout: 8000 });
  await page.close();
});

await check('prefers-reduced-motion switches the slide off for both kinds', async () => {
  const { page } = await openApp({
    reducedMotion: 'reduce',
    statuses: [
      { state: 'building', progress: 0, message: '', job: 'job-1' },
      { state: 'done', progress: 100, message: '', job: 'job-1' },
    ],
  });
  await applyARename(page);
  const applying = statusWith(page, 'Applying your changes');
  await applying.waitFor();
  // Sonner moves a toast with a transition on transform; the global
  // reduced-motion rule in index.css (and Sonner's own) must have zeroed it.
  const status = await applying.evaluate((el) => getComputedStyle(el).transitionDuration);
  assert.ok(/^(0s)(, 0s)*$/.test(status), `the status still slides under reduced motion: ${status}`);
  const done = toastWith(page, 'Changes applied');
  await done.waitFor({ timeout: 10000 });
  const motion = await done.evaluate((el) => getComputedStyle(el).animationName);
  assert.equal(motion, 'none', `the confirmation still animates under reduced motion: ${motion}`);
  await page.close();
});

await check('unlocking the admin pages is a confirmation, and a wrong password raises nothing', async () => {
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
  assert.equal(await toasts(page).count() + await statuses(page).count(), 0, 'a refused password must stay under the field');

  await page.locator('#owner-password').fill(PASSWORD);
  await page.getByRole('button', { name: 'Unlock' }).click();
  const unlocked = toastWith(page, 'Unlocked');
  await unlocked.waitFor();
  assert.equal(await unlocked.getAttribute('data-tone'), 'success');
  await page.close();
});

/* The wizard has its own shell and so its own corner: step 2 raises the
 * readiness as a status and "Password set" as the confirmation that
 * settles it. */
await check('the wizard raises both kinds: a readiness status on step 2, then the confirmation that settles it', async () => {
  const page = await browser.newPage({ viewport: { width: 1280, height: 900 }, locale: 'en-US' });
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  let claims = 0;
  await page.route('**/api/**', (route) => json(route, 200, {}));
  await page.route('**/api/setup/claim', (route) => {
    if (route.request().method() === 'POST') {
      return json(route, 200, { claimed: true, user: 'notshared', token: '0'.repeat(64) });
    }
    // Not ready for the first two polls, ready from the third: the owner
    // sits through the waiting panel and is told when it lifts.
    claims += 1;
    return claims <= 2
      ? json(route, 200, { claimed: false, ready: false, waitingFor: 'the files app is starting' })
      : json(route, 200, { claimed: false, ready: true, waitingFor: null });
  });
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
  const ready = statusWith(page, /ready/i);
  await ready.waitFor({ timeout: 15000 });
  assert.equal(await ready.getAttribute('data-type'), 'info', 'the box becoming ready is a status, not an outcome');
  await page.locator('input[name="new-password"]:not([disabled])').waitFor();
  await page.locator('input[name="new-password"]').fill(PASSWORD);
  await page.locator('input[name="confirm-password"]').fill(PASSWORD);
  await page.getByRole('button', { name: /^Set the password$/ }).click();
  const set = toastWith(page, 'Password set');
  await set.waitFor();
  assert.equal(await set.getAttribute('data-tone'), 'success');
  assert.match(await set.innerText(), /notshared/, 'the account name is not on the confirmation');
  await ready.waitFor({ state: 'hidden', timeout: 2000 });

  // The copy button on the key: its outcome is a confirmation as well.
  await page.getByRole('button', { name: /^Copy/ }).first().click();
  await toastWith(page, /copied|Not copied/i).first().waitFor();
  assert.deepEqual(errors, []);
  await page.close();
});

/* The real policy: the bundle served with the admin vhost's CSP header. A
 * status must still come out styled (position, the surface colour, the
 * house radius and stripe), which it only does when Sonner's stylesheet
 * reached the page as a file and the palette came from a class, since the
 * policy refuses the <style> element Sonner injects and any style
 * attribute; the refused element is a console line, never a page error.
 * And the Radix confirmation must come out styled and animated too. */
await check('under the appliance CSP both kinds are styled and the page raises no error', async () => {
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
  await page.route('**/api/storage', (route) => json(route, 200, STORAGE));
  await page.route('**/api/grow', (route) =>
    json(route, 200, { grew: true, beforeBytes: STORAGE.totalBytes, afterBytes: STORAGE.totalBytes + STORAGE.reserveBytes, claimedBytes: STORAGE.reserveBytes }),
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
  const starting = statusWith(page, 'Applying your changes');
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
  assert.ok(inTheCorner(await starting.boundingBox(), 1280), 'the status is not in the corner');

  await page.getByRole('link', { name: 'Storage' }).first().click();
  await page.getByRole('button', { name: /^Use reserve/ }).click();
  await page.getByRole('button', { name: /^Use it/ }).click();
  const grew = toastWith(page, 'more room');
  await grew.waitFor({ timeout: 10000 });
  const confirmation = await grew.evaluate((el) => {
    const s = getComputedStyle(el);
    return { background: s.backgroundColor, shadow: s.boxShadow, animation: s.animationName, radius: s.borderRadius };
  });
  assert.equal(confirmation.background, 'rgb(255, 255, 255)', `the confirmation's surface is not applied: ${confirmation.background}`);
  assert.match(confirmation.shadow, /inset/, "the confirmation's tone stripe is missing");
  assert.equal(confirmation.radius, '9px', `the confirmation's radius is not the card radius: ${confirmation.radius}`);
  assert.equal(confirmation.animation, 'toast-in', `the confirmation did not slide in: ${confirmation.animation}`);
  assert.ok(inTheCorner(await grew.boundingBox(), 1280), 'the confirmation is not in the corner');
  assert.deepEqual(errors, []);
  await page.close();
  await close();
});

await browser.close();
await closeServer();
finish();
