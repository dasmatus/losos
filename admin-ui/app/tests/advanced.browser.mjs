/* The Advanced pane (every losos.* option, typed editors) and the History
 * pane (the configuration repository on LosOS Git).
 *
 * The option document under test is the real one: LOSOS_OPTIONS_JSON points
 * at the file flake/options-doc.nix generated from the install configuration
 * (tests/admin-ui.nix passes the `losos-options-doc` check), and the fixture
 * at tests/fixtures/options.json is a copy of it for a run outside nix.
 * Rendering the real document is the CI gate the pane exists for: an option
 * whose editor kind the pane cannot draw fails here, and one whose Nix type
 * flake/options-doc.nix cannot classify failed earlier, in the eval job.
 *
 * Same arrangement as app.browser.mjs: the dist/ bundle, the API stubbed with
 * page.route(), under the admin CSP so a control that styles itself with an
 * injected element fails here before it fails on the box.
 */

import assert from 'node:assert';
import { readFile } from 'node:fs/promises';
import { launch, runner, serve } from './harness.mjs';

const TOKEN = 'a'.repeat(64);
const { origin, close: closeServer } = await serve({ csp: true });
const browser = await launch();
const { check, finish } = runner();

const DOC = JSON.parse(await readFile(process.env.LOSOS_OPTIONS_JSON ?? 'tests/fixtures/options.json', 'utf8'));
assert.equal(DOC.version, 1, 'the option document is version 1');
assert.ok(DOC.options.length > 40, `the document carries ${DOC.options.length} options; expected the whole losos.* tree`);

/* The sixteen names lib/api.ts's buildOverridesNix always writes, and the
 * pane links to another pane for (or edits through the form, for the
 * planned Market pane). Kept as a list here so the test does not import
 * the bundle it is testing. */
const OWNED = {
  sharingMyStorage: 'market',
  'nextcloud.mode': 'apps',
  'forgejo.mode': 'apps',
  hostName: 'network',
  'nextcloud.https': 'network',
  'gpu.enable': 'hardware',
  'nextcloud.apachePort': 'network',
  'proxy.enable': 'network',
  'cluster.enable': 'mesh',
  'cluster.shareCompute': 'mesh',
  'cluster.computeWindow.start': 'mesh',
  'cluster.computeWindow.end': 'mesh',
  'hardening.apparmor': 'security',
  'hardening.malloc': 'security',
  'hardening.nosmt': 'security',
  'hardening.usbguard': 'security',
};
const KINDS = new Set(['bool', 'str', 'int', 'float', 'enum', 'list', 'nullable', 'opaque']);

