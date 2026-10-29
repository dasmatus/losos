/* Your account (src/screens/settings/pane-account.tsx): a person's own name,
 * email address, password and second factors on LosOS Login.
 *
 * What is checked: the pane is in the sidebar for a person with the Own
 * account section and never for the spare key; the details and a new
 * password each go out as their own request, only once they pass; the
 * one-time code comes back behind the password, and a wrong password or no
 * code yet says so; LosOS Login's own page is one link away; someone a rule
 * holds sees this pane at every address and nothing else in the sidebar,
 * and the rest opens once the box lets them go; Slovak names the pane; and
 * none of it adds to the Content-Security-Policy violations a plain load
 * logs.
 *
 * Same arrangement as the other files: the real dist/ bundle under the real
 * policy, the routes stubbed with page.route() as a small stateful lososd. */

import assert from './assert.mjs';
import { launch, runner, serve } from './harness.mjs';

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
const GOOD_PASSWORD = 'Salmon-river-42';
const SET_AT = Date.UTC(2026, 8, 1) / 1000;

const json = (route, status, body) =>
  route.fulfill({ status, contentType: 'application/json', body: JSON.stringify(body) });

/* Eva, who may change her own account and her backups, signed in through
 * LosOS Login. `locked` is what a rule holds her for; `spareKey` signs in
 * with the spare key instead. */
async function open({ path = '/settings/account', locked = [], spareKey = false, locale = 'en-US' } = {}) {
  const context = await browser.newContext({ viewport: { width: 1280, height: 900 }, locale });
  const page = await context.newPage();
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  const box = {
    locked: [...locked],
    displayName: 'Eva Novak',
    email: 'eva@mattbox.local',
    app: false,
    code: null,
  };
  const posts = [];
  const body = (route) => JSON.parse(route.request().postData() ?? '{}');
  const sections = () => (box.locked.length > 0 ? ['account'] : ['backup', 'account']);

  await page.route('**/api/**', (route) => json(route, 200, {}));
  await page.route('**/api/setup/claim', (route) => json(route, 200, { claimed: true }));
  await page.route('**/api/me', (route) =>
    json(
      route,
      200,
      spareKey
        ? { admin: true, spareKey: true, sections: [], standalone: true }
        : {
            name: 'eva',
            displayName: box.displayName,
            groups: ['losos-backup', 'losos-account'],
            admin: false,
            spareKey: false,
            sections: sections(),
            standalone: true,
            locked: box.locked,
          },
    ),
  );
  await page.route('**/api/state', (route) => json(route, 200, { mode: 'local', sharing: false }));
  await page.route('**/api/settings', (route) => json(route, 200, SETTINGS));
  await page.route('**/api/status', (route) => json(route, 200, { state: 'idle', progress: 0, message: '' }));
  await page.route('**/api/account', (route) => {
    if (spareKey) return json(route, 409, { error: 'the spare key has no account of its own' });
    if (route.request().method() === 'GET') {
      return json(route, 200, {
        name: 'eva',
        displayName: box.displayName,
        email: box.email,
        factors: { app: box.app, passkeys: [] },
        passwordSetAt: SET_AT,
        passwordExpiresAt: null,
        rules: { twoFactor: true, passwordDays: 0 },
        locked: box.locked,
      });
    }
    const b = body(route);
    posts.push({ path: '/api/account', body: b });
    box.displayName = b.displayName;
    box.email = b.email;
    return json(route, 200, { name: 'eva', ...b });
  });
  await page.route('**/api/profiles/eva', (route) =>
    json(route, 200, {
      name: 'eva',
      displayName: box.displayName,
      pronouns: '',
      about: '',
      accent: 'violet',
      accentChosen: false,
      picture: null,
      banner: null,
    }),
  );
  await page.route('**/api/account/code', (route) => {
    const b = body(route);
    posts.push({ path: '/api/account/code', body: b });
    if (b.password !== GOOD_PASSWORD) return json(route, 403, { error: 'that is not your password', wrongPassword: true });
    if (box.code === null) return json(route, 404, { error: 'no code yet', noCode: true });
    return json(route, 200, { code: box.code });
  });
  await page.route('**/api/set-password', (route) => {
    const b = body(route);
    posts.push({ path: '/api/set-password', body: b });
    box.locked = box.locked.filter((r) => r !== 'password');
    return json(route, 200, { name: 'eva', changed: true, cloud: null });
  });

  await page.addInitScript(() => {
    window.localStorage.setItem('losos-tour', 'seen');
    window.__violations = [];
    document.addEventListener('securitypolicyviolation', (e) =>
      window.__violations.push(`${e.violatedDirective} ${e.blockedURI}`),
    );
  });
  await page.goto(origin + path, { waitUntil: 'networkidle' });
  return { page, context, errors, posts, box };
}

