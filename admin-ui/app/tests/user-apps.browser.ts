/* Your apps: an app the AI on the box builds, step by step, tried in the
 * preview, saved, and used on its own page.
 *
 * The real dist/ bundle under the admin page's CSP, and the app frame under
 * its own (harness.mjs, APP_FRAME_CSP), the API stubbed with page.route().
 * The app is a real one: tests/fixtures/user-app.json is its source and the
 * script lososd compiled from it. The builder stub plays the box's half of
 * a build (backend/src/builder.rs): it writes, checks, asks the page to try
 * the app, takes the page's report, and finishes. What the page reports is
 * what the app frame's own try found (src/app-frame/runtime.tsx), so the
 * checks here read that report as the box would. */

import assert from './assert.mjs';
import { launch, runner, serve } from './harness.mjs';

const TOKEN = 'a'.repeat(64);
const APP = await Bun.file(`${import.meta.dir}/fixtures/user-app.json`).json();
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

/* The first answer reads a list before the line that makes it, as a small
 * model's fix once did: it compiles, and stops the moment it starts. */
const BROKEN = APP.code.replace('function App() {', 'function App() {\n\tconst first = names[0];\n\tconst names = ["Milk"];');
assert.notEqual(BROKEN, APP.code);

const step = (kind, running, extra = {}) => ({ kind, running, problems: [], shown: [], skipped: false, started_at: 1, ...extra });

/* The box's half of one build. Each read of the build moves it on: write,
 * check, a try the page must answer, then a fix with new code and a second
 * try; once that is answered, done. */
function builder() {
  const state = { reads: 0, tried: [], phase: 'writing' };
  const build = () => {
    const base = {
      id: 'b1-1', kind: 'app', prompt: 'a shopping list', revision: false, created_at: 1,
      fixing: false, fault: null, at_limit: false, problems: [], notes: null, name: null,
      files: null, code: null, trial: null, finished_at: null,
    };
    switch (state.phase) {
      case 'writing':
        return { ...base, status: 'running', lines: 12, steps: [step('write', true)] };
      case 'trying':
        return {
          ...base, status: 'running', lines: 38, code: BROKEN,
          trial: { id: 't0', code: BROKEN },
          steps: [step('write', false), step('check', false), step('try', true)],
        };
      case 'fixed':
        return {
          ...base, status: 'running', lines: 38, code: APP.code, fixing: true,
          trial: { id: 't1', code: APP.code },
          steps: [
            step('write', false), step('check', false), step('try', false, { problems: ['When the app starts, it stops with: boom'] }),
            step('fix', false), step('check', false), step('try', true),
          ],
        };
      default: {
        const report = state.tried[1];
        return {
          ...base, status: 'done', lines: 38, finished_at: 2, code: APP.code, name: 'Shopping',
          notes: 'A list of things to buy.', files: [{ name: 'app.jsx', content: APP.source }],
          steps: [
            step('write', false), step('check', false), step('try', false, { problems: ['When the app starts, it stops with: boom'] }),
            step('fix', false), step('check', false), step('try', false, { shown: report.shown }),
          ],
        };
      }
    }
  };
  const handle = (route) => {
    const url = new URL(route.request().url());
    const method = route.request().method();
    if (url.pathname === '/api/builder') return json(route, 200, { available: true, model: 'qwen2.5-coder:1.5b', builds: [] });
    if (url.pathname === '/api/builder/builds' && method === 'POST') {
      state.started = route.request().postDataJSON();
      return json(route, 200, build());
    }
    if (url.pathname === '/api/builder/builds/b1-1/tried' && method === 'POST') {
      state.tried.push(route.request().postDataJSON());
      state.phase = state.phase === 'trying' ? 'fixed' : 'done';
      return json(route, 200, { taken: true });
    }
    if (url.pathname === '/api/builder/builds/b1-1') {
      state.reads += 1;
      if (state.phase === 'writing' && state.reads >= 2) state.phase = 'trying';
      return json(route, 200, build());
    }
    return json(route, 404, { error: 'not found' });
  };
  return { state, handle };
}

