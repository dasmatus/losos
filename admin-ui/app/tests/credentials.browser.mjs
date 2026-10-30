/* The Credentials pane (src/screens/settings/pane-credentials.tsx): every
 * secret the box keeps in its credential store (modules/credentials.nix),
 * listed without its value.
 *
 * What is checked: the pane sits right after Access tokens and search finds
 * it, the seal says TPM or box key, the groups come in their order with an
 * unknown app last and an unknown name on lososd's own sentence, the badges
 * and dates render, only a rotatable row offers Make new and the owner's
 * bucket keys link the Backup pane, a new value asks first and sends no body,
 * a refusal is a toast, a box without the routes says so, the page holds no
 * field a value could be typed into, and none of it adds to the
 * Content-Security-Policy violations a plain load logs.
 *
 * Same arrangement as the other files: the real dist/ bundle under the real
 * policy, the two routes stubbed with page.route() as a small lososd that
 * enforces the Bearer token. Data: tests/fixtures/credentials.json.
 */

import assert from './assert.mjs';
import { launch, runner, serve } from './harness.mjs';

const TOKEN = 'a'.repeat(64);
const FIXTURE = await Bun.file('tests/fixtures/credentials.json').json();
const { origin, close: closeServer } = await serve({ csp: true });
const browser = await launch();
const { check, finish } = runner();

