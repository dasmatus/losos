/* Pins on the sidebar: apps, LosOS Cloud files, LosOS Git repositories
 * and the tracker's issues and projects the owner pins, kept on the box (backend/src/pins.rs).
 *
 * Same arrangement as the other files: the real dist/ bundle, the API
 * stubbed with page.route(), the stub enforcing the Bearer token. The stub
 * keeps the list the way lososd does, so a pin added in one check is there
 * on the next GET, and every PUT is recorded for the checks to read.
 */

import assert from './assert.mjs';
import { launch, runner, serve } from './harness.mjs';

const TOKEN = 'a'.repeat(64);
const { origin, close: closeServer } = await serve();
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

const PINNED = [
  { kind: 'tile', app: 'photos' },
  { kind: 'file', id: 42, path: '/Documents/plan.pdf', folder: false },
  { kind: 'repo', owner: 'notshared', name: 'site' },
  { kind: 'app', release: 'jellyfin' },
];

const APPS = {
  available: true,
  sharedAvailable: false,
  apps: [
    {
      release: 'jellyfin',
      kind: 'chart',
      title: null,
      runtime: 'podman',
      chart: { repo: 'jellyfin', name: 'jellyfin', version: '2.1.0' },
      runAs: 'notshared',
      values: {},
      valuesYaml: null,
      frontPort: 30001,
      appPort: 8096,
      phase: 'running',
      message: null,
      updatedAt: 0,
    },
  ],
};

const FOLDERS = {
  '/': [
    { name: 'Documents', path: '/Documents', id: 10, folder: true, size: 4096 },
    { name: 'Photos', path: '/Photos', id: 11, folder: true, size: 900000 },
    { name: 'notes.md', path: '/notes.md', id: 12, folder: false, size: 812 },
  ],
  '/Documents': [
    { name: 'Tax', path: '/Documents/Tax', id: 20, folder: true, size: 1000 },
    { name: 'plan.pdf', path: '/Documents/plan.pdf', id: 42, folder: false, size: 120000 },
    { name: 'budget.ods', path: '/Documents/budget.ods', id: 43, folder: false, size: 9000 },
  ],
};

const REPOS = {
  available: true,
  repos: [
    { owner: 'notshared', name: 'losos-config', private: true, description: "This box's NixOS configuration." },
    { owner: 'notshared', name: 'site', private: false, description: 'My web page' },
  ],
};

// LosOS Git's tracker, which the page asks as the person signed in, not
// through lososd: names for issue and project pins, and the issue search.
const ISSUES = [
  { key: 'WEB-12', title: 'Dark mode for the photo grid', state: { name: 'In progress', category: 'started' } },
  { key: 'WEB-7', title: 'Share links expire', state: { name: 'Todo', category: 'unstarted' } },
];
const PROJECTS = [{ key: 'WEB', name: 'Website' }];

const json = (route, status, body) =>
  route.fulfill({ status, contentType: 'application/json', body: JSON.stringify(body) });

