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
const { check, skip, finish } = runner();

/* The Market pane is planned, not open (`planned: true` on its row in
 * src/screens/settings/panes.ts): the sidebar shows it greyed with a
 * "soon(TM)" badge, nothing navigates to it, and its address falls back to
 * the default pane. The pane's own checks below are kept behind this switch
 * so that opening the tab is two flips, there and here, and brings its
 * coverage straight back; the checks on the greyed row run only until then.
 * Both states were run green before the switch was set. */
const MARKET_TAB_OPEN = false;
const whenMarketOpen = (name, fn) =>
  MARKET_TAB_OPEN ? check(name, fn) : skip(name, 'the Market tab is planned, not open');
const whenMarketPlanned = (name, fn) =>
  MARKET_TAB_OPEN ? skip(name, 'the Market tab is open') : check(name, fn);

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
  market = { available: false },
  checkoutUrl = 'https://checkout.stripe.com/c/pay/cs_test_1',
  onboardUrl = 'https://connect.stripe.com/setup/e/acct_test/abc',
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
  // The market relay: `market` is GET /api/market's document, every action is
  // recorded, and each answers the way lososd does on success.
  const marketPosts = [];
  await page.route('**/api/market', (route) =>
    authed(route) ? json(route, 200, market) : json(route, 401, { error: 'unauthorized' }),
  );
  await page.route('**/api/market/**', (route) => {
    if (!authed(route)) return json(route, 401, { error: 'unauthorized' });
    const path = new URL(route.request().url()).pathname;
    marketPosts.push([path, route.request().postDataJSON()]);
    if (path === '/api/market/onboard') return json(route, 200, { available: true, ready: false, url: onboardUrl });
    if (path === '/api/market/orders') return json(route, 201, { available: true, checkout_url: checkoutUrl });
    return json(route, 201, { available: true });
  });
  // Stripe's own pages, so a tab sent there has something to land on.
  for (const host of ['checkout.stripe.com', 'connect.stripe.com']) {
    await page.context().route(`https://${host}/**`, (route) =>
      route.fulfill({ status: 200, contentType: 'text/html', body: '<title>Stripe</title>' }),
    );
  }

  if (stored) {
    await page.addInitScript((t) => window.sessionStorage.setItem('losos-token', t), TOKEN);
  }
  await page.goto(origin + path, { waitUntil: 'networkidle' });
  return { page, errors, marketPosts };
}

/* A box the market is offered to: one listing on the shelf, payouts set up,
 * sharing storage only. */
const MARKET = {
  available: true,
  listings: [
    { id: 'lst_ab12', kind: 'storage', unit: 'GiB-month', unit_price: 5, currency: 'eur', available: 40 },
  ],
  account: {
    fee_bps: 400,
    currency: 'eur',
    seller_onboarded: true,
    seller_ready: true,
    can_sell_storage: true,
    can_sell_compute: false,
    listings: [],
    entitlements: { storage_gib: 0, compute_vcpu_hours: 0, next_expiry: null },
    purchases: [],
    sales: [],
  },
};

const body = (page) => page.locator('body').innerText();
const nav = (page) => page.getByRole('navigation', { name: 'Sections', exact: true });

console.log('admin-ui app browser checks');

/* The gate takes the owner's password, and lososd checks it with LosOS cloud
 * (POST /api/sign-in). The stub below plays that route: the right password
 * is answered with the token, anything else with 401 — or, when `cloudDown`
 * is set, with the 503 lososd sends while LosOS cloud cannot be asked. */
const PASSWORD = 'Correct-horse battery staple 1';
async function openGate({ cloudDown = false } = {}) {
  const opened = await open();
  const signIns = [];
  await opened.page.route('**/api/sign-in', (route) => {
    const req = route.request();
    signIns.push({ contentType: req.headers()['content-type'], body: req.postDataJSON() });
    if (cloudDown) {
      return json(route, 503, { error: 'LosOS cloud did not answer', ready: false, waitingFor: 'LosOS cloud did not answer' });
    }
    return req.postDataJSON()?.password === PASSWORD
      ? json(route, 200, { user: 'notshared', token: TOKEN })
      : json(route, 401, { error: 'that password was not accepted' });
  });
  return { ...opened, signIns };
}

