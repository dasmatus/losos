/* The Moderation pane (src/screens/settings/pane-moderation.tsx): the
 * reports people filed, the people a moderator may mute or suspend, and the
 * log of what moderators did.
 *
 * What is checked: a moderator who is not an administrator sees Moderation
 * and none of the administrators' panes; a report about a child carries no
 * note, a warning, and one button that hides the thing without opening it;
 * a spam report in mail is held, then let go, each its own request; a
 * report in an app the queue cannot hide in is dismissed or closed; a mute
 * goes out with the minutes and the reason the dialog holds, and an unmute
 * with the name alone; someone the caller may not act on has no buttons;
 * the log says who did what; an empty queue says so; Slovak names the
 * pane; and none of it adds to the Content-Security-Policy violations a
 * plain load logs. lososd orders the queue (open first, child reports
 * first), so the stub serves it in that order.
 *
 * Same arrangement as the other files: the real dist/ bundle under the real
 * policy, the routes stubbed with page.route() as a small stateful lososd. */

import assert from './assert.mjs';
import { launch, runner, serve } from './harness.mjs';

const { origin, close: closeServer } = await serve({ csp: true });
const browser = await launch();
const { check, finish } = runner();

const NOW = 1792650000;
const person = (name, displayName, role, extra = {}) => ({
  name,
  displayName,
  pronouns: '',
  about: '',
  accent: 'salmon',
  accentChosen: false,
  picture: null,
  banner: null,
  role,
  roleName: null,
  muted: null,
  suspended: null,
  disabled: false,
  actionable: role === 'member' || role === 'guest',
  ...extra,
});
const PEOPLE = [
  person('matus', 'Matus', 'owner'),
  person('jan', 'Jan', 'admin'),
  person('mia', 'Mia', 'moderator'),
  person('eva', 'Eva Novak', 'member'),
  person('tom', 'Tom', 'member'),
  person('gus', 'Gus', 'guest'),
];
const REPORTS = [
  {
    id: 'r2',
    at: NOW - 600,
    by: 'gus',
    app: 'profile',
    about: 'tom',
    ref: 'picture',
    reason: 'child',
    note: '',
    state: 'open',
    handledBy: null,
    handledAt: null,
  },
  {
    id: 'r1',
    at: NOW - 3600,
    by: 'eva',
    app: 'mail',
    about: 'tom',
    ref: 'Mb7f2',
    reason: 'spam',
    note: 'Twelve copies of the same offer this morning.',
    state: 'open',
    handledBy: null,
    handledAt: null,
  },
  {
    id: 'r3',
    at: NOW - 7200,
    by: 'tom',
    app: 'chat',
    about: 'eva',
    ref: '$event',
    reason: 'abuse',
    note: 'Called me names in the family room.',
    state: 'open',
    handledBy: null,
    handledAt: null,
  },
];
const LOG = [
  { at: NOW - 3600, by: 'eva', action: 'report', about: 'tom', detail: 'spam' },
  { at: NOW - 86400, by: 'jan', action: 'role', about: 'mia', detail: 'moderator' },
];

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

/* A box with `you` signed in as a moderator (or an administrator with
 * `admin`). `reports` replaces the queue. */
