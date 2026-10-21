/* Documents (src/screens/Office.tsx) and the Office editor
 * (src/cloud/office/editor.tsx) inside the shell.
 *
 * What is checked: Documents is in the sidebar, after Apps, only on a box
 * whose /office/api/files answers JSON (a box with no Office folder answers
 * with the page itself); the list sits under the shell's own pane header in
 * a card, with each document's folder and a Read only badge; an empty
 * folder shows the empty state and a failed list a retry; a document opens
 * under the shell's top bar with no sidebar, framed from /browser/ with the
 * token, the language and the LosOS tokens as Collabora's css_variables;
 * Back returns to the list; and none of it adds to the
 * Content-Security-Policy violations a plain load logs.
 *
 * Same arrangement as the other files: the real dist/ bundle under the real
 * policy, the routes stubbed with page.route(). */

import assert from 'node:assert';
import { launch, runner, serve } from './harness.mjs';

const { origin, close: closeServer } = await serve({ csp: true });
const browser = await launch();
const { check, finish } = runner();

const SECTIONS = ['storage', 'backup', 'apps', 'market', 'network', 'look', 'lab', 'access', 'account'];
const FILES = [
  { id: '54726970206275646765742e6f6473', name: 'Trip budget.ods', folder: '', size: 10240, modified: 1792580040000, writable: true },
  { id: '5472697020706c616e2e6f6474', name: 'Trip plan.odt', folder: '', size: 11264, modified: 1792580040000, writable: true },
  { id: '576f726b2f4d656574696e67', name: 'Meeting notes.odt', folder: 'Work', size: 9830, modified: 1792580040000, writable: false },
];

const json = (route, status, body) =>
  route.fulfill({ status, contentType: 'application/json', body: JSON.stringify(body) });

/* A box with an administrator signed in through LosOS Login. `files` is
 * what GET /office/api/files answers: a list, null for a box with no Office
 * folder (the page itself), or a status number. */
async function open({ path = '/office/', files = FILES, colorScheme = 'dark' } = {}) {
  const context = await browser.newContext({ viewport: { width: 1280, height: 900 }, locale: 'en-US', colorScheme });
  const page = await context.newPage();
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  const opened = [];

  await page.route('**/api/**', (route) => json(route, 200, {}));
  await page.route('**/api/setup/claim', (route) => json(route, 200, { claimed: true }));
  await page.route('**/api/me', (route) =>
    json(route, 200, {
      name: 'matus',
      displayName: 'Matus',
      groups: ['losos-admins', 'losos-owner'],
      admin: true,
      spareKey: false,
      sections: SECTIONS,
      standalone: true,
    }),
  );
  await page.route('**/api/state', (route) => json(route, 200, { mode: 'local', sharing: false }));
  await page.route('**/api/status', (route) => json(route, 200, { state: 'idle', progress: 0, message: '' }));
  await page.route('**/office/api/files', (route) => {
    if (files === null) {
      return route.fulfill({ status: 200, contentType: 'text/html', body: '<!doctype html><title>LosOS</title>' });
    }
    if (typeof files === 'number') return json(route, files, { error: 'no' });
    return json(route, 200, { files });
  });
  await page.route('**/office/api/open', (route) => {
    const { fileId } = JSON.parse(route.request().postData() ?? '{}');
    opened.push(fileId);
    const file = FILES.find((f) => f.id === fileId);
    return json(route, 200, {
      url: '/browser/abc/cool.html?WOPISrc=http%3A%2F%2F127.0.0.1%3A8093%2Fwopi%2Ffiles%2Fx',
      accessToken: 'token-1',
      accessTokenTtl: 1792616040000,
      writable: file.writable,
      name: file.name,
      https: false,
    });
  });
  // The editor itself is coolwsd's page; here a blank one in its place.
  await page.route('**/browser/**', (route) =>
    route.fulfill({ status: 200, contentType: 'text/html', body: '<!doctype html><title>editor</title>' }),
  );

  await page.addInitScript(() => {
    window.localStorage.setItem('losos-tour', 'seen');
    window.__violations = [];
    document.addEventListener('securitypolicyviolation', (e) =>
      window.__violations.push(`${e.violatedDirective} ${e.blockedURI}`),
    );
  });
  await page.goto(origin + path, { waitUntil: 'networkidle' });
  return { page, context, errors, opened };
}

const violations = (page) => page.evaluate(() => [...new Set(window.__violations)].sort());
const nav = (page) => page.getByRole('navigation', { name: 'Sections', exact: true });
const labels = async (page) => (await nav(page).getByRole('link').allInnerTexts()).map((l) => l.trim());

async function baseline() {
  const { page, context } = await open({ path: '/office/', files: null });
  await page.waitForTimeout(300);
  const seen = await violations(page);
  await context.close();
  return seen;
}
const BASELINE = await baseline();

console.log('admin-ui documents checks');

await check('a box with no Office folder has no Documents in the sidebar', async () => {
  const { page, context, errors } = await open({ path: '/office/', files: null });
  await nav(page).getByRole('link', { name: 'Overview', exact: true }).waitFor();
  await page.waitForTimeout(300);
  assert.ok(!(await labels(page)).includes('Documents'), (await labels(page)).join(' | '));
  assert.deepEqual(errors, []);
  await context.close();
});