const SETTINGS = {
  sharingMyStorage: false,
  nextcloudMode: 'container',
  forgejoMode: 'container',
  hostName: 'mattbox',
  https: false,
  gpuEnable: false,
  apachePort: 11000,
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

const CONFIG = {
  enabled: true,
  repository: {
    owner: 'notshared',
    name: 'losos-config',
    url: '/forgejo/notshared/losos-config',
    clone: 'http://127.0.0.1:3000/notshared/losos-config.git',
    viaForgejo: true,
  },
  head: { sha: 'b'.repeat(40), branch: 'main' },
  log: [
    { sha: 'b'.repeat(40), when: '2026-10-07T10:12:00+02:00', subject: 'Change hostName, cluster.enable' },
    { sha: 'a'.repeat(40), when: '2026-10-06T03:00:00+02:00', subject: 'losos install' },
  ],
  sync: { state: 'ok', detail: 'In step with LosOS Git.', syncedAt: '2026-10-07T10:12:30+02:00', remoteHead: 'b'.repeat(40) },
};

const json = (route, status, body) =>
  route.fulfill({ status, contentType: 'application/json', body: JSON.stringify(body) });

/** The document as lososd joins it: `set` per option from `overrides`, plus the strays. */
function joined(overrides = {}, stray = []) {
  return {
    available: true,
    version: DOC.version,
    options: DOC.options.map((option) => ({ ...option, set: overrides[option.name] ?? null })),
    stray,
    excluded: DOC.excluded,
  };
}

async function open({ path = '/settings/advanced', overrides = {}, stray = [], optionsStatus = 200, config = CONFIG, locale = 'en-US' } = {}) {
  const page = await browser.newPage({ viewport: { width: 1280, height: 900 }, locale });
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  await page.addInitScript(() => {
    window.__violations = [];
    document.addEventListener('securitypolicyviolation', (e) => window.__violations.push(e.violatedDirective));
  });
  const applies = [];
  const syncs = [];
  const authed = (route) => route.request().headers()['authorization'] === `Bearer ${TOKEN}`;
  await page.route('**/api/**', (route) => json(route, 200, {}));
  await page.route('**/api/setup/claim', (route) => json(route, 200, { claimed: true }));
  await page.route('**/api/state', (route) => json(route, 200, { mode: 'local', sharing: false }));
  await page.route('**/api/settings', (route) =>
    authed(route) ? json(route, 200, SETTINGS) : json(route, 401, { error: 'unauthorized' }),
  );
  await page.route('**/api/status', (route) => json(route, 200, { state: 'idle', progress: 0, message: '' }));
  await page.route('**/api/options', (route) =>
    optionsStatus === 200 ? json(route, 200, joined(overrides, stray)) : json(route, optionsStatus, { error: 'no such route' }),
  );
  await page.route('**/api/apply', (route) => {
    applies.push(route.request().postData());
    return json(route, 202, { job: 'job-1', state: 'building' });
  });
  await page.route('**/api/config', (route) => json(route, 200, config));
  await page.route('**/api/config/sync', (route) => {
    syncs.push(route.request().method());
    return json(route, 200, { ...config, sync: { ...config.sync, state: 'waiting', detail: 'A rebuild is running; the push waits.' } });
  });
  await page.addInitScript((t) => window.sessionStorage.setItem('losos-token', t), TOKEN);
  await page.goto(origin + path, { waitUntil: 'networkidle' });
  /* The bundle raises two style-src-elem reports at first paint on every
   * page (app.browser.mjs measures the same baseline); what this file guards
   * is that the two panes add none. */
  page.atLoad = await page.evaluate(() => window.__violations.length);
  return { page, errors, applies, syncs };
}

const main = (page) => page.locator('main');
const row = (page, name) => page.locator(`[data-option="${name}"]`);
/* React Aria's switch is an <input> under a <label> track; the track takes the click. */
const flip = (scope) => scope.locator('[data-slot="switch"]').click();
const applyButton = (page) => page.getByRole('button', { name: 'Apply', exact: true });
const nixLines = (body) => body.split('\n').filter((line) => /^\s*losos\./.test(line));
const noViolations = async (page) => {
  const violations = await page.evaluate(() => window.__violations);
  assert.equal(violations.length, page.atLoad, `the pane added CSP violations: ${violations.slice(page.atLoad).join(', ')}`);
};

console.log('admin-ui advanced + history browser checks');

await check('every option in the document is drawn, with an editor kind the pane knows', async () => {
  const { page, errors } = await open();
  await main(page).getByTestId('advanced-count').waitFor();
  assert.equal(await main(page).getByTestId('advanced-count').innerText(), `${DOC.options.length} options`);
  const rows = await page.locator('[data-option][data-kind]').evaluateAll((nodes) =>
    nodes.map((n) => [n.dataset.option, n.dataset.kind]),
  );
  assert.equal(rows.length, DOC.options.length, 'one row per option');
  for (const [name, kind] of rows) {
    assert.ok(KINDS.has(kind), `losos.${name} has editor kind ${kind}, which the pane cannot draw`);
  }
  const drawn = new Set(rows.map(([name]) => name));
  for (const option of DOC.options) assert.ok(drawn.has(option.name), `losos.${option.name} is not on the pane`);
  for (const option of DOC.options) {
    const r = row(page, option.name);
    const controls = await r.locator('input, select, textarea').count();
    if (option.readOnly) {
      assert.equal(controls, 0, `read-only losos.${option.name} has an editor`);
    } else if (option.name in OWNED && OWNED[option.name] !== 'market') {
      assert.equal(controls, 0, `owned losos.${option.name} has a second editor`);
      const href = await r.getByRole('link').getAttribute('href');
      assert.ok(href.endsWith(`/${OWNED[option.name]}`), `losos.${option.name} links to ${href}`);
    } else {
      assert.ok(controls >= 1, `losos.${option.name} (${option.editor.kind}) has no editor`);
    }
  }
  // The excluded prefixes are named, so a reader knows what is not here.
  for (const prefix of Object.keys(DOC.excluded)) {
    await main(page).getByText(`losos.${prefix}.*`, { exact: false }).first().waitFor();
  }
  await noViolations(page);
  assert.deepEqual(errors, []);
  await page.close();
});

await check('a plain switch writes one more line after the sixteen, and Apply sends it', async () => {
  const { page, applies } = await open();
  const r = row(page, 'configRepo.enable');
  await flip(r);
  await applyButton(page).waitFor();
  await main(page).getByText('1 change not applied yet').waitFor();
  await applyButton(page).click();
  await page.waitForTimeout(200);
  assert.equal(applies.length, 1);
  const lines = nixLines(applies[0]);
  assert.equal(lines.length, 17, `expected the sixteen lines plus one, got:\n${lines.join('\n')}`);
  assert.ok(lines.includes('  losos.configRepo.enable = false;'), lines.join('\n'));
  assert.ok(lines[0].startsWith('  losos.sharingMyStorage'), 'the sixteen come first');
  await page.close();
});

await check('a careful option asks first, and Keep as is changes nothing', async () => {
  const { page } = await open();
  await flip(row(page, 'hardening.enable'));
  const dialog = page.getByRole('dialog');
  await dialog.getByText('Change losos.hardening.enable?').waitFor();
  await dialog.getByRole('button', { name: 'Keep as is' }).click();
  await page.waitForTimeout(200);
  assert.equal(await applyButton(page).count(), 0, 'declining the change raised the apply bar');
  await flip(row(page, 'hardening.enable'));
  await page.getByRole('dialog').getByRole('button', { name: 'Change it' }).click();
  await applyButton(page).waitFor();
  // Once said yes to, the option does not ask again this visit.
  await flip(row(page, 'hardening.enable'));
  await page.waitForTimeout(200);
  assert.equal(await page.getByRole('dialog').count(), 0, 'asked a second time');
  assert.equal(await applyButton(page).count(), 0, 'toggling back left a change');
  await page.close();
});

await check('a number outside its bounds is refused by the field and keeps Apply off', async () => {
  const { page } = await open();
  const field = row(page, 'storage.fillPercent').getByRole('textbox');
  await field.fill('10');
  await field.press('Enter');
  await page.getByRole('dialog').getByRole('button', { name: 'Change it' }).click();
  await row(page, 'storage.fillPercent').getByText('Must be between 50 and 100.').waitFor();
  await applyButton(page).waitFor();
  assert.ok(await applyButton(page).isDisabled(), 'Apply is live with an invalid value');
  await field.fill('80');
  await field.press('Enter');
  await page.waitForTimeout(200);
  assert.equal(await row(page, 'storage.fillPercent').getByText('Must be between').count(), 0);
  assert.ok(!(await applyButton(page).isDisabled()));
  await page.close();
});

await check('a text option set on the box shows Set here, and Use default drops its line', async () => {
  const { page, applies } = await open({ overrides: { 'proxy.hostname': '"box.example"', 'tls.validityDays': '30' } });
  const r = row(page, 'proxy.hostname');
  await r.getByText('Set here').waitFor();
  assert.equal(await r.getByRole('textbox').inputValue(), 'box.example');
  await r.getByRole('button', { name: 'Use default' }).click();
  await main(page).getByText('1 change not applied yet').waitFor();
  await applyButton(page).click();
  await page.waitForTimeout(200);
  const lines = nixLines(applies[0]);
  assert.ok(!lines.some((line) => line.includes('proxy.hostname')), 'the dropped line was still written');
  assert.ok(lines.includes('  losos.tls.validityDays = 30;'), 'the other set line was lost');
  await page.close();
});

await check('a list is one item per line and a nullable path empties to unset', async () => {
  const { page, applies } = await open();
  const list = row(page, 'setup.finderOrigins').getByRole('textbox');
  await list.fill('https://a.example\nhttps://b.example');
  await list.blur();
  const path = row(page, 'proxy.noisePublicKeyFile').getByRole('textbox');
  await path.fill('/var/lib/x.pub');
  await path.press('Enter');
  await main(page).getByText('2 changes not applied yet').waitFor();
  await applyButton(page).click();
  await page.waitForTimeout(200);
  const lines = nixLines(applies[0]);
  assert.ok(lines.includes('  losos.setup.finderOrigins = [ "https://a.example" "https://b.example" ];'), lines.join('\n'));
  assert.ok(lines.includes('  losos.proxy.noisePublicKeyFile = "/var/lib/x.pub";'), lines.join('\n'));
  await page.close();
});

await check('${ in a value is refused by the field', async () => {
  const { page } = await open();
  const field = row(page, 'proxy.hostname').getByRole('textbox');
  await field.fill('${builtins.readFile "/etc/shadow"}');
  await field.press('Enter');
  await row(page, 'proxy.hostname').getByText('Must not contain ${.').waitFor();
  assert.ok(await applyButton(page).isDisabled());
  await page.close();
});

await check('a stray line is shown, blocks Apply, and Remove clears it', async () => {
  const { page, applies } = await open({ stray: [{ key: 'nextcloud.oldKnob', value: '1' }] });
  await main(page).getByText('Lines this box has no option for').waitFor();
  const stray = page.locator('[data-option="nextcloud.oldKnob"]');
  await stray.getByText('Not an option here').waitFor();
  // Toggle something so the bar is up, then see it refuse while the stray stays.
  await flip(row(page, 'configRepo.enable'));
  await applyButton(page).waitFor();
  assert.ok(await applyButton(page).isDisabled(), 'Apply is live with a stray line');
  await stray.getByRole('button', { name: 'Remove' }).click();
  await main(page).getByText('2 changes not applied yet').waitFor();
  assert.ok(!(await applyButton(page).isDisabled()));
  await applyButton(page).click();
  await page.waitForTimeout(200);
  assert.ok(!applies[0].includes('oldKnob'));
  await page.close();
});

await check('an owned option shows the form\'s value and links to its pane; the planned Market one edits here', async () => {
  const { page } = await open();
  const host = row(page, 'hostName');
  await host.getByText('mattbox', { exact: true }).waitFor();
  const link = host.getByRole('link', { name: 'Change under Network' });
  assert.equal(await link.getAttribute('href'), '/settings/network');
  await link.click();
  await page.waitForURL(origin + '/settings/network');
  await page.goBack({ waitUntil: 'networkidle' });
  await flip(row(page, 'sharingMyStorage'));
  await main(page).getByText('1 change not applied yet').waitFor();
  await page.close();
});

await check('search narrows the rows and says when nothing matches', async () => {
  const { page } = await open();
  const search = main(page).getByRole('searchbox');
  await search.fill('fillPercent');
  await main(page).getByText('1 option', { exact: true }).waitFor();
  assert.equal(await page.locator('[data-option]').count(), 1);
  await search.fill('zzz-nothing');
  await main(page).getByText('No option matches “zzz-nothing”.').waitFor();
  await page.close();
});

await check('a box without the options route still applies its sixteen lines', async () => {
  const { page, applies, errors } = await open({ path: '/settings/network', optionsStatus: 404 });
  await main(page).getByText('Encrypt the connection', { exact: true }).click();
  await applyButton(page).click();
  await page.waitForTimeout(200);
  assert.equal(nixLines(applies[0]).length, 16);
  await page.goto(origin + '/settings/advanced', { waitUntil: 'networkidle' });
  await main(page).getByText('does not serve its option document yet').waitFor();
  assert.deepEqual(errors, []);
  await page.close();
});

await check('History shows the repository, the sync state and the commits, and Sync now posts', async () => {
  const { page, syncs, errors } = await open({ path: '/settings/history' });
  const link = main(page).getByTestId('history-repo-link');
  assert.equal(await link.getAttribute('href'), '/forgejo/notshared/losos-config');
  await main(page).getByText(`${origin}/forgejo/notshared/losos-config.git`).waitFor();
  assert.equal(await main(page).getByTestId('history-sync-state').innerText(), 'In sync');
  await main(page).getByText('Change hostName, cluster.enable').waitFor();
  assert.equal(await page.locator('[data-commit]').count(), 2);
  await main(page).getByRole('button', { name: 'Sync now' }).click();
  await main(page).getByText('A rebuild is running; the push waits.').waitFor();
  assert.deepEqual(syncs, ['POST']);
  assert.equal(await main(page).getByTestId('history-sync-state').innerText(), 'Waiting');
  await noViolations(page);
  assert.deepEqual(errors, []);
  await page.close();
});

await check('History on a box with LosOS Git off says so and still lists the commits', async () => {
  const off = { enabled: false, repository: null, head: { sha: 'a'.repeat(40), branch: 'main' }, log: CONFIG.log, sync: { state: 'off', detail: 'LosOS Git is off on this box; changes are still committed here.', syncedAt: null, remoteHead: null } };
  const { page, errors } = await open({ path: '/settings/history', config: off });
  await main(page).getByText('LosOS Git is off on this box, so the configuration stays', { exact: false }).waitFor();
  assert.equal(await main(page).getByTestId('history-repo-link').count(), 0);
  assert.equal(await main(page).getByRole('button', { name: 'Sync now' }).count(), 0);
  assert.equal(await page.locator('[data-commit]').count(), 2);
  assert.deepEqual(errors, []);
  await page.close();
});

for (const locale of ['sk-SK', 'de-DE']) {
  await check(`both panes render in ${locale} without errors`, async () => {
    for (const path of ['/settings/advanced', '/settings/history']) {
      const { page, errors } = await open({ path, locale });
      await main(page).waitFor();
      await page.waitForTimeout(200);
      assert.deepEqual(errors, [], `${path} threw in ${locale}`);
      await page.close();
    }
  });
}

await browser.close();
await closeServer();
finish();
