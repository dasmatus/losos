/* The signed-in half of the admin UI: what an owner gets after the gate.
 *
 * wizard.browser.mjs covers which screen a box lands on. This covers what
 * happens next: the key prompt accepts and refuses the right things, every
 * sidebar entry lands on the address it claims, a deep link survives a reload
 * (nginx's `try_files` in production, the static server's fallback here), an
 * unknown path says so, and no route throws while rendering.
 *
 * Same arrangement as the wizard file: the real dist/ bundle, the API stubbed
 * with page.route(). The stub enforces the Bearer token the way lososd does, so
 * a wrong key really is refused rather than assumed to be.
 */

import assert from 'node:assert';
import { launch, runner, serve } from './harness.mjs';

const TOKEN = 'a'.repeat(64);
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
  apachePort: 80,
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

/* A claimed box whose API refuses anything without the right Bearer token.
 * `stored` pre-seeds sessionStorage, i.e. a returning owner in the same tab. */
async function open({
  path = '/',
  stored = false,
  viewport = { width: 1280, height: 900 },
  locale = 'en-US',
  market = null,
} = {}) {
  const page = await browser.newPage({ viewport, locale });
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));

  // Catch-all first: the last-registered matching route wins (see the wizard file).
  await page.route('**/api/**', (route) => json(route, 200, {}));
  await page.route('**/api/setup/claim', (route) => json(route, 200, { claimed: true }));
  const authed = (route) =>
    route.request().headers()['authorization'] === `Bearer ${TOKEN}`;
  await page.route('**/api/state', (route) =>
    authed(route) ? json(route, 200, { mode: 'local', sharing: false }) : json(route, 401, { error: 'unauthorized' }),
  );
  await page.route('**/api/settings', (route) =>
    authed(route) ? json(route, 200, SETTINGS) : json(route, 401, { error: 'unauthorized' }),
  );
  await page.route('**/api/status', (route) =>
    authed(route)
      ? json(route, 200, { state: 'idle', progress: 0, message: '' })
      : json(route, 401, { error: 'unauthorized' }),
  );
  await page.route('**/api/market', (route) =>
    market === null ? json(route, 200, { available: false }) : market.get(route),
  );
  await page.route('**/api/market/**', (route) =>
    market === null ? json(route, 200, { available: false }) : market.action(route),
  );

  if (stored) {
    await page.addInitScript((t) => window.sessionStorage.setItem('losos-token', t), TOKEN);
  }
  await page.goto(origin + path, { waitUntil: 'networkidle' });
  return { page, errors };
}

const body = (page) => page.locator('body').innerText();
const nav = (page) => page.getByRole('navigation', { name: 'Sections', exact: true });

console.log('admin-ui app browser checks');

await check('a wrong key is refused and the prompt stays up', async () => {
  const { page } = await open();
  await page.getByRole('textbox').fill('b'.repeat(64));
  await page.getByRole('button', { name: 'Unlock' }).click();
  await page.waitForTimeout(300);
  assert.ok(/Unlock this box/i.test(await body(page)), 'the prompt closed on a wrong key');
  assert.equal(
    await page.getByRole('button', { name: 'Sign out' }).count(),
    0,
    'the app unlocked on a wrong key',
  );
  assert.equal(await page.evaluate(() => sessionStorage.getItem('losos-token')), null);
  await page.close();
});

await check('the right key opens the app and is kept for the tab', async () => {
  const { page } = await open();
  await page.getByRole('textbox').fill(TOKEN);
  await page.getByRole('button', { name: 'Unlock' }).click();
  await page.getByRole('button', { name: 'Sign out' }).waitFor();
  await page.getByText('Unlock this box').waitFor({ state: 'hidden' });
  assert.equal(await page.evaluate(() => sessionStorage.getItem('losos-token')), TOKEN);
  await page.close();
});

await check('signing out returns to the prompt and forgets the key', async () => {
  const { page } = await open({ stored: true });
  await page.getByRole('button', { name: 'Sign out' }).click();
  await page.getByText('Unlock this box').waitFor();
  assert.equal(await page.evaluate(() => sessionStorage.getItem('losos-token')), null);
  await page.close();
});