async function open({
  path = '/',
  pins = PINNED,
  files = 'ready',
  viewport = { width: 1280, height: 900 },
  colorScheme = 'light',
  csp = null,
  tracker = false,
} = {}) {
  const page = await browser.newPage({ viewport, locale: 'en-US', colorScheme });
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  if (csp !== null) {
    await page.addInitScript(() => {
      window.__violations = [];
      document.addEventListener('securitypolicyviolation', (e) => window.__violations.push(e.violatedDirective));
    });
  }
  const authed = (route) => route.request().headers()['authorization'] === `Bearer ${TOKEN}`;
  const box = { pins: [...pins], puts: [], fileAsks: [], trackerAsks: [] };

  await page.route('**/api/**', (route) => json(route, 200, {}));
  await page.route('**/api/setup/claim', (route) => json(route, 200, { claimed: true }));
  await page.route('**/api/settings', (route) =>
    authed(route) ? json(route, 200, SETTINGS) : json(route, 401, { error: 'unauthorized' }),
  );
  await page.route('**/api/apps', (route) =>
    authed(route) ? json(route, 200, APPS) : json(route, 401, { error: 'unauthorized' }),
  );
  await page.route('**/api/pins', (route) => {
    if (!authed(route)) return json(route, 401, { error: 'unauthorized' });
    if (route.request().method() === 'PUT') {
      const body = JSON.parse(route.request().postData() ?? '{}');
      box.puts.push(body.pins);
      box.pins = body.pins;
    }
    return json(route, 200, { pins: box.pins, limit: 24 });
  });
  await page.route('**/api/cloud/files?**', (route) => {
    if (!authed(route)) return json(route, 401, { error: 'unauthorized' });
    const url = new URL(route.request().url());
    const at = url.searchParams.get('path') ?? '/';
    box.fileAsks.push(at);
    if (files === 'signIn') {
      return json(route, 200, { available: false, reason: 'signIn', path: at, query: null, entries: [] });
    }
    return json(route, 200, { available: true, path: at, query: null, entries: FOLDERS[at] ?? [], more: false });
  });
  await page.route('**/api/git/repos', (route) =>
    authed(route) ? json(route, 200, REPOS) : json(route, 401, { error: 'unauthorized' }),
  );
  // The palette's search for repositories carries the query.
  await page.route('**/api/git/repos?**', (route) =>
    authed(route) ? json(route, 200, REPOS) : json(route, 401, { error: 'unauthorized' }),
  );
  // The apps the Overview probes, answering as on a box that is set up.
  await page.route('**/status.php', (route) => json(route, 200, { installed: true }));
  await page.route('**/git/api/v1/version', (route) => route.fulfill({ status: 200, contentType: 'text/html', body: '' }));
  if (tracker) {
    await page.route('**/git/api/v1/tracker**', (route) => {
      const url = new URL(route.request().url());
      box.trackerAsks.push(url.pathname + url.search);
      if (url.pathname.endsWith('/tracker')) return json(route, 200, { assigned: 1, projects: PROJECTS });
      const keys = url.searchParams.get('key');
      const q = (url.searchParams.get('q') ?? '').toLowerCase();
      let issues = ISSUES;
      if (keys !== null) issues = issues.filter((i) => keys.split(',').includes(i.key));
      else if (q !== '') issues = issues.filter((i) => `${i.key} ${i.title}`.toLowerCase().includes(q));
      else issues = issues.slice(0, 1);
      return json(route, 200, { issues });
    });
  }
  await page.addInitScript((t) => window.sessionStorage.setItem('losos-token', t), TOKEN);
  await page.goto((csp ?? origin) + path, { waitUntil: 'networkidle' });
  return { page, errors, box };
}

const nav = (page) => page.getByRole('navigation', { name: 'Sections', exact: true });
const pinned = (page) => nav(page).getByRole('group', { name: 'Pinned' });

await check('the pins the box keeps sit under the sections, each opening in its own app', async () => {
  const { page, errors } = await open();
  const group = pinned(page);
  await group.waitFor();
  const links = group.getByRole('link');
  assert.deepEqual(await links.allInnerTexts(), ['Photos', 'plan.pdf', 'site', 'jellyfin']);
  const href = (name) => group.getByRole('link', { name, exact: true }).getAttribute('href');
  assert.equal(await href('Photos'), '/cloud/photos');
  assert.equal(await href('plan.pdf'), '/cloud/files?fileid=42');
  assert.equal(await href('site'), '/git/notshared/site');
  // An installed app opens on the port the box gave it, in a tab of its own.
  assert.equal(await href('jellyfin'), 'http://127.0.0.1:30001/');
  assert.equal(await group.getByRole('link', { name: 'jellyfin' }).getAttribute('target'), '_blank');
  assert.deepEqual(errors, []);
  await page.close();
});

