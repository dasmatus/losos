/* The Users pane (src/screens/settings/pane-users.tsx): the people who may
 * sign in to the box's own LosOS Login, and what each of them can change.
 *
 * What is checked: the pane is in the sidebar and the search only on a box
 * that signs people in itself, between Security and Access tokens; the list
 * marks the owner and the caller and gives neither a button; a new person
 * goes out with the body backend/schema.json names, and only once the name
 * and password pass the box's rules; a refusal shows beside the form;
 * access, a new password and a removal each send their own request, the
 * removal only after asking; the sign-in rules go out as one request and a
 * 409 for a missing second factor of the caller's own says what to do; a
 * second factor and a rule's hold show on the person, and resetting the
 * factors asks first; a role is picked from one list (administrator,
 * moderator, member, guest and the box's own roles) and goes out as its
 * groups; the table says what each role can do; a role of the box's own is
 * saved and removed with its own requests; Slovak names the pane; and none
 * of it adds to the Content-Security-Policy violations a plain load logs.
 *
 * Same arrangement as the other files: the real dist/ bundle under the real
 * policy, the routes stubbed with page.route() as a small stateful lososd.
 * The caller signed in through LosOS Login, so there is no Bearer token. */

import assert from './assert.mjs';
import { launch, runner, serve } from './harness.mjs';

const { origin, close: closeServer } = await serve({ csp: true });
const browser = await launch();
const { check, finish } = runner();

const SECTIONS = ['storage', 'backup', 'apps', 'market', 'network', 'look', 'lab', 'access', 'account', 'moderation'];
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
const PEOPLE = [
  { name: 'eva', displayName: 'Eva Novak', email: 'eva@mattbox.local', groups: ['losos-storage', 'losos-backup'], disabled: false },
  { name: 'jan', displayName: 'Jan', email: 'jan@mattbox.local', groups: ['losos-admins'], disabled: false },
  { name: 'matus', displayName: 'Matus', email: 'matus@mattbox.local', groups: ['losos-admins', 'losos-owner'], disabled: false },
  { name: 'oma', displayName: 'Oma', email: 'oma@mattbox.local', groups: ['losos-viewers'], disabled: false },
  {
    name: 'ivo',
    displayName: 'Ivo',
    email: 'ivo@mattbox.local',
    groups: ['losos-account', 'losos-two-factor'],
    disabled: false,
    twoFactor: true,
    passwordSetAt: 1790000000,
    locked: [],
  },
  {
    name: 'zoe',
    displayName: 'Zoe',
    email: 'zoe@mattbox.local',
    groups: ['losos-account', 'losos-locked'],
    disabled: false,
    twoFactor: false,
    passwordSetAt: 1790000000,
    locked: ['password'],
  },
];
const GOOD_PASSWORD = 'Salmon-river-42';

const json = (route, status, body) =>
  route.fulfill({ status, contentType: 'application/json', body: JSON.stringify(body) });

/* A box with `you` signed in as an administrator. `standalone` is what
 * GET /api/me and GET /api/people say; `refuse` answers POST /api/people
 * with that status and sentence. */
