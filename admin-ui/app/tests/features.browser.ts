/* The Features pane (pane-features.tsx): one switch per bool
 * losos.<name>.enable the box declares, drawn from the option document.
 *
 * Same arrangement as advanced.browser.ts: the dist/ bundle, the API stubbed
 * with page.route(), under the admin CSP. The option document is the real
 * one (LOSOS_OPTIONS_JSON) or the fixture copy for a run outside nix, so
 * the rows expected here are whatever the document declares — the pane's
 * rule (lib/option-value.ts, featurePaneOwns) is re-derived from the
 * document below rather than copied as a list of names, so a feature the
 * rule admits but the pane misses fails here, and a document without a
 * feature draws no row for it.
 */

import { type Locator, type Page, type Route } from 'playwright';

declare global {
  interface Window {
    __violations: string[];
  }
}

import assert from './assert.ts';
import { launch, runner, serve } from './harness.ts';

const TOKEN = 'a'.repeat(64);
const { origin, close: closeServer } = await serve({ csp: true });
const browser = await launch();
const { check, finish } = runner();

const DOC: {
  version: number;
  options: {
    name: string;
    readOnly?: boolean;
    danger?: boolean;
    hidden?: boolean;
    needs?: string;
    editor: { kind: string };
    default: unknown;
  }[];
  excluded: Record<string, unknown>;
} = await Bun.file(Bun.env.LOSOS_OPTIONS_JSON ?? 'tests/fixtures/options.json').json();

/* The thirteen names lib/api.ts's buildOverridesNix always writes; the
 * Features pane owns none of them. Kept as a list here so the test does not
 * import the bundle it is testing. */
