/* The pages as MDX (src/pages/README.md): a page nobody changed, a page an
 * administrator changed, their stylesheet, and the editor that docks beside
 * the page.
 *
 * Same arrangement as the other files: the real dist/ bundle under the admin
 * CSP, the API stubbed with page.route(). The HTML a changed page arrives
 * with is made here by the same losos-mdx the box runs (src/mdx/pkg), so
 * what the stub answers is what lososd's pages plugin answers.
 *
 * With LOSOS_PAGES_BEFORE set to the dist/ of a build from before the pages
 * were MDX, one more check draws the Overview and Storage in both and
 * compares them pixel for pixel. */

import assert from './assert.mjs';
import { launch, runner, serve, SHELL_ME } from './harness.mjs';
import { initSync, page as renderPage } from '../src/mdx/pkg/losos_mdx.js';

initSync({ module: await Bun.file(new URL('../src/mdx/pkg/losos_mdx_bg.wasm', import.meta.url)).bytes() });
const REGISTRY = await Bun.file(new URL('../src/pages/blocks.json', import.meta.url)).text();
const DEFAULT_OVERVIEW = await Bun.file(new URL('../src/pages/overview.mdx', import.meta.url)).text();

/** What the box makes of a page's source: its HTML, and what it refused. */
function render(source, id = 'overview') {
  return JSON.parse(renderPage(source, REGISTRY, id));
}

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
};

const json = (route, status, body) =>
  route.fulfill({ status, contentType: 'application/json', body: JSON.stringify(body) });

/* A claimed box, someone signed in (`me`), and the pages plugin answering
 * from `sources` (page id to the administrator's MDX) and `style`. Every
 * write is recorded in `writes` as [method, path, body]. */
async function open({
  path = '/',
  me = SHELL_ME,
  sources = {},
  style = '',
  viewport = { width: 1280, height: 900 },
  dist = null,
} = {}) {
  const page = await browser.newPage({ viewport, locale: 'en-US' });
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  await page.addInitScript(() => {
    window.__violations = [];
    document.addEventListener('securitypolicyviolation', (e) =>
      window.__violations.push(`${e.violatedDirective} ${e.blockedURI}`),
    );
  });
  const writes = [];
  let current = { ...sources };
  let css = style;
  let version = css === '' ? 0 : 1;
  const list = () => ({
    pages: Object.fromEntries(
      Object.entries(current).map(([id, source]) => [id, { html: render(source, id).html, at: '2026-10-29T10:00:00Z', by: 'matus' }]),
    ),
    style: css === '' ? null : String(version),
  });

  await page.route('**/api/**', (route) => json(route, 200, {}));
  await page.route('**/api/setup/claim', (route) => json(route, 200, { claimed: true }));
  await page.route('**/api/me', (route) => json(route, 200, me));
  await page.route('**/api/state', (route) => json(route, 200, { mode: 'local', sharing: false }));
  await page.route('**/api/settings', (route) => json(route, 200, SETTINGS));
  await page.route('**/api/status', (route) => json(route, 200, { state: 'idle', progress: 0, message: '' }));
  await page.route('**/api/pages', (route) => json(route, 200, list()));
  await page.route('**/api/pages/**', (route) => {
    const method = route.request().method();
    const url = new URL(route.request().url());
    const id = decodeURIComponent(url.pathname.split('/').pop());
    if (id === 'style.css') {
      if (method === 'GET') {
        return css === ''
          ? json(route, 404, { error: 'no stylesheet' })
          : route.fulfill({ status: 200, contentType: 'text/css', body: css });
      }
      if (!me.admin) return json(route, 403, { error: 'administrators only' });
      const body = method === 'PUT' ? route.request().postDataJSON() : null;
      writes.push([method, url.pathname, body]);
      css = body?.source ?? '';
      version += 1;
      return json(route, 200, list());
    }
    if (!me.admin) return json(route, 403, { error: 'administrators only' });
    if (method === 'GET') {
      return json(route, 200, {
        id,
        source: current[id] ?? DEFAULT_OVERVIEW,
        default: DEFAULT_OVERVIEW,
        custom: id in current,
        defaultChanged: false,
        history: id in current ? [{ commit: 'c1', at: '2026-10-29T10:00:00Z', by: 'matus', message: 'Change the Overview page' }] : [],
      });
    }
    const body = method === 'PUT' ? route.request().postDataJSON() : null;
    writes.push([method, url.pathname, body]);
    if (method === 'PUT') {
      const problems = render(body.source, id).problems;
      if (problems.length > 0) return json(route, 400, { error: problems[0].message });
      current = { ...current, [id]: body.source };
    } else {
      const { [id]: _gone, ...rest } = current;
      current = rest;
    }
    return json(route, 200, list());
  });
  await page.addInitScript((t) => {
    try {
      window.sessionStorage.setItem('losos-token', t);
    } catch {}
  }, TOKEN);
  const base = dist === null ? origin : dist;
  await page.goto(base + path, { waitUntil: 'networkidle' });
  return { page, errors, writes };
}