await check('the right password opens the app and the token is kept for the tab', async () => {
  const { page, signIns } = await openGate();
  await page.locator('#owner-password').fill(PASSWORD);
  await page.getByRole('button', { name: 'Unlock' }).click();
  await page.getByRole('button', { name: 'Sign out' }).waitFor();
  await page.getByText('Unlock this box').waitFor({ state: 'hidden' });
  assert.equal(await page.evaluate(() => sessionStorage.getItem('losos-token')), TOKEN);
  assert.equal(signIns.length, 1, 'expected exactly one sign-in request');
  assert.equal(signIns[0].contentType, 'application/json', 'lososd refuses a sign-in that is not application/json');
  assert.deepEqual(signIns[0].body, { password: PASSWORD });
  await page.close();
});

await check('a wrong password is refused and the prompt stays up', async () => {
  const { page } = await openGate();
  await page.locator('#owner-password').fill('Wrong-horse battery staple 1');
  await page.getByRole('button', { name: 'Unlock' }).click();
  await page.waitForTimeout(300);
  const text = await body(page);
  assert.ok(/Unlock this box/i.test(text), 'the prompt closed on a wrong password');
  assert.ok(/was not accepted/i.test(text), `no refusal shown:\n${text}`);
  assert.equal(await page.getByRole('button', { name: 'Sign out' }).count(), 0, 'the app unlocked on a wrong password');
  assert.equal(await page.evaluate(() => sessionStorage.getItem('losos-token')), null);
  await page.close();
});

await check('while LosOS cloud is down the prompt says so and points at the spare key', async () => {
  const { page } = await openGate({ cloudDown: true });
  await page.locator('#owner-password').fill(PASSWORD);
  await page.getByRole('button', { name: 'Unlock' }).click();
  await page.waitForTimeout(300);
  const text = await body(page);
  assert.ok(/LosOS cloud is not running/i.test(text), `the 503 was not explained:\n${text}`);
  assert.ok(/spare admin key/i.test(text), 'the spare key is not offered');
  assert.equal(await page.evaluate(() => sessionStorage.getItem('losos-token')), null);
  await page.close();
});

/* The spare: the admin key behind a link, for when LosOS cloud cannot check
 * the password. Same checks as the password half — a wrong one is refused
 * and the right one unlocks and is kept. */