async function open({
  path = '/settings/users',
  standalone = true,
  refuse = null,
  refuseRules = null,
  locale = 'en-US',
  you = 'jan',
} = {}) {
  const context = await browser.newContext({ viewport: { width: 1280, height: 900 }, locale });
  const page = await context.newPage();
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  let people = structuredClone(PEOPLE);
  let rules = { twoFactor: false, passwordDays: 0 };
  let roles = [{ id: 'helpers', name: 'Backup helpers', sections: ['backup', 'account'] }];
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
      return json(route, 200, { standalone, you, sections: SECTIONS, rules, roles, people: standalone ? people : [] });
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
  await page.route('**/api/people/rules', (route) => {
    const b = body(route);
    posts.push({ path: '/api/people/rules', body: b });
    if (refuseRules !== null) return json(route, refuseRules.status, refuseRules.body);
    rules = b;
    return json(route, 200, b);
  });
  await page.route('**/api/people/two-factor/reset', (route) => {
    const b = body(route);
    posts.push({ path: '/api/people/two-factor/reset', body: b });
    people = people.map((p) => (p.name === b.name ? { ...p, twoFactor: false } : p));
    return json(route, 200, { name: b.name, reset: true });
  });
  await page.route('**/api/roles', (route) => {
    const b = body(route);
    posts.push({ path: '/api/roles', body: b });
    const id = b.id ?? b.name.toLowerCase().replace(/[^a-z0-9]+/g, '-');
    roles = [...roles.filter((r) => r.id !== id), { id, name: b.name, sections: b.sections }];
    return json(route, 200, { id, roles });
  });
  await page.route('**/api/roles/remove', (route) => {
    const b = body(route);
    posts.push({ path: '/api/roles/remove', body: b });
    roles = roles.filter((r) => r.id !== b.id);
    return json(route, 200, { roles });
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

await check('on a box that signs people in itself, Your account, Users and Moderation sit between Security and Access tokens', async () => {
  const { page, context } = await open({ path: '/settings/security' });
  const labels = (await nav(page).getByRole('link').allInnerTexts()).map((l) => l.trim());
  const at = labels.indexOf('Security');
  assert.deepEqual(
    labels.slice(at, at + 5),
    ['Security', 'Your account', 'Users', 'Moderation', 'Access tokens'],
    labels.join(' | '),
  );
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
  assert.ok(!labels.includes('Your account'), labels.join(' | '));
  assert.ok(!labels.includes('Moderation'), labels.join(' | '));
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
  assert.equal(await page.getByTestId('users-person').count(), PEOPLE.length);
  const owner = row(page, 'matus');
  assert.match(await owner.innerText(), /Owner/);
  assert.equal(await owner.getByRole('button').count(), 0);
  const me = row(page, 'jan');
  assert.match(await me.innerText(), /You/);
  assert.equal(await me.getByRole('button').count(), 0);
  const eva = row(page, 'eva');
  assert.match(await eva.innerText(), /Eva Novak/);
  assert.match(await eva.innerText(), /eva · eva@mattbox\.local/);
  assert.match(await eva.innerText(), /Member/);
  assert.match(await eva.innerText(), /Can change: Storage, Backup/);
  assert.match(await row(page, 'oma').innerText(), /Member/);
  assert.match(await row(page, 'oma').innerText(), /Sees every pane and changes nothing/);
  // The owner and administrators carry a mark on their picture; members none.
  assert.equal(await owner.locator('[data-slot="avatar-role"][data-role="owner"]').count(), 1);
  assert.equal(await me.locator('[data-slot="avatar-role"][data-role="admin"]').count(), 1);
  assert.equal(await eva.locator('[data-slot="avatar-role"]').count(), 0);
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
      body: {
        name: 'lea',
        displayName: 'Lea Novak',
        password: GOOD_PASSWORD,
        groups: ['losos-account', 'losos-storage', 'losos-look'],
      },
    },
  ]);
  assert.equal(await page.getByTestId('users-name').inputValue(), '', 'the form kept the name');
  assert.equal(await page.getByTestId('users-password').inputValue(), '', 'the form kept the password');
  await context.close();
});

await check('each role goes out as its groups', async () => {
  const { page, context, posts } = await open();
  const form = page.getByTestId('users-form');
  await form.waitFor();
  const add = async (name, role, switches = []) => {
    await page.getByTestId('users-name').fill(name);
    await page.getByTestId('users-password').fill(GOOD_PASSWORD);
    await form.getByTestId('users-role-select').selectOption(role);
    for (const s of switches) await toggle(form, s);
    await page.getByTestId('users-add').click();
    await row(page, name).waitFor();
    return posts.at(-1).body.groups;
  };
  await form.getByTestId('users-role-select').selectOption('admin');
  assert.equal(await form.getByRole('switch').count(), 0, 'switches stay for an administrator');
  await form.getByTestId('users-role-select').selectOption('guest');
  assert.equal(await form.getByRole('switch').count(), 0, 'switches stay for a guest');
  // Moderation is the moderator's role, never a switch of its own.
  await form.getByTestId('users-role-select').selectOption('member');
  assert.equal(await form.getByRole('switch', { name: 'Moderation', exact: true }).count(), 0);
  assert.deepEqual(await add('ida', 'admin'), ['losos-admins']);
  assert.deepEqual(await add('ola', 'member', ['Sees every part', 'Backup']), [
    'losos-viewers',
    'losos-account',
    'losos-backup',
  ]);
  assert.deepEqual(await add('max', 'moderator'), ['losos-moderation', 'losos-account']);
  assert.deepEqual(await add('gus', 'guest'), ['losos-guests', 'losos-account']);
  assert.deepEqual(await add('bea', 'custom:helpers'), ['losos-role-helpers']);
  await context.close();
});

await check('the table says what each role can do', async () => {
  const { page, context } = await open();
  const table = page.getByTestId('users-role-table');
  await table.waitFor();
  const head = (await table.locator('th').allInnerTexts()).map((h) => h.trim());
  assert.deepEqual(head, ['On this box', 'Owner', 'Administrator', 'Moderator', 'Member', 'Guest']);
  const settings = (await table.locator('tr', { hasText: 'Settings' }).locator('td').allInnerTexts()).map((c) => c.trim());
  assert.deepEqual(settings, ['Settings', 'Everything', 'Everything', 'Parts given', 'Parts given', 'Own account']);
  await context.close();
});

