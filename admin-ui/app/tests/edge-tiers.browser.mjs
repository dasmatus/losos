/* The tiers of a box's edge link on the Mesh page
 * (src/screens/settings/pane-mesh.tsx, EdgeGroup and ClusterGroup).
 *
 * What is checked: a link through a gateway reads as secondary and names its
 * cluster; a box on an official edge reads as tertiary and lists the edge's
 * official cluster; a box held by its custom domain says so and offers no
 * move to another cluster; a box that left a banned edge names it.
 *
 * LOSOS_SHOTS=<dir> also saves one picture per state there, for the PR.
 * Same arrangement as the other files: the real dist/ bundle under the real
 * policy, the routes stubbed with page.route(). */

import assert from 'node:assert';
import { mkdir } from 'node:fs/promises';
import { join } from 'node:path';
import { launch, runner, serve } from './harness.mjs';

const { origin, close: closeServer } = await serve({ csp: true });
const browser = await launch();
const { check, finish } = runner();
const SHOTS = process.env.LOSOS_SHOTS;
if (SHOTS) await mkdir(SHOTS, { recursive: true });

const json = (route, status, body) =>
  route.fulfill({ status, contentType: 'application/json', body: JSON.stringify(body) });

const SECTIONS = ['storage', 'backup', 'apps', 'market', 'network', 'look', 'lab', 'access'];
const SETTINGS = {
  sharingMyStorage: true,
  hostName: 'office-box',
  gpuEnable: false,
  proxyEnable: true,
  clusterEnable: true,
  shareCompute: false,
  computeWindowStart: '23:00',
  computeWindowEnd: '07:00',
  hardeningApparmor: false,
  hardeningMalloc: false,
  hardeningNosmt: false,
  hardeningUsbguard: false,
};

const GW_A = { name: 'gw-a', url: 'http://192.168.1.2:8080', source: 'lan', official: false, tier: 'secondary', cluster: 'office', member: 'gw-a' };
const GW_B = { name: 'gw-b', url: 'http://192.168.1.3:8080', source: 'lan', official: false, tier: 'secondary', cluster: 'office', member: 'gw-b' };
const EU_1 = { name: 'eu-1.losos.dasmat.us', url: 'https://eu-1.losos.dasmat.us', source: 'configured', official: true, tier: 'tertiary', officialCluster: 'eu' };
const EU_2 = { name: 'eu-2.losos.dasmat.us', url: 'https://eu-2.losos.dasmat.us', source: 'configured', official: true, tier: 'tertiary', officialCluster: 'eu' };

const pathOf = (e) => ({ name: e.name, url: e.url, source: e.source, tunnel: 'edge:2333', tier: e.tier, ...(e.cluster ? { cluster: e.cluster, member: e.member } : {}), ...(e.officialCluster ? { officialCluster: e.officialCluster } : {}) });

const STATES = {
  secondary: {
    reachable: true,
    official: true,
    edges: [GW_A, GW_B, EU_1],
    lanSearched: true,
    configuredUrl: EU_1.url,
    checkedAt: 1,
    path: pathOf(GW_A),
    clusters: [
      { name: 'office', gateways: ['gw-a', 'gw-b'] },
      { name: 'lab', gateways: ['lab-1'] },
    ],
    cluster: 'office',
    held: null,
    leftBanned: null,
  },
  held: {
    reachable: true,
    official: true,
    edges: [GW_A, GW_B, EU_1],
    lanSearched: true,
    configuredUrl: EU_1.url,
    checkedAt: 1,
    path: pathOf(GW_A),
    clusters: [
      { name: 'office', gateways: ['gw-a', 'gw-b'] },
      { name: 'lab', gateways: ['lab-1'] },
    ],
    cluster: 'office',
    held: GW_A.url,
    leftBanned: null,
  },
  banned: {
    reachable: true,
    official: true,
    edges: [EU_2],
    lanSearched: true,
    configuredUrl: EU_1.url,
    checkedAt: 1,
    path: pathOf(EU_2),
    clusters: [],
    cluster: null,
    held: null,
    leftBanned: EU_1.url,
  },
};

async function open(state) {
  const context = await browser.newContext({ viewport: { width: 1280, height: 900 }, locale: 'en-US' });
  const page = await context.newPage();
  await page.route('**/api/**', (route) => json(route, 200, {}));
  await page.route('**/api/setup/claim', (route) => json(route, 200, { claimed: true }));
  await page.route('**/api/me', (route) =>
    json(route, 200, { name: 'eva', displayName: 'Eva', groups: ['losos-admins'], admin: true, spareKey: false, sections: SECTIONS, standalone: true }),
  );
  await page.route('**/api/state', (route) => json(route, 200, { mode: 'mesh', sharing: true }));
  await page.route('**/api/settings', (route) => json(route, 200, SETTINGS));
  await page.route('**/api/status', (route) => json(route, 200, { state: 'idle', progress: 0, message: '' }));
  await page.route('**/api/edge', (route) => json(route, 200, STATES[state]));
  await page.addInitScript(() => window.localStorage.setItem('losos-tour', 'seen'));
  await page.goto(origin + '/settings/mesh', { waitUntil: 'networkidle' });
  return { page, context };
}

async function shoot(page, name) {
  if (!SHOTS) return;
  await page.screenshot({ path: join(SHOTS, `${name}.png`), fullPage: true });
}

await check('a link through a gateway is secondary and names its cluster', async () => {
  const { page, context } = await open('secondary');
  await page.getByText('Secondary connection through gw-a').waitFor();
  await page.getByText(/A local gateway of the cluster office/).waitFor();
  assert.equal(await page.locator('[data-link-held="yes"]').count(), 0);
  assert.equal(await page.getByRole('button', { name: 'Move here' }).isDisabled(), false);
  await shoot(page, 'secondary');
  await context.close();
});

await check('a custom domain holds the box: it says so and offers no move', async () => {
  const { page, context } = await open('held');
  await page.getByText(/Your custom domain is served through this edge/).waitFor();
  await page.getByText(/so the box cannot move to another cluster/).waitFor();
  assert.equal(await page.getByRole('button', { name: 'Move here' }).isDisabled(), true);
  await shoot(page, 'held');
  await context.close();
});

await check('a box that left a banned official edge names it and its official cluster', async () => {
  const { page, context } = await open('banned');
  await page.getByText('Tertiary connection to eu-2.losos.dasmat.us').waitFor();
  await page.getByText(`The box moved here because ${EU_1.url} is banned.`).waitFor();
  await page.getByText(/cluster eu/).first().waitFor();
  await shoot(page, 'banned');
  await context.close();
});

await browser.close();
await closeServer();
finish();