const OWNED: Record<string, string> = {
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
/* Lines of overrides.nix the other panes draw an editor for themselves
 * (lib/option-value.ts, ON_PANE), as their rules over the document. */
const ON_PANE_OTHER: Record<string, string> = Object.fromEntries(
  DOC.options.flatMap((option) =>
    option.name === 'cloud.trashDays' || /^git\.site\./.test(option.name)
      ? [[option.name, 'apps']]
      : /^inference\.lend\.(enable|memoryMiB)$/.test(option.name)
        ? [[option.name, 'market']]
        : option.name === 'cloud.windows.enable' || /^hidden\.(tor|i2p)\.enable$/.test(option.name)
          ? [[option.name, 'network']]
          : option.name === 'updates.channel'
            ? [[option.name, 'history']]
            : [],
  ),
);
/* The rule pane-features draws by, re-derived from the document: every
 * bool *.enable no other pane owns, less the read-only, dangerous and
 * hidden ones. */
const featurePaneOwns = (option: (typeof DOC.options)[number]): boolean =>
  option.name.endsWith('.enable') &&
  option.editor.kind === 'bool' &&
  !option.readOnly &&
  !option.danger &&
  option.hidden !== true &&
  !(option.name in OWNED) &&
  !(option.name in ON_PANE_OTHER);
const DRAWN = DOC.options.filter(featurePaneOwns).map((option) => option.name);

const SETTINGS = {
  sharingMyStorage: false,
  sharingLocally: false,
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

const json = (route: Route, status: number, body: unknown) =>
  route.fulfill({ status, contentType: 'application/json', body: JSON.stringify(body) });

/** The document as lososd joins it: `set` per option from `overrides`. */
function joined(overrides: Record<string, string> = {}) {
  return {
    available: true,
    version: DOC.version,
    options: DOC.options.map((option) => ({ ...option, set: overrides[option.name] ?? null })),
    stray: [],
    excluded: DOC.excluded,
  };
}

async function open({ overrides = {} }: { overrides?: Record<string, string> } = {}): Promise<{
  page: Page;
  errors: string[];
  applies: (string | null)[];
  overrides: Record<string, string>;
}> {
  const page = await browser.newPage({ viewport: { width: 1280, height: 900 } });
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  await page.addInitScript(() => {
    window.__violations = [];
    document.addEventListener('securitypolicyviolation', (e) => window.__violations.push(e.violatedDirective));
  });
  const applies: (string | null)[] = [];
  const authed = (route: Route) => route.request().headers()['authorization'] === `Bearer ${TOKEN}`;
  await page.route('**/api/**', (route) => json(route, 200, {}));
  await page.route('**/api/setup/claim', (route) => json(route, 200, { claimed: true }));
  await page.route('**/api/state', (route) => json(route, 200, { mode: 'local', sharing: false }));
  await page.route('**/api/settings', (route) =>
    authed(route) ? json(route, 200, SETTINGS) : json(route, 401, { error: 'unauthorized' }),
  );
  await page.route('**/api/status', (route) => json(route, 200, { state: 'idle', progress: 0, message: '' }));
  await page.route('**/api/options', (route) => json(route, 200, joined(overrides)));
  await page.route('**/api/apply', (route) => {
    applies.push(route.request().postData());
    return json(route, 202, { job: 'job-1', state: 'building' });
  });
  await page.addInitScript((t: string) => window.sessionStorage.setItem('losos-token', t), TOKEN);
  await page.goto(origin + '/settings/features', { waitUntil: 'networkidle' });
  const atLoad = await page.evaluate(() => window.__violations.length);
  assert.equal(atLoad, 0, 'CSP violations at load');
  return { page, errors, applies, overrides };
}

const pane = (page: Page) => page.getByTestId('features-pane');
const row = (page: Page, name: string) => page.locator(`[data-option="${name}"]`);
/* React Aria's switch is a <label data-slot="switch"> holding a visually
 * hidden <input type="checkbox" role="switch">; the track takes the click,
 * the input's checked property says the state (it carries no aria-checked). */
const flip = (scope: Locator) => scope.locator('[data-slot="switch"]').click();
const checked = async (scope: Locator) => scope.locator('input[role="switch"]').isChecked();
const applyButton = (page: Page) => page.getByRole('button', { name: 'Apply', exact: true });
const nixLines = (body: string) => body.split('\n').filter((line) => /^\s*losos\./.test(line));
const noViolations = async (page: Page, atLoad: number) => {
  const violations = await page.evaluate(() => window.__violations);
  assert.equal(violations.length, atLoad, `the pane added CSP violations: ${violations.slice(atLoad).join(', ')}`);
};
/* What the row opens with: the override's literal, else the default. */
const opensOn = (name: string, overrides: Record<string, string>): boolean => {
  const raw = overrides[name];
  if (raw === 'true') return true;
  if (raw === 'false') return false;
  const option = DOC.options.find((o) => o.name === name)!;
  return typeof option.default === 'boolean' ? option.default : true;
};

/* The document's own anchors for the two toggle scenarios: a feature the
 * bare option set ships off, and one it ships on. */
const shipsOff = DRAWN.find((name) => opensOn(name, {}) === false);
const shipsOn = DRAWN.find((name) => opensOn(name, {}) === true);

console.log('admin-ui features browser checks');

await check('one switch per feature the box declares, and none for one it does not', async () => {
  const { page, errors, overrides } = await open({ overrides: { 'git.enable': 'false' } });
  const drawn = await pane(page)
    .locator('[data-option]')
    .evaluateAll((nodes) => nodes.map((n) => (n as HTMLElement).dataset.option ?? ''));
  assert.deepEqual(drawn.sort(), [...DRAWN].sort(), 'the pane draws exactly the features this box declares');
  // A switch per row, checked with the value in force: the override's
  // literal, else the option's default.
  for (const name of DRAWN) {
    const expect = opensOn(name, overrides);
    assert.equal(await row(page, name).locator('[data-slot="switch"]').count(), 1, `losos.${name} has one switch`);
    assert.equal(await checked(row(page, name)), expect, `losos.${name} opens ${expect ? 'on' : 'off'}`);
  }
  assert.deepEqual(errors, []);
  await noViolations(page, 0);
  await page.close();
});

await check(`a switch ships off (${shipsOff}) writes one line, and Apply sends it`, async () => {
  if (shipsOff === undefined) {
    console.log('  skip (the document ships every feature on)');
    return;
  }
  const { page, applies, errors } = await open();
  assert.equal(await checked(row(page, shipsOff)), false, `${shipsOff} opens off, its default`);
  await flip(row(page, shipsOff));
  assert.equal(await checked(row(page, shipsOff)), true, 'the switch turned on');
  await applyButton(page).waitFor();
  await page.getByText('1 change not applied yet').waitFor();
  await applyButton(page).click();
  await page.waitForTimeout(200);
  assert.equal(applies.length, 1);
  const lines = nixLines(applies[0]!);
  assert.ok(lines.includes(`  losos.${shipsOff} = true;`), `Apply carried the new line: ${lines.join(' | ')}`);
  // The thirteen keys are still there: the body is the whole file, not a
  // patch, and the thirteen come first as buildOverridesNix writes them.
  assert.ok(lines[0]!.startsWith('  losos.sharingMyStorage'), 'the thirteen come first');
  assert.deepEqual(errors, []);
  await noViolations(page, 0);
  await page.close();
});

await check(`a feature the box ships on (${shipsOn}) draws on, and off then on leaves no line`, async () => {
  if (shipsOn === undefined) {
    console.log('  skip (the document ships every feature off)');
    return;
  }
  const { page, applies, errors } = await open();
  assert.equal(await checked(row(page, shipsOn)), true, `${shipsOn} opens on, its default`);
  await flip(row(page, shipsOn));
  assert.equal(await checked(row(page, shipsOn)), false, 'turned off');
  await applyButton(page).click();
  await page.waitForTimeout(200);
  assert.ok(nixLines(applies[0]!).includes(`  losos.${shipsOn} = false;`), 'the off line was sent');
  // Back to the default: the line goes. The bar is no longer armed by
  // itself — the draft matches what the box runs with — so a second
  // change goes along and the one Apply carries both.
  await flip(row(page, shipsOn));
  assert.equal(await checked(row(page, shipsOn)), true, 'turned back on');
  assert.equal(await applyButton(page).count(), 0, 'returning to the default left nothing to apply');
  const other = DRAWN.find((name) => name !== shipsOn);
  if (other === undefined) return;
  await flip(row(page, other));
  await applyButton(page).click();
  await page.waitForTimeout(200);
  const lines = nixLines(applies[1]!);
  assert.ok(lines.includes(`  losos.${other} = true;`) || lines.includes(`  losos.${other} = false;`), 'the second change was sent');
  assert.ok(
    !lines.some((line) => line.startsWith(`  losos.${shipsOn}`)),
    'no line restates the default',
  );
  assert.deepEqual(errors, []);
  await page.close();
});

await check('an override the box already carries opens the row with it, and Apply keeps it', async () => {
  // A feature the box ships on, carried as off: the row must open with the
  // override, and an Apply of a change elsewhere must send that line
  // unchanged — the whole file goes, what the box already carries with it.
  const overridden = DRAWN.find((name) => opensOn(name, {}) === true);
  if (overridden === undefined) {
    console.log('  skip (no feature the document ships on)');
    return;
  }
  const { page, applies, errors, overrides } = await open({ overrides: { [overridden]: 'false' } });
  assert.equal(await checked(row(page, overridden)), false, `${overridden} opens off, its override`);
  const changed = DRAWN.find((name) => name !== overridden);
  if (changed === undefined) {
    await page.close();
    return;
  }
  await flip(row(page, changed));
  await applyButton(page).click();
  await page.waitForTimeout(200);
  const lines = nixLines(applies[0]!);
  assert.ok(lines.includes(`  losos.${overridden} = false;`), 'the override survived Apply');
  assert.ok(
    lines.includes(`  losos.${changed} = ${opensOn(changed, overrides) ? 'false' : 'true'};`),
    'the new change was sent beside it',
  );
  assert.deepEqual(errors, []);
  await page.close();
});

await browser.close();
await closeServer();
finish();