await check('unpinning from the sidebar writes the list without it', async () => {
  const { page, box } = await open();
  await pinned(page).getByRole('link', { name: 'site' }).hover();
  await pinned(page).getByRole('button', { name: 'Unpin site' }).click();
  await pinned(page).getByRole('link', { name: 'site' }).waitFor({ state: 'detached' });
  assert.deepEqual(box.puts.at(-1), PINNED.filter((p) => p.kind !== 'repo'));
  await page.close();
});

await check('an Overview tile pins its app, and says so once it is pinned', async () => {
  const { page, box } = await open({ pins: [] });
  await pinned(page).getByRole('button', { name: 'Pin apps, files, repositories and issues here.' }).waitFor();
  const grid = page.getByRole('list', { name: /apps/i }).first();
  await grid.getByRole('link', { name: 'Code' }).hover();
  const pin = page.getByRole('button', { name: 'Pin Code' });
  await pin.click();
  await pinned(page).getByRole('link', { name: 'Code' }).waitFor();
  assert.deepEqual(box.puts.at(-1), [{ kind: 'tile', app: 'code' }]);
  const unpin = page.getByRole('main').getByRole('button', { name: 'Unpin Code' });
  assert.equal(await unpin.getAttribute('aria-pressed'), 'true');
  await page.close();
});

await check('the dialog pins a file from a folder of LosOS Cloud and a repository from LosOS Git', async () => {
  const { page, box } = await open({ pins: [{ kind: 'tile', app: 'code' }] });
  await pinned(page).getByRole('button', { name: 'Edit pins' }).click();
  const dialog = page.getByRole('dialog', { name: 'Pins' });
  await dialog.waitFor();
  await dialog.getByRole('tab', { name: 'Files' }).click();
  await dialog.getByRole('button', { name: 'Open Documents' }).click();
  await dialog.getByText('plan.pdf').waitFor();
  assert.deepEqual(box.fileAsks, ['/', '/Documents']);
  await dialog.getByRole('button', { name: 'Pin plan.pdf' }).click();
  await dialog.getByRole('button', { name: 'Unpin plan.pdf' }).first().waitFor();
  assert.deepEqual(box.puts.at(-1), [
    { kind: 'tile', app: 'code' },
    { kind: 'file', id: 42, path: '/Documents/plan.pdf', folder: false },
  ]);
  // Up one folder, and a folder pins as a folder.
  await dialog.getByRole('button', { name: 'Up one folder' }).click();
  await dialog.getByRole('button', { name: 'Pin Photos' }).click();
  await dialog.getByRole('button', { name: 'Unpin Photos' }).first().waitFor();
  assert.deepEqual(box.puts.at(-1).at(-1), { kind: 'file', id: 11, path: '/Photos', folder: true });

  await dialog.getByRole('tab', { name: 'Repositories' }).click();
  await dialog.getByRole('button', { name: 'Pin site' }).click();
  await dialog.getByRole('button', { name: 'Unpin site' }).first().waitFor();
  assert.deepEqual(box.puts.at(-1).at(-1), { kind: 'repo', owner: 'notshared', name: 'site' });

  // The repository moves to the top of the list, and the sidebar follows.
  await dialog.getByRole('button', { name: 'Move site up' }).click();
  await dialog.getByRole('button', { name: 'Move site up' }).click();
  await dialog.getByRole('button', { name: 'Move site up' }).click();
  await page.waitForFunction(() => {
    const group = document.querySelector('[data-pins]');
    return group?.querySelector('a')?.textContent === 'site';
  });
  assert.deepEqual(
    box.puts.at(-1).map((p) => p.kind),
    ['repo', 'tile', 'file', 'file'],
  );
  await dialog.getByRole('button', { name: 'Done' }).click();
  await dialog.waitFor({ state: 'hidden' });
  await page.close();
});

await check('the apps tab pins an app installed from the search', async () => {
  const { page, box } = await open({ pins: [] });
  await pinned(page).getByRole('button', { name: 'Edit pins' }).click();
  const dialog = page.getByRole('dialog', { name: 'Pins' });
  await dialog.getByRole('button', { name: 'Pin jellyfin' }).click();
  await pinned(page).getByRole('link', { name: 'jellyfin' }).waitFor();
  assert.deepEqual(box.puts.at(-1), [{ kind: 'app', release: 'jellyfin' }]);
  await page.close();
});

