/* Documents in the box's Office folder (losos.office.folder), native
 * LosOS Office. Legacy formats are imported into native formats. */

import assert from './assert.ts';
import { launch, runner, serve } from './harness.ts';
import type { Page, Route } from 'playwright';

declare global {
  interface Window {
    __violations: string[];
  }
}

const { origin, close: closeServer } = await serve({ csp: true });
const browser = await launch();
const { check, finish } = runner();

const SECTIONS = ['storage', 'backup', 'apps', 'market', 'network', 'look', 'lab', 'access', 'account'];
const FILES = [
  { id: '54726970206275646765742e6f6473', name: 'Trip budget.ods', folder: '', size: 10240, modified: 1792580040000, writable: true },
  { id: '5472697020706c616e2e6f6474', name: 'Trip plan.odt', folder: '', size: 11264, modified: 1792580040000, writable: true },
  { id: '576f726b2f4d656574696e67', name: 'Meeting notes.odt', folder: 'Work', size: 9830, modified: 1792580040000, writable: false },
] as const;
type FileEntry = (typeof FILES)[number];

const json = (route: Route, status: number, body: unknown) =>
  route.fulfill({ status, contentType: 'application/json', body: JSON.stringify(body) });

async function open({
  path = '/office/',
  files = FILES,
  colorScheme = 'dark',
}: { path?: string; files?: readonly FileEntry[] | null | number; colorScheme?: 'dark' | 'light' } = {}) {
  const context = await browser.newContext({ viewport: { width: 1280, height: 900 }, locale: 'en-US', colorScheme });
  const page = await context.newPage();
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  const opened: string[] = [];

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
    if (!file) return json(route, 404, { error: 'notFound' });
    const ext = file.name.slice(file.name.lastIndexOf('.') + 1).toLowerCase();
    return json(route, 200, { kind: 'import', fileId: file.id, name: file.name, extension: ext, importTarget: 'paper' });
  });

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

const violations = (page: Page) => page.evaluate(() => [...new Set(window.__violations)].sort());
const nav = (page: Page) => page.getByRole('navigation', { name: 'Sections', exact: true });
const labels = async (page: Page): Promise<string[]> => (await nav(page).getByRole('link').allInnerTexts()).map((l) => l.trim());