const violations = (page) => page.evaluate(() => window.__violations);
const hostName = (page) => page.getByRole('heading', { level: 1, name: 'mattbox' });

console.log('admin-ui pages checks');

await check('a page nobody changed draws its blocks and nothing around them', async () => {
  const { page, errors } = await open();
  await hostName(page).waitFor();
  assert.equal(await page.locator('.mdx-page-text').count(), 0, 'a default page has no text of its own');
  assert.equal(await page.locator('losos-block').count(), 0, 'a placeholder reached the page');
  assert.equal(await page.locator('html').getAttribute('data-page'), 'overview');
  assert.deepEqual(errors, []);
  assert.deepEqual(await violations(page), []);
  await page.close();
});

await check('the box compiles the defaults the bundle carries', async () => {
  // The default is placeholders only, in order, which is the guarantee that
  // nothing on a box nobody customized moved.
  const { html, problems } = render(DEFAULT_OVERVIEW);
  assert.deepEqual(problems, []);
  assert.equal(
    html.replace(/\s+/g, ''),
    ['BoxName', 'Applying', 'Apps', 'Glance']
      .map((b) => `<losos-blockdata-block="${b}"data-props="{}"></losos-block>`)
      .join(''),
  );
});

await check('a changed page draws its text, its button and only the blocks it keeps', async () => {
  const source = [
    '# Welcome to the office box',
    '',
    'Ask Matus before you install anything.',
    '',
    '<Button href="/apps" variant="secondary">Open the apps</Button>',
    '',
    '<Apps hideStorage />',
    '',
  ].join('\n');
  const { page, errors } = await open({ sources: { overview: source } });
  await page.getByRole('heading', { level: 1, name: 'Welcome to the office box' }).waitFor();
  assert.equal(await hostName(page).count(), 0, 'BoxName was left out and still drawn');
  assert.ok(await page.getByText('Ask Matus before you install anything.').isVisible());
  const button = page.getByRole('link', { name: 'Open the apps' });
  assert.match(await button.getAttribute('class'), /mdx-button/);
  assert.match(await button.getAttribute('class'), /inline-flex/, 'not drawn with the page\'s own button classes');
  // An in-app link goes through the router: no new document.
  await page.evaluate(() => {
    window.__sameDocument = true;
  });
  await button.click();
  await page.waitForURL('**/apps');
  assert.equal(await page.evaluate(() => window.__sameDocument), true, 'the button reloaded the page');
  assert.deepEqual(errors, []);
  assert.deepEqual(await violations(page), []);
  await page.close();
});

await check('a reload draws the changed page at once, from what the last visit kept', async () => {
  const { page } = await open({ sources: { overview: '# Kept\n\n<BoxName />\n' } });
  await page.getByRole('heading', { name: 'Kept' }).waitFor();
  // The box goes quiet; the page still draws the change, not the default.
  await page.route('**/api/pages', (route) => route.abort());
  await page.reload({ waitUntil: 'networkidle' });
  await page.getByRole('heading', { name: 'Kept' }).waitFor({ timeout: 2000 });
  await page.close();
});

await check('the administrators\' stylesheet is linked last and wins, under the strict CSP', async () => {
  const style = 'html[data-page="overview"] .mdx-page-text h2 { color: rgb(1, 2, 3); }';
  const { page, errors } = await open({ sources: { overview: '## Styled\n\n<BoxName />\n' }, style });
  const heading = page.getByRole('heading', { level: 2, name: 'Styled' });
  await heading.waitFor();
  const link = page.locator('body > link#losos-pages-style');
  assert.equal(await link.getAttribute('href'), '/api/pages/style.css?v=1');
  assert.equal(
    await page.evaluate(() => document.body.lastElementChild?.id),
    'losos-pages-style',
    'the stylesheet is not the last thing in <body>',
  );
  await page.waitForFunction(() => {
    const h = [...document.querySelectorAll('h2')].find((e) => e.textContent === 'Styled');
    return h !== undefined && getComputedStyle(h).color === 'rgb(1, 2, 3)';
  });
  assert.deepEqual(errors, []);
  assert.deepEqual(await violations(page), []);
  await page.close();
});