const SETTINGS = {
  sharingMyStorage: false,
  forgejoMode: 'container',
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

/* A signed-in box. `view` is what GET /api/credentials answers, or a status
 * number to answer with instead; a rotation stamps the row's date so the
 * next read shows it. `refuse` answers every rotation with 409. */
async function open({ path = '/settings/credentials', view = FIXTURE, refuse = false, locale = 'en-US' } = {}) {
  const context = await browser.newContext({ viewport: { width: 1280, height: 900 }, locale });
  const page = await context.newPage();
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  const state = typeof view === 'number' ? view : structuredClone(view);
  const posts = [];

  const authed = (route) => route.request().headers()['authorization'] === `Bearer ${TOKEN}`;
  const guard = (fn) => (route) => (authed(route) ? fn(route) : json(route, 401, { error: 'unauthorized' }));

  await page.route('**/api/**', (route) => json(route, 200, {}));
  await page.route('**/api/setup/claim', (route) => json(route, 200, { claimed: true }));
  await page.route('**/api/state', guard((route) => json(route, 200, { mode: 'local', sharing: false })));
  await page.route('**/api/settings', guard((route) => json(route, 200, SETTINGS)));
  await page.route('**/api/status', guard((route) => json(route, 200, { state: 'idle', progress: 0, message: '' })));
  await page.route('**/api/credentials', guard((route) =>
    typeof state === 'number' ? json(route, state, { error: 'not found' }) : json(route, 200, state)));
  await page.route('**/api/credentials/*/rotate', guard((route) => {
    const request = route.request();
    const name = decodeURIComponent(new URL(request.url()).pathname.split('/')[3]);
    posts.push({ name, method: request.method(), body: request.postData() ?? '' });
    if (refuse) return json(route, 409, { error: `${name} is not made by the box` });
    const row = state.credentials.find((c) => c.name === name);
    row.updated = 1792500000;
    row.remade = false;
    return json(route, 200, { credential: row });
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
const row = (page, name) => page.locator(`[data-testid="credential"][data-name="${name}"]`);

async function baseline() {
  const { page, context } = await open({ path: '/settings/security' });
  await page.waitForTimeout(300);
  const seen = await violations(page);
  await context.close();
  return seen;
}
const BASELINE = await baseline();

console.log('admin-ui credentials checks');

await check('the sidebar lists Credentials right after Access tokens, and search finds it', async () => {
  const { page, context } = await open({ path: '/settings/security' });
  const labels = await nav(page).getByRole('link').allInnerTexts();
  const at = labels.findIndex((l) => l.trim() === 'Access tokens');
  assert.equal(labels[at + 1]?.trim(), 'Credentials', `after Access tokens: ${labels[at + 1]}`);
  await nav(page).getByRole('button', { name: /^Search or jump to/ }).click();
  const box = page.getByRole('combobox', { name: /^Search pages/ });
  for (const word of ['tpm', 'secret', 'systemd-creds']) {
    await box.fill(word);
    await page.getByRole('option', { name: /^Credentials/ }).waitFor({ timeout: 2000 });
  }
  await page.getByRole('option', { name: /^Credentials/ }).click();
  await page.waitForURL(origin + '/settings/credentials');
  await page.getByRole('heading', { name: 'Credentials', level: 1 }).waitFor();
  await context.close();
});

await check('the seal, the groups in order and every row render', async () => {
  const { page, context, errors } = await open();
  const seal = page.getByTestId('credentials-seal');
  await seal.waitFor();
  const sealText = await seal.innerText();
  assert.ok(sealText.includes("Sealed to this box's TPM"), sealText);
  assert.ok(sealText.includes('7 secrets'), sealText);
  assert.ok(sealText.includes('TPM'), sealText);
  assert.equal(await seal.getByRole('link', { name: 'How it works' }).getAttribute('href'), '/handbook/reference/credentials/');

  const apps = await page.getByTestId('credentials-app').evaluateAll((els) => els.map((e) => e.dataset.app));
  assert.deepEqual(apps, ['admin', 'login', 'mail', 'backup', 'market', 'weather']);
  const titles = await page.getByTestId('credentials-app').evaluateAll((els) =>
    els.map((e) => e.querySelector('h2, h3')?.textContent?.trim()));
  assert.deepEqual(titles, ['This box', 'LosOS Login', 'LosOS Mail', 'Backups', 'Market', 'weather']);

  const spare = await row(page, 'admin-token').innerText();
  for (const part of ['Spare key', 'Made by the box', 'Moved from a plain file', 'Read by lososd.service']) {
    assert.ok(spare.includes(part), `${part} missing:\n${spare}`);
  }
  assert.ok(!spare.includes('Made again at boot'), spare);
  const session = await row(page, 'authelia-session').innerText();
  for (const part of ['Sessions', 'Made again at boot', 'Changed']) assert.ok(session.includes(part), `${part} missing:\n${session}`);
  assert.ok((await row(page, 'mail-master').innerText()).includes('Read by lososd.service, losos-mail.service'));
  const bucket = await row(page, 'backup-bucket').innerText();
  assert.ok(bucket.includes('Bucket keys') && bucket.includes('Not set'), bucket);
  assert.ok(!bucket.includes('Changed'), bucket);
  const escrow = await row(page, 'market-escrow-key').innerText();
  for (const part of ['Escrow key', 'Made by the box', 'Read by lososd.service']) assert.ok(escrow.includes(part), `${part} missing:\n${escrow}`);
  // A name this build has no words for reads lososd's sentence.
  assert.ok((await row(page, 'weather-feed').innerText()).includes('The key of the weather service the Overview tile reads.'));

  // Make new on the two rotatable rows only; the bucket keys link the Backup pane.
  const makers = await page.getByRole('button', { name: 'Make new' }).count();
  assert.equal(makers, 2);
  for (const name of ['authelia-session', 'weather-feed']) {
    assert.equal(await row(page, name).getByRole('button', { name: 'Make new' }).count(), 1, name);
  }
  for (const name of ['admin-token', 'authelia-storage', 'mail-master', 'backup-bucket', 'market-escrow-key']) {
    assert.equal(await row(page, name).getByRole('button').count(), 0, name);
  }
  assert.equal(await row(page, 'backup-bucket').getByRole('link', { name: 'Set up' }).getAttribute('href'), '/settings/backup');

  // Nothing on the pane takes a value.
  assert.equal(await page.locator('main input, main textarea, main [contenteditable="true"]').count(), 0);
  assert.deepEqual(await violations(page), BASELINE, 'the pane added CSP violations');
  assert.deepEqual(errors, []);
  await context.close();
});

await check('a box without a TPM says its own key seals them', async () => {
  const view = { ...FIXTURE, key: 'host', credentials: FIXTURE.credentials.map((c) => ({ ...c, sealedWith: 'host' })) };
  const { page, context, errors } = await open({ view });
  const seal = page.getByTestId('credentials-seal');
  await seal.waitFor();
  const text = await seal.innerText();
  assert.ok(text.includes("Sealed with this box's own key") && text.includes('Box key'), text);
  assert.ok(text.includes('no TPM'), text);
  assert.deepEqual(errors, []);
  await context.close();
});

await check('a new value asks first and sends the name alone', async () => {
  const { page, context, errors, posts } = await open();
  const session = row(page, 'authelia-session');
  await session.getByRole('button', { name: 'Make new' }).click();
  const dialog = page.getByRole('dialog', { name: 'Make a new value for Sessions?' });
  await dialog.waitFor();
  assert.ok((await dialog.innerText()).includes('Everyone is signed out, you too'));
  await dialog.getByRole('button', { name: 'Keep it' }).click();
  await dialog.waitFor({ state: 'hidden' });
  assert.deepEqual(posts, [], 'Keep it sent a request');

  await session.getByRole('button', { name: 'Make new' }).click();
  await dialog.waitFor();
  await dialog.getByRole('button', { name: 'Make a new one' }).click();
  await page.getByText('Made a new value for Sessions.', { exact: true }).waitFor({ timeout: 3000 });
  assert.deepEqual(posts, [{ name: 'authelia-session', method: 'POST', body: '' }]);
  // The list is read again: the boot mark is gone.
  await session.getByText('Made again at boot').waitFor({ state: 'detached', timeout: 3000 });
  assert.deepEqual(await violations(page), BASELINE, 'the dialog added CSP violations');
  assert.deepEqual(errors, []);
  await context.close();
});

await check('a refusal from the box is a toast and changes nothing', async () => {
  const { page, context, posts } = await open({ refuse: true });
  await row(page, 'weather-feed').getByRole('button', { name: 'Make new' }).click();
  await page.getByRole('dialog').getByRole('button', { name: 'Make a new one' }).click();
  await page.getByText('The new value could not be made.', { exact: true }).waitFor({ timeout: 3000 });
  assert.equal(posts.length, 1);
  assert.equal(await page.getByRole('button', { name: 'Make new' }).count(), 2);
  await context.close();
});

await check('a box without the routes says so plainly', async () => {
  const { page, context, errors } = await open({ view: 404 });
  const empty = page.getByTestId('credentials-unavailable');
  await empty.waitFor();
  assert.ok((await empty.innerText()).includes('does not keep its credentials in a credential store yet'));
  assert.equal(await page.getByTestId('credentials-app').count(), 0);
  assert.deepEqual(errors, []);
  await context.close();
});

for (const [locale, title, seal] of [
  ['sk-SK', 'Prihlasovacie údaje', 'Zapečatené čipom TPM tohto zariadenia'],
  ['de-DE', 'Zugangsdaten', 'Mit dem TPM dieser Box versiegelt'],
]) {
  await check(`the pane renders in ${locale} without errors`, async () => {
    const { page, context, errors } = await open({ locale });
    await page.getByRole('heading', { name: title, level: 1 }).waitFor();
    assert.ok((await page.getByTestId('credentials-seal').innerText()).includes(seal));
    assert.equal(await page.getByTestId('credential').count(), 7);
    assert.deepEqual(errors, []);
    await context.close();
  });
}

await browser.close();
await closeServer();
finish();
