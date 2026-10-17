/* The Users pane (src/screens/settings/pane-users.tsx): the people who may
 * sign in to the box's own LosOS Login, and what each of them can change.
 *
 * What is checked: the pane is in the sidebar and the search only on a box
 * that signs people in itself, between Security and Access tokens; the list
 * marks the owner and the caller and gives neither a button; a new person
 * goes out with the body backend/schema.json names, and only once the name
 * and password pass the box's rules; a refusal shows beside the form;
 * access, a new password and a removal each send their own request, the
 * removal only after asking; Slovak names the pane; and none of it adds to
 * the Content-Security-Policy violations a plain load logs.
 *
 * Same arrangement as the other files: the real dist/ bundle under the real
 * policy, the routes stubbed with page.route() as a small stateful lososd.
 * The caller signed in through LosOS Login, so there is no Bearer token. */

import assert from 'node:assert';
import { launch, runner, serve } from './harness.mjs';

const { origin, close: closeServer } = await serve({ csp: true });
const browser = await launch();
const { check, finish } = runner();

const SECTIONS = ['storage', 'backup', 'apps', 'market', 'network', 'look', 'lab', 'access'];
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
const PEOPLE = [
  { name: 'eva', displayName: 'Eva Novak', email: 'eva@mattbox.local', groups: ['losos-storage', 'losos-backup'], disabled: false },
  { name: 'jan', displayName: 'Jan', email: 'jan@mattbox.local', groups: ['losos-admins'], disabled: false },
  { name: 'matus', displayName: 'Matus', email: 'matus@mattbox.local', groups: ['losos-admins', 'losos-owner'], disabled: false },
  { name: 'oma', displayName: 'Oma', email: 'oma@mattbox.local', groups: ['losos-viewers'], disabled: false },
];
const GOOD_PASSWORD = 'Salmon-river-42';

const json = (route, status, body) =>
  route.fulfill({ status, contentType: 'application/json', body: JSON.stringify(body) });

/* A box with `you` signed in as an administrator. `standalone` is what
 * GET /api/me and GET /api/people say; `refuse` answers POST /api/people
 * with that status and sentence. */
async function open({ path = '/settings/users', standalone = true, refuse = null, locale = 'en-US', you = 'jan' } = {}) {
  const context = await browser.newContext({ viewport: { width: 1280, height: 900 }, locale });
  const page = await context.newPage();
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  let people = structuredClone(PEOPLE);
  const posts = [];
  const body = (route) => JSON.parse(route.request().postData() ?? '{}');

  await page.route('**/api/**', (route) => json(route, 200, {}));
  await page.route('**/api/setup/claim', (route) => json(route, 200, { claimed: true }));
  await page.route('**/api/me', (route) =>
    json(route, 200, {
      name: you,
      displayName: you,
      groups: ['losos-admins'],
      admin: true,
      spareKey: false,
      sections: SECTIONS,
      standalone,
    }),
  );
  await page.route('**/api/state', (route) => json(route, 200, { mode: 'local', sharing: false }));
  await page.route('**/api/settings', (route) => json(route, 200, SETTINGS));
  await page.route('**/api/status', (route) => json(route, 200, { state: 'idle', progress: 0, message: '' }));
  await page.route('**/api/people', (route) => {
    if (route.request().method() === 'GET') {
      return json(route, 200, { standalone, you, sections: SECTIONS, people: standalone ? people : [] });
    }
    const b = body(route);
    posts.push({ path: '/api/people', body: b });
    if (refuse !== null) return json(route, refuse.status, { error: refuse.error });
    people.push({ name: b.name, displayName: b.displayName || b.name, email: `${b.name}@mattbox.local`, groups: b.groups, disabled: false });
    people.sort((a, z) => a.name.localeCompare(z.name));
    return json(route, 200, { name: b.name });
  });
  await page.route('**/api/people/access', (route) => {
    const b = body(route);
    posts.push({ path: '/api/people/access', body: b });
    people = people.map((p) => (p.name === b.name ? { ...p, groups: b.groups } : p));
    return json(route, 200, b);
  });
  await page.route('**/api/people/password', (route) => {
    const b = body(route);
    posts.push({ path: '/api/people/password', body: b });
    return json(route, 200, { name: b.name, changed: true });
  });
  await page.route('**/api/people/remove', (route) => {
    const b = body(route);
    posts.push({ path: '/api/people/remove', body: b });
    people = people.filter((p) => p.name !== b.name);
    return json(route, 200, { name: b.name, removed: true });
  });

  await page.addInitScript(() => {
    window.localStorage.setItem('losos-tour', 'seen');
    window.__violations = [];
    document.addEventListener('securitypolicyviolation', (e) =>
      window.__violations.push(`${e.violatedDirective} ${e.blockedURI}`),
    );
  });
  await page.goto(origin + path, { waitUntil: 'networkidle' });
  return { page, context, errors, posts };
}