await check('a role of the box\'s own is saved, given and removed with its own requests', async () => {
  const { page, context, posts } = await open();
  const section = page.getByTestId('users-roles');
  await section.waitFor();
  assert.match(await section.innerText(), /Backup helpers/);
  assert.match(await section.innerText(), /Gives: Backup, Own account/);
  await section.getByTestId('users-role-new').click();
  const dialog = page.getByRole('dialog', { name: 'New role' });
  await dialog.waitFor();
  await dialog.getByTestId('users-role-name').fill('Lab crew');
  await toggle(dialog, 'Diagrams');
  await dialog.getByTestId('users-role-save').click();
  await dialog.waitFor({ state: 'hidden' });
  assert.deepEqual(posts.at(-1), { path: '/api/roles', body: { name: 'Lab crew', sections: ['account', 'lab'] } });
  await section.locator('[data-role="lab-crew"]').waitFor();
  // The new role is in the picker at once.
  const options = await page.getByTestId('users-form').getByTestId('users-role-select').locator('option').allInnerTexts();
  assert.ok(options.includes('Lab crew'), options.join(' | '));
  await section.locator('[data-role="helpers"]').getByRole('button', { name: 'Remove' }).click();
  const confirm = page.getByRole('dialog', { name: 'Remove Backup helpers?' });
  await confirm.waitFor();
  await confirm.getByTestId('users-role-remove-confirm').click();
  await section.locator('[data-role="helpers"]').waitFor({ state: 'detached' });
  assert.deepEqual(posts.at(-1), { path: '/api/roles/remove', body: { id: 'helpers' } });
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
  await accessDialog.getByTestId('users-role-select').selectOption('admin');
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
  assert.match(await removeDialog.innerText(), /files in LosOS Cloud and LosOS Git stay/);
  assert.equal(posts.length, before, 'Remove sent before it asked');
  await removeDialog.getByTestId('users-remove-confirm').click();
  await eva.waitFor({ state: 'detached' });
  assert.deepEqual(posts.at(-1), { path: '/api/people/remove', body: { name: 'eva' } });
  await context.close();
});

await check('a second factor and a rule\'s hold show on the person, and a reset asks first', async () => {
  const { page, context, posts } = await open();
  const ivo = row(page, 'ivo');
  await ivo.waitFor();
  await ivo.getByTestId('users-two-factor').waitFor();
  assert.equal(await row(page, 'eva').getByTestId('users-two-factor').count(), 0);
  const zoe = row(page, 'zoe');
  await zoe.getByTestId('users-held').waitFor();
  assert.match(await zoe.innerText(), /until they choose a new password/);
  assert.match(await ivo.innerText(), /Can change: Own account/);
  assert.equal(await zoe.getByTestId('users-reset-factors').count(), 0, 'a reset for nothing to reset');

  const before = posts.length;
  await ivo.getByTestId('users-reset-factors').click();
  const dialog = page.getByRole('dialog', { name: "Remove Ivo's authenticator app and passkeys?" });
  await dialog.waitFor();
  assert.match(await dialog.innerText(), /lost their phone or key/);
  assert.equal(posts.length, before, 'the reset went out before it asked');
  await dialog.getByTestId('users-reset-factors-confirm').click();
  await ivo.getByTestId('users-two-factor').waitFor({ state: 'detached' });
  assert.deepEqual(posts.at(-1), { path: '/api/people/two-factor/reset', body: { name: 'ivo' } });
  await context.close();
});

await check('the sign-in rules go out together, with the days the field holds', async () => {
  const { page, context, posts } = await open();
  const rules = page.getByTestId('users-rules');
  await rules.waitFor();
  const save = page.getByTestId('users-rules-save');
  assert.ok(await save.isDisabled(), 'Save is on with nothing changed');
  assert.equal(await page.getByTestId('users-rules-days').count(), 0, 'days shown with the rule off');
  await toggle(rules, 'Passwords run out');
  assert.equal(await page.getByTestId('users-rules-days').inputValue(), '90');
  await page.getByTestId('users-rules-days').fill('0');
  assert.ok(await save.isDisabled(), 'Save is on with 0 days');
  await page.getByTestId('users-rules-days').fill('1201');
  assert.ok(await save.isDisabled(), 'Save is on past the most days');
  await page.getByTestId('users-rules-days').fill('30');
  await toggle(rules, 'Require two-factor sign-in');
  await save.click();
  await page.waitForFunction(() => document.querySelector('[data-testid="users-rules-save"]')?.disabled === true);
  assert.deepEqual(posts.at(-1), { path: '/api/people/rules', body: { twoFactor: true, passwordDays: 30 } });
  await context.close();
});

await check('turning on two-factor sign-in without a second factor of your own says what to do', async () => {
  const { page, context } = await open({
    refuseRules: {
      status: 409,
      body: { error: 'add an authenticator app or a passkey to your own account first', needsOwnFactor: true },
    },
  });
  const rules = page.getByTestId('users-rules');
  await rules.waitFor();
  await toggle(rules, 'Require two-factor sign-in');
  await page.getByTestId('users-rules-save').click();
  const error = page.getByTestId('users-rules-error');
  await error.waitFor();
  assert.match(await error.innerText(), /to your own account first, or the rule would hold you too/);
  await error.getByRole('link', { name: 'Open your account' }).click();
  await page.waitForURL(origin + '/settings/account');
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