await check('without a key to LosOS Cloud the files tab says how to get one', async () => {
  const { page } = await open({ files: 'signIn' });
  await pinned(page).getByRole('button', { name: 'Edit pins' }).click();
  const dialog = page.getByRole('dialog', { name: 'Pins' });
  await dialog.getByRole('tab', { name: 'Files' }).click();
  await dialog.getByText(/once the owner's password is set again/i).waitFor();
  await page.close();
});

await check('the palette lists the pins first and finds one by name or by its folder', async () => {
  const { page } = await open();
  await pinned(page).waitFor();
  await page.keyboard.press('Control+Shift+P');
  const field = page.getByRole('combobox', { name: /^Search pages/ });
  await field.waitFor();
  const palette = page.locator('dialog[open]');
  const pins = palette.getByRole('group', { name: 'Pinned', exact: true }).getByRole('option');
  await pins.first().waitFor();
  assert.equal(await pins.count(), 4);
  assert.match(await palette.getByRole('group').first().getAttribute('aria-labelledby'), /-g-pins$/);
  await field.fill('plan');
  await pins.filter({ hasText: 'plan.pdf' }).waitFor();
  assert.equal(await pins.count(), 1);
  await field.fill('documents');
  await pins.filter({ hasText: 'plan.pdf' }).waitFor();
  assert.equal(await pins.count(), 1);
  await field.fill('zzz');
  await page.waitForTimeout(200);
  assert.equal(await pins.count(), 0);
  // The sidebar keeps every pin while the palette searches.
  assert.equal(await pinned(page).getByRole('link').count(), 4);
  await page.close();
});

const menu = (page) => page.locator('[data-slot="context-menu"]');

await check('right-clicking a file or a repository in the palette pins it, and the palette stays open', async () => {
  const { page, box } = await open({ pins: [] });
  await pinned(page).getByRole('button', { name: 'Pin apps, files, repositories and issues here.' }).waitFor();
  await page.keyboard.press('Control+Shift+P');
  const field = page.getByRole('combobox', { name: /^Search pages/ });
  await field.fill('notes');
  const palette = page.locator('dialog[open]');
  const file = palette.getByRole('option').filter({ hasText: 'notes.md' });
  await file.click({ button: 'right' });
  // Inside the palette's <dialog>, or the top layer would hide it.
  await palette.locator('[data-slot="context-menu"]').waitFor();
  await menu(page).getByRole('menuitem', { name: 'Pin to sidebar' }).click();
  await pinned(page).getByRole('link', { name: 'notes.md' }).waitFor();
  assert.deepEqual(box.puts.at(-1), [{ kind: 'file', id: 12, path: '/notes.md', folder: false }]);
  assert.equal(await field.isVisible(), true);

  await field.fill('site');
  const repo = palette
    .getByRole('group', { name: 'Repositories in LosOS Git', exact: true })
    .getByRole('option')
    .filter({ hasText: 'notshared/site' });
  await repo.click({ button: 'right' });
  await menu(page).getByRole('menuitem', { name: 'Pin to sidebar' }).click();
  await pinned(page).getByRole('link', { name: 'site' }).waitFor();
  assert.deepEqual(box.puts.at(-1).at(-1), { kind: 'repo', owner: 'notshared', name: 'site' });

  // A pinned one offers the way back, and Escape closes the menu, not the palette.
  await repo.click({ button: 'right' });
  await menu(page).getByRole('menuitem', { name: 'Unpin from sidebar' }).waitFor();
  await page.keyboard.press('Escape');
  await menu(page).waitFor({ state: 'detached' });
  assert.equal(await field.isVisible(), true);
  await page.close();
});

await check('right-clicking an Overview tile or a pin in the sidebar pins or unpins it', async () => {
  const { page, box } = await open();
  await pinned(page).waitFor();
  await page.getByRole('main').getByRole('link', { name: 'Code' }).click({ button: 'right' });
  await menu(page).getByRole('menuitem', { name: 'Pin to sidebar' }).click();
  await pinned(page).getByRole('link', { name: 'Code' }).waitFor();
  assert.deepEqual(box.puts.at(-1), [...PINNED, { kind: 'tile', app: 'code' }]);

  await pinned(page).getByRole('link', { name: 'plan.pdf' }).click({ button: 'right' });
  assert.deepEqual(await menu(page).getByRole('menuitem').allInnerTexts(), ['Open in new tab', 'Unpin from sidebar']);
  await menu(page).getByRole('menuitem', { name: 'Unpin from sidebar' }).click();
  await pinned(page).getByRole('link', { name: 'plan.pdf' }).waitFor({ state: 'detached' });
  assert.equal(box.puts.at(-1).some((pin) => pin.kind === 'file'), false);
  await page.close();
});

await check('a full sidebar greys out Pin to sidebar in the menu', async () => {
  const many = Array.from({ length: 24 }, (_, i) => ({ kind: 'file', id: 1000 + i, path: `/f${i}.txt`, folder: false }));
  const { page, box } = await open({ pins: many });
  await pinned(page).waitFor();
  await page.getByRole('main').getByRole('link', { name: 'Code' }).click({ button: 'right' });
  const item = menu(page).getByRole('menuitem', { name: 'Pin to sidebar' });
  assert.equal(await item.getAttribute('aria-disabled'), 'true');
  assert.equal(box.puts.length, 0);
  await page.close();
});

await check('on the rail each pin is an icon that names itself', async () => {
  const { page } = await open();
  await page.keyboard.press('Control+b');
  await page.waitForTimeout(300);
  await pinned(page).getByRole('link', { name: 'site' }).hover();
  await page.locator('[data-slot="tooltip-content"]').filter({ hasText: /^site$/ }).waitFor({ timeout: 2000 });
  await page.close();
});

await check('an issue and a project pin take their names from LosOS Git and open in the tracker', async () => {
  const { page, errors, box } = await open({
    pins: [
      { kind: 'issue', key: 'WEB-12' },
      { kind: 'project', key: 'WEB' },
    ],
    tracker: true,
  });
  const group = pinned(page);
  await group.getByRole('link', { name: 'Dark mode for the photo grid' }).waitFor();
  assert.deepEqual(await group.getByRole('link').allInnerTexts(), ['Dark mode for the photo grid', 'Website']);
  const href = (name) => group.getByRole('link', { name, exact: true }).getAttribute('href');
  assert.equal(await href('Dark mode for the photo grid'), '/git/issues/WEB-12');
  assert.equal(await href('Website'), '/git/issues/p/WEB');
  assert.ok(box.trackerAsks.includes('/git/api/v1/tracker/issues?key=WEB-12'), box.trackerAsks.join(' '));
  assert.deepEqual(errors, []);
  await page.close();
});

await check('a pin to an issue LosOS Git does not answer for keeps its key', async () => {
  const { page } = await open({ pins: [{ kind: 'issue', key: 'OPS-3' }] });
  await pinned(page).getByRole('link', { name: 'OPS-3' }).waitFor();
  assert.equal(await pinned(page).getByRole('link', { name: 'OPS-3' }).getAttribute('href'), '/git/issues/OPS-3');
  await page.close();
});

await check("the dialog's Issues tab finds an issue by its words and pins it under its title", async () => {
  const { page, box } = await open({ pins: [], tracker: true });
  await pinned(page).getByRole('button', { name: 'Pin apps, files, repositories and issues here.' }).click();
  const dialog = page.getByRole('dialog', { name: 'Pins' });
  await dialog.getByRole('tab', { name: 'Issues' }).click();
  // Nothing typed: the issues given to the person.
  await dialog.getByText('Dark mode for the photo grid').waitFor();
  await dialog.getByRole('searchbox', { name: 'Find an issue by its words or key' }).fill('share');
  await dialog.getByText('Share links expire').waitFor();
  assert.equal(await dialog.getByText('Dark mode for the photo grid').count(), 0);
  await dialog.getByRole('button', { name: 'Pin Share links expire' }).click();
  await pinned(page).getByRole('link', { name: 'Share links expire' }).waitFor();
  assert.deepEqual(box.puts.at(-1), [{ kind: 'issue', key: 'WEB-7' }]);
  await page.close();
});

await check('the palette offers the issues given to the person, finds one by key, and pins it from its menu', async () => {
  const { page, box } = await open({ pins: [], tracker: true });
  await pinned(page).getByRole('button', { name: 'Pin apps, files, repositories and issues here.' }).waitFor();
  await page.keyboard.press('Control+Shift+P');
  const field = page.getByRole('combobox', { name: /^Search pages/ });
  const palette = page.locator('dialog[open]');
  const issues = palette.getByRole('group', { name: 'Issues in LosOS Git', exact: true }).getByRole('option');
  await issues.filter({ hasText: 'Dark mode for the photo grid' }).waitFor();
  assert.ok(box.trackerAsks.some((a) => a.includes('assignee=me')), box.trackerAsks.join(' '));
  await field.fill('WEB-7');
  const hit = issues.filter({ hasText: 'Share links expire' });
  await hit.waitFor();
  assert.match(await hit.innerText(), /WEB-7 · Todo/);
  await hit.click({ button: 'right' });
  await menu(page).getByRole('menuitem', { name: 'Pin to sidebar' }).click();
  await pinned(page).getByRole('link', { name: 'Share links expire' }).waitFor();
  assert.deepEqual(box.puts.at(-1), [{ kind: 'issue', key: 'WEB-7' }]);
  await hit.click();
  await page.waitForURL(origin + '/git/issues/WEB-7');
  await page.close();
});

await check('without LosOS Git the palette shows no issues group at all', async () => {
  const { page } = await open({ pins: [] });
  await page.route('**/git/api/v1/tracker**', (route) => json(route, 404, { error: 'not found' }));
  await page.keyboard.press('Control+Shift+P');
  const field = page.getByRole('combobox', { name: /^Search pages/ });
  await field.fill('storage');
  const palette = page.locator('dialog[open]');
  await palette.getByRole('option').first().waitFor();
  await page.waitForTimeout(400);
  assert.equal(await palette.getByRole('group', { name: 'Issues in LosOS Git', exact: true }).count(), 0);
  await page.close();
});

await check('under the appliance CSP the pins, the tile button, the dialog and the right-click menu add no violation', async () => {
  const { origin: strict, close } = await serve({ csp: true });
  try {
    const { page, errors } = await open({ csp: strict });
    await pinned(page).waitFor();
    const atLoad = await page.evaluate(() => window.__violations.length);
    await page.getByRole('link', { name: 'Code' }).first().hover();
    await page.getByRole('button', { name: 'Pin Code' }).click();
    await pinned(page).getByRole('button', { name: 'Edit pins' }).click();
    const dialog = page.getByRole('dialog', { name: 'Pins' });
    await dialog.getByRole('tab', { name: 'Files' }).click();
    await dialog.getByRole('button', { name: 'Open Documents' }).waitFor();
    await dialog.getByRole('tab', { name: 'Repositories' }).click();
    await dialog.getByText('losos-config').waitFor();
    await dialog.getByRole('button', { name: 'Done' }).click();
    await pinned(page).getByRole('link', { name: 'site' }).click({ button: 'right' });
    await page.locator('[data-slot="context-menu"]').waitFor();
    await page.keyboard.press('Escape');
    const after = await page.evaluate(() => window.__violations);
    assert.equal(after.length, atLoad, `pins added CSP violations: ${after.slice(atLoad).join(', ')}`);
    assert.deepEqual(errors, []);
    await page.close();
  } finally {
    await close();
  }
});

await browser.close();
await closeServer();
finish();