await check('a box with an Office folder lists Documents after Apps, and it opens the list', async () => {
  const { page, context, errors } = await open({ path: '/office/' });
  await nav(page).getByRole('link', { name: 'Documents', exact: true }).waitFor({ timeout: 3000 });
  const all = await labels(page);
  assert.equal(all[all.indexOf('Apps') + 1], 'Documents', all.join(' | '));
  await nav(page).getByRole('link', { name: 'Documents', exact: true }).click();
  await page.getByRole('heading', { level: 1, name: 'Documents' }).waitFor();
  assert.equal(new URL(page.url()).pathname, '/office/');
  assert.equal(
    await nav(page).getByRole('link', { name: 'Documents', exact: true }).getAttribute('aria-current'),
    'page',
  );
  assert.deepEqual(errors, []);
  await context.close();
});

await check('the list shows each document, its folder and whether it is read only, in a card', async () => {
  const { page, context, errors } = await open();
  const table = page.locator('[data-slot="card"] table');
  await table.waitFor();
  const rows = table.locator('tbody tr');
  assert.equal(await rows.count(), 3);
  assert.match(await rows.nth(0).innerText(), /Trip budget\.ods/);
  assert.match(await rows.nth(2).innerText(), /Meeting notes\.odt\s+Work\s+Read only/);
  assert.equal(await rows.nth(1).getByRole('link').getAttribute('href'), `#file=${FILES[1].id}`);
  assert.equal(await page.locator('main[aria-busy="true"], [aria-busy="true"]').count(), 0);
  assert.deepEqual(errors, []);
  await context.close();
});

await check('an empty folder says so, and a failed list offers to try again', async () => {
  let { page, context } = await open({ files: [] });
  await page.getByText('No documents yet').waitFor();
  await context.close();
  ({ page, context } = await open({ files: 503 }));
  await page.getByText('Office is still starting. Try again in a minute.').waitFor();
  await page.getByRole('button', { name: 'Try again' }).waitFor();
  await context.close();
});

await check('a document opens under the top bar, without the sidebar, in the LosOS look', async () => {
  const { page, context, errors, opened } = await open();
  await page.getByRole('link', { name: /Trip plan\.odt/ }).click();
  const frame = page.locator('main iframe');
  await frame.waitFor();
  assert.deepEqual(opened, [FILES[1].id]);
  assert.equal(new URL(page.url()).hash, `#file=${FILES[1].id}`);
  // The shell's top bar stays; the sidebar gives the editor its width.
  await page.getByTestId('signed-in-as').waitFor();
  assert.equal(await nav(page).count(), 0);
  await page.getByRole('heading', { level: 1, name: 'Trip plan.odt' }).waitFor();
  const box = await frame.boundingBox();
  assert.ok(box.width >= 1270, `the editor is ${box.width} px wide`);

  const src = new URL(await frame.getAttribute('src'), origin);
  assert.equal(src.pathname, '/browser/abc/cool.html');
  assert.equal(src.searchParams.get('access_token'), 'token-1');
  assert.equal(src.searchParams.get('lang'), 'en-GB');
  const look = Object.fromEntries(
    src.searchParams.get('css_variables').split(';').map((pair) => pair.split('=')),
  );
  const tokens = await page.evaluate(() => {
    const s = getComputedStyle(document.documentElement);
    return Object.fromEntries(['--surface', '--ink', '--accent', '--line'].map((n) => [n, s.getPropertyValue(n).trim()]));
  });
  assert.equal(look['--color-main-background'], tokens['--surface']);
  assert.equal(look['--color-main-text'], tokens['--ink']);
  assert.equal(look['--color-primary'], tokens['--accent']);
  assert.equal(look['--color-border'], tokens['--line']);
  // Colours only, as coolwsd takes them.
  assert.ok(Object.values(look).every((v) => /^#[0-9a-f]{3,8}$/i.test(v)), JSON.stringify(look));

  // The editor's own light or dark look follows the page's.
  // Dark here: the editor opens dark, in the dark tokens.
  assert.equal(await page.evaluate(() => localStorage.getItem('darkTheme')), 'true');
  assert.equal(look['--color-main-background'], '#141b22');

  await page.getByRole('button', { name: 'Back to files' }).click();
  await page.getByRole('heading', { level: 1, name: 'Documents' }).waitFor();
  await nav(page).waitFor();
  assert.deepEqual(errors, []);
  await context.close();
});

await check('in the light theme the editor opens light, in the light tokens', async () => {
  const { page, context } = await open({ path: `/office/#file=${FILES[1].id}`, colorScheme: 'light' });
  const frame = page.locator('main iframe');
  await frame.waitFor();
  const src = new URL(await frame.getAttribute('src'), origin);
  const surface = await page.evaluate(() => getComputedStyle(document.documentElement).getPropertyValue('--surface').trim());
  assert.match(surface, /^#fff(fff)?$/i, `the light surface is ${surface}`);
  assert.ok(src.searchParams.get('css_variables').split(';').includes(`--color-main-background=${surface}`));
  assert.equal(await page.evaluate(() => localStorage.getItem('darkTheme')), 'false');
  await context.close();
});

await check('a read-only document says so in the editor bar', async () => {
  const { page, context } = await open({ path: `/office/#file=${FILES[2].id}` });
  await page.locator('main iframe').waitFor();
  await page.getByText('Read only', { exact: true }).waitFor();
  await context.close();
});

await check('Documents and the editor add nothing to the policy violations a plain load logs', async () => {
  const { page, context } = await open();
  await page.getByRole('link', { name: /Trip plan\.odt/ }).click();
  await page.locator('main iframe').waitFor();
  await page.waitForTimeout(300);
  assert.deepEqual(await violations(page), BASELINE);
  await context.close();
});

await browser.close();
await closeServer();
finish();