async function open(path = '/apps', repo = undefined) {
  const page = await browser.newPage({ viewport: { width: 1280, height: 900 }, locale: 'en-US' });
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  await page.addInitScript(() => {
    window.__violations = [];
    document.addEventListener('securitypolicyviolation', (e) =>
      window.__violations.push(`${e.violatedDirective} ${e.blockedURI}`),
    );
  });
  await page.addInitScript((t) => {
    try {
      window.sessionStorage.setItem('losos-token', t);
    } catch {}
  }, TOKEN);
  const box = builder();
  const saved = [];
  const data = { collections: {}, stored: {} };
  const writes = [];
  let apps = [];
  await page.route('**/api/**', (route) => json(route, 200, {}));
  await page.route('**/api/state', (route) => json(route, 200, { mode: 'local', sharing: false }));
  await page.route('**/api/status', (route) => json(route, 200, { state: 'idle', progress: 0, message: '' }));
  await page.route('**/api/setup/claim', (route) => json(route, 200, { claimed: true }));
  await page.route('**/api/settings', (route) => json(route, 200, SETTINGS));
  await page.route('**/api/apps', (route) => json(route, 200, { available: true, sharedAvailable: false, apps: [] }));
  await page.route('**/api/cloud/apps', (route) => json(route, 404, { error: 'not found' }));
  await page.route('**/api/builder**', box.handle);
  await page.route('**/api/user-apps', (route) => {
    if (route.request().method() === 'GET') return json(route, 200, { apps, limits: { apps: 50, nameChars: 60, sourceBytes: 65536, dataBytes: 1048576 } });
    const body = route.request().postDataJSON();
    saved.push(body);
    const app = { id: 'u1', name: body.name, summary: body.summary, prompt: body.prompt, source: body.source, code: APP.code, createdAt: 1, updatedAt: 1 };
    apps = [{ id: 'u1', name: body.name, summary: body.summary, createdAt: 1, updatedAt: 1 }];
    return json(route, 200, { app, problems: [] });
  });
  await page.route('**/api/user-apps/check', (route) => {
    const { source } = route.request().postDataJSON();
    return json(route, 200, source === APP.source ? { code: APP.code, problems: [] } : { code: null, problems: ['It does not compile.'] });
  });
  await page.route('**/api/user-apps/u1', (route) =>
    json(route, 200, { app: { id: 'u1', name: 'Shopping', summary: 'A list of things to buy.', prompt: 'a shopping list', source: APP.source, code: APP.code, createdAt: 1, updatedAt: 1, repo } }),
  );
  let n = 0;
  await page.route('**/api/user-apps/u1/data', (route) => {
    if (route.request().method() === 'GET') return json(route, 200, data);
    const op = route.request().postDataJSON();
    writes.push(op);
    if (op.op === 'add') {
      const item = { ...op.value, id: `i${++n}`, createdAt: 1 };
      (data.collections[op.collection] ??= []).push(item);
      return json(route, 200, { item });
    }
    return json(route, 200, {});
  });
  await page.goto(origin + path, { waitUntil: 'networkidle' });
  page.__atLoad = await page.evaluate(() => window.__violations.length);
  return { page, errors, box, saved, writes, data };
}

const violations = async (page) => (await page.evaluate(() => window.__violations)).slice(page.__atLoad);

console.log('admin-ui your apps checks');

