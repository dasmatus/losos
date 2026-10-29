/* The Advanced pane (every losos.* option, typed editors) and the History
 * pane (the configuration repository on LosOS Git).
 *
 * The option document under test is the real one: LOSOS_OPTIONS_JSON points
 * at the file nix/options-doc.nix generated from the install configuration
 * (nix/tests/admin-ui.nix passes the `losos-options-doc` check), and the fixture
 * at tests/fixtures/options.json is a copy of it for a run outside nix.
 * Rendering the real document is the CI gate the pane exists for: an option
 * whose editor kind the pane cannot draw fails here, and one whose Nix type
 * nix/options-doc.nix cannot classify failed earlier, in the eval job.
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

/* The thirteen names lib/api.ts's buildOverridesNix always writes, and the
 * pane links to another pane for. Kept as a list here so the test does not
 * import the bundle it is testing. */
const OWNED = {
  sharingMyStorage: 'market',
  sharingLocally: 'mesh',
  hostName: 'network',
  'gpu.enable': 'hardware',
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
/* Lines of overrides.nix another pane draws its own editor for: each app's
 * box-wide settings, on the Apps pane, the
 * memory this box lends to AI models, on the Market pane, and the Windows
 * share and the Tor and I2P addresses, on the Network pane
 * (lib/option-value.ts, ON_PANE). The Advanced
 * row shows the value and links there, to the app's fold on the Apps pane. */
const ON_PANE = Object.fromEntries(
  DOC.options.flatMap((option) =>
    option.name === 'cloud.trashDays' || /^git\.site\./.test(option.name)
      ? [[option.name, 'apps']]
      : /^inference\.lend\.(enable|memoryMiB)$/.test(option.name)
        ? [[option.name, 'market']]
        : option.name === 'cloud.windows.enable' || /^hidden\.(tor|i2p)\.enable$/.test(option.name)
          ? [[option.name, 'network']]
          : [],
  ),
);
const fold = (name) => (name.startsWith('cloud.') ? 'cloud' : name.startsWith('git.') ? 'git' : null);
const ON_APPS = Object.keys(ON_PANE).filter((name) => ON_PANE[name] === 'apps');
const KINDS = new Set(['bool', 'str', 'int', 'float', 'enum', 'list', 'nullable', 'opaque']);

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

const CONFIG = {
  enabled: true,
  repository: {
    owner: 'notshared',
    name: 'losos-config',
    url: '/git/notshared/losos-config',
    clone: 'http://127.0.0.1:3000/notshared/losos-config.git',
    viaServer: true,
  },
  head: { sha: 'b'.repeat(40), branch: 'main' },
  log: [
    { sha: 'b'.repeat(40), when: '2026-10-07T10:12:00+02:00', subject: 'Change hostName, cluster.enable' },
    { sha: 'a'.repeat(40), when: '2026-10-06T03:00:00+02:00', subject: 'losos install' },
  ],
  sync: { state: 'ok', detail: 'In step with LosOS Git.', syncedAt: '2026-10-07T10:12:30+02:00', remoteHead: 'b'.repeat(40) },
  upstream: {
    url: 'https://github.com/dasmatus/losos.git',
    branch: 'main',
    report: { state: 'ok', detail: 'Took dasmatus/losos main at a088a5cb03; the box builds it at the next update.', syncedAt: '2026-10-07T03:12:00+02:00', remoteHead: 'a088a5cb03e9d1d0d770e6e1fd4bbfe17d5d5143' },
  },
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

async function open({ path = '/settings/advanced', overrides = {}, stray = [], optionsStatus = 200, config = CONFIG, locale = 'en-US', settings = SETTINGS } = {}) {
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
    authed(route) ? json(route, 200, settings) : json(route, 401, { error: 'unauthorized' }),
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
  /* The page loads with no violation (app.browser.mjs checks the same at
   * the Overview), and the two panes must add none. */
  page.atLoad = await page.evaluate(() => window.__violations.length);
  assert.equal(page.atLoad, 0, 'CSP violations at load');
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
  // visible = false options (losos.lab.ordering.enable) stay off the pane;
  // /api/apply and LosOS Git still take them.
  const SHOWN = DOC.options.filter((option) => !option.hidden);
  await main(page).getByTestId('advanced-count').waitFor();
  assert.equal(await main(page).getByTestId('advanced-count').innerText(), `${SHOWN.length} options`);
  const rows = await page.locator('[data-option][data-kind]').evaluateAll((nodes) =>
    nodes.map((n) => [n.dataset.option, n.dataset.kind]),
  );
  assert.equal(rows.length, SHOWN.length, 'one row per shown option');
  for (const [name, kind] of rows) {
    assert.ok(KINDS.has(kind), `losos.${name} has editor kind ${kind}, which the pane cannot draw`);
  }
  const drawn = new Set(rows.map(([name]) => name));
  for (const option of DOC.options) {
    assert.equal(drawn.has(option.name), !option.hidden, `losos.${option.name} ${option.hidden ? 'is hidden but drawn' : 'is not on the pane'}`);
  }
  for (const option of SHOWN) {
    const r = row(page, option.name);
    const controls = await r.locator('input, select, textarea').count();
    if (option.readOnly) {
      assert.equal(controls, 0, `read-only losos.${option.name} has an editor`);
    } else if (option.name in OWNED) {
      assert.equal(controls, 0, `owned losos.${option.name} has a second editor`);
      const href = await r.getByRole('link').getAttribute('href');
      assert.ok(href.endsWith(`/${OWNED[option.name]}`), `losos.${option.name} links to ${href}`);
    } else if (option.name in ON_PANE) {
      assert.equal(controls, 0, `losos.${option.name} has a second editor beside the ${ON_PANE[option.name]} pane's`);
      const href = await r.getByRole('link').getAttribute('href');
      const hash = fold(option.name) === null ? '' : `#${fold(option.name)}`;
      assert.ok(href.endsWith(`/${ON_PANE[option.name]}${hash}`), `losos.${option.name} links to ${href}`);
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

await check('a plain switch writes one more line after the thirteen, and Apply sends it', async () => {
  const { page, applies } = await open();
  const r = row(page, 'configRepo.enable');
  await flip(r);
  await applyButton(page).waitFor();
  await main(page).getByText('1 change not applied yet').waitFor();
  await applyButton(page).click();
  await page.waitForTimeout(200);
  assert.equal(applies.length, 1);
  const lines = nixLines(applies[0]);
  assert.equal(lines.length, 14, `expected the thirteen lines plus one, got:\n${lines.join('\n')}`);
  assert.ok(lines.includes('  losos.configRepo.enable = false;'), lines.join('\n'));
  assert.ok(lines[0].startsWith('  losos.sharingMyStorage'), 'the thirteen come first');
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

await check('a list is one item per line and a nullable string is written quoted', async () => {
  const { page, applies } = await open();
  const list = row(page, 'setup.finderOrigins').getByRole('textbox');
  await list.fill('https://a.example\nhttps://b.example');
  await list.blur();
  const domain = row(page, 'mail.domain').getByRole('textbox');
  await domain.fill('box.example.org');
  await domain.press('Enter');
  await main(page).getByText('2 changes not applied yet').waitFor();
  await applyButton(page).click();
  await page.waitForTimeout(200);
  const lines = nixLines(applies[0]);
  assert.ok(lines.includes('  losos.setup.finderOrigins = [ "https://a.example" "https://b.example" ];'), lines.join('\n'));
  assert.ok(lines.includes('  losos.mail.domain = "box.example.org";'), lines.join('\n'));
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
  const { page, applies } = await open({ stray: [{ key: 'oldApp.knob', value: '1' }] });
  await main(page).getByText('Lines this box has no option for').waitFor();
  const stray = page.locator('[data-option="oldApp.knob"]');
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

await check('an owned option shows the form\'s value and links to its pane', async () => {
  const { page } = await open();
  const host = row(page, 'hostName');
  await host.getByText('mattbox', { exact: true }).waitFor();
  const link = host.getByRole('link', { name: 'Change under Network' });
  assert.equal(await link.getAttribute('href'), '/settings/network');
  await link.click();
  await page.waitForURL(origin + '/settings/network');
  await page.goBack({ waitUntil: 'networkidle' });
  // Disk sharing belongs to the Market pane.
  const sharing = row(page, 'sharingMyStorage').getByRole('link', { name: 'Change under Market' });
  assert.equal(await sharing.getAttribute('href'), '/settings/market');
  await page.close();
});

await check('search narrows the rows and says when nothing matches', async () => {
  const { page } = await open();
  const search = main(page).getByRole('searchbox');
  await search.fill('fillPercent');
  await main(page).getByText('1 option', { exact: true }).waitFor();
  assert.equal(await page.locator('[data-option]').count(), 1);
  await search.fill('zzz-nothing');
  await main(page).getByText('No option matches "zzz-nothing".').waitFor();
  await page.close();
});

await check('a box without the options route still applies its thirteen lines', async () => {
  const { page, applies, errors } = await open({ path: '/settings/network', optionsStatus: 404 });
  await main(page).getByText('Reachable from outside your home', { exact: true }).click();
  await applyButton(page).click();
  await page.waitForTimeout(200);
  assert.equal(nixLines(applies[0]).length, 13);
  await page.goto(origin + '/settings/advanced', { waitUntil: 'networkidle' });
  await main(page).getByText('does not serve its option document yet').waitFor();
  assert.deepEqual(errors, []);
  await page.close();
});

await check('History shows the repository, the sync state and the commits, and Sync now posts', async () => {
  const { page, syncs, errors } = await open({ path: '/settings/history' });
  const link = main(page).getByTestId('history-repo-link');
  assert.equal(await link.getAttribute('href'), '/git/notshared/losos-config');
  await main(page).getByText(`${origin}/git/notshared/losos-config.git`).waitFor();
  assert.equal(await main(page).getByTestId('history-sync-state').innerText(), 'In sync');
  await main(page).getByText('Change hostName, cluster.enable').waitFor();
  assert.equal(await page.locator('[data-commit]').count(), 2);
  await main(page).getByRole('button', { name: 'Sync now' }).click();
  await main(page).getByText('A rebuild is running; the push waits.').waitFor();
  assert.deepEqual(syncs, ['POST']);
  assert.equal(await main(page).getByTestId('history-sync-state').innerText(), 'Waiting');
  assert.equal(await main(page).getByTestId('history-update-state').innerText(), 'Up to date');
  await main(page).getByText('https://github.com/dasmatus/losos.git').waitFor();
  await main(page).getByText('Newest published version a088a5cb03').waitFor();
  await noViolations(page);
  assert.deepEqual(errors, []);
  await page.close();
});

await check('History says when the box kept an edit instead of updating, and when an older daemon sends no upstream', async () => {
  const kept = { ...CONFIG, upstream: { ...CONFIG.upstream, report: { state: 'kept', detail: 'Files other than overrides.nix and install-target.nix were changed.', syncedAt: '2026-10-08T03:05:00+02:00', remoteHead: 'd'.repeat(40) } } };
  let { page, errors } = await open({ path: '/settings/history', config: kept });
  assert.equal(await main(page).getByTestId('history-update-state').innerText(), 'Kept your edits');
  await main(page).getByText('Files other than overrides.nix and install-target.nix were changed.').waitFor();
  assert.deepEqual(errors, []);
  await page.close();
  const { upstream: _, ...older } = CONFIG;
  ({ page, errors } = await open({ path: '/settings/history', config: older }));
  assert.equal(await main(page).getByTestId('history-update-state').innerText(), 'Off');
  await main(page).getByText('This box does not take new versions.', { exact: false }).waitFor();
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

await check('the Apps pane draws every box-wide setting of both apps, and Apply writes their lines', async () => {
  assert.equal(ON_APPS.length, 4, `expected 1 LosOS Cloud and 3 LosOS Git settings, the document has ${ON_APPS.length}`);
  const { page, applies, errors } = await open({ path: '/apps', overrides: { 'cloud.trashDays': '14' } });
  // Each app's settings are folded away until its fold is opened.
  await main(page).getByTestId('accordion-cloud').waitFor();
  assert.equal(await main(page).getByTestId('site-cloud').isVisible(), false, 'the LosOS Cloud fold starts open');
  await main(page).getByTestId('accordion-cloud').getByRole('button').first().click();
  await main(page).getByTestId('accordion-git').getByRole('button').first().click();
  await main(page).getByTestId('site-cloud').waitFor();
  await main(page).getByTestId('site-git').waitFor();
  for (const name of ON_APPS) {
    assert.equal(await row(page, name).count(), 1, `losos.${name} has no row on the Apps pane`);
    assert.ok((await row(page, name).locator('input, select').count()) >= 1, `losos.${name} has no control on the Apps pane`);
  }
  // A value the box already carries is shown, not the default.
  const trash = row(page, 'cloud.trashDays').getByRole('textbox');
  assert.equal(await trash.inputValue(), '14');
  await trash.fill('0');
  await trash.press('Enter');
  await row(page, 'cloud.trashDays').getByText('Must be between 1 and 365.', { exact: false }).waitFor();
  // Cleared, the field goes back to the default and the line goes.
  await trash.fill('');
  await trash.press('Enter');
  // The field takes the default from the draft one render later, so wait for it.
  await trash.evaluate((input) => new Promise((done) => {
    const settled = () => (input.value === '30' ? done() : requestAnimationFrame(settled));
    setTimeout(done, 2000);
    settled();
  }));
  assert.equal(await trash.inputValue(), '30');
  await flip(row(page, 'git.site.requireSignIn'));
  await flip(row(page, 'git.site.pushCreate'));
  // Picking the default adds no line; clearing a field above dropped one.
  await row(page, 'git.site.defaultPrivate').locator('select').selectOption('private');
  await row(page, 'git.site.defaultPrivate').locator('select').selectOption('last');

  await main(page).getByText('3 changes not applied yet').waitFor();
  await applyButton(page).click();
  await page.waitForTimeout(200);
  assert.equal(applies.length, 1);
  const lines = nixLines(applies[0]);
  for (const line of ['  losos.git.site.requireSignIn = true;', '  losos.git.site.pushCreate = true;']) {
    assert.ok(lines.includes(line), `missing ${line.trim()} in:\n${lines.join('\n')}`);
  }
  assert.ok(!lines.some((line) => line.includes('trashDays')), 'clearing a field back to the default kept its line');
  assert.ok(!lines.some((line) => line.includes('defaultPrivate')), 'a default the box had unset was written');
  assert.equal(lines.length, 13 + 2);
  await noViolations(page);
  assert.deepEqual(errors, []);
  await page.close();
});

await check('the Market pane lends memory to AI models with two lines of overrides.nix, and only with the edge proxy on', async () => {
  let { page, applies, errors } = await open({ path: '/settings/market' });
  const lend = main(page).getByTestId('lend-memory');
  await lend.waitFor();
  // Lending reaches the box only through the edge's tunnel.
  assert.equal(await row(page, 'inference.lend.enable').locator('input').isDisabled(), true, 'lending can be turned on with the proxy off');
  await lend.getByText('Turn on "Reachable from outside your home" in Network first.', { exact: false }).waitFor();
  assert.deepEqual(errors, []);
  await page.close();

  ({ page, applies, errors } = await open({ path: '/settings/market', settings: { ...SETTINGS, proxyEnable: true } }));
  await main(page).getByTestId('lend-memory').waitFor();
  await flip(row(page, 'inference.lend.enable'));
  const amount = page.locator('[data-option="inference.lend.memoryMiB"]');
  assert.equal(await amount.inputValue(), '1', 'the default 1024 MiB is not shown as 1 GiB');
  await amount.fill('300');
  await main(page).getByText('Lend a whole number of GiB, from 1 to 256.').waitFor();
  await amount.fill('24');
  await main(page).getByText('2 changes not applied yet').waitFor();
  await applyButton(page).click();
  await page.waitForTimeout(200);
  assert.equal(applies.length, 1);
  const lines = nixLines(applies[0]);
  assert.ok(lines.includes('  losos.inference.lend.enable = true;'), lines.join('\n'));
  assert.ok(lines.includes('  losos.inference.lend.memoryMiB = 24576;'), lines.join('\n'));
  await noViolations(page);
  assert.deepEqual(errors, []);
  await page.close();

  // A box that already lends shows its amount, and turning it off drops
  // both lines back to the defaults.
  ({ page, applies, errors } = await open({
    path: '/settings/market',
    overrides: { 'inference.lend.enable': 'true', 'inference.lend.memoryMiB': '8192' },
  }));
  await main(page).getByTestId('lend-memory').waitFor();
  assert.equal(await page.locator('[data-option="inference.lend.memoryMiB"]').inputValue(), '8');
  assert.equal(await row(page, 'inference.lend.enable').locator('input').isDisabled(), false, 'lending cannot be turned off with the proxy off');
  await flip(row(page, 'inference.lend.enable'));
  await applyButton(page).click();
  await page.waitForTimeout(200);
  assert.equal(applies.length, 1);
  assert.ok(!nixLines(applies[0]).some((line) => line.includes('inference.lend.enable')), 'turning lending off kept its line');
  assert.ok(nixLines(applies[0]).includes('  losos.inference.lend.memoryMiB = 8192;'), 'the amount was dropped');
  assert.deepEqual(errors, []);
  await page.close();
});

await check('the Advanced pane links each box-wide setting to the Apps pane', async () => {
  const { page } = await open({ overrides: { 'git.site.defaultPrivate': '"public"' } });
  const r = row(page, 'git.site.defaultPrivate');
  await r.getByText('public', { exact: true }).waitFor();
  await r.getByText('Set here').waitFor();
  const link = r.getByRole('link', { name: 'Change under Apps' });
  assert.equal(await link.getAttribute('href'), '/apps#git');
  await link.click();
  await page.waitForURL(origin + '/apps#git');
  // The link opens the LosOS Git fold and leaves LosOS Cloud's shut.
  await main(page).getByTestId('site-git').waitFor();
  assert.equal(await main(page).getByTestId('site-cloud').isVisible(), false);
  await page.close();
});

for (const locale of ['sk-SK', 'de-DE']) {
  await check(`both panes render in ${locale} without errors`, async () => {
    for (const path of ['/settings/advanced', '/settings/history', '/apps']) {
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