const violations = (page) => page.evaluate(() => [...new Set(window.__violations)].sort());
const nav = (page) => page.getByRole('navigation', { name: 'Sections', exact: true });
const navLabels = async (page) => (await nav(page).getByRole('link').allInnerTexts()).map((l) => l.trim());

async function baseline() {
  const { page, context } = await open({ path: '/settings/backup' });
  await page.waitForTimeout(300);
  const seen = await violations(page);
  await context.close();
  return seen;
}
const BASELINE = await baseline();

console.log('admin-ui account pane checks');

await check('a person with their own account finds it in the sidebar and the search', async () => {
  const { page, context } = await open({ path: '/settings/backup' });
  const labels = await navLabels(page);
  assert.ok(labels.includes('Your account'), labels.join(' | '));
  assert.ok(!labels.includes('Users'), labels.join(' | '));
  await nav(page).getByRole('button', { name: /^Search or jump to/ }).click();
  const search = page.getByRole('combobox', { name: /^Search pages/ });
  for (const word of ['passkey', '2fa', 'profile']) {
    await search.fill(word);
    await page.getByRole('option', { name: /^Your account/ }).waitFor({ timeout: 2000 });
  }
  await page.getByRole('option', { name: /^Your account/ }).click();
  await page.waitForURL(origin + '/settings/account');
  await page.getByRole('heading', { name: 'Your account', level: 1 }).waitFor();
  await context.close();
});

await check('the spare key has no account of its own', async () => {
  const { page, context } = await open({ spareKey: true });
  assert.ok(!(await navLabels(page)).includes('Your account'));
  await page.getByTestId('account-none').waitFor();
  await context.close();
});

await check('the details go out once they changed, and the password once it passes', async () => {
  const { page, context, posts, errors } = await open();
  await page.getByTestId('account-details').waitFor();
  assert.equal(await page.getByTestId('account-sign-in-name').innerText(), 'eva');
  const save = page.getByTestId('account-details-save');
  assert.ok(await save.isDisabled(), 'Save is on with nothing changed');
  await page.getByTestId('account-name').fill('Eva Nováková');
  await page.getByTestId('account-email').fill('eva@example.org');
  await save.click();
  await page.waitForFunction(() => document.querySelector('[data-testid="account-details-save"]')?.disabled === true);
  assert.deepEqual(posts.at(-1), {
    path: '/api/account',
    body: { displayName: 'Eva Nováková', email: 'eva@example.org' },
  });
  await page.waitForFunction(() => document.querySelector('[data-testid="signed-in-as"]')?.textContent?.includes('Eva Nováková'));

  const change = page.getByTestId('account-password-save');
  await page.getByTestId('account-new-password').fill('longenoughbutplain');
  assert.ok(await change.isDisabled(), 'Change is on with a password that breaks the rules');
  await page.getByTestId('account-new-password').fill(GOOD_PASSWORD);
  await change.click();
  await page.waitForFunction(() => document.querySelector('[data-testid="account-new-password"]')?.value === '');
  assert.deepEqual(posts.at(-1), { path: '/api/set-password', body: { password: GOOD_PASSWORD } });
  assert.equal(await page.getByTestId('account-password-expires').innerText(), 'Never');
  assert.deepEqual(errors, []);
  await context.close();
});