await check('only administrators see the Pages section', async () => {
  const admin = await open({ path: '/settings/look' });
  const section = admin.page.getByTestId('pages-section');
  await section.waitFor();
  assert.ok(await section.getByText('Overview').isVisible());
  assert.ok(await section.getByText('Storage').isVisible());
  assert.equal(await section.getByText('As shipped').count(), 2);
  await admin.page.close();

  const viewer = await open({
    path: '/settings/look',
    me: { ...SHELL_ME, groups: ['losos-look'], admin: false, sections: ['look'] },
  });
  await viewer.page.getByRole('radiogroup', { name: 'Background' }).waitFor();
  assert.equal(await viewer.page.getByTestId('pages-section').count(), 0);
  await viewer.page.close();
});

await check('the editor docks beside the page, previews each change and saves it', async () => {
  const { page, errors, writes } = await open({ path: '/settings/look' });
  await page.getByRole('button', { name: 'Edit Overview' }).click();
  await page.waitForURL(origin + '/');
  const dock = page.getByTestId('page-editor');
  await dock.waitFor();
  // The page moves out from under the dock rather than being covered by it.
  const [main, docked] = await Promise.all([page.locator('main').boundingBox(), dock.boundingBox()]);
  assert.ok(main.x + main.width <= docked.x, `the page runs under the editor: ${JSON.stringify({ main, docked })}`);
  const box = dock.getByRole('textbox', { name: 'Source of the Overview page' });
  await box.waitFor();
  await box.click();
  await page.keyboard.press('ControlOrMeta+Home');
  await page.keyboard.insertText('## Hello from the editor\n\n');
  await page.getByRole('heading', { level: 2, name: 'Hello from the editor' }).waitFor();
  assert.equal(writes.length, 0, 'a preview wrote to the box');

  // A block the page does not have is a problem, and Save waits for it.
  await page.keyboard.insertText('<Nope />\n\n');
  await dock.getByTestId('page-problems').waitFor();
  assert.match(await dock.getByTestId('page-problems').innerText(), /<Nope> is not a block of this page/);
  assert.ok(await dock.getByRole('button', { name: 'Save' }).isDisabled());
  for (let i = 0; i < '<Nope />\n\n'.length; i++) await page.keyboard.press('Backspace');
  await dock.getByTestId('page-problems').waitFor({ state: 'detached' });

  await dock.getByRole('button', { name: 'Save' }).click();
  await page.getByText('Saved. Every browser that opens this box draws it now.').first().waitFor();
  assert.equal(writes.length, 1);
  assert.equal(writes[0][0], 'PUT');
  assert.equal(writes[0][1], '/api/pages/overview');
  assert.match(writes[0][2].source, /^## Hello from the editor\n\n<BoxName \/>/);

  // Closed, the page draws what was saved.
  await dock.getByRole('button', { name: 'Close' }).click();
  await dock.waitFor({ state: 'detached' });
  await page.getByRole('heading', { level: 2, name: 'Hello from the editor' }).waitFor();
  assert.deepEqual(errors, []);
  assert.deepEqual(await violations(page), []);
  await page.close();
});

await check('the stylesheet editor previews on the page and drops the preview when closed', async () => {
  const { page, errors, writes } = await open({ path: '/settings/look' });
  await page.getByRole('button', { name: 'Edit the stylesheet' }).click();
  const dock = page.getByTestId('page-editor');
  const box = dock.getByRole('textbox', { name: 'The pages\' stylesheet' });
  await box.waitFor();
  await box.click();
  await page.keyboard.insertText('main { outline: 3px solid rgb(4, 5, 6); }');
  await page.waitForFunction(() => getComputedStyle(document.querySelector('main')).outlineColor === 'rgb(4, 5, 6)');
  assert.equal(writes.length, 0);
  await dock.getByRole('button', { name: 'Close' }).click();
  await page.waitForFunction(() => getComputedStyle(document.querySelector('main')).outlineColor !== 'rgb(4, 5, 6)');
  assert.deepEqual(errors, []);
  assert.deepEqual(await violations(page), []);
  await page.close();
});

const BEFORE = Bun.env.LOSOS_PAGES_BEFORE;
if (BEFORE !== undefined && BEFORE !== '') {
  const { origin: before, close: closeBefore } = await serve({ csp: true, dist: BEFORE });
  for (const path of ['/', '/storage']) {
    await check(`${path} looks exactly as it did before it was MDX`, async () => {
      const shots = [];
      for (const dist of [before, origin]) {
        const { page } = await open({ path, dist });
        await page.waitForTimeout(500);
        shots.push(await page.screenshot({ fullPage: true, animations: 'disabled' }));
        await page.close();
      }
      assert.ok(Bun.deepEquals(new Uint8Array(shots[0]), new Uint8Array(shots[1])), `${path} differs from the build before`);
    });
  }
  await closeBefore();
}

await browser.close();
await closeServer();
finish();