const violations = (page) => page.evaluate(() => [...new Set(window.__violations)].sort());
const nav = (page) => page.getByRole('navigation', { name: 'Sections', exact: true });
/* The switch's input is hidden under its track; a person clicks the track. */
const toggle = (scope, name) =>
  scope.locator('[data-slot="switch"]').filter({ has: scope.page().getByRole('switch', { name, exact: true }) }).click();
const row = (page, name) => page.locator(`[data-testid="users-person"][data-person="${name}"]`);

/* What a load logs without this pane: the baseline it must not add to. */
async function baseline() {
  const { page, context } = await open({ path: '/settings/security' });
  await page.waitForTimeout(300);
  const seen = await violations(page);
  await context.close();
  return seen;
}
const BASELINE = await baseline();

console.log('admin-ui users pane checks');

await check('on a box that signs people in itself, Users sits between Security and Access tokens', async () => {
  const { page, context } = await open({ path: '/settings/security' });
  const labels = (await nav(page).getByRole('link').allInnerTexts()).map((l) => l.trim());
  const at = labels.indexOf('Security');
  assert.deepEqual(labels.slice(at, at + 3), ['Security', 'Users', 'Access tokens'], labels.join(' | '));
  await nav(page).getByRole('button', { name: /^Search or jump to/ }).click();
  const box = page.getByRole('combobox', { name: /^Search pages/ });
  for (const word of ['people', 'family', 'add']) {
    await box.fill(word);
    await page.getByRole('option', { name: /^Users/ }).waitFor({ timeout: 2000 });
  }
  await page.getByRole('option', { name: /^Users/ }).click();
  await page.waitForURL(origin + '/settings/users');
  await page.getByRole('heading', { name: 'Users', level: 1 }).waitFor();
  await context.close();
});

await check('a box whose sign-in runs elsewhere has no Users pane', async () => {
  const { page, context } = await open({ path: '/settings/security', standalone: false });
  const labels = (await nav(page).getByRole('link').allInnerTexts()).map((l) => l.trim());
  assert.ok(!labels.includes('Users'), labels.join(' | '));
  await nav(page).getByRole('button', { name: /^Search or jump to/ }).click();
  await page.getByRole('combobox', { name: /^Search pages/ }).fill('people');
  await page.waitForTimeout(300);
  assert.equal(await page.getByRole('option', { name: /^Users/ }).count(), 0);
  // The address still answers, and says why there is nothing to do.
  await page.goto(origin + '/settings/users', { waitUntil: 'networkidle' });
  await page.getByTestId('users-elsewhere').waitFor();
  assert.match(await page.getByTestId('users-elsewhere').innerText(), /does not sign people in itself/);
  await context.close();
});

await check('the list marks the owner and the caller and gives neither a button', async () => {
  const { page, context, errors } = await open();
  await page.getByTestId('users-people').waitFor();
  assert.equal(await page.getByTestId('users-person').count(), 4);
  const owner = row(page, 'matus');
  assert.match(await owner.innerText(), /Owner/);
  assert.equal(await owner.getByRole('button').count(), 0);
  const me = row(page, 'jan');
  assert.match(await me.innerText(), /You/);
  assert.equal(await me.getByRole('button').count(), 0);
  const eva = row(page, 'eva');
  assert.match(await eva.innerText(), /Eva Novak/);
  assert.match(await eva.innerText(), /eva · eva@mattbox\.local/);
  assert.match(await eva.innerText(), /Chosen parts/);
  assert.match(await eva.innerText(), /Can change: Storage, Backup/);
  assert.match(await row(page, 'oma').innerText(), /Sees everything/);
  assert.match(await row(page, 'oma').innerText(), /Sees every pane and changes nothing/);
  assert.deepEqual(
    await eva.getByRole('button').allInnerTexts(),
    ['Access', 'New password', 'Remove'],
  );
  assert.deepEqual(errors, []);
  await context.close();
});

await check('a new person goes out once the name and password pass, with the groups chosen', async () => {
  const { page, context, posts } = await open();
  await page.getByTestId('users-form').waitFor();
  const add = page.getByTestId('users-add');
  assert.ok(await add.isDisabled(), 'Add is on with an empty form');
  await page.getByTestId('users-name').fill('Lea');
  assert.equal(await page.getByTestId('users-name').inputValue(), 'lea', 'the name is not lower-cased');
  await page.getByTestId('users-display-name').fill('Lea Novak');
  await page.getByTestId('users-password').fill('short');
  assert.ok(await add.isDisabled(), 'Add is on with a short password');
  await page.getByTestId('users-password').fill('longenoughbutplain');
  assert.ok(await add.isDisabled(), 'Add is on with a password that breaks the rules');
  await page.getByTestId('users-password').fill(GOOD_PASSWORD);
  const form = page.getByTestId('users-form');
  await toggle(form, 'Storage');
  await toggle(form, 'Look');
  await add.click();
  await row(page, 'lea').waitFor();
  assert.deepEqual(posts, [
    {
      path: '/api/people',
      body: { name: 'lea', displayName: 'Lea Novak', password: GOOD_PASSWORD, groups: ['losos-storage', 'losos-look'] },
    },
  ]);
  assert.equal(await page.getByTestId('users-name').inputValue(), '', 'the form kept the name');
  assert.equal(await page.getByTestId('users-password').inputValue(), '', 'the form kept the password');
  await context.close();
});