await check('the one-time code comes back behind the password', async () => {
  const { page, context, box } = await open();
  const row = page.getByTestId('account-code');
  await row.waitFor();
  const link = page.getByTestId('account-open-login');
  assert.equal(await link.getAttribute('href'), `https://${new URL(origin).host}/auth/settings/two-factor-authentication`);
  assert.equal(await link.getAttribute('target'), '_blank');
  assert.equal(await page.getByTestId('account-app').innerText(), 'Not added');

  await page.getByTestId('account-code-password').fill('not-it');
  await page.getByTestId('account-code-show').click();
  await page.getByTestId('account-code-error').waitFor();
  assert.equal(await page.getByTestId('account-code-error').innerText(), 'That is not your password.');

  await page.getByTestId('account-code-password').fill(GOOD_PASSWORD);
  await page.getByTestId('account-code-show').click();
  await page.waitForFunction(() =>
    document.querySelector('[data-testid="account-code-error"]')?.textContent?.startsWith('LosOS Login has not made a code'),
  );

  box.code = 'K7QX2M9P';
  await page.getByTestId('account-code-password').fill(GOOD_PASSWORD);
  await page.getByTestId('account-code-show').click();
  await page.getByTestId('account-code-value').waitFor();
  assert.equal(await page.getByTestId('account-code-value').innerText(), 'K7QX2M9P');
  assert.equal(await page.getByTestId('account-code-password').inputValue(), '', 'the password stayed in the field');
  await context.close();
});

await check('someone a rule holds sees only their account, until they comply', async () => {
  const { page, context, box } = await open({ path: '/storage', locked: ['twoFactor'] });
  await page.getByTestId('account-held').waitFor();
  assert.match(await page.getByTestId('account-held').innerText(), /Add an authenticator app or a passkey/);
  await page.getByRole('heading', { name: 'Your account', level: 1 }).waitFor();
  assert.deepEqual(await navLabels(page), ['Your account']);
  await page.goto(origin + '/', { waitUntil: 'networkidle' });
  await page.getByTestId('account-held').waitFor();

  // They add an authenticator app on LosOS Login's page, in another tab.
  box.app = true;
  box.locked = [];
  await page.evaluate(() => window.dispatchEvent(new Event('focus')));
  await page.getByTestId('account-held').waitFor({ state: 'detached' });
  await nav(page).getByRole('link', { name: 'Overview', exact: true }).waitFor();
  await page.goto(origin + '/settings/backup', { waitUntil: 'networkidle' });
  await page.getByRole('heading', { name: 'Backup', level: 1 }).waitFor();
  assert.ok((await navLabels(page)).includes('Backup'));
  await context.close();
});

await check('a password that ran out is changed from the notice', async () => {
  const { page, context, posts } = await open({ path: '/', locked: ['password'] });
  await page.getByTestId('account-held').waitFor();
  assert.match(await page.getByTestId('account-held').innerText(), /Your password has run out/);
  await page.getByTestId('account-new-password').fill(GOOD_PASSWORD);
  await page.getByTestId('account-password-save').click();
  await page.getByTestId('account-held').waitFor({ state: 'detached' });
  assert.deepEqual(posts.at(-1), { path: '/api/set-password', body: { password: GOOD_PASSWORD } });
  await context.close();
});

await check('the pane speaks Slovak', async () => {
  const { page, context } = await open({ locale: 'sk-SK' });
  await page.getByRole('heading', { name: 'Váš účet', level: 1 }).waitFor();
  assert.match(await page.getByTestId('account-factors').innerText(), /Dvojstupňové prihlásenie/);
  await context.close();
});

await check('the pane adds no Content-Security-Policy violation', async () => {
  const { page, context } = await open();
  await page.getByTestId('account-details').waitFor();
  await page.waitForTimeout(300);
  assert.deepEqual(await violations(page), BASELINE);
  await context.close();
});

await browser.close();
await closeServer();
finish();