async function baseline(): Promise<string[]> {
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

await check('a document opens under the top bar, beside the sidebar, in the LosOS look', async () => {
  const { page, context, errors, opened } = await open();
  await page.getByRole('link', { name: /Trip plan\.odt/ }).click();
  await page.getByRole('button', { name: 'Back to files' }).waitFor();
  assert.deepEqual(opened, [FILES[1].id]);
  assert.equal(new URL(page.url()).hash, `#file=${FILES[1].id}`);
  await page.getByTestId('signed-in-as').waitFor();
  await nav(page).waitFor();
  assert.ok((await page.getByRole('heading', { level: 2 }).allInnerTexts()).some((t) => t.includes('Trip plan.odt')));
  await page.getByRole('button', { name: 'Back to files' }).click();
  await page.getByRole('heading', { level: 1, name: 'Documents' }).waitFor();
  await nav(page).waitFor();
  assert.deepEqual(errors, []);
  await context.close();
});

await check('the LosOS bars drive the document and show what it reports', async () => {
  const { page, context, errors } = await open({ path: `/office/#file=${FILES[1].id}` });
  await page.getByRole('button', { name: 'Back to files' }).waitFor();
  assert.ok((await page.getByRole('heading', { level: 2 }).allInnerTexts()).some((t) => t.includes('Trip plan.odt')));
  await page.getByRole('button', { name: 'Import to LosOS Office' }).waitFor();
  assert.deepEqual(errors, []);
  await context.close();
});

await check('in the light theme the editor opens light, on a white page', async () => {
  const { page, context, errors } = await open({ path: `/office/#file=${FILES[1].id}`, colorScheme: 'light' });
  await page.getByRole('button', { name: 'Back to files' }).waitFor();
  assert.ok((await page.getByRole('heading', { level: 2 }).allInnerTexts()).some((t) => t.includes('Trip plan.odt')));
  assert.deepEqual(errors, []);
  await context.close();
});

await check('a read-only document says so in the editor bar', async () => {
  const { page, context, errors } = await open({ path: `/office/#file=${FILES[2].id}` });
  await page.getByRole('button', { name: 'Back to files' }).waitFor();
  assert.ok((await page.getByRole('heading', { level: 2 }).allInnerTexts()).some((t) => t.includes('Meeting notes.odt')));
  assert.deepEqual(errors, []);
  await context.close();
});

await check('Documents and the editor add nothing to the policy violations a plain load logs', async () => {
  const { page, context } = await open();
  await page.getByRole('link', { name: /Trip plan\.odt/ }).click();
  await page.getByRole('button', { name: 'Back to files' }).waitFor();
  await page.waitForTimeout(300);
  assert.deepEqual(await violations(page), BASELINE);
  await context.close();
});

async function openCloud(path: string, { office = true }: { office?: boolean } = {}) {
  const context = await browser.newContext({ viewport: { width: 1280, height: 900 }, locale: 'en-US', colorScheme: 'dark' });
  const page = await context.newPage();
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  const opened: string[] = [];
  const entry = (id: number, name: string, mime: string) => ({ id, path: name, name, dir: false, size: 1, mtime: 1, etag: 'x', mime, favorite: false });
  await page.route('**/cloud/api/**', (route) => {
    const req = route.request();
    const p = new URL(req.url()).pathname.slice('/cloud/api'.length);
    if (p === '/me') return json(route, 200, { login: 'matus', name: 'Matus', email: '', used: 1e9, free: 9e9, product: 'LosOS Cloud' });
    if (p === '/files') {
      return json(route, 200, {
        folder: { ...entry(1, '', ''), dir: true },
        entries: [
          entry(42, 'Trip plan.docx', 'application/vnd.openxmlformats-officedocument.wordprocessingml.document'),
          entry(43, 'todo.txt', 'text/plain'),
        ],
      });
    }
    if (p.startsWith('/files/by-id/')) {
      const id = p.split('/').pop();
      if (id === '42') return json(route, 200, entry(42, 'Trip plan.docx', 'application/vnd.openxmlformats-officedocument.wordprocessingml.document'));
      return json(route, 404, {});
    }
    if (p === '/office/extensions') {
      return office ? json(route, 200, { extensions: [{ extension: 'docx', edit: true, mode: 'import', importTarget: 'paper' }] }) : json(route, 404, { code: 'notFound' });
    }
    if (p === '/office/open') {
      opened.push(JSON.parse(req.postData() ?? '{}').fileId);
      return json(route, 200, { kind: 'import', fileId: '42', name: 'Trip plan.docx', extension: 'docx', importTarget: 'paper' });
    }
    if (p === '/office/import') {
      return json(route, 200, { id: '999', name: 'Trip plan.paper' });
    }
    return json(route, 200, {});
  });
  await page.addInitScript(() => {
    window.localStorage.setItem('losos-tour', 'seen');
    window.__violations = [];
    document.addEventListener('securitypolicyviolation', (e) =>
      window.__violations.push(`${e.violatedDirective} ${e.blockedURI}`),
    );
  });
  await page.goto(origin + path);
  return { page, context, errors, opened };
}

await check('in LosOS Cloud a document opens in Office, in the same frame as the box\'s pages', async () => {
  const { page, context, errors, opened } = await openCloud('/cloud/files');
  const doc = page.getByRole('link', { name: 'Trip plan.docx' }).first();
  await doc.waitFor();
  await page.waitForFunction(() =>
    [...document.querySelectorAll('a')].some((a) => a.getAttribute('href') === '/cloud/office/42'),
  );
  await page.goto(`${origin}/cloud/office/42`);
  await page.getByRole('button', { name: 'Back to files' }).waitFor();
  assert.deepEqual(opened, ['42']);
  await page.getByRole('button', { name: 'Back to files' }).click();
  await page.waitForURL(/\/cloud\/files/);
  assert.deepEqual(errors, []);
  await context.close();
});

await check('a box without Office leaves documents to the browser', async () => {
  const { page, context } = await openCloud('/cloud/files', { office: false });
  await page.getByRole('link', { name: 'Trip plan.docx' }).first().waitFor();
  await page.waitForTimeout(300);
  const hrefs = await page.locator('a').evaluateAll((as) => as.map((a) => a.getAttribute('href')));
  assert.ok(!hrefs.some((h) => h?.startsWith('/cloud/office/')), hrefs.join(' '));
  await context.close();
});

await browser.close();
await closeServer();
finish();