const DESTINATIONS = [
  ['Overview', '/'],
  ['Apps', '/apps'],
  ['Storage', '/storage'],
  ['Mesh', '/mesh'],
  ['Settings', '/settings'],
];
await check('every sidebar entry lands on its own address', async () => {
  const { page } = await open({ stored: true });
  for (const [label, path] of DESTINATIONS) {
    await nav(page).getByRole('link', { name: label, exact: true }).click();
    await page.waitForURL(origin + path);
    // aria-current follows the route a render later than the URL changes.
    await nav(page)
      .locator('a[aria-current="page"]')
      .filter({ hasText: label })
      .waitFor({ timeout: 2000 });
    assert.equal(
      await nav(page).locator('a[aria-current="page"]').count(),
      1,
      `${label}: expected exactly one entry marked current`,
    );
  }
  await page.close();
});

for (const path of ['/apps', '/storage', '/mesh', '/settings', '/settings/hardware', '/settings/about', '/settings/reset', '/settings/market']) {
  await check(`a deep link to ${path} renders without errors and survives a reload`, async () => {
    const { page, errors } = await open({ path, stored: true });
    await nav(page).waitFor();
    assert.ok(!/Nothing here/.test(await body(page)), `${path} fell through to not-found`);
    await page.reload({ waitUntil: 'networkidle' });
    await nav(page).waitFor();
    assert.equal(new URL(page.url()).pathname, path);
    assert.deepStrictEqual(errors, [], `page errors: ${errors.join('; ')}`);
    await page.close();
  });
}

await check('the market deep link shows the unavailable state', async () => {
  const { page, errors } = await open({ path: '/settings/market', stored: true });
  await page.getByText('The market is not available on this box').waitFor();
  assert.deepStrictEqual(errors, []);
  await page.reload({ waitUntil: 'networkidle' });
  await page.getByText('The market is not available on this box').waitFor();
  await page.close();
});

await check('market onboarding, listing validation, and checkout use their API and Stripe tabs', async () => {
  let sellerReady = false;
  let maliciousCheckout = false;
  const calls = [];
  const marketAccount = () => ({
    fee_bps: 400,
    currency: 'eur',
    seller_onboarded: sellerReady,
    seller_ready: sellerReady,
    can_sell_storage: true,
    can_sell_compute: false,
    listings: [],
    entitlements: { storage_gib: 0, compute_vcpu_hours: 0, next_expiry: null },
    purchases: [],
    sales: [],
  });
  const market = {
    get: (route) =>
      json(route, 200, {
        available: true,
        listings: [
          { id: 'lst_browser', kind: 'storage', unit: 'GiB-month', unit_price: 100, currency: 'eur', available: 4 },
        ],
        account: marketAccount(),
      }),
    action: async (route) => {
      const path = new URL(route.request().url()).pathname;
      const body = route.request().postDataJSON();
      calls.push({ path, body });
      if (path === '/api/market/onboard') {
        sellerReady = true;
        return json(route, 200, { available: true, ready: false, url: 'https://connect.stripe.com/setup/browser' });
      }
      if (path === '/api/market/listings') return json(route, 200, { available: true });
      if (path === '/api/market/orders') {
        return json(route, 200, {
          available: true,
          checkout_url: maliciousCheckout
            ? 'https://checkout.stripe.com.attacker.example/c/pay/cs_browser'
            : 'https://checkout.stripe.com/c/pay/cs_browser',
        });
      }
      return json(route, 200, { available: true });
    },
  };
  const { page, errors } = await open({ path: '/settings/market', stored: true, market });
  await page.context().route('https://connect.stripe.com/**', (route) => route.fulfill({ body: 'stub' }));
  await page.context().route('https://checkout.stripe.com/**', (route) => route.fulfill({ body: 'stub' }));

  const setupPopupPromise = page.waitForEvent('popup');
  await page.getByRole('button', { name: 'Set up payouts' }).click();
  const setupPopup = await setupPopupPromise;
  await setupPopup.waitForURL('https://connect.stripe.com/setup/browser');
  assert.equal(await setupPopup.evaluate(() => window.opener), null);
  await setupPopup.close();

  await page.getByLabel('Price per unit (EUR)').fill('invalid');
  await page.getByLabel('Capacity (units)').fill('2');
  await page.getByRole('button', { name: 'Offer for sale' }).click();
  await page.getByText('Enter a price like 0.05 (at most two decimals, more than zero).').waitFor();
  assert.equal(calls.some((call) => call.path === '/api/market/listings'), false);

  await page.getByLabel('Price per unit (EUR)').fill('1.25');
  const listingRequestPromise = page.waitForRequest((request) => request.url().endsWith('/api/market/listings'));
  await page.getByRole('button', { name: 'Offer for sale' }).click();
  const listingRequest = await listingRequestPromise;
  assert.deepStrictEqual(listingRequest.postDataJSON(), { kind: 'storage', unit_price: 125, capacity: 2 });

  await page.getByLabel('Quantity').fill('2');
  const orderRequestPromise = page.waitForRequest((request) => request.url().endsWith('/api/market/orders'));
  const checkoutPopupPromise = page.waitForEvent('popup');
  await page.getByRole('button', { name: 'Buy', exact: true }).click();
  const [orderRequest, checkoutPopup] = await Promise.all([orderRequestPromise, checkoutPopupPromise]);
  assert.deepStrictEqual(orderRequest.postDataJSON(), { listing_id: 'lst_browser', quantity: 2 });
  await checkoutPopup.waitForURL('https://checkout.stripe.com/c/pay/cs_browser');
  await checkoutPopup.close();

  maliciousCheckout = true;
  const blockedPopupPromise = page.waitForEvent('popup');
  await page.getByRole('button', { name: 'Buy', exact: true }).click();
  const blockedPopup = await blockedPopupPromise;
  await page.getByText('The order was made, but no payment page came back. Refresh and try again.').waitFor();
  assert.equal(blockedPopup.isClosed(), true);

  assert.deepStrictEqual(errors, []);
  await page.close();
});

