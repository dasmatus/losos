/* The Access tokens pane (src/screens/settings/pane-access.tsx): tokens for
 * the `losos` terminal client, each tied to a GPG key the installer seeded
 * (docs/terminal-access.md, "The web UI's side").
 *
 * What is checked: the box's key and the seeded keys show their
 * fingerprints in groups of four, the tokens list their facts, a new token
 * goes out with the body the contract names and its secret is on screen
 * once, a refusal shows beside the form, revoking asks first and sends the
 * id, a box with no seeded key says so and offers no form, and none of it
 * adds to the Content-Security-Policy violations a plain load logs.
 *
 * Same arrangement as the other files: the real dist/ bundle under the real
 * policy, the three routes stubbed with page.route() as a small stateful
 * lososd that enforces the Bearer token. Data: tests/fixtures/access.json.
 */

import assert from './assert.mjs';
import { launch, runner, serve } from './harness.mjs';

const TOKEN = 'a'.repeat(64);
const FIXTURE = await Bun.file('tests/fixtures/access.json').json();
const NEW_SECRET = 'losos_pat_0a1b2c3d_' + '5e'.repeat(20);
const { origin, close: closeServer } = await serve({ csp: true });
const browser = await launch();
const { check, finish } = runner();

const SETTINGS = {
  sharingMyStorage: false,
  hostName: 'mattbox',
  gpuEnable: false,
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

/* A signed-in box. `access` is the view GET /api/access starts from; the
 * stub then keeps it, so a token made or revoked shows on the next read.
 * `refuse` answers POST /api/access/tokens with that status and sentence. */
async function open({ path = '/settings/access', access = FIXTURE, refuse = null, locale = 'en-US' } = {}) {
  const context = await browser.newContext({ viewport: { width: 1280, height: 900 }, locale });
  const page = await context.newPage();
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  const view = structuredClone(access);
  const posts = [];

  const authed = (route) => route.request().headers()['authorization'] === `Bearer ${TOKEN}`;
  const guard = (fn) => (route) => (authed(route) ? fn(route) : json(route, 401, { error: 'unauthorized' }));

  await page.route('**/api/**', (route) => json(route, 200, {}));
  await page.route('**/api/setup/claim', (route) => json(route, 200, { claimed: true }));
  await page.route('**/api/state', guard((route) => json(route, 200, { mode: 'local', sharing: false })));
  await page.route('**/api/settings', guard((route) => json(route, 200, SETTINGS)));
  await page.route('**/api/status', guard((route) => json(route, 200, { state: 'idle', progress: 0, message: '' })));
  await page.route('**/api/access', guard((route) => json(route, 200, view)));
  await page.route('**/api/access/tokens', guard((route) => {
    const body = JSON.parse(route.request().postData() ?? '{}');
    posts.push({ path: '/api/access/tokens', body });
    if (refuse !== null) return json(route, refuse.status, { error: refuse.error });
    const record = {
      id: '0a1b2c3d',
      name: body.name,
      fingerprint: body.fingerprint,
      scope: body.scope,
      created: 1792000000,
      expires: body.expiresInDays === null ? null : 1792000000 + body.expiresInDays * 86400,
      lastUsed: null,
    };
    view.tokens.push(record);
    return json(route, 200, { token: NEW_SECRET, record });
  }));
  await page.route('**/api/access/tokens/revoke', guard((route) => {
    const body = JSON.parse(route.request().postData() ?? '{}');
    posts.push({ path: '/api/access/tokens/revoke', body });
    const before = view.tokens.length;
    view.tokens = view.tokens.filter((t) => t.id !== body.id);
    return view.tokens.length < before
      ? json(route, 200, { revoked: true })
      : json(route, 404, { error: 'no such token' });
  }));

  await page.addInitScript((token) => {
    window.sessionStorage.setItem('losos-token', token);
    window.localStorage.setItem('losos-tour', 'seen');
    window.__violations = [];
    document.addEventListener('securitypolicyviolation', (e) =>
      window.__violations.push(`${e.violatedDirective} ${e.blockedURI}`),
    );
  }, TOKEN);
  await page.goto(origin + path, { waitUntil: 'networkidle' });
  return { page, context, errors, posts };
}

const violations = (page) => page.evaluate(() => [...new Set(window.__violations)].sort());
const nav = (page) => page.getByRole('navigation', { name: 'Sections', exact: true });

/* What a load logs without this pane: the baseline the pane must not add to
 * (Sonner's refused stylesheet is logged at every load). */
async function baseline() {
  const { page, context } = await open({ path: '/settings/security' });
  await page.waitForTimeout(300);
  const seen = await violations(page);
  await context.close();
  return seen;
}
const BASELINE = await baseline();

console.log('admin-ui access token checks');

await check('the sidebar lists Access tokens right after Security, and search finds it', async () => {
  const { page, context } = await open({ path: '/settings/security' });
  const labels = await nav(page).getByRole('link').allInnerTexts();
  const at = labels.findIndex((l) => l.trim() === 'Security');
  assert.equal(labels[at + 1]?.trim(), 'Access tokens', `after Security: ${labels[at + 1]}`);
  // The sidebar's search box opens the command palette (src/palette/).
  await nav(page).getByRole('button', { name: /^Search or jump to/ }).click();
  const box = page.getByRole('combobox', { name: /^Search pages/ });
  for (const word of ['gpg', 'terminal', 'token']) {
    await box.fill(word);
    await page.getByRole('option', { name: /^Access tokens/ }).waitFor({ timeout: 2000 });
  }
  await page.getByRole('option', { name: /^Access tokens/ }).click();
  await page.waitForURL(origin + '/settings/access');
  await page.getByRole('heading', { name: 'Access tokens', level: 1 }).waitFor();
  await context.close();
});

await check('the box key, the seeded keys and the tokens render', async () => {
  const { page, context, errors } = await open();
  await page.getByTestId('access-host-key').waitFor();
  assert.equal(
    await page.getByTestId('access-host-fingerprint').innerText(),
    '3F9A 1C27 B04E 58D6 A2C1 F0E9 D8B7 A6C5 E4D3 F2A1',
  );
  const keys = page.getByTestId('access-key');
  assert.equal(await keys.count(), 2);
  const first = await keys.nth(0).innerText();
  assert.ok(first.includes('Matus <matus@example.org>'), first);
  assert.ok(first.includes('8E2D 4B6A 1C3F 5E7D 9B0A 2C4E 6F8A 1B3C 5D7E 9F02'), first);
  const second = await keys.nth(1).innerText();
  assert.ok(second.includes('Matus (work laptop) <matus@work.example>') && second.includes('Matus <m@example.net>'), second);
  assert.ok((await page.getByTestId('access-keys').innerText()).includes('nothing on this page can change them'));

  const tokens = page.getByTestId('access-token');
  assert.equal(await tokens.count(), 2);
  const laptop = await tokens.nth(0).innerText();
  for (const part of ['laptop', 'Full access', '6F8A1B3C5D7E9F02', 'Revoke']) assert.ok(laptop.includes(part), `${part} missing:\n${laptop}`);
  assert.ok(!laptop.includes('Never'), laptop);
  const script = await tokens.nth(1).innerText();
  for (const part of ['status script', 'Read only', 'E8B0D2F4A6C8E0B2D4F6'.slice(-16), 'Never', 'Never used']) {
    assert.ok(script.includes(part), `${part} missing:\n${script}`);
  }
  // The form offers both seeded keys, Read only and 90 days by default.
  const form = page.getByTestId('access-form');
  assert.equal(await form.getByLabel('Key').locator('option').count(), 2);
  assert.equal(await form.getByLabel('Access').inputValue(), 'read');
  assert.equal(await form.getByLabel('Expires').inputValue(), '90');
  assert.deepEqual(errors, []);
  await context.close();
});

await check('a new token goes out as the contract says and is shown once', async () => {
  const { page, context, errors, posts } = await open();
  const form = page.getByTestId('access-form');
  const create = form.getByRole('button', { name: 'Create token' });
  assert.equal(await create.isDisabled(), true, 'Create is enabled with no name');
  await form.getByLabel('Name').fill('  desktop  ');
  await form.getByLabel('Key').selectOption(FIXTURE.keys[1].fingerprint);
  await form.getByLabel('Access').selectOption('full');
  await form.getByLabel('Expires').selectOption('never');
  await create.click();
  const callout = page.getByTestId('access-created');
  await callout.waitFor({ timeout: 3000 });
  assert.deepEqual(posts, [{
    path: '/api/access/tokens',
    body: { name: 'desktop', fingerprint: FIXTURE.keys[1].fingerprint, scope: 'full', expiresInDays: null },
  }]);
  assert.equal(await page.getByTestId('access-created-token').innerText(), NEW_SECRET);
  assert.equal(await page.getByTestId('access-created-command').innerText(), `losos login ${origin}`);
  assert.ok((await callout.innerText()).includes('will not show it again'));
  await callout.getByRole('button', { name: 'Copy' }).waitFor();
  // The list is read again and holds the new one; the name field is clear.
  await page.getByTestId('access-token').nth(2).waitFor();
  assert.ok((await page.getByTestId('access-token').nth(2).innerText()).includes('desktop'));
  assert.equal(await form.getByLabel('Name').inputValue(), '');
  // Done takes the secret off the screen, and leaving and coming back does not bring it back.
  await callout.getByRole('button', { name: 'Done' }).click();
  await callout.waitFor({ state: 'detached' });
  await nav(page).getByRole('link', { name: 'About', exact: true }).click();
  await page.waitForURL(origin + '/settings/about');
  await nav(page).getByRole('link', { name: 'Access tokens', exact: true }).click();
  await page.getByTestId('access-tokens').waitFor();
  assert.equal(await page.getByText(NEW_SECRET).count(), 0, 'the secret came back');
  assert.equal(await page.evaluate((s) => JSON.stringify({ ...localStorage, ...sessionStorage }).includes(s), NEW_SECRET), false, 'the secret is in storage');
  assert.deepEqual(await violations(page), BASELINE, 'the pane added CSP violations');
  assert.deepEqual(errors, []);
  await context.close();
});

await check('a refusal from the box shows beside the form', async () => {
  const { page, context } = await open({ refuse: { status: 409, error: 'this box already holds 32 tokens' } });
  const form = page.getByTestId('access-form');
  await form.getByLabel('Name').fill('one more');
  await form.getByRole('button', { name: 'Create token' }).click();
  const error = page.getByTestId('access-error');
  await error.waitFor({ timeout: 3000 });
  assert.equal(await error.innerText(), 'this box already holds 32 tokens');
  assert.equal(await page.getByTestId('access-created').count(), 0);
  await context.close();
});

await check('revoking asks first, and sends the token id', async () => {
  const { page, context, errors, posts } = await open();
  const laptop = page.getByTestId('access-token').filter({ hasText: 'laptop' });
  await laptop.getByRole('button', { name: 'Revoke' }).click();
  const dialog = page.getByRole('dialog', { name: 'Revoke laptop?' });
  await dialog.waitFor();
  await dialog.getByRole('button', { name: 'Keep it' }).click();
  await dialog.waitFor({ state: 'hidden' });
  assert.deepEqual(posts, [], 'Keep it sent a request');
  await laptop.getByRole('button', { name: 'Revoke' }).click();
  await dialog.waitFor();
  await dialog.getByRole('button', { name: 'Revoke', exact: true }).click();
  await laptop.waitFor({ state: 'detached', timeout: 3000 });
  assert.deepEqual(posts, [{ path: '/api/access/tokens/revoke', body: { id: '4f1c9a2e' } }]);
  assert.equal(await page.getByTestId('access-token').count(), 1);
  assert.deepEqual(await violations(page), BASELINE, 'the dialog added CSP violations');
  assert.deepEqual(errors, []);
  await context.close();
});

await check('a box with no seeded key says so, links the handbook and offers no form', async () => {
  const { page, context, errors } = await open({ access: { ...FIXTURE, keys: [], tokens: [] } });
  const empty = page.getByTestId('access-keys-empty');
  await empty.waitFor();
  assert.ok((await empty.innerText()).includes('No key was seeded at install'));
  assert.equal(await empty.getByRole('link').getAttribute('href'), '/handbook/manual/terminal/');
  assert.ok((await page.getByTestId('access-tokens').innerText()).includes('No tokens yet.'));
  const form = page.getByTestId('access-form');
  assert.equal(await form.getByLabel('Name').isDisabled(), true);
  assert.equal(await form.getByRole('button', { name: 'Create token' }).isDisabled(), true);
  assert.ok((await form.innerText()).includes('Needs a seeded key.'));
  assert.deepEqual(errors, []);
  await context.close();
});

for (const [locale, title] of [['sk-SK', 'Prístupové tokeny'], ['de-DE', 'Zugriffstokens']]) {
  await check(`the pane renders in ${locale} without errors`, async () => {
    const { page, context, errors } = await open({ locale });
    await page.getByRole('heading', { name: title, level: 1 }).waitFor();
    assert.equal(await page.getByTestId('access-token').count(), 2);
    assert.deepEqual(errors, []);
    await context.close();
  });
}

await browser.close();
await closeServer();
finish();
