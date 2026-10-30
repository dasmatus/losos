/* The command palette (src/palette/): Ctrl/Cmd+Shift+P, a colon, or the
 * sidebar's search box, opens one list of pages, apps, files in LosOS Cloud, repositories in
 * LosOS Git, apps to install and actions.
 *
 * The real dist/ bundle under the admin page's CSP, with lososd's routes
 * stubbed: GET /api/cloud/files?q= and GET /api/git/repos[?q=] the way
 * backend/src/pins.rs answers them, and the catalogue search the Apps pane
 * uses. A palette that styles itself with an injected <style> would add a
 * violation to the baseline and fail here. */

import assert from './assert.mjs';
import { launch, runner, serve } from './harness.mjs';

const TOKEN = 'a'.repeat(64);
const { origin, close: closeServer } = await serve({ csp: true });
const browser = await launch();
const { check, finish } = runner();

const json = (route, status, body) =>
  route.fulfill({ status, contentType: 'application/json', body: JSON.stringify(body) });

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

const FILES = [
  { name: 'Taxes 2025', path: '/Documents/Taxes 2025', id: 412, folder: true, size: 0 },
  { name: 'tax-return-2025.pdf', path: '/Documents/Taxes 2025/tax-return-2025.pdf', id: 418, folder: false, size: 482113 },
];
/* What the two searches inside files answer: every hit whose text holds the
 * query, as the box marks it. */
const CLOUD_TEXT = [
  { path: 'Documents/Daňové priznanie 2025.odt', snippet: ['The ', 'tax', ' return is due in March'] },
  { path: 'notes.md', snippet: ['remember the ', 'tax', ' office hours'] },
];
const GIT_TEXT = [
  { owner: 'notshared', repo: 'tax-scripts', branch: 'main', path: 'src/sort.py', line: 12, snippet: ['def ', 'tax', '_year(receipt):'] },
];
const REPOS = [
  { owner: 'notshared', name: 'losos-config', private: true, description: "This box's configuration" },
  { owner: 'notshared', name: 'tax-scripts', private: true, description: 'Scripts that sort receipts' },
];
const CATALOGUE = {
  sources: ['Artifact Hub'],
  results: [
    {
      id: 'jellyfin/jellyfin',
      name: 'jellyfin',
      source: 'Artifact Hub',
      summary: 'A media server',
      version: '10.10.3',
      chart: { repo: 'https://jellyfin.github.io/jellyfin-helm', name: 'jellyfin', version: '2.1.0' },
    },
    // No chart: nothing to install, so the palette leaves it out.
    { id: 'x/listed-only', name: 'jellyfin-docs', source: 'Artifact Hub' },
  ],
};

/* `files` and `repos` are the two lookups' documents, or a status to answer
 * with (404: the box does not serve the route). */