await check('an unknown address says so instead of showing a blank page', async () => {
  const { page } = await open({ path: '/no/such/page', stored: true });
  await page.getByText('Nothing here').waitFor();
  await page.close();
});

await check('the theme toggle stamps the choice on <html> and it survives a reload', async () => {
  const { page } = await open({ stored: true });
  const stamp = () => page.evaluate(() => document.documentElement.className + '|' + document.documentElement.dataset.theme);
  const before = await stamp();
  const dark = page.getByRole('radio', { name: 'Dark' });
  await dark.click();
  const after = await stamp();
  assert.notEqual(after, before, 'choosing Dark changed nothing on <html>');
  await page.reload({ waitUntil: 'networkidle' });
  const reloaded = await stamp();
  assert.equal(reloaded, after, 'the theme was lost on reload (theme-boot.js did not stamp it)');
  await page.close();
});

await check('a Slovak browser gets Slovak, and the picker switches to German for good', async () => {
  const { page, errors } = await open({ stored: true, locale: 'sk-SK' });
  await page.getByRole('navigation', { name: 'Sekcie', exact: true }).getByText('Prehľad').waitFor();
  assert.equal(await page.evaluate(() => document.documentElement.lang), 'sk');
  await page.getByLabel('Jazyk').selectOption('de');
  await page.getByRole('navigation', { name: 'Bereiche', exact: true }).getByText('Übersicht').waitFor();
  await page.reload({ waitUntil: 'networkidle' });
  await page.getByRole('navigation', { name: 'Bereiche', exact: true }).getByText('Übersicht').waitFor();
  assert.equal(await page.evaluate(() => document.documentElement.lang), 'de');
  assert.deepEqual(errors, []);
  await page.close();
});

await check('a browser in a language we do not carry falls back to English', async () => {
  const { page } = await open({ stored: true, locale: 'fr-FR' });
  await nav(page).getByText('Overview').waitFor();
  await page.close();
});

for (const locale of ['sk-SK', 'de-DE']) {
  await check(`every section renders in ${locale} without errors`, async () => {
    for (const path of ['/', '/apps', '/storage', '/mesh', '/settings/network', '/settings/hardware', '/settings/security', '/settings/about', '/settings/reset', '/settings/market']) {
      const { page, errors } = await open({ path, stored: true, locale });
      await page.locator('main').waitFor();
      assert.deepEqual(errors, [], `${path} threw in ${locale}`);
      await page.close();
    }
  });
}

await check('a phone-width viewport does not scroll the page sideways', async () => {
  const { page } = await open({ stored: true, viewport: { width: 375, height: 800 } });
  await nav(page).waitFor();
  const overflow = await page.evaluate(
    () => document.documentElement.scrollWidth - document.documentElement.clientWidth,
  );
  assert.ok(overflow <= 0, `page is ${overflow}px wider than the viewport`);
  await page.close();
});

await browser.close();
await closeServer();
finish();