await check('the AI builds an app step by step, the preview tries each version for the box, and the saved app keeps its data', async () => {
  const { page, errors, box, saved, writes, data } = await open();
  await page.getByTestId('your-apps-empty').waitFor();
  await page.getByTestId('your-apps-make').click();
  await page.waitForURL(`${origin}/apps/new`);
  const panel = page.getByTestId('builder-panel');
  await panel.getByLabel('What should the app do?').fill('a shopping list');
  await panel.getByRole('button', { name: 'Build', exact: true }).click();

  const steps = page.getByTestId('builder-steps');
  await steps.getByText('Writing, 12 lines so far').waitFor({ timeout: 15000 });
  // The build asks for a try: the preview shows the app and the status says
  // it is being tried.
  await page.getByTestId('builder-running').filter({ hasText: 'Trying it in the preview' }).waitFor({ timeout: 15000 });
  // The page's report, as the frame made it: what the app shows, the
  // button it pressed, what that kept.
  await page.getByTestId('builder-outcome').waitFor({ timeout: 45000 });
  assert.equal(box.state.started.kind, 'app');
  assert.equal(box.state.tried.length, 2, 'two tries, a report for each');
  const [broken, report] = box.state.tried;
  // The first app stopped as it started, and the page said so.
  assert.equal(broken.trial, 't0');
  assert.equal(broken.errors.length, 1, JSON.stringify(broken));
  assert.match(broken.errors[0], /names/);
  // The fix came with new code, and the page tried that in a fresh frame.
  assert.equal(report.trial, 't1');
  assert.equal(report.pressed, 'Add');
  assert.deepEqual(report.kept, ['items: 0 to 1']);
  assert.deepEqual(report.errors, []);
  assert.deepEqual(report.threwOnPress, []);
  assert.ok(report.shown.includes('h1: Shopping'), JSON.stringify(report.shown));
  assert.ok(report.shown.includes('input[text] "Item"'), JSON.stringify(report.shown));
  // The steps as they ended, each in words.
  assert.deepEqual(
    await steps.locator('> li').evaluateAll((li) => li.map((l) => l.dataset.step)),
    ['write', 'check', 'try', 'fix', 'check', 'try'],
  );
  await steps.getByText('Checked the code: nothing wrong').first().waitFor();
  await steps.getByText('Tried it: nothing went wrong').waitFor();

  // The try's entry went with the frame it was made in: the preview is the
  // app afresh, with nothing in it.
  const preview = page.frameLocator('[data-testid=app-preview] iframe');
  await preview.locator('h1').filter({ hasText: 'Shopping' }).waitFor();
  await preview.getByText('Nothing to buy.').waitFor();
  assert.equal(await page.getByLabel('Name', { exact: true }).inputValue(), 'Shopping');

  // Saved, the app runs on its own page and writes to the box.
  await page.getByTestId('app-save').click();
  await page.waitForURL(`${origin}/apps/u1`);
  assert.equal(saved[0].source, APP.source);
  const app = page.frameLocator('[data-testid=user-app] iframe');
  await app.getByLabel('Item').fill('Milk');
  // The collection updates optimistically; wait for the box's write response
  // before asserting the request and the persisted data.
  const writeResponse = page.waitForResponse((response) =>
    response.url().endsWith('/api/user-apps/u1/data') && response.request().method() === 'POST',
  );
  await app.getByRole('button', { name: 'Add' }).click();
  await writeResponse;
  await app.getByText('Milk').waitFor();
  assert.deepEqual(writes, [{ op: 'add', collection: 'items', value: { name: 'Milk', done: false } }]);
  assert.equal(data.collections.items[0].name, 'Milk');

  assert.deepEqual(errors, []);
  assert.deepEqual(await violations(page), []);
  await page.close();
});

await check("an app's page links its repository, and says so when a push was not taken", async () => {
  const synced = await open('/apps/u1', { name: 'notshared/app-shopping', state: 'synced' });
  const link = synced.page.getByTestId('user-app-repo');
  await link.waitFor();
  assert.equal(await link.getAttribute('href'), '/git/notshared/app-shopping');
  assert.equal((await link.textContent()).trim(), 'Source');
  assert.equal(await synced.page.getByTestId('user-app-repo-note').count(), 0);
  await synced.page.close();

  const detail = 'The version pushed as 0123456 was not taken: the app does not compile.';
  const refused = await open('/apps/u1', { name: 'notshared/app-shopping', state: 'refused', detail });
  const note = refused.page.getByTestId('user-app-repo-note');
  await note.getByText('Your last push was not taken').waitFor();
  await note.getByText(detail).waitFor();
  await refused.page.close();

  // With LosOS Git off there is no link and nothing to say.
  const local = await open('/apps/u1', { state: 'local', detail: 'LosOS Git is off on this box.' });
  await local.page.getByTestId('user-app').waitFor();
  assert.equal(await local.page.getByTestId('user-app-repo').count(), 0);
  assert.equal(await local.page.getByTestId('user-app-repo-note').count(), 0);
  assert.deepEqual(local.errors, []);
  await local.page.close();
});

await finish();
await browser.close();
await closeServer();
