/* Settings › Logs (src/screens/settings/pane-logs.tsx): the journal of the
 * box's services, from the log reader (backend/src/logs/).
 *
 * What is checked: the pane sits right after History and search finds it,
 * the lines render oldest first with their severity, unit and a day's date,
 * a filter asks the reader again and puts itself on the address, an
 * address with a filter (a problem report's link) opens on it, Follow asks
 * for what came after the newest line and adds it, older lines come in
 * above, Download fetches the file with the key and saves it under the
 * box's name, the reader's complaint is shown, and none of it adds to the
 * Content-Security-Policy violations a plain load logs.
 *
 * Same arrangement as the other files: the real dist/ bundle under the real
 * policy, the routes stubbed with page.route() as a small reader that
 * enforces the Bearer token.
 */

import assert from 'node:assert';
import { launch, runner, serve } from './harness.mjs';

const TOKEN = 'a'.repeat(64);
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

const SOURCES = ['lososd', 'rebuilds', 'login', 'web', 'cloud', 'kernel', 'losos', 'everything'];

const line = (i, message, extra = {}) => ({
  cursor: `s=1;i=${i.toString(16)}`,
  date: i < 3 ? '2026-10-28' : '2026-10-29',
  time: `12:00:0${i % 10}`,
  stamp: `2026-10-29T12:00:0${i % 10}+01:00`,
  unit: 'lososd.service',
  pid: 42,
  priority: 6,
  message,
  ...extra,
});

const json = (route, status, body) =>
  route.fulfill({ status, contentType: 'application/json', body: JSON.stringify(body) });

/* A signed-in box whose reader holds lines 1 to 9: the first page is 4 to
 * 9, older ones come by `before`, and each follow adds one line. */
async function open({ path = '/settings/logs', complaint = null } = {}) {
  const context = await browser.newContext({ viewport: { width: 1280, height: 900 }, locale: 'en-US', acceptDownloads: true });
  const page = await context.newPage();
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  const asked = [];
  let next = 10;

  const authed = (route) => route.request().headers()['authorization'] === `Bearer ${TOKEN}`;
  const guard = (fn) => (route) => (authed(route) ? fn(route) : json(route, 401, { error: 'signIn' }));

  await page.route('**/api/**', (route) => json(route, 200, {}));
  await page.route('**/api/setup/claim', (route) => json(route, 200, { claimed: true }));
  await page.route('**/api/state', guard((route) => json(route, 200, { mode: 'local', sharing: false })));
  await page.route('**/api/settings', guard((route) => json(route, 200, SETTINGS)));
  await page.route('**/api/status', guard((route) => json(route, 200, { state: 'idle', progress: 0, message: '' })));
  await page.route('**/api/logs?*', guard((route) => {
    const q = new URL(route.request().url()).searchParams;
    asked.push(Object.fromEntries(q));
    const base = { sources: SOURCES, unit: q.get('unit'), error: complaint };
    if (q.has('after')) {
      const added = [line(next, `new line ${next}`)];
      next += 1;
      return json(route, 200, { ...base, lines: added, older: null, newer: added[0].cursor });
    }
    if (q.has('before')) {
      return json(route, 200, {
        ...base,
        lines: [1, 2, 3].map((i) => line(i, `old line ${i}`)),
        older: null,
        newer: line(3, '').cursor,
      });
    }
    const lines = [4, 5, 6, 7, 8, 9].map((i) =>
      line(i, i === 8 ? 'password=[hidden] given' : `line ${i}`, i === 7 ? { priority: 3, unit: 'nginx.service', pid: 7 } : {}),
    );
    return json(route, 200, { ...base, lines, older: lines[0].cursor, newer: lines.at(-1).cursor });
  }));
  await page.route('**/api/logs/download?*', guard((route) =>
    route.fulfill({
      status: 200,
      contentType: 'text/plain; charset=utf-8',
      headers: { 'Content-Disposition': 'attachment; filename="losos-lososd-20261029-1200.log"' },
      body: '2026-10-29 12:00:04 lososd.service[42] info: line 4\n',
    })));

  await page.addInitScript((token) => {
    window.sessionStorage.setItem('losos-token', token);
    window.localStorage.setItem('losos-tour', 'seen');
    window.__violations = [];
    document.addEventListener('securitypolicyviolation', (e) =>
      window.__violations.push(`${e.violatedDirective} ${e.blockedURI}`),
    );
  }, TOKEN);
  await page.goto(origin + path, { waitUntil: 'networkidle' });
  return { page, context, errors, asked };
}

const violations = (page) => page.evaluate(() => [...new Set(window.__violations)].sort());
const nav = (page) => page.getByRole('navigation', { name: 'Sections', exact: true });
const lines = (page) => page.locator('[data-slot="log-line"]');
// The pane writes its filter with history.replaceState; waitForURL also
// waits for a load event that a same-document change can leave pending.
const addressHas = (page, part) =>
  page.waitForFunction((p) => decodeURIComponent(location.search).includes(p), part, { timeout: 10000 });

async function baseline() {
  const { page, context } = await open({ path: '/settings/security' });
  await page.waitForTimeout(300);
  const seen = await violations(page);
  await context.close();
  return seen;
}
const BASELINE = await baseline();

console.log('admin-ui logs checks');

