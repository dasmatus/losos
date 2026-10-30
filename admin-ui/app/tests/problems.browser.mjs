/* The Problems pane (src/screens/settings/pane-problems.tsx): what the box
 * noticed going wrong (backend/plugins/problems), each report read in full
 * before it is sent.
 *
 * What is checked: the pane sits right after Credentials and search finds
 * it, the list says where reports go and draws each kind, a documented
 * problem links its handbook page and offers no report, a report opens with
 * its whole text and sends the note as JSON, the code is shown while the box
 * waits for GitHub and the link once it is filed, Stop and Dismiss call
 * their routes, a box that sends nowhere offers no Send, a box without the
 * routes says so, and none of it adds to the Content-Security-Policy
 * violations a plain load logs.
 *
 * Same arrangement as the other files: the real dist/ bundle under the real
 * policy, the routes stubbed with page.route() as a small lososd that
 * enforces the Bearer token.
 */

import assert from './assert.mjs';
import { launch, runner, serve } from './harness.mjs';

const TOKEN = 'a'.repeat(64);
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

const CRASH = {
  id: '0badc0de', kind: 'crashed', unit: 'eva-sync.service', process: 'eva-sync', signal: 'SEGV',
  result: null, title: 'eva-sync.service crashed (SIGSEGV)', first: 1793246400, last: 1793250000,
  count: 3, dismissed: false, documented: null, sent: null,
};
const FULL = {
  id: '5ca1ab1e', kind: 'failed', unit: 'filler.service', process: null, signal: null,
  result: 'exit-code', title: 'filler.service failed (exit-code)', first: 1793240000, last: 1793240000,
  count: 1, dismissed: false, sent: null,
  documented: { id: 'out-of-room', page: 'troubleshooting/out-of-room', title: { en: 'Out of room', sk: 'Nedostatok miesta', de: 'Kein Platz mehr' } },
};
const OOM = {
  id: 'feedface', kind: 'out-of-memory', unit: null, process: 'ollama', signal: null, result: null,
  title: 'ollama was stopped: out of memory', first: 1793200000, last: 1793200000, count: 1,
  dismissed: true, documented: null, sent: null,
};
const VIEW = { reporting: { repository: 'dasmatus/losos' }, problems: [CRASH, FULL, OOM] };

const json = (route, status, body) =>
  route.fulfill({ status, contentType: 'application/json', body: JSON.stringify(body) });

/* A signed-in box. `view` is what GET /api/problems answers, or a status to
 * answer with instead. `sending` is the states GET …/send answers in turn,
 * the last one repeated. */