async function open({ path = '/settings/moderation', locale = 'en-US', you = 'mia', admin = false, reports = REPORTS } = {}) {
  const context = await browser.newContext({ viewport: { width: 1280, height: 900 }, locale });
  const page = await context.newPage();
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  const state = { reports: structuredClone(reports), people: structuredClone(PEOPLE), log: structuredClone(LOG) };
  const posts = [];
  const body = (route) => JSON.parse(route.request().postData() ?? '{}');
  const view = () => ({
    standalone: true,
    you,
    admin,
    hides: ['mail', 'profile'],
    reports: state.reports,
    people: state.people,
    log: state.log,
  });
  const note = (action, about, detail = '') => state.log.unshift({ at: NOW, by: you, action, about, detail });

  await page.route('**/api/**', (route) => json(route, 200, {}));
  await page.route('**/api/setup/claim', (route) => json(route, 200, { claimed: true }));
  await page.route('**/api/me', (route) =>
    json(route, 200, {
      name: you,
      displayName: you,
      groups: admin ? ['losos-admins'] : ['losos-account', 'losos-moderation'],
      admin,
      spareKey: false,
      sections: ['account', 'moderation'],
      standalone: true,
      role: admin ? 'admin' : 'moderator',
      roleName: null,
      reports: state.reports.filter((r) => r.state === 'open').length,
    }),
  );
  await page.route('**/api/state', (route) => json(route, 200, { mode: 'local', sharing: false }));
  await page.route('**/api/settings', (route) => json(route, 200, SETTINGS));
  await page.route('**/api/status', (route) => json(route, 200, { state: 'idle', progress: 0, message: '' }));
  await page.route('**/api/moderation', (route) => json(route, 200, view()));
  await page.route('**/api/moderation/reports/*', (route) => {
    const id = decodeURIComponent(route.request().url().split('/').at(-1));
    const b = body(route);
    posts.push({ path: `/api/moderation/reports/${id}`, body: b });
    const next = { hide: 'hidden', release: 'resolved', dismiss: 'dismissed', resolve: 'resolved' }[b.action];
    state.reports = state.reports.map((r) =>
      r.id === id ? { ...r, state: next, handledBy: you, handledAt: NOW } : r,
    );
    note(b.action, state.reports.find((r) => r.id === id)?.about ?? null);
    return json(route, 200, view());
  });
  for (const kind of ['mute', 'unmute', 'suspend', 'unsuspend']) {
    await page.route(`**/api/moderation/${kind}`, (route) => {
      const b = body(route);
      posts.push({ path: `/api/moderation/${kind}`, body: b });
      const hold = { at: NOW, by: you, until: b.minutes == null ? null : NOW + b.minutes * 60, reason: b.reason ?? '' };
      const field = kind.endsWith('mute') ? 'muted' : 'suspended';
      state.people = state.people.map((p) =>
        p.name === b.name ? { ...p, [field]: kind.startsWith('un') ? null : hold } : p,
      );
      note(kind, b.name, b.reason ?? '');
      return json(route, 200, view());
    });
  }

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
const report = (page, reason, state = 'open') =>
  page.locator(`[data-testid="moderation-report"][data-reason="${reason}"][data-state="${state}"]`);
const who = (page, name) => page.locator(`[data-testid="moderation-person"][data-person="${name}"]`);

async function baseline() {
  const { page, context } = await open({ path: '/settings/account' });
  await page.waitForTimeout(300);
  const seen = await violations(page);
  await context.close();
  return seen;
}
const BASELINE = await baseline();

console.log('admin-ui moderation pane checks');

await check('a moderator sees Moderation and none of the administrators\' panes', async () => {
  const { page, context, errors } = await open();
  await page.getByRole('heading', { name: 'Moderation', level: 1 }).waitFor();
  const labels = (await nav(page).getByRole('link').allInnerTexts()).map((l) => l.trim());
  assert.ok(labels.includes('Moderation'), labels.join(' | '));
  for (const pane of ['Users', 'Security', 'Advanced', 'Reset']) assert.ok(!labels.includes(pane), labels.join(' | '));
  assert.deepEqual(errors, []);
  await context.close();
});

await check('a report about a child carries no note, warns, and can only be hidden unopened', async () => {
  const { page, context, posts } = await open();
  const reports = page.getByTestId('moderation-reports');
  await reports.waitFor();
  const child = report(page, 'child');
  await child.getByTestId('moderation-child').waitFor();
  assert.match(await child.getByTestId('moderation-child').innerText(), /Do not open it/);
  assert.equal(await child.getByRole('img').count(), 0, 'no picture is drawn');
  assert.equal(await child.getByTestId('moderation-dismiss').count(), 0);
  assert.equal(await child.getByTestId('moderation-resolve').count(), 0);
  assert.equal(await child.getByRole('link', { name: /Where to report it/i }).count(), 1);
  await child.getByTestId('moderation-hide').click();
  await report(page, 'child', 'hidden').waitFor();
  assert.deepEqual(posts.at(-1), { path: '/api/moderation/reports/r2', body: { action: 'hide' } });
  // A hidden child report has nothing to let go.
  assert.equal(await report(page, 'child', 'hidden').getByTestId('moderation-release').count(), 0);
  await context.close();
});

await check('spam in mail is held and let go, each its own request', async () => {
  const { page, context, posts } = await open();
  const spam = report(page, 'spam');
  await spam.waitFor();
  assert.match(await spam.innerText(), /Twelve copies of the same offer/);
  assert.match(await spam.innerText(), /Reported by Eva Novak/);
  await spam.getByTestId('moderation-hide').click();
  const held = report(page, 'spam', 'hidden');
  await held.waitFor();
  assert.deepEqual(posts.at(-1), { path: '/api/moderation/reports/r1', body: { action: 'hide' } });
  await held.getByTestId('moderation-release').click();
  await report(page, 'spam', 'resolved').waitFor();
  assert.deepEqual(posts.at(-1), { path: '/api/moderation/reports/r1', body: { action: 'release' } });
  await context.close();
});

await check('a report in an app the queue cannot hide in is dismissed or closed, never hidden', async () => {
  const { page, context, posts } = await open();
  const abuse = report(page, 'abuse');
  await abuse.waitFor();
  assert.equal(await abuse.getByTestId('moderation-hide').count(), 0);
  await abuse.getByTestId('moderation-dismiss').click();
  await report(page, 'abuse', 'dismissed').waitFor();
  assert.deepEqual(posts.at(-1), { path: '/api/moderation/reports/r3', body: { action: 'dismiss' } });
  await context.close();
});

await check('a mute goes out with the minutes and the reason; an unmute with the name alone', async () => {
  const { page, context, posts } = await open();
  const tom = who(page, 'tom');
  await tom.waitFor();
  await tom.getByTestId('moderation-mute').click();
  const dialog = page.getByRole('dialog', { name: 'Mute Tom' });
  await dialog.waitFor();
  await dialog.getByTestId('moderation-duration').selectOption('week');
  await dialog.getByTestId('moderation-reason').fill('Spam in mail');
  await dialog.getByTestId('moderation-hold-confirm').click();
  await dialog.waitFor({ state: 'hidden' });
  assert.deepEqual(posts.at(-1), {
    path: '/api/moderation/mute',
    body: { name: 'tom', minutes: 7 * 24 * 60, reason: 'Spam in mail' },
  });
  await tom.getByTestId('moderation-muted').waitFor();
  await tom.getByTestId('moderation-unmute').click();
  await tom.getByTestId('moderation-muted').waitFor({ state: 'detached' });
  assert.deepEqual(posts.at(-1), { path: '/api/moderation/unmute', body: { name: 'tom' } });
  await context.close();
});

await check('a suspension until lifted sends no minutes and shows on the person', async () => {
  const { page, context, posts } = await open();
  const gus = who(page, 'gus');
  await gus.getByTestId('moderation-suspend').click();
  const dialog = page.getByRole('dialog', { name: 'Suspend Gus' });
  await dialog.getByTestId('moderation-duration').selectOption('ever');
  await dialog.getByTestId('moderation-hold-confirm').click();
  await dialog.waitFor({ state: 'hidden' });
  assert.deepEqual(posts.at(-1), { path: '/api/moderation/suspend', body: { name: 'gus', minutes: null } });
  assert.match(await gus.getByTestId('moderation-suspended').innerText(), /Suspended/);
  assert.equal(await gus.getByTestId('moderation-unsuspend').count(), 1);
  await context.close();
});

await check('the owner, administrators and other moderators have no buttons for a moderator', async () => {
  const { page, context } = await open();
  await page.getByTestId('moderation-people').waitFor();
  for (const name of ['matus', 'jan', 'mia']) {
    assert.equal(await who(page, name).getByRole('button').count(), 0, name);
  }
  assert.equal(await who(page, 'eva').getByTestId('moderation-mute').count(), 1);
  await context.close();
});

await check('the log says who did what', async () => {
  const { page, context } = await open();
  const log = page.getByTestId('moderation-log');
  await log.waitFor();
  const lines = await log.getByTestId('moderation-log-line').allInnerTexts();
  assert.ok(lines.some((l) => /Eva Novak/.test(l) && /reported Tom/.test(l)), lines.join(' | '));
  assert.ok(lines.some((l) => /Jan/.test(l) && /made Mia Moderator/.test(l)), lines.join(' | '));
  await who(page, 'eva').getByTestId('moderation-mute').click();
  await page.getByTestId('moderation-hold-confirm').click();
  await log.getByTestId('moderation-log-line').filter({ hasText: 'muted Eva Novak' }).waitFor();
  await context.close();
});

await check('an empty queue says so', async () => {
  const { page, context } = await open({ reports: [] });
  await page.getByTestId('moderation-reports-empty').waitFor();
  await context.close();
});

await check('the pane speaks Slovak', async () => {
  const { page, context } = await open({ locale: 'sk-SK' });
  await page.getByRole('heading', { name: 'Moderovanie', level: 1 }).waitFor();
  assert.match(await report(page, 'child').innerText(), /Neotvárajte to/);
  assert.match(await page.getByTestId('moderation-people').innerText(), /Stíšiť/);
  await context.close();
});

await check('the pane adds no Content-Security-Policy violation', async () => {
  const { page, context } = await open();
  await page.getByTestId('moderation-reports').waitFor();
  await who(page, 'tom').getByTestId('moderation-suspend').click();
  await page.waitForTimeout(300);
  assert.deepEqual(await violations(page), BASELINE);
  await context.close();
});

await browser.close();
await closeServer();
finish();