await check('the sidebar lists Logs right after History, and search finds it', async () => {
  const { page, context } = await open({ path: '/settings/security' });
  const labels = await nav(page).getByRole('link').allInnerTexts();
  const at = labels.findIndex((l) => l.trim() === 'History');
  assert.equal(labels[at + 1]?.trim(), 'Logs', `after History: ${labels[at + 1]}`);
  await nav(page).getByRole('button', { name: /^Search or jump to/ }).click();
  const box = page.getByRole('combobox', { name: /^Search pages/ });
  for (const word of ['journal', 'kernel', 'error']) {
    await box.fill(word);
    await page.getByRole('option', { name: /^Logs/ }).waitFor({ timeout: 2000 });
  }
  await page.getByRole('option', { name: /^Logs/ }).click();
  await page.waitForURL(origin + '/settings/logs');
  await page.getByRole('heading', { name: 'Logs', level: 1 }).waitFor();
  await context.close();
});

await check('the lines render oldest first, with severity, unit and the day', async () => {
  const { page, context, errors, asked } = await open();
  await lines(page).first().waitFor();
  assert.equal(await lines(page).count(), 6);
  const texts = await lines(page).allInnerTexts();
  assert.ok(texts[0].includes('line 4'), texts[0]);
  assert.ok(texts[5].includes('line 9'), texts[5]);
  assert.ok(texts[4].includes('password=[hidden] given'), texts[4]);
  const err = lines(page).nth(3);
  assert.equal(await err.getAttribute('data-priority'), '3');
  assert.equal(await err.locator('[data-slot="badge"]').innerText(), 'Error');
  assert.ok((await err.innerText()).includes('nginx'), await err.innerText());
  assert.equal(await err.locator('[title="nginx.service[7]"]').count(), 1);
  assert.equal(await page.locator('[data-slot="log-day"]').allInnerTexts().then((d) => d.join()), '2026-10-29');
  assert.deepEqual(asked[0], { unit: 'lososd', priority: '6', span: 'boot' });
  const options = await page.locator('#logs-unit option').allInnerTexts();
  assert.deepEqual(options, [
    'Control service (lososd)',
    'Updates and changes',
    'LosOS Login',
    'Web server',
    'LosOS Cloud',
    'Kernel',
    'All LosOS services',
    'The whole journal',
  ]);
  assert.deepEqual(errors, []);
  assert.deepEqual(await violations(page), BASELINE);
  await context.close();
});

await check('a filter asks again and puts itself on the address', async () => {
  const { page, context, asked } = await open();
  await lines(page).first().waitFor();
  await page.locator('#logs-priority').selectOption('3');
  await addressHas(page, 'priority=3');
  await page.locator('#logs-unit').selectOption('cloud');
  await addressHas(page, 'unit=cloud');
  await page.locator('#logs-q').fill('disk full');
  await page.getByRole('button', { name: 'Show', exact: true }).click();
  await addressHas(page, 'q=disk+full');
  const want = { unit: 'cloud', priority: '3', span: 'boot', q: 'disk full' };
  for (let i = 0; i < 100 && asked.at(-1)?.q === undefined; i += 1) await page.waitForTimeout(50);
  assert.deepEqual(asked.at(-1), want);
  await context.close();
});

await check("a problem report's link opens on its unit", async () => {
  const { page, context, asked } = await open({ path: '/settings/logs?unit=losos-cloud.service&span=hour' });
  await lines(page).first().waitFor();
  assert.deepEqual(asked[0], { unit: 'losos-cloud.service', priority: '6', span: 'hour' });
  assert.equal(await page.locator('#logs-unit').inputValue(), 'losos-cloud.service');
  assert.equal(await page.locator('#logs-span').inputValue(), 'hour');
  await context.close();
});

await check('Follow adds what came after the newest line, and older lines come in above', async () => {
  const { page, context, asked } = await open();
  await lines(page).first().waitFor();
  await page.getByRole('button', { name: 'Follow', exact: true }).click();
  await page.locator('[data-slot="log-following"]').waitFor();
  await page.getByText('new line 10').waitFor({ timeout: 8000 });
  await page.getByText('new line 11').waitFor({ timeout: 8000 });
  const follows = asked.filter((a) => a.after !== undefined).map((a) => a.after);
  assert.deepEqual(follows.slice(0, 2), ['s=1;i=9', 's=1;i=a']);
  await page.getByRole('button', { name: 'Stop following', exact: true }).click();
  assert.equal(await page.locator('[data-slot="log-following"]').count(), 0);

  await page.getByRole('button', { name: 'Show older lines' }).click();
  await page.getByText('old line 1').waitFor();
  const texts = await lines(page).allInnerTexts();
  assert.ok(texts[0].includes('old line 1'), texts[0]);
  assert.equal(asked.find((a) => a.before !== undefined)?.before, 's=1;i=4');
  assert.equal(await page.getByRole('button', { name: 'Show older lines' }).count(), 0);
  await context.close();
});

await check('Download saves the file under the name the box gave it', async () => {
  const { page, context } = await open();
  await lines(page).first().waitFor();
  const [download] = await Promise.all([
    page.waitForEvent('download'),
    page.getByRole('button', { name: 'Download', exact: true }).click(),
  ]);
  assert.equal(download.suggestedFilename(), 'losos-lososd-20261029-1200.log');
  await context.close();
});

await check("the reader's complaint is shown", async () => {
  const { page, context } = await open({ complaint: 'No journal files were opened' });
  await page.getByText('The journal could not be read: No journal files were opened').waitFor();
  await context.close();
});

await browser.close();
await closeServer();
finish();