await check('an administrator is asked for without sections, a viewer with them', async () => {
  const { page, context, posts } = await open();
  const form = page.getByTestId('users-form');
  await form.waitFor();
  await page.getByTestId('users-name').fill('ida');
  await page.getByTestId('users-password').fill(GOOD_PASSWORD);
  await form.getByTestId('users-level-select').selectOption('admin');
  assert.equal(await form.getByRole('switch').count(), 0, 'switches stay for an administrator');
  await page.getByTestId('users-add').click();
  await row(page, 'ida').waitFor();
  assert.deepEqual(posts.at(-1).body.groups, ['losos-admins']);
  await page.getByTestId('users-name').fill('ola');
  await page.getByTestId('users-password').fill(GOOD_PASSWORD);
  await form.getByTestId('users-level-select').selectOption('viewer');
  await toggle(form, 'Backup');
  await page.getByTestId('users-add').click();
  await row(page, 'ola').waitFor();
  assert.deepEqual(posts.at(-1).body.groups, ['losos-viewers', 'losos-backup']);
  await context.close();
});

await check('a refusal shows beside the form', async () => {
  const { page, context } = await open({ refuse: { status: 409, error: 'eva already has an account' } });
  await page.getByTestId('users-form').waitFor();
  await page.getByTestId('users-name').fill('eva');
  await page.getByTestId('users-password').fill(GOOD_PASSWORD);
  await page.getByTestId('users-add').click();
  await page.getByTestId('users-error').waitFor();
  assert.equal(await page.getByTestId('users-error').innerText(), 'eva already has an account');
  await context.close();
});

await check('access, a new password and a removal each send their own request', async () => {
  const { page, context, posts } = await open();
  const eva = row(page, 'eva');
  await eva.waitFor();

  await eva.getByRole('button', { name: 'Access' }).click();
  const accessDialog = page.getByRole('dialog', { name: 'Access for Eva Novak' });
  await accessDialog.waitFor();
  assert.ok(await accessDialog.getByRole('switch', { name: 'Storage', exact: true }).isChecked());
  await accessDialog.getByTestId('users-level-select').selectOption('admin');
  await accessDialog.getByTestId('users-access-save').click();
  await accessDialog.waitFor({ state: 'hidden' });
  assert.deepEqual(posts.at(-1), { path: '/api/people/access', body: { name: 'eva', groups: ['losos-admins'] } });
  await page.waitForFunction(() =>
    document.querySelector('[data-person="eva"] [data-testid="users-level"]')?.textContent === 'Administrator',
  );

  await eva.getByRole('button', { name: 'New password' }).click();
  const pwDialog = page.getByRole('dialog', { name: 'New password for Eva Novak' });
  await pwDialog.waitFor();
  await pwDialog.getByLabel('Password').fill(GOOD_PASSWORD);
  await pwDialog.getByRole('button', { name: 'Save' }).click();
  await pwDialog.waitFor({ state: 'hidden' });
  assert.deepEqual(posts.at(-1), { path: '/api/people/password', body: { name: 'eva', password: GOOD_PASSWORD } });

  const before = posts.length;
  await eva.getByRole('button', { name: 'Remove' }).click();
  const removeDialog = page.getByRole('dialog', { name: 'Remove Eva Novak?' });
  await removeDialog.waitFor();
  assert.match(await removeDialog.innerText(), /files in LosOS cloud and LosOS Git stay/);
  assert.equal(posts.length, before, 'Remove sent before it asked');
  await removeDialog.getByTestId('users-remove-confirm').click();
  await eva.waitFor({ state: 'detached' });
  assert.deepEqual(posts.at(-1), { path: '/api/people/remove', body: { name: 'eva' } });
  await context.close();
});

await check('the pane speaks Slovak', async () => {
  const { page, context } = await open({ locale: 'sk-SK' });
  await page.getByRole('heading', { name: 'Používatelia', level: 1 }).waitFor();
  assert.match(await row(page, 'matus').innerText(), /Vlastník/);
  assert.match(await page.getByTestId('users-form').innerText(), /Pridať človeka/);
  await context.close();
});

await check('the pane adds no Content-Security-Policy violation', async () => {
  const { page, context } = await open();
  await page.getByTestId('users-people').waitFor();
  await row(page, 'eva').getByRole('button', { name: 'Access' }).click();
  await page.waitForTimeout(300);
  assert.deepEqual(await violations(page), BASELINE);
  await context.close();
});

await browser.close();
await closeServer();
finish();