async function open({
  path = '/',
  files = { available: true },
  repos = { available: true },
  cloudText = { available: true, partial: false },
  gitText = { available: true },
  viewport,
  cloudPage = false,
} = {}) {
  const page = await browser.newPage({ viewport: viewport ?? { width: 1280, height: 860 } });
  const errors = [];
  const csp = [];
  const asked = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  page.on('console', (m) => {
    if (/Content Security Policy/.test(m.text())) csp.push(m.text());
  });
  await page.route('**/api/**', (route) => json(route, 200, {}));
  await page.route('**/api/setup/claim', (route) => json(route, 200, { claimed: true }));
  await page.route('**/api/settings', (route) => json(route, 200, SETTINGS));
  await page.route('**/api/state', (route) => json(route, 200, { mode: 'local', sharing: false }));
  await page.route('**/api/status', (route) => json(route, 200, { state: 'idle', progress: 0, message: '' }));
  await page.route('**/api/apps', (route) => json(route, 200, { available: true, sharedAvailable: false, apps: [] }));
  await page.route('**/api/apps/search**', (route) => {
    const q = new URL(route.request().url()).searchParams.get('q') ?? '';
    return json(route, 200, { ...CATALOGUE, results: CATALOGUE.results.filter((r) => r.name.includes(q)) });
  });
  await page.route('**/api/apps/chart**', (route) =>
    json(route, 200, { chart: CATALOGUE.results[0].chart, release: 'jellyfin', valuesYaml: 'replicaCount: 1\n', values: { replicaCount: 1 } }),
  );
  await page.route('**/api/cloud/files**', (route) => {
    const q = new URL(route.request().url()).searchParams.get('q') ?? '';
    asked.push(['files', q]);
    if (typeof files === 'number') return json(route, files, { error: 'not found' });
    if (files.available === false) return json(route, 200, files);
    const hits = FILES.filter((f) => f.name.toLowerCase().includes(q.toLowerCase()));
    return json(route, 200, { available: true, path: '/', query: q, entries: hits, more: false });
  });
  await page.route('**/api/git/repos**', (route) => {
    const q = new URL(route.request().url()).searchParams.get('q') ?? '';
    asked.push(['repos', q]);
    if (typeof repos === 'number') return json(route, repos, { error: 'not found' });
    if (repos.available === false) return json(route, 200, repos);
    return json(route, 200, { available: true, repos: REPOS.filter((r) => r.name.includes(q)) });
  });
  for (const [kind, route, answer, hits] of [
    ['cloudText', '**/api/cloud/text**', cloudText, CLOUD_TEXT],
    ['gitText', '**/api/git/text**', gitText, GIT_TEXT],
  ]) {
    await page.route(route, (r) => {
      const q = (new URL(r.request().url()).searchParams.get('q') ?? '').toLowerCase();
      asked.push([kind, q]);
      if (answer.available === false) return json(r, 200, answer);
      return json(r, 200, { ...answer, hits: hits.filter((h) => h.snippet.join('').toLowerCase().includes(q)) });
    });
  }
  // The two apps' own pages, so a jump there has somewhere to land. A regex,
  // because `**/cloud/**` would also catch /api/cloud/files.
  for (const app of cloudPage ? ['git'] : ['cloud', 'git']) {
    await page.route(new RegExp(`^[a-z]+://[^/]+/${app}(/|\\?|$)`), (route) =>
      route.fulfill({ status: 200, contentType: 'text/html', body: `<title>${app}</title>` }),
    );
  }
  // LosOS Cloud's own page and API, for a check that opens it for real.
  if (cloudPage) {
    await page.route('**/cloud/api/**', (route) => {
      const p = new URL(route.request().url()).pathname;
      if (p === '/cloud/api/me') return json(route, 200, { login: 'notshared', name: 'Matus', email: '', used: 1, free: 99, product: 'LosOS Cloud' });
      if (p === '/cloud/api/files') {
        return json(route, 200, {
          folder: { id: 1, path: '', name: '', dir: true, size: 0, mtime: 0, etag: 'root', mime: '', favorite: false },
          entries: [{ id: 2, path: 'notes.txt', name: 'notes.txt', dir: false, size: 5, mtime: 1792580040, etag: 'a', mime: 'text/plain', favorite: false }],
        });
      }
      if (p.startsWith('/cloud/api/files/by-id/')) {
        return json(route, 200, { id: 418, path: 'Documents/Taxes 2025/tax-return-2025.pdf', name: 'tax-return-2025.pdf', dir: false, size: 1, mtime: 0, etag: 'b', mime: 'application/pdf', favorite: false });
      }
      return json(route, 404, { code: 'notFound' });
    });
  }
  await page.addInitScript((t) => {
    window.sessionStorage.setItem('losos-token', t);
    window.localStorage.setItem('losos-apps-seen', JSON.stringify({ files: 'present', code: 'present' }));
  }, TOKEN);
  await page.goto(origin + path, { waitUntil: 'networkidle' });
  return { page, errors, csp, asked };
}