async function open({ path = '/settings/problems', view = VIEW, sending = [{ state: 'idle' }] } = {}) {
  const context = await browser.newContext({ viewport: { width: 1280, height: 900 } });
  const page = await context.newPage();
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  const calls = [];
  const states = [...sending];

  const authed = (route) => route.request().headers()['authorization'] === `Bearer ${TOKEN}`;
  const guard = (fn) => (route) => (authed(route) ? fn(route) : json(route, 401, { error: 'unauthorized' }));

  await page.route('**/api/**', (route) => json(route, 200, {}));
  await page.route('**/api/setup/claim', (route) => json(route, 200, { claimed: true }));
  await page.route('**/api/state', guard((route) => json(route, 200, { mode: 'local', sharing: false })));
  await page.route('**/api/settings', guard((route) => json(route, 200, SETTINGS)));
  await page.route('**/api/status', guard((route) => json(route, 200, { state: 'idle', progress: 0, message: '' })));
  await page.route('**/api/problems', guard((route) =>
    typeof view === 'number' ? json(route, view, { error: 'not found' }) : json(route, 200, view)));
  await page.route('**/api/problems/*', guard((route) => {
    const id = new URL(route.request().url()).pathname.split('/')[3];
    const p = VIEW.problems.find((x) => x.id === id);
    return json(route, 200, {
      title: `${p.title} [${id}]`,
      body: `${p.title}\n\n### Journal\n\neva-sync: the calendar answered with something it cannot read\n`,
      problem: p,
    });
  }));
  await page.route('**/api/problems/*/*', guard((route) => {
    const request = route.request();
    const [, , , id, what] = new URL(request.url()).pathname.split('/');
    if (request.method() === 'GET' && what === 'send') {
      return json(route, 200, states.length > 1 ? states.shift() : states[0]);
    }
    calls.push({ id, what, method: request.method(), body: request.postData() ?? '' });
    if (what === 'send') return json(route, 202, { state: 'starting', repository: 'dasmatus/losos' });
    return json(route, 200, {});
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
  return { page, context, errors, calls };
}

const violations = (page) => page.evaluate(() => [...new Set(window.__violations)].sort());
const nav = (page) => page.getByRole('navigation', { name: 'Sections', exact: true });
const row = (page, id) => page.locator(`[data-testid="problem"][data-id="${id}"]`);

async function baseline() {
  const { page, context } = await open({ path: '/settings/security' });
  await page.waitForTimeout(300);
  const seen = await violations(page);
  await context.close();
  return seen;
}
const BASELINE = await baseline();

console.log('admin-ui problems checks');

await check('the sidebar lists Problems right after Credentials, and search finds it', async () => {
  const { page, context } = await open({ path: '/settings/security' });
  const labels = await nav(page).getByRole('link').allInnerTexts();
  const at = labels.findIndex((l) => l.trim() === 'Credentials');
  assert.equal(labels[at + 1]?.trim(), 'Problems', `after Credentials: ${labels[at + 1]}`);
  await nav(page).getByRole('button', { name: /^Search or jump to/ }).click();
  const box = page.getByRole('combobox', { name: /^Search pages/ });
  for (const word of ['crash', 'abrt', 'bug']) {
    await box.fill(word);
    await page.getByRole('option', { name: /^Problems/ }).waitFor({ timeout: 2000 });
  }
  await page.getByRole('option', { name: /^Problems/ }).click();
  await page.waitForURL(origin + '/settings/problems');
  await page.getByRole('heading', { name: 'Problems', level: 1 }).waitFor();
  await context.close();
});

await check('the list says where reports go and draws each kind', async () => {
  const { page, context, errors } = await open();
  await row(page, CRASH.id).waitFor();
  const main = await page.locator('main').innerText();
  assert.ok(main.includes('Reports go to dasmatus/losos on GitHub'), main);
  assert.ok(main.includes('Asks first'), main);
  const crash = await row(page, CRASH.id).innerText();
  assert.ok(crash.includes('eva-sync.service crashed (SIGSEGV)'), crash);
  assert.ok(crash.includes('3 times'), crash);
  assert.ok(crash.includes('Crash'), crash);
  assert.equal(await row(page, CRASH.id).getByRole('button', { name: 'Read and send' }).count(), 1);

  // The handbook explains the full disk: its page, and no report.
  const full = row(page, FULL.id);
  assert.ok((await full.innerText()).includes('In the handbook: Out of room'));
  assert.equal(await full.getByRole('link', { name: 'What to do' }).getAttribute('href'), '/handbook/troubleshooting/out-of-room/');
  assert.equal(await full.getByRole('button', { name: /Read/ }).count(), 0);

  // A dismissed one sits under its own heading, without Dismiss.
  const oom = row(page, OOM.id);
  assert.ok((await oom.innerText()).includes('ollama was stopped: the box ran out of memory'));
  assert.equal(await oom.getByRole('button', { name: 'Dismiss' }).count(), 0);
  assert.ok(main.includes('Dismissed'), main);

  assert.deepEqual(await violations(page), BASELINE, 'the pane added CSP violations');
  assert.deepEqual(errors, []);
  await context.close();
});

await check('a report opens whole, sends its note, shows the code and then the issue', async () => {
  const { page, context, errors, calls } = await open({
    sending: [
      { state: 'idle' },
      { state: 'starting' },
      { state: 'sign-in', uri: 'https://github.com/login/device', code: 'WDJB-MJHT', expires: 900 },
      { state: 'sent', url: 'https://github.com/dasmatus/losos/issues/7', existing: false },
    ],
  });
  await row(page, CRASH.id).getByRole('button', { name: 'Read and send' }).click();
  const dialog = page.getByRole('dialog');
  await dialog.getByTestId('problem-report-body').waitFor();
  const body = await dialog.getByTestId('problem-report-body').innerText();
  assert.ok(body.includes('the calendar answered with something it cannot read'), body);
  assert.ok((await dialog.innerText()).includes('This is the whole text dasmatus/losos would get, in public.'));
  await dialog.getByTestId('problem-note').fill('After a phone sync.');
  await dialog.getByRole('button', { name: 'Send on GitHub' }).click();
  await dialog.getByTestId('problem-code').waitFor({ timeout: 8000 });
  assert.equal(await dialog.getByTestId('problem-code').innerText(), 'WDJB-MJHT');
  assert.equal(
    await dialog.getByRole('link', { name: 'Open github.com/login/device' }).getAttribute('href'),
    'https://github.com/login/device',
  );
  await dialog.getByTestId('problem-sent').waitFor({ timeout: 8000 });
  assert.equal(
    await dialog.getByTestId('problem-sent').getByRole('link').getAttribute('href'),
    'https://github.com/dasmatus/losos/issues/7',
  );
  assert.deepEqual(calls, [{ id: CRASH.id, what: 'send', method: 'POST', body: '{"note":"After a phone sync."}' }]);
  assert.deepEqual(errors, []);
  await context.close();
});

await check('Stop and Dismiss call their routes', async () => {
  const { page, context, errors, calls } = await open({
    sending: [{ state: 'idle' }, { state: 'sign-in', uri: 'https://github.com/login/device', code: 'WDJB-MJHT', expires: 900 }],
  });
  await row(page, CRASH.id).getByRole('button', { name: 'Read and send' }).click();
  const dialog = page.getByRole('dialog');
  await dialog.getByRole('button', { name: 'Send on GitHub' }).click();
  await dialog.getByTestId('problem-code').waitFor({ timeout: 8000 });
  await dialog.getByRole('button', { name: 'Stop' }).click();
  await page.waitForTimeout(300);
  assert.ok(calls.some((c) => c.what === 'cancel' && c.method === 'POST'), JSON.stringify(calls));
  await page.keyboard.press('Escape');
  await page.reload({ waitUntil: 'networkidle' });
  await row(page, CRASH.id).getByRole('button', { name: 'Dismiss' }).click();
  await page.waitForTimeout(300);
  assert.ok(calls.some((c) => c.id === CRASH.id && c.what === 'dismiss'), JSON.stringify(calls));
  assert.deepEqual(errors, []);
  await context.close();
});

await check('a box that sends nowhere offers no Send, and an old box says so', async () => {
  const { page, context, errors } = await open({ view: { reporting: null, problems: [CRASH] } });
  await row(page, CRASH.id).waitFor();
  assert.ok((await page.locator('main').innerText()).includes('Reports stay on this box'));
  await row(page, CRASH.id).getByRole('button', { name: 'Read the report' }).click();
  const dialog = page.getByRole('dialog');
  await dialog.getByTestId('problem-report-body').waitFor();
  assert.equal(await dialog.getByRole('button', { name: 'Send on GitHub' }).count(), 0);
  assert.equal(await dialog.getByTestId('problem-note').count(), 0);
  assert.deepEqual(errors, []);
  await context.close();

  const old = await open({ view: 404 });
  await old.page.getByText('This box does not watch for problems yet. It will after its next update.', { exact: true }).waitFor();
  await old.context.close();
});

await browser.close();
await closeServer();
finish();