await check('a wrong spare key is refused and the prompt stays up', async () => {
  const { page } = await openGate();
  await page.getByRole('button', { name: /Use the spare admin key instead/ }).click();
  await page.locator('#admin-key').fill('b'.repeat(64));
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

await check('the right spare key opens the app and is kept for the tab', async () => {
  const { page, signIns } = await openGate();
  await page.getByRole('button', { name: /Use the spare admin key instead/ }).click();
  await page.locator('#admin-key').fill(TOKEN);
  await page.getByRole('button', { name: 'Unlock' }).click();
  await page.getByRole('button', { name: 'Sign out' }).waitFor();
  await page.getByText('Unlock this box').waitFor({ state: 'hidden' });
  assert.equal(await page.evaluate(() => sessionStorage.getItem('losos-token')), TOKEN);
  assert.equal(signIns.length, 0, 'the key path must not go through the password route');
  await page.close();
});

await check('signing out returns to the prompt and forgets the token', async () => {
  const { page } = await open({ stored: true });
  await page.getByRole('button', { name: 'Sign out' }).click();
  await page.getByText('Unlock this box').waitFor();
  assert.equal(await page.evaluate(() => sessionStorage.getItem('losos-token')), null);
  await page.close();
});

/* One sidebar (src/components/app-sidebar.tsx): Overview, Apps, Storage and
 * Mesh are links at the top; Settings is a fold whose entries are the
 * settings panes. Every entry, top or folded, must land on its own address
 * and be the only one marked current there. */
const DESTINATIONS = [
  ['Overview', '/'],
  ['Apps', '/apps'],
  ['Storage', '/storage'],
  ['Mesh', '/mesh'],
  ['Network', '/settings/network', 'Settings'],
  ['Hardware', '/settings/hardware', 'Settings'],
  ['Security', '/settings/security', 'Settings'],
  ['About', '/settings/about', 'Settings'],
  ['Reset', '/settings/reset', 'Settings'],
];
/* Open a fold by its name, if it is not open already. */
async function unfold(page, name) {
  const fold = nav(page).getByRole('button', { name, exact: true });
  if ((await fold.getAttribute('aria-expanded')) !== 'true') await fold.click();
}
await check('every sidebar entry, folded or not, lands on its own address', async () => {
  const { page } = await open({ stored: true });
  for (const [label, path, under] of DESTINATIONS) {
    if (under !== undefined) await unfold(page, under);
    await nav(page).getByRole('link', { name: label, exact: true }).click();
    await page.waitForURL(origin + path);
    await nav(page).locator('a[aria-current="page"]').filter({ hasText: label }).waitFor({ timeout: 2000 });
    assert.equal(
      await nav(page).locator('a[aria-current="page"]').count(),
      1,
      `${label}: expected exactly one entry marked current`,
    );
  }
  // /settings itself opens the first entry under Settings.
  await page.goto(origin + '/settings', { waitUntil: 'networkidle' });
  await page.getByRole('heading', { name: 'Network', exact: true, level: 1 }).waitFor();
  await nav(page).locator('a[aria-current="page"]').filter({ hasText: 'Network' }).waitFor();
  await page.close();
});

await check('there is one panel: no second list of panes beside the content', async () => {
  const { page } = await open({ path: '/settings/hardware', stored: true });
  await nav(page).waitFor();
  assert.equal(await page.getByRole('navigation').count(), 1, 'more than one <nav> on a settings page');
  assert.equal(await page.locator('main').getByRole('link').filter({ hasText: /^(Network|Hardware|Security)$/ }).count(), 0);
  await page.close();
});

await check('a fold opens by itself on one of its pages, and folds away from the keyboard', async () => {
  const { page } = await open({ path: '/settings/security', stored: true });
  const settings = nav(page).getByRole('button', { name: 'Settings', exact: true });
  assert.equal(await settings.getAttribute('aria-expanded'), 'true', 'Settings is folded on one of its own pages');
  const mesh = nav(page).getByRole('button', { name: 'Entries under Mesh', exact: true });
  assert.equal(await mesh.getAttribute('aria-expanded'), 'false', 'Mesh is unfolded while elsewhere');
  // Folded entries are inert: neither a click target nor a Tab stop.
  const market = nav(page).getByRole('button', { name: 'Market soon(TM)', exact: true });
  assert.equal(await market.isVisible(), false, 'the folded Market entry is visible');
  // Fold Settings with the keyboard: its pages leave the Tab order with it.
  await settings.focus();
  await page.keyboard.press('Enter');
  assert.equal(await settings.getAttribute('aria-expanded'), 'false');
  const hardware = nav(page).getByRole('link', { name: 'Hardware', exact: true, includeHidden: true });
  await page.waitForTimeout(300);
  assert.equal(await hardware.isVisible(), false, 'a folded page link is still visible');
  assert.equal(await hardware.evaluate((el) => el.closest('[inert]') !== null), true, 'a folded page link is still reachable');
  await page.keyboard.press('Enter');
  assert.equal(await settings.getAttribute('aria-expanded'), 'true');
  await nav(page).getByRole('link', { name: 'Hardware', exact: true }).click();
  await page.waitForURL(origin + '/settings/hardware');
  await page.close();
});

await check('the panel narrows to icons with its trigger or Ctrl+B, and remembers it', async () => {
  const { page, errors } = await open({ path: '/mesh', stored: true });
  const panel = nav(page);
  const wide = (await panel.boundingBox()).width;
  await panel.getByRole('button', { name: 'Collapse the sidebar' }).click();
  await page.waitForTimeout(350);
  const narrow = (await panel.boundingBox()).width;
  assert.ok(narrow < 70 && wide > 200, `the panel did not narrow to a rail: ${wide} -> ${narrow}`);
  // On the rail every entry is still a named link, and the current one is marked.
  await panel.getByRole('link', { name: 'Mesh', exact: true }).waitFor();
  assert.equal(await panel.getByRole('searchbox').isVisible(), false, 'the search box is still on the rail');
  await panel.getByRole('link', { name: 'Settings', exact: true }).click();
  await page.waitForURL(origin + '/settings');
  // The choice survives a reload; Ctrl+B widens it again.
  await page.reload({ waitUntil: 'networkidle' });
  assert.ok((await panel.boundingBox()).width < 70, 'the rail was forgotten on reload');
  await page.keyboard.press('Control+b');
  await page.waitForTimeout(350);
  assert.ok((await panel.boundingBox()).width > 200, 'Ctrl+B did not widen the panel');
  await panel.getByRole('searchbox').waitFor();
  assert.deepEqual(errors, []);
  await page.close();
});

await check('on a phone the panel is one row naming where you are, and opens in place', async () => {
  const { page } = await open({ path: '/settings/about', stored: true, viewport: { width: 375, height: 800 } });
  const panel = nav(page);
  await panel.getByRole('button', { name: 'About', exact: true }).waitFor();
  assert.equal(await panel.getByRole('link').count(), 0, 'the folded phone panel shows its entries');
  await panel.getByRole('button', { name: 'Expand the sidebar' }).click();
  await panel.getByRole('link', { name: 'Storage', exact: true }).click();
  await page.waitForURL(origin + '/storage');
  // Choosing a destination folds the panel back to its one row.
  await panel.getByRole('button', { name: 'Storage', exact: true }).waitFor();
  assert.equal(await panel.getByRole('link').count(), 0);
  await page.close();
});

for (const path of ['/apps', '/storage', '/mesh', '/settings', '/settings/hardware', '/settings/about', '/settings/reset']) {
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

await check('About leads with the address this browser is using and names the .local address separately', async () => {
  /* The harness serves on 127.0.0.1:<port> and the stubbed box is called
   * mattbox, so the two differ — the libvirt case, where the owner reached
   * the box by IP and mattbox.local does not resolve on the host. The pane
   * must not send them to the name as if it were the one address. */
  const { page, errors } = await open({ path: '/settings/about', stored: true });
  const main = page.locator('main');
  await main.getByText(origin, { exact: true }).waitFor();
  await main.getByText('http://mattbox.local', { exact: true }).waitFor();
  const text = await main.innerText();
  assert.ok(text.indexOf(origin) < text.indexOf('http://mattbox.local'), 'the address in use comes before the name');
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
    for (const path of ['/', '/apps', '/storage', '/mesh', '/settings/network', '/settings/hardware', '/settings/security', '/settings/about', '/settings/reset']) {
      const { page, errors } = await open({ path, stored: true, locale, market: MARKET });
      await page.locator('main').waitFor();
      assert.deepEqual(errors, [], `${path} threw in ${locale}`);
      await page.close();
    }
  });
}

/* The greyed Market entry, under Mesh in the one sidebar. */
const settingsNav = (page, name = 'Sections') => page.getByRole('navigation', { name, exact: true });
const MARKET_ROW = {
  en: ['Sections', 'Market', 'Disk sharing'],
  sk: ['Sekcie', 'Trh', 'Zdieľanie disku'],
  de: ['Bereiche', 'Markt', 'Festplattenfreigabe'],
};

for (const [locale, [navName, label, sharing]] of Object.entries(MARKET_ROW)) {
  await whenMarketPlanned(`the Market entry is greyed out as soon(TM) under Mesh, disk sharing with it, and neither opens (${locale})`, async () => {
    const tag = { en: 'en-US', sk: 'sk-SK', de: 'de-DE' }[locale];
    // On /mesh the Mesh entry is unfolded, which shows Market under it.
    const { page, errors } = await open({ path: '/mesh', stored: true, locale: tag, market: MARKET });
    const row = settingsNav(page, navName).getByRole('button', { name: `${label} soon(TM)`, exact: true });
    await row.waitFor();
    assert.equal(await row.isDisabled(), true, 'the row is not disabled');
    assert.equal(await row.getAttribute('aria-disabled'), 'true');
    const share = settingsNav(page, navName).getByRole('button', { name: sharing, exact: true });
    await share.waitFor();
    assert.equal(await share.isDisabled(), true, 'disk sharing is not greyed with the market');
    // Under Market, not beside it: its row sits further right.
    const [m, d] = await Promise.all([row.boundingBox(), share.boundingBox()]);
    assert.ok(d.x > m.x && d.y > m.y, 'disk sharing is not nested under Market');
    // A disabled button is not in the Tab order: Tab from the search field
    // must land on the next open row, never on Market.
    const before = new URL(page.url()).pathname;
    await row.click({ force: true }).catch(() => {});
    await page.waitForTimeout(200);
    assert.equal(new URL(page.url()).pathname, before, 'clicking the greyed row navigated');
    assert.equal(
      await settingsNav(page, navName).locator('button[aria-current="page"]').filter({ hasText: label }).count(),
      0,
      'the Market row became current',
    );
    assert.deepEqual(errors, []);
    await page.close();
  });
}

await whenMarketPlanned('the greyed Market row is skipped by the keyboard', async () => {
  const { page } = await open({ path: '/mesh', stored: true });
  const search = settingsNav(page).getByRole('searchbox');
  await search.fill('m');
  // "m" matches Mesh, Market (keywords) and more; Enter must open the first
  // *open* match, and Tab from the field must never rest on the Market row.
  const labels = await settingsNav(page).getByRole('button').allInnerTexts();
  assert.ok(labels.some((l) => /Market/.test(l)), `Market is not among the matches: ${labels.join(', ')}`);
  await search.press('Enter');
  await page.waitForTimeout(200);
  assert.notEqual(new URL(page.url()).pathname, '/settings/market');
  // Walk the Tab order from the field until focus leaves the sidebar; every
  // stop is a row (or the clear button), and Market must not be among them.
  const seen = [];
  for (let i = 0; i < labels.length + 2; i++) {
    await page.keyboard.press('Tab');
    const stop = await page.evaluate(() => {
      const el = document.activeElement;
      return el?.closest('nav[aria-label="Sections"]') ? (el.textContent ?? '') : null;
    });
    if (stop === null) break;
    seen.push(stop);
  }
  assert.ok(seen.length >= 2, `Tab never walked the sidebar rows: ${seen.join(' | ')}`);
  assert.ok(!seen.some((t) => /Market/.test(t)), `Tab rested on the Market row: ${seen.join(' | ')}`);
  await page.close();
});

await whenMarketPlanned('a deep link to the planned Market pane lands on the first Settings pane and asks the market nothing', async () => {
  const marketCalls = [];
  const { page, errors } = await open({ path: '/settings/market', stored: true, market: MARKET });
  page.on('request', (request) => {
    if (/\/api\/market/.test(request.url())) marketCalls.push(request.url());
  });
  await page.locator('main').waitFor();
  await page.getByRole('heading', { name: 'Network', exact: true, level: 1 }).waitFor();
  assert.ok(!/Nothing here/.test(await body(page)), 'fell through to not-found');
  // The disk-sharing switch moved onto the Market pane, so a planned market
  // means no way to switch sharing on: Storage must not still carry it.
  assert.equal(
    await page.getByRole('switch', { name: 'Share this box’s disk with the mesh' }).count(),
    0,
    'the disk-sharing switch is still on the Storage pane',
  );
  await page.reload({ waitUntil: 'networkidle' });
  await page.getByRole('heading', { name: 'Network', exact: true, level: 1 }).waitFor();
  assert.deepEqual(marketCalls, [], 'the Market pane was mounted (it asked /api/market)');
  assert.deepEqual(errors, []);
  await page.close();
});

await whenMarketOpen('a box the market is not offered to says so quietly, and still offers the sharing switch', async () => {
  const { page, errors } = await open({ path: '/settings/market', stored: true });
  await page.getByText('The market is not available on this box').waitFor();
  await page.getByRole('switch', { name: 'Share this box’s disk with the mesh' }).waitFor();
  assert.deepEqual(errors, []);
  await page.close();
});

await whenMarketOpen('buying opens Stripe Checkout in a new tab', async () => {
  const { page, errors, marketPosts } = await open({ path: '/settings/market', stored: true, market: MARKET });
  const tab = page.context().waitForEvent('page');
  await page.getByRole('button', { name: 'Buy', exact: true }).click();
  const checkout = await tab;
  await checkout.waitForURL('https://checkout.stripe.com/c/pay/cs_test_1');
  assert.deepEqual(marketPosts, [['/api/market/orders', { listing_id: 'lst_ab12', quantity: 1 }]]);
  assert.deepEqual(errors, []);
  await page.close();
});

await whenMarketOpen('a payment page that is not Stripe is never opened', async () => {
  const { page, marketPosts } = await open({
    path: '/settings/market',
    stored: true,
    market: MARKET,
    checkoutUrl: 'https://checkout.stripe.com.evil.example/pay',
  });
  const tab = page.context().waitForEvent('page');
  await page.getByRole('button', { name: 'Buy', exact: true }).click();
  const placeholder = await tab;
  await page.getByText('no payment page came back').waitFor();
  await placeholder.waitForEvent('close', { timeout: 2000 }).catch(() => {});
  assert.ok(placeholder.isClosed(), 'the placeholder tab was left open');
  assert.equal(marketPosts.length, 1);
  await page.close();
});

await whenMarketOpen('setting up payouts opens Stripe onboarding in a tab with no opener', async () => {
  const unready = { ...MARKET, account: { ...MARKET.account, seller_onboarded: false, seller_ready: false } };
  const { page, errors, marketPosts } = await open({ path: '/settings/market', stored: true, market: unready });
  const tab = page.waitForEvent('popup');
  await page.getByRole('button', { name: 'Set up payouts' }).click();
  const onboarding = await tab;
  await onboarding.waitForURL('https://connect.stripe.com/setup/e/acct_test/abc');
  assert.equal(await onboarding.evaluate(() => window.opener), null);
  assert.deepEqual(marketPosts, [['/api/market/onboard', {}]]);
  assert.deepEqual(errors, []);
  await page.close();
});

await whenMarketOpen('a valid listing is sent in minor units', async () => {
  const { page, marketPosts } = await open({ path: '/settings/market', stored: true, market: MARKET });
  await page.getByLabel('Price per unit (EUR)').fill('1,25');
  await page.getByLabel('Capacity (units)').fill('2');
  const sent = page.waitForRequest((request) => request.url().endsWith('/api/market/listings'));
  await page.getByRole('button', { name: 'Offer for sale' }).click();
  await sent;
  assert.deepEqual(marketPosts, [['/api/market/listings', { kind: 'storage', unit_price: 125, capacity: 2 }]]);
  await page.close();
});

await whenMarketOpen('the listing form refuses a bad price before anything is sent', async () => {
  const { page, marketPosts } = await open({ path: '/settings/market', stored: true, market: MARKET });
  await page.getByPlaceholder('Price', { exact: false }).fill('0.005');
  await page.getByPlaceholder('Capacity', { exact: false }).fill('10');
  await page.getByRole('button', { name: 'Offer for sale' }).click();
  await page.getByText('Enter a price above zero, like 0.05.').waitFor();
  const price = page.getByPlaceholder('Price', { exact: false });
  assert.equal(await price.getAttribute('aria-invalid'), 'true');
  const described = await price.getAttribute('aria-describedby');
  assert.match(await page.locator(`[id="${described}"]`).innerText(), /like 0\.05/);
  assert.deepEqual(marketPosts, []);
  await page.close();
});

await whenMarketOpen('a zero-decimal currency is priced in whole units, not hundredths', async () => {
  const yen = { ...MARKET, account: { ...MARKET.account, currency: 'jpy' } };
  const { page, marketPosts } = await open({ path: '/settings/market', stored: true, market: yen });
  await page.getByLabel('Price per unit (JPY)').fill('1,5');
  await page.getByLabel('Capacity (units)').fill('2');
  await page.getByRole('button', { name: 'Offer for sale' }).click();
  await page.getByText('Enter a price above zero, like 5.').waitFor();
  assert.deepEqual(marketPosts, []);
  await page.getByLabel('Price per unit (JPY)').fill('500');
  const sent = page.waitForRequest((request) => request.url().endsWith('/api/market/listings'));
  await page.getByRole('button', { name: 'Offer for sale' }).click();
  await sent;
  assert.deepEqual(marketPosts, [['/api/market/listings', { kind: 'storage', unit_price: 500, capacity: 2 }]]);
  await page.close();
});

await whenMarketOpen('a blocked payouts pop-up says so next to the button', async () => {
  const unready = { ...MARKET, account: { ...MARKET.account, seller_onboarded: false, seller_ready: false } };
  const { page, errors, marketPosts } = await open({ path: '/settings/market', stored: true, market: unready });
  await page.evaluate(() => {
    window.open = () => null;
  });
  await page.getByRole('button', { name: 'Set up payouts' }).click();
  await page.getByRole('alert').filter({ hasText: 'Allow pop-ups for this page' }).waitFor();
  assert.deepEqual(marketPosts, []);
  assert.deepEqual(errors, []);
  await page.close();
});

await whenMarketOpen('a quantity out of range is tied to its field', async () => {
  const { page, marketPosts } = await open({ path: '/settings/market', stored: true, market: MARKET });
  const qty = page.getByLabel('Quantity');
  await qty.fill('41');
  await page.getByRole('button', { name: 'Buy', exact: true }).click();
  await page.getByText('Enter a whole number from 1 to 40.').waitFor();
  assert.equal(await qty.getAttribute('aria-invalid'), 'true');
  const described = await qty.getAttribute('aria-describedby');
  assert.match(await page.locator(`[id="${described}"]`).innerText(), /from 1 to 40/);
  assert.deepEqual(marketPosts, []);
  await page.close();
});

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