/** Poll until `fn` resolves to something truthy, or fail with `what`. */
async function until(fn, what, timeout = 3000) {
  const end = Date.now() + timeout;
  for (;;) {
    if (await fn()) return;
    if (Date.now() > end) throw new Error(what);
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
}

const box = (page) => page.getByRole('combobox', { name: /^Search pages/ });
const option = (page, name) => page.getByRole('option', { name });
const group = (page, name) => page.getByRole('group', { name, exact: true });

console.log('admin-ui command palette checks');

/* What a page load logs before the palette opens: the bar it must not add to. */
const { page: plain, csp: BASELINE } = await open();
await plain.waitForTimeout(300);
const baseline = BASELINE.length;
await plain.close();

await check('Ctrl+Shift+P opens it with the input focused; Escape closes it and gives focus back', async () => {
  const { page, errors, csp } = await open({ path: '/storage' });
  await page.getByRole('heading', { name: 'Storage', level: 1 }).click();
  await page.keyboard.press('Control+Shift+P');
  await box(page).waitFor();
  assert.equal(await box(page).evaluate((el) => el === document.activeElement), true, 'the input is not focused');
  // Before anything is typed: pages, the repositories, the actions.
  await group(page, 'Pages').waitFor();
  await option(page, /^notshared\/losos-config/).waitFor();
  await group(page, 'Actions').waitFor();
  assert.equal(await group(page, 'Files in LosOS Cloud').count(), 0, 'files are listed before anything is typed');
  await page.keyboard.press('Escape');
  await box(page).waitFor({ state: 'detached' });
  assert.equal(await page.evaluate(() => document.activeElement?.tagName), 'BODY');
  // Cmd+Shift+P on a Mac is the same chord, and it closes what it opened.
  await page.keyboard.press('Meta+Shift+P');
  await box(page).waitFor();
  await page.keyboard.press('Control+Shift+P');
  await box(page).waitFor({ state: 'detached' });
  // A colon opens it too, and lands in the box as nothing.
  await page.keyboard.type(':');
  await box(page).waitFor();
  assert.equal(await box(page).inputValue(), '');
  await page.keyboard.type(':x');
  assert.equal(await box(page).inputValue(), ':x', 'a colon in the box is text');
  await page.keyboard.press('Escape');
  await box(page).waitFor({ state: 'detached' });
  // But not a colon typed into a field.
  await page.evaluate(() => {
    const field = document.createElement('input');
    field.id = 'probe';
    document.body.append(field);
    field.focus();
  });
  await page.keyboard.type('a:b');
  assert.equal(await page.locator('#probe').inputValue(), 'a:b');
  assert.equal(await box(page).count(), 0, 'a colon in a field opened the palette');
  assert.equal(csp.length, baseline, `the palette added CSP violations:\n${csp.join('\n')}`);
  assert.deepEqual(errors, []);
  await page.close();
});

await check('in LosOS Cloud the same keys open it, and it goes to the admin pages and to files', async () => {
  const { page, errors } = await open({ path: '/cloud/files', cloudPage: true });
  await page.locator('[data-entry="notes.txt"]').waitFor();
  await page.evaluate(() => {
    window.__sameDocument = true;
  });
  await page.mouse.click(1200, 820);
  await page.keyboard.press('Control+Shift+P');
  await box(page).waitFor();
  await page.keyboard.press('Escape');
  await box(page).waitFor({ state: 'detached' });
  await page.keyboard.type(':');
  await box(page).waitFor();
  // A file of LosOS Cloud opens in this page, by its router.
  await box(page).fill('tax-return');
  await option(page, /^tax-return-2025\.pdf/).click();
  await page.waitForURL(/\/cloud\/files/);
  assert.equal(await page.evaluate(() => window.__sameDocument), true, 'a file in LosOS Cloud loaded the page again');
  // A page of the admin UI is a page load to it.
  await page.keyboard.press('Control+Shift+P');
  await box(page).fill('gpu');
  await option(page, /^Hardware/).waitFor();
  await page.keyboard.press('Enter');
  await page.waitForURL(origin + '/settings/hardware');
  await page.getByRole('heading', { name: 'Hardware', level: 1 }).waitFor();
  assert.deepEqual(errors, []);
  await page.close();
});

await check('the sidebar search box opens it, and the arrows and Enter go to a page', async () => {
  const { page, errors } = await open();
  await page.getByRole('navigation', { name: 'Sections', exact: true }).getByRole('button', { name: /^Search or jump to/ }).click();
  await box(page).fill('gpu');
  // Hardware matches on a keyword, not its name; it is the only page left.
  await option(page, /^Hardware/).waitFor();
  assert.equal(await box(page).getAttribute('aria-activedescendant'), await option(page, /^Hardware/).getAttribute('id'));
  await page.keyboard.press('Enter');
  await page.waitForURL(origin + '/settings/hardware');
  await box(page).waitFor({ state: 'detached' });
  // Arrow keys move the highlight, and wrap.
  await page.keyboard.press('Control+Shift+P');
  await box(page).fill('st');
  const first = await box(page).getAttribute('aria-activedescendant');
  await page.keyboard.press('ArrowDown');
  const second = await box(page).getAttribute('aria-activedescendant');
  assert.notEqual(first, second, 'ArrowDown did not move');
  await page.keyboard.press('ArrowUp');
  assert.equal(await box(page).getAttribute('aria-activedescendant'), first, 'ArrowUp did not move back');
  assert.equal(await page.locator('[role=option][aria-selected=true]').count(), 1);
  assert.deepEqual(errors, []);
  await page.close();
});

await check('a page matches in the language on screen and in English, with or without accents', async () => {
  const { page } = await open();
  await page.evaluate(() => window.localStorage.setItem('losos-lang', 'sk'));
  await page.reload({ waitUntil: 'networkidle' });
  await page.keyboard.press('Control+Shift+P');
  const field = page.getByRole('combobox', { name: /^Hľadať stránky/ });
  for (const word of ['ulozisko', 'Úložisko', 'storage']) {
    await field.fill(word);
    await option(page, /^Úložisko/).waitFor({ timeout: 2000 });
  }
  await field.fill('zzzz');
  await page.getByText('Hľadaniu „zzzz“ nič nezodpovedá.').waitFor();
  await page.close();
});

await check('files in LosOS Cloud and repositories in LosOS Git open in their app', async () => {
  const { page, asked } = await open();
  await page.keyboard.press('Control+Shift+P');
  await box(page).fill('tax');
  await option(page, /^Taxes 2025/).waitFor();
  await option(page, /^notshared\/tax-scripts/).waitFor();
  // A file's second line is the folder it is in.
  assert.match(await option(page, /^tax-return-2025\.pdf/).innerText(), /\/Documents\/Taxes 2025/);
  assert.ok(asked.some(([kind, q]) => kind === 'files' && q === 'tax'), 'the box was not asked for files');
  await option(page, /^tax-return-2025\.pdf/).click();
  await page.waitForURL(origin + '/cloud/files?fileid=418');
  await page.goto(origin + '/', { waitUntil: 'networkidle' });
  await page.keyboard.press('Control+Shift+P');
  await box(page).fill('tax-scr');
  await option(page, /^notshared\/tax-scripts/).waitFor();
  await page.keyboard.press('Enter');
  await page.waitForURL(origin + '/git/notshared/tax-scripts');
  await page.close();
});

await check('text inside files: cloud first, then Git, with the match marked, each opening in its app', async () => {
  const { page, asked } = await open();
  await page.keyboard.press('Control+Shift+P');
  await box(page).fill('ta');
  await page.waitForTimeout(400);
  assert.equal(asked.filter(([kind]) => kind === 'cloudText' || kind === 'gitText').length, 0, 'two letters searched inside files');
  await box(page).fill('tax');
  const text = group(page, 'Text in files');
  await text.waitFor();
  await until(async () => (await text.getByRole('option').count()) === 3, 'the text group does not list three hits');
  const names = await text.getByRole('option').evaluateAll((els) => els.map((el) => el.querySelector('.font-medium')?.textContent));
  assert.deepEqual(names, ['Daňové priznanie 2025.odt', 'notes.md', 'sort.py']);
  const first = text.getByRole('option').first();
  assert.match(await first.innerText(), /LosOS Cloud · Documents/);
  assert.equal(await first.locator('mark').innerText(), 'tax');
  assert.match(await text.getByRole('option').last().innerText(), /notshared\/tax-scripts · line 12/);
  await first.click();
  await page.waitForURL(origin + '/cloud/files/Documents');
  await page.goto(origin + '/', { waitUntil: 'networkidle' });
  await page.keyboard.press('Control+Shift+P');
  await box(page).fill('tax_year');
  await option(page, /^sort\.py/).waitFor();
  await page.keyboard.press('Enter');
  await page.waitForURL(origin + '/git/notshared/tax-scripts/src/branch/main/src/sort.py#L12');
  await page.close();
});

await check('the text group says the cloud index is still being read, and hides a search the box does not run', async () => {
  const { page } = await open({ cloudText: { available: true, partial: true }, gitText: { available: false, reason: 'off' } });
  await page.keyboard.press('Control+Shift+P');
  await box(page).fill('tax');
  await page.getByText('LosOS Cloud is still reading your files, so more may turn up.').waitFor();
  assert.equal(await group(page, 'Text in files').getByRole('option').count(), 2);
  await page.close();
  const off = await open({ cloudText: { available: false, reason: 'off' }, gitText: { available: false, reason: 'noGit' } });
  await off.page.keyboard.press('Control+Shift+P');
  await box(off.page).fill('tax');
  await option(off.page, /^Taxes 2025/).waitFor();
  await off.page.waitForTimeout(500);
  assert.equal(await group(off.page, 'Text in files').count(), 0, 'a search the box does not run left a group');
  await off.page.close();
});

await check('one letter does not search the files; the cloud\'s pages are listed as apps', async () => {
  const { page, asked } = await open();
  await page.keyboard.press('Control+Shift+P');
  await box(page).fill('p');
  await option(page, /^Photos/).waitFor();
  await page.waitForTimeout(400);
  assert.equal(asked.filter(([kind]) => kind === 'files').length, 0, 'one letter searched the files');
  await box(page).fill('photos');
  await option(page, /^Photos/).click();
  await page.waitForURL(origin + '/cloud/photos');
  await page.close();
});

await check('says why it cannot search the files before the first password sign-in', async () => {
  const { page } = await open({ files: { available: false, reason: 'signIn' } });
  await page.keyboard.press('Control+Shift+P');
  await box(page).fill('tax');
  await page.getByText('File search works after your next sign-in with your password.').waitFor();
  await page.close();
});

await check('a box without the lookups shows no file or repository group, and asks once', async () => {
  const { page, asked, errors } = await open({ files: 404, repos: 404 });
  await page.keyboard.press('Control+Shift+P');
  await box(page).fill('tax');
  await page.waitForTimeout(500);
  await box(page).fill('taxes');
  await page.waitForTimeout(500);
  assert.equal(await group(page, 'Files in LosOS Cloud').count(), 0);
  assert.equal(await group(page, 'Repositories in LosOS Git').count(), 0);
  assert.equal(asked.filter(([kind]) => kind === 'files').length, 1, 'a 404 was asked again');
  assert.deepEqual(errors, []);
  await page.close();
});

await check('an app picked to install opens the Apps pane with its install dialog', async () => {
  const { page, errors } = await open();
  await page.keyboard.press('Control+Shift+P');
  await box(page).fill('jellyfin');
  await option(page, /^jellyfin/).first().waitFor();
  assert.equal(await option(page, /^jellyfin-docs/).count(), 0, 'an app with nothing to install was offered');
  await option(page, /^jellyfin/).first().click();
  await page.waitForURL(origin + '/apps');
  // The pane searched what was typed, and the dialog is its usual first step.
  const field = page.getByPlaceholder('Search for an app');
  await until(async () => (await field.inputValue()) === 'jellyfin', 'the Apps pane did not search what was typed');
  await page.getByRole('dialog').filter({ hasText: 'jellyfin' }).waitFor();
  // The pick was router state, dropped at once: a reload opens no dialog.
  await page.reload({ waitUntil: 'networkidle' });
  await page.waitForTimeout(500);
  assert.equal(await page.getByRole('dialog').filter({ hasText: 'jellyfin' }).count(), 0, 'a reload opened the dialog again');
  assert.deepEqual(errors, []);
  await page.close();
});

await check('actions run in place: the theme changes, and the palette closes', async () => {
  const { page } = await open();
  await page.keyboard.press('Control+Shift+P');
  await box(page).fill('dark');
  await option(page, /^Use the dark theme/).click();
  await box(page).waitFor({ state: 'detached' });
  assert.equal(await page.evaluate(() => document.documentElement.classList.contains('dark') || document.documentElement.dataset.theme === 'dark'), true);
  await page.keyboard.press('Control+Shift+P');
  await box(page).fill('theme');
  assert.equal(await option(page, /^Use the dark theme/).count(), 0, 'the theme already on screen is offered');
  await page.close();
});

await check('on a phone the sheet hands over to the palette', async () => {
  const { page } = await open({ viewport: { width: 375, height: 800 } });
  await page.getByRole('navigation', { name: 'Main places' }).getByRole('button', { name: 'More' }).click();
  await page.getByRole('dialog', { name: 'Sections' }).getByRole('button', { name: /^Search or jump to/ }).click();
  await box(page).waitFor();
  const panel = await page.locator('dialog[open]').last().boundingBox();
  assert.ok(panel.width <= 375 - 16, `the palette is wider than the phone: ${panel.width}`);
  await box(page).fill('back');
  await option(page, /^Backup/).click();
  await page.waitForURL(origin + '/settings/backup');
  await page.close();
});

await browser.close();
await closeServer();
finish();
