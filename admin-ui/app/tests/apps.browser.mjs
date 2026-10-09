/* Installing an app the Apps pane's search found.
 *
 * The real dist/ bundle under the admin page's CSP, the API stubbed with
 * page.route(). The flow is two steps in one native <dialog>: the app's
 * properties and the user it runs as, then a confirmation; nothing reaches
 * POST /api/apps/install before the second step's button. Every check also
 * fails on a page error or a CSP violation, because the dialog, the tabs,
 * the folds and the switches are all things that could have pulled in a
 * library that styles itself at runtime.
 *
 * SCREENSHOTS=<dir> also writes the screens the pull request shows. */

import assert from 'node:assert';
import { mkdir } from 'node:fs/promises';
import { join } from 'node:path';
import { launch, runner, serve } from './harness.mjs';

const TOKEN = 'a'.repeat(64);
const SHOTS = process.env.SCREENSHOTS ?? null;
const { origin, close: closeServer } = await serve({ csp: true });
const browser = await launch();
const { check, finish } = runner();
if (SHOTS !== null) await mkdir(SHOTS, { recursive: true });

const SETTINGS = {
  sharingMyStorage: false,
  nextcloudMode: 'container',
  forgejoMode: 'container',
  hostName: 'mattbox',
  https: false,
  gpuEnable: false,
  apachePort: 80,
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

const JELLYFIN = { repo: 'https://utkuozdemir.org/helm-charts', name: 'jellyfin', version: '2.0.0' };

const SEARCH = {
  sources: ['Artifact Hub'],
  results: [
    {
      id: 'jellyfin',
      name: 'jellyfin',
      version: '2.0.0',
      summary: 'Jellyfin Helm chart',
      source: 'utkuozdemir',
      homepage: 'https://jellyfin.org',
      chart: JELLYFIN,
    },
    // A hit the box cannot fetch an app from gets no button.
    { id: 'jellyseerr', name: 'jellyseerr', version: '1.4.0', summary: 'Requests for Jellyfin', source: 'loeken' },
  ],
};

const VALUES_YAML = [
  'image:',
  '  repository: docker.io/jellyfin/jellyfin',
  '  tag: ""',
  'service:',
  '  type: ClusterIP',
  '  port: 8096',
  'persistence:',
  '  config:',
  '    enabled: true',
  '    size: 5Gi',
  'metrics:',
  '  enabled: false',
  '',
].join('\n');

const CHART = {
  chart: JELLYFIN,
  release: 'jellyfin',
  valuesYaml: VALUES_YAML,
  values: {
    image: { repository: 'docker.io/jellyfin/jellyfin', tag: '' },
    service: { type: 'ClusterIP', port: 8096 },
    persistence: { config: { enabled: true, size: '5Gi' } },
    metrics: { enabled: false },
  },
  schema: {
    type: 'object',
    properties: {
      service: {
        type: 'object',
        properties: {
          port: { type: 'integer', description: 'The port Jellyfin listens on.' },
          type: { enum: ['ClusterIP', 'NodePort'] },
        },
      },
    },
  },
};

const json = (route, status, body) =>
  route.fulfill({ status, contentType: 'application/json', body: JSON.stringify(body) });

/* A signed-in box on the Apps pane. `apps` is GET /api/apps; installs and
 * removals are recorded in `writes` and change what the next GET answers. */
async function open({
  apps = { available: true, sharedAvailable: false, apps: [] },
  viewport = { width: 1280, height: 900 },
  locale = 'en-US',
  installRefusal = null,
} = {}) {
  const page = await browser.newPage({ viewport, locale });
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  await page.addInitScript(() => {
    window.__violations = [];
    document.addEventListener('securitypolicyviolation', (e) =>
      window.__violations.push(`${e.violatedDirective} ${e.blockedURI}`),
    );
  });
  await page.addInitScript((t) => window.sessionStorage.setItem('losos-token', t), TOKEN);

  let current = structuredClone(apps);
  const writes = [];
  const authed = (route) => route.request().headers()['authorization'] === `Bearer ${TOKEN}`;
  const guard = (fn) => (route) => (authed(route) ? fn(route) : json(route, 401, { error: 'unauthorized' }));

  await page.route('**/api/**', (route) => json(route, 200, {}));
  await page.route('**/api/setup/claim', (route) => json(route, 200, { claimed: true }));
  await page.route('**/api/state', guard((route) => json(route, 200, { mode: 'local', sharing: false })));
  await page.route('**/api/settings', guard((route) => json(route, 200, SETTINGS)));
  await page.route('**/api/status', guard((route) => json(route, 200, { state: 'idle', progress: 0, message: '' })));
  await page.route('**/api/apps', guard((route) => json(route, 200, current)));
  await page.route('**/api/apps/search**', guard((route) => json(route, 200, SEARCH)));
  await page.route('**/api/apps/chart**', guard((route) => {
    const q = new URL(route.request().url()).searchParams;
    writes.push(['GET', '/api/apps/chart', Object.fromEntries(q)]);
    return json(route, 200, CHART);
  }));
  await page.route('**/api/apps/install', guard((route) => {
    const body = route.request().postDataJSON();
    writes.push(['POST', '/api/apps/install', body]);
    if (installRefusal !== null) return json(route, 409, { error: installRefusal });
    const record = {
      release: body.release,
      chart: body.chart,
      runAs: body.runAs,
      values: body.values,
      valuesYaml: body.valuesYaml,
      frontPort: 30000,
      appPort: null,
      phase: 'installing',
      message: null,
      updatedAt: 1,
    };
    current = { ...current, apps: [...current.apps.filter((a) => a.release !== record.release), record] };
    return json(route, 200, record);
  }));
  await page.route('**/api/apps/remove', guard((route) => {
    const body = route.request().postDataJSON();
    writes.push(['POST', '/api/apps/remove', body]);
    current = {
      ...current,
      apps: current.apps.map((a) => (a.release === body.release ? { ...a, phase: 'removing' } : a)),
    };
    return json(route, 200, current.apps.find((a) => a.release === body.release));
  }));

  await page.goto(origin + '/apps', { waitUntil: 'networkidle' });
  // The shell's own load-time reports are app.browser.mjs's business; what
  // counts here is anything the install flow adds after it.
  const atLoad = await page.evaluate(() => window.__violations.length);
  page.__atLoad = atLoad;
  return { page, errors, writes };
}

async function search(page) {
  await page.getByPlaceholder('Search for an app').fill('jellyfin');
  await page.locator('[data-slot="table"] tbody tr').first().waitFor();
}

const violations = async (page) => (await page.evaluate(() => window.__violations)).slice(page.__atLoad);
const clean = async (page, errors) => {
  assert.deepEqual(errors, [], 'page errors');
  assert.deepEqual(await violations(page), [], 'CSP violations: ' + JSON.stringify(await violations(page)));
};
const shot = async (page, name, target = page) => {
  if (SHOTS !== null) await target.screenshot({ path: join(SHOTS, `${name}.png`) });
};

const RUNNING = {
  release: 'jellyfin',
  chart: JELLYFIN,
  runAs: 'notshared',
  values: { service: { port: 8097 } },
  valuesYaml: null,
  frontPort: 30000,
  appPort: 8097,
  phase: 'running',
  message: null,
  updatedAt: 1,
};

console.log('admin-ui app install checks');

await check('a hit the box can fetch has an Install button; one without a chart has none', async () => {
  const { page, errors } = await open();
  await search(page);
  const rows = page.locator('[data-slot="table"] tbody tr');
  assert.equal(await rows.nth(0).getByRole('button', { name: 'Install jellyfin' }).count(), 1);
  assert.equal(await rows.nth(1).getByRole('button').count(), 0, 'a hit with no chart offers an install');
  await shot(page, 'search-row', page.locator('main'));
  await clean(page, errors);
  await page.close();
});

await check('with no cluster on the box, no hit offers an install and the footer says why', async () => {
  const { page, errors } = await open({ apps: { available: false, reason: 'noCluster', apps: [] } });
  await search(page);
  assert.equal(await page.locator('[data-slot="table"]').getByRole('button').count(), 0);
  assert.match(await page.locator('main').innerText(), /install only while Files or Code is set to Kept separate/);
  await clean(page, errors);
  await page.close();
});

await check('the dialog shows the properties and the user; shared is greyed with its reason while sharing is off', async () => {
  const { page, errors, writes } = await open();
  await search(page);
  await page.getByRole('button', { name: 'Install jellyfin' }).click();
  const dialog = page.locator('dialog[open]');
  await dialog.getByLabel('Name on this box').waitFor();
  assert.deepEqual(writes[0], ['GET', '/api/apps/chart', JELLYFIN], 'the dialog asked for another chart');
  assert.equal(await dialog.getByLabel('Name on this box').inputValue(), 'jellyfin');
  const notshared = dialog.getByRole('radio', { name: /notshared/ });
  const shared = dialog.getByRole('radio', { name: /^shared/ });
  assert.ok(await notshared.isChecked(), 'notshared is not the default');
  assert.ok(await shared.isDisabled(), 'shared is open on a box that does not share its disk');
  assert.match(await dialog.innerText(), /Off while this box does not share its disk/);
  for (const section of ['image', 'service', 'persistence', 'metrics']) {
    assert.equal(await dialog.locator('summary', { hasText: section }).count(), 1, `no fold for ${section}`);
  }
  await dialog.locator('summary', { hasText: 'service' }).click();
  assert.match(await dialog.innerText(), /The port Jellyfin listens on/, 'the schema description is missing');
  await shot(page, 'dialog-properties', dialog);
  await clean(page, errors);
  await page.close();
});

await check('nothing is sent before the confirmation; Install then posts the name, the user and only what changed', async () => {
  const { page, errors, writes } = await open();
  await search(page);
  await page.getByRole('button', { name: 'Install jellyfin' }).click();
  const dialog = page.locator('dialog[open]');
  await dialog.locator('summary', { hasText: 'service' }).click();
  const port = dialog.getByLabel('port', { exact: true });
  await port.fill('8097');
  assert.match(await dialog.innerText(), /1 change/);
  await dialog.getByRole('button', { name: 'Continue' }).click();

  await dialog.getByRole('heading', { name: 'Install jellyfin on this box?' }).waitFor();
  assert.equal(writes.filter((w) => w[0] === 'POST').length, 0, 'Continue sent the install');
  const text = await dialog.innerText();
  assert.match(text, /You are trusting utkuozdemir/);
  assert.match(text, /runs as notshared, with no admin rights, and keeps its data in \/home\/notshared\/data\/apps\/jellyfin/);
  assert.match(text, /You changed 1 property/);
  await shot(page, 'dialog-confirm', dialog);

  await dialog.getByRole('button', { name: 'Install', exact: true }).click();
  await page.locator('dialog[open]').waitFor({ state: 'detached' }).catch(() => {});
  const post = writes.find((w) => w[1] === '/api/apps/install');
  assert.deepEqual(post[2], {
    release: 'jellyfin',
    chart: JELLYFIN,
    runAs: 'notshared',
    values: { service: { port: 8097 } },
    valuesYaml: null,
  });
  await page.getByRole('heading', { name: 'Installed apps' }).waitFor();
  assert.match(await page.locator('main').innerText(), /jellyfin\s*Installing/);
  await clean(page, errors);
  await page.close();
});

await check('Back keeps the changes, a renamed app is posted under its new name, and a refusal shows in the dialog', async () => {
  const { page, errors, writes } = await open({ installRefusal: 'Port 80 belongs to the box.' });
  await search(page);
  await page.getByRole('button', { name: 'Install jellyfin' }).click();
  const dialog = page.locator('dialog[open]');
  const name = dialog.getByLabel('Name on this box');
  await name.fill('Bad Name');
  assert.ok(await dialog.getByRole('button', { name: 'Continue' }).isDisabled(), 'an invalid name can continue');
  await name.fill('films');
  await dialog.getByRole('button', { name: 'Continue' }).click();
  await dialog.getByRole('button', { name: 'Back' }).click();
  assert.equal(await name.inputValue(), 'films', 'Back lost the name');
  await dialog.getByRole('button', { name: 'Continue' }).click();
  await dialog.getByRole('button', { name: 'Install', exact: true }).click();
  await dialog.getByText('Port 80 belongs to the box.').waitFor();
  assert.equal(writes.find((w) => w[1] === '/api/apps/install')[2].release, 'films');
  await clean(page, errors);
  await page.close();
});

await check('with sharing on, shared can be picked and the text tab sends the edited file', async () => {
  const { page, errors, writes } = await open({ apps: { available: true, sharedAvailable: true, apps: [] } });
  await search(page);
  await page.getByRole('button', { name: 'Install jellyfin' }).click();
  const dialog = page.locator('dialog[open]');
  await dialog.getByRole('radio', { name: /^shared/ }).check();
  assert.match(await dialog.innerText(), /\/home\/shared\/data\/apps\/jellyfin/);
  await dialog.getByRole('tab', { name: 'As text' }).click();
  const yaml = dialog.locator('textarea');
  await yaml.fill(VALUES_YAML.replace('enabled: false', 'enabled: true'));
  await dialog.getByRole('button', { name: 'Continue' }).click();
  await dialog.getByRole('button', { name: 'Install', exact: true }).click();
  await page.getByRole('heading', { name: 'Installed apps' }).waitFor();
  const body = writes.find((w) => w[1] === '/api/apps/install')[2];
  assert.equal(body.runAs, 'shared');
  assert.match(body.valuesYaml, /metrics:\n {2}enabled: true/);
  assert.deepEqual(body.values, {});
  await clean(page, errors);
  await page.close();
});

await check('an installed app opens on its own port, changes in place and is removed after a confirmation', async () => {
  const { page, errors, writes } = await open({ apps: { available: true, sharedAvailable: false, apps: [RUNNING] } });
  await page.getByRole('heading', { name: 'Installed apps' }).waitFor();
  const link = page.getByRole('link', { name: 'Open' });
  assert.equal(await link.getAttribute('href'), `http://127.0.0.1:30000/`);
  assert.match(await page.locator('main').innerText(), /jellyfin 2\.0\.0, runs as notshared/);
  await shot(page, 'installed', page.locator('main'));

  // A search hit for an app already installed is a change, not a second install.
  await search(page);
  assert.equal(await page.locator('[data-slot="table"]').getByRole('button', { name: 'Change' }).count(), 1);

  await page.getByRole('button', { name: 'Change' }).first().click();
  const dialog = page.locator('dialog[open]');
  await dialog.getByRole('heading', { name: 'Change jellyfin' }).waitFor();
  assert.ok(await dialog.getByLabel('Name on this box').isDisabled(), 'the name of an installed app can change');
  assert.match(await dialog.innerText(), /1 change/, 'the earlier change was not carried in');
  await dialog.getByRole('button', { name: 'Cancel' }).click();

  await page.getByRole('button', { name: 'Remove' }).click();
  const confirm = page.locator('dialog[open]');
  await confirm.getByRole('heading', { name: 'Remove jellyfin?' }).waitFor();
  assert.match(await confirm.innerText(), /Its data stays in \/home\/notshared\/data\/apps\/jellyfin/);
  assert.equal(writes.filter((w) => w[1] === '/api/apps/remove').length, 0);
  await confirm.getByRole('button', { name: 'Remove' }).click();
  await page.getByText('Removing').waitFor();
  assert.deepEqual(writes.find((w) => w[1] === '/api/apps/remove')[2], { release: 'jellyfin' });
  await clean(page, errors);
  await page.close();
});

await check('the dialog speaks Slovak and German', async () => {
  for (const [locale, title, runAs] of [
    ['sk-SK', 'Inštalovať jellyfin', 'Spustiť ako'],
    ['de-DE', 'jellyfin installieren', 'Ausführen als'],
  ]) {
    const { page, errors } = await open({ locale });
    await page.locator('main input[type="search"]').first().fill('jellyfin');
    await page.locator('[data-slot="table"] tbody tr').first().waitFor();
    await page.getByRole('button', { name: title }).click();
    const dialog = page.locator('dialog[open]');
    await dialog.getByRole('heading', { name: title }).waitFor();
    // The heading is there before the chart's properties; the form comes with them.
    await dialog.getByText(runAs, { exact: true }).waitFor();
    assert.match(await dialog.innerText(), new RegExp(runAs));
    await clean(page, errors);
    await page.close();
  }
});

await browser.close();
await closeServer();
finish();
