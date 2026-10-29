/* A company edge managing the boxes behind it: the edge's page at /manage/
 * (src/manage/) and the owner's switch on the box's Mesh page
 * (src/screens/settings/pane-mesh.tsx, ManageGroup).
 *
 * What is checked: the page asks for the edge's manage key and says what is
 * wrong with a bad one; it lists the boxes and opens only one whose owner
 * switched management on; every call for that box goes through the edge's
 * relay with the key; the box's LosOS Cloud and LosOS Git rows change only
 * the options named, through POST /api/site, and the bar follows the rebuild
 * to its end; the apps it lists have no link to their port and no pin; a
 * box that refuses the edge says so; a deep link reloads; and the page logs
 * no Content-Security-Policy violation under the edge's own policy beyond
 * what the admin page itself logs. On the
 * box, the switch sends POST /api/edge/manage, for a checked gateway on the
 * LAN as for any other edge there.
 *
 * Same arrangement as the other files: the real dist/ bundle under the real
 * policies, the routes stubbed with page.route(). */

import assert from './assert.mjs';
import { launch, runner, serve } from './harness.mjs';

const { origin, close: closeServer } = await serve({ csp: true });
const browser = await launch();
const { check, finish } = runner();

const KEY = 'a'.repeat(64);
const BOXES = [
  { id: 'office-box', hostname: 'office-box', seenAgoSecs: 12, managed: true },
  { id: 'reception', hostname: 'reception', seenAgoSecs: 300, managed: false },
];
const DOC = await Bun.file(Bun.env.LOSOS_OPTIONS_JSON ?? 'tests/fixtures/options.json').json();
const SITE_NAMES = /^(cloud\.trashDays|git\.site\..+)$/;
const SITE = {
  available: true,
  version: DOC.version,
  options: DOC.options.filter((o) => SITE_NAMES.test(o.name)).map((o) => ({ ...o, set: null })),
  stray: [],
  excluded: {},
};
const APPS = {
  available: true,
  sharedAvailable: false,
  apps: [
    {
      release: 'pihole',
      kind: 'chart',
      title: null,
      runtime: 'podman',
      chart: { repo: 'https://mojo2600.github.io/pihole-kubernetes/', name: 'pihole', version: '2.27.0' },
      runAs: 'notshared',
      values: {},
      valuesYaml: null,
      frontPort: 30080,
      appPort: 80,
      phase: 'running',
      message: null,
      updatedAt: 0,
    },
  ],
};

const json = (route, status, body) =>
  route.fulfill({ status, contentType: 'application/json', body: JSON.stringify(body) });

const violations = (page) => page.evaluate(() => [...new Set(window.__violations)].sort());

/* The edge's page. `signedIn` puts the key in the tab first; `refuse`
 * makes the box answer 403, as the edge does once the owner switched off. */
async function openEdge({ path = '/manage/', signedIn = true, refuse = false, gateway = null } = {}) {
  const context = await browser.newContext({ viewport: { width: 1100, height: 900 }, locale: 'en-US' });
  const page = await context.newPage();
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  const calls = [];
  const posts = [];
  const writes = [];
  let job = 0;
  let polls = 0;

  await page.route('**/manage/api/**', (route) => {
    const request = route.request();
    const url = new URL(request.url());
    calls.push({ method: request.method(), path: url.pathname + url.search, auth: request.headers()['authorization'] ?? null });
    if (request.headers()['authorization'] !== `Bearer ${KEY}`) return json(route, 401, { error: 'sign in with the manage key' });
    if (url.pathname === '/manage/api/boxes') return json(route, 200, { boxes: BOXES });
    if (url.pathname.startsWith('/manage/api/gateway')) {
      if (gateway === null) return json(route, 404, { error: 'no such route' });
      if (request.method() === 'GET') return json(route, 200, gateway);
      const body = request.postData() === null ? null : JSON.parse(request.postData());
      writes.push({ method: request.method(), path: url.pathname, body });
      if (url.pathname === '/manage/api/gateway/uplink' && request.method() === 'PUT') {
        if (!/^[^:\s]+:\d+$/.test(body.tunnelEndpoint)) return json(route, 400, { error: 'the tunnel endpoint must be host:port' });
        gateway.uplink.set = { id: body.id, registrarUrl: body.registrarUrl, tunnelEndpoint: body.tunnelEndpoint };
        return json(route, 200, { set: true });
      }
      if (url.pathname === '/manage/api/gateway/uplink') {
        gateway.uplink.set = null;
        return json(route, 200, { set: false });
      }
      if (url.pathname === '/manage/api/gateway/cluster' && request.method() === 'PUT') {
        if (!/^[a-z0-9-]+$/.test(body.name)) return json(route, 400, { error: 'the cluster name must be a DNS label' });
        const joined = {
          name: body.name,
          member: 'losos-edge',
          secret: body.secret ?? 'c'.repeat(64),
          url: 'http://losos-edge.local:8443',
          gateways: [{ member: 'losos-edge', url: 'http://losos-edge.local:8443' }],
        };
        gateway.cluster.joined = joined;
        return json(route, 200, joined);
      }
      if (url.pathname === '/manage/api/gateway/cluster') {
        gateway.cluster.joined = null;
        return json(route, 200, { joined: null });
      }
      const forget = url.pathname.match(/^\/manage\/api\/gateway\/boxes\/([^/]+)$/);
      if (forget !== null) {
        gateway.enrolled = gateway.enrolled.filter((id) => id !== forget[1]);
        return json(route, 200, { forgotten: forget[1] });
      }
      return json(route, 404, { error: 'no such route' });
    }
    const box = url.pathname.match(/^\/manage\/api\/boxes\/([^/]+)(\/api\/.*)$/);
    if (box === null) return json(route, 404, { error: 'no such route' });
    const [, id, rest] = box;
    if (id !== 'office-box' || refuse) return json(route, 403, { error: 'the box does not let this edge manage it' });
    if (rest === '/api/site' && request.method() === 'POST') {
      posts.push({ path: rest, body: JSON.parse(request.postData() ?? '{}') });
      job += 1;
      polls = 0;
      return json(route, 200, { job: `job-${job}`, commit: null });
    }
    if (rest === '/api/site') return json(route, 200, SITE);
    if (rest === '/api/apps') return json(route, 200, APPS);
    if (rest === '/api/status') {
      polls += 1;
      return json(route, 200, polls < 2
        ? { state: 'building', progress: 0, message: 'building the new system', job: `job-${job}` }
        : { state: 'done', progress: 100, message: '', job: `job-${job}` });
    }
    return json(route, 200, {});
  });

  await page.route('**/passport/settings', (route) => {
    const request = route.request();
    const body = JSON.parse(request.postData() ?? '{}');
    writes.push({ method: request.method(), path: '/passport/settings', auth: request.headers()['authorization'] ?? null, body });
    if (!body.signature.includes('BEGIN PGP SIGNATURE')) return json(route, 403, { error: 'not signed by the owner' });
    return json(route, 200, { accepted: true, rebooting: true });
  });

  await page.addInitScript(
    ([key, signedIn]) => {
      if (signedIn) window.sessionStorage.setItem('losos-token', key);
      window.__violations = [];
      document.addEventListener('securitypolicyviolation', (e) =>
        window.__violations.push(`${e.violatedDirective} ${e.blockedURI}`),
      );
    },
    [KEY, signedIn],
  );
  await page.goto(origin + path, { waitUntil: 'networkidle' });
  return { page, context, errors, calls, posts, writes };
}

/* What GET /manage/api/gateway says about an unclaimed gateway with no hub
 * and one box that enrolled on first contact. A fresh copy per page. */
const gatewayFixture = () => ({
  name: 'losos-edge',
  passport: { edgeKey: 'e'.repeat(64), claimed: false, owner: null },
  uplink: { set: null, relaying: 0 },
  enrolled: ['reception'],
  cluster: { joined: null },
  main: [],
});

console.log('admin-ui edge manage page checks');

await check('the page asks for the key and says what is wrong with a bad one', async () => {
  const { page, context, errors } = await openEdge({ signedIn: false });
  await page.getByRole('heading', { name: 'Manage the boxes behind this edge', level: 1 }).waitFor();
  const field = page.getByLabel('Manage key');
  await field.fill('not-a-key');
  await page.getByRole('button', { name: 'Sign in' }).click();
  assert.match(await page.getByRole('alert').innerText(), /64 characters/);
  await field.fill('b'.repeat(64));
  await page.getByRole('button', { name: 'Sign in' }).click();
  await page.getByRole('alert').filter({ hasText: 'does not know that key' }).waitFor();
  await field.fill(KEY);
  await page.getByRole('button', { name: 'Sign in' }).click();
  await page.getByRole('heading', { name: 'Boxes', level: 1 }).waitFor();
  assert.deepEqual(errors, []);
  await context.close();
});

await check('only a box whose owner switched management on can be opened', async () => {
  const { page, context, calls } = await openEdge();
  const office = page.getByTestId('box-office-box');
  await office.waitFor();
  assert.match(await office.innerText(), /Managed here/);
  assert.match(await office.innerText(), /just now/);
  const reception = page.getByTestId('box-reception');
  assert.match(await reception.innerText(), /Not switched on/);
  assert.match(await reception.innerText(), /5 minutes ago/);
  assert.match(await reception.innerText(), /has not let this edge manage it/);
  assert.equal(await reception.getByRole('link').count(), 0);
  assert.ok(calls.every((c) => c.auth === `Bearer ${KEY}`), JSON.stringify(calls));
  await office.getByRole('link', { name: 'Manage' }).click();
  await page.waitForURL(origin + '/manage/boxes/office-box');
  await page.getByRole('heading', { name: 'office-box', level: 1 }).waitFor();
  await context.close();
});

await check("a box's settings change through the edge's relay, only the lines named", async () => {
  const { page, context, calls, posts, errors } = await openEdge({ path: '/manage/boxes/office-box' });
  await page.getByTestId('site-cloud').waitFor();
  // Everything for the box went through the relay with the key.
  const boxCalls = calls.filter((c) => c.path.startsWith('/manage/api/boxes/office-box/'));
  assert.ok(boxCalls.some((c) => c.path === '/manage/api/boxes/office-box/api/site'), JSON.stringify(calls));
  assert.ok(boxCalls.some((c) => c.path === '/manage/api/boxes/office-box/api/apps'), JSON.stringify(calls));
  assert.ok(calls.every((c) => c.path.startsWith('/manage/api/')), JSON.stringify(calls));

  const trash = page.getByLabel('Empty the trash after');
  await trash.fill('14');
  await trash.blur();
  await page.getByLabel('A new repository starts out').selectOption('public');
  await page.getByRole('button', { name: 'Apply' }).click();
  await page.getByText('The box runs its new settings', { exact: true }).waitFor({ timeout: 15000 });
  assert.deepEqual(posts, [
    {
      path: '/api/site',
      body: { set: { 'git.site.defaultPrivate': '"public"', 'cloud.trashDays': '14' } },
    },
  ]);
  assert.deepEqual(errors, []);
  await context.close();
});

await check('the apps listed from the edge have no port link and no pin', async () => {
  const { page, context } = await openEdge({ path: '/manage/boxes/office-box' });
  await page.getByText('pihole', { exact: true }).waitFor();
  assert.equal(await page.getByRole('link', { name: 'Open' }).count(), 0);
  assert.equal(await page.getByRole('button', { name: /^Pin/ }).count(), 0);
  assert.equal(await page.getByRole('button', { name: 'Change', exact: true }).count(), 1);
  await context.close();
});

await check('a box that refuses the edge says so', async () => {
  const { page, context } = await openEdge({ path: '/manage/boxes/office-box', refuse: true });
  await page.getByText('The box refused this edge').waitFor();
  await context.close();
});

await check('the gateway is set up from the page: its owner, its hub and the boxes that enrolled', async () => {
  const gateway = gatewayFixture();
  const { page, context, writes, errors } = await openEdge({ gateway });
  const section = page.getByTestId('gateway');
  await section.waitFor();
  assert.match(await section.innerText(), /losos-edge/);
  assert.match(await section.innerText(), /No owner yet/);
  assert.match(await section.innerText(), /no console login and no SSH/);

  // Claim: an unsigned paste is refused and says why; a signed one is sent
  // without the manage key, because the signature is the proof.
  await section.getByRole('button', { name: 'Claim' }).click();
  await page.getByLabel('Settings file').fill('{"version":1}');
  await page.getByLabel('Signature').fill('not a signature');
  await section.getByRole('button', { name: 'Save' }).click();
  await section.getByRole('alert').filter({ hasText: "not from this gateway's owner" }).waitFor();
  await page.getByLabel('Signature').fill('-----BEGIN PGP SIGNATURE-----\nabc\n-----END PGP SIGNATURE-----');
  await section.getByRole('button', { name: 'Save' }).click();
  await page.getByText('Settings accepted', { exact: true }).waitFor();
  const claim = writes.filter((w) => w.path === '/passport/settings').at(-1);
  assert.equal(claim.auth, null);
  assert.equal(claim.body.settings, '{"version":1}\n');

  // The hub: a malformed endpoint is refused with the edge's words, a good
  // row is saved, then cleared.
  const hub = page.getByTestId('uplink');
  assert.match(await hub.innerText(), /reachable on this network only/);
  await hub.getByRole('button', { name: 'Set up' }).click();
  await hub.getByLabel('Tenant id').fill('acme');
  await hub.getByLabel('Registrar URL').fill('https://register.example.test');
  await hub.getByLabel('Tunnel endpoint').fill('relay');
  await hub.getByLabel('Token', { exact: true }).fill('hub-token');
  await hub.getByRole('button', { name: 'Save' }).click();
  await hub.getByText('must be host:port').waitFor();
  await hub.getByLabel('Tunnel endpoint').fill('relay.example.test:2333');
  await hub.getByRole('button', { name: 'Save' }).click();
  await hub.getByText('acme via https://register.example.test').waitFor();
  const put = writes.filter((w) => w.method === 'PUT').at(-1);
  assert.deepEqual(put.body, {
    id: 'acme',
    token: 'hub-token',
    registrarUrl: 'https://register.example.test',
    tunnelEndpoint: 'relay.example.test:2333',
  });
  await hub.getByRole('button', { name: 'Stop relaying' }).click();
  await hub.getByText('Not relaying').waitFor();

  // A box that enrolled on first contact is forgotten.
  const enrolled = page.getByTestId('enrolled');
  await enrolled.getByRole('button', { name: 'Forget' }).click();
  await enrolled.waitFor({ state: 'detached' });
  assert.ok(writes.some((w) => w.method === 'DELETE' && w.path === '/manage/api/gateway/boxes/reception'), JSON.stringify(writes));
  assert.deepEqual(errors, []);
  await context.close();
});

await check('the gateway starts, shows and leaves a secondary cluster from the page, with no shell', async () => {
  const gateway = gatewayFixture();
  gateway.main = [{ site: 'acme', gateway: 'losos-edge', cluster: null, boxes: 3 }];
  const { page, context, writes, errors } = await openEdge({ gateway });
  const section = page.getByTestId('cluster');
  await section.getByText('In no cluster').waitFor();
  await section.getByRole('button', { name: 'Start a cluster' }).click();
  await section.getByLabel('Name').fill('Office!');
  await section.getByRole('button', { name: 'Start a cluster' }).click();
  await section.getByRole('alert').filter({ hasText: 'DNS label' }).waitFor();
  await section.getByLabel('Name').fill('office');
  await section.getByRole('button', { name: 'Start a cluster' }).click();
  await section.getByText('Cluster office').waitFor();
  const line = section.getByTestId('cluster-join-line');
  assert.match(await line.innerText(), /c{64}/);
  assert.match(await line.innerText(), /http:\/\/losos-edge\.local:8443/);
  assert.deepEqual(writes.filter((w) => w.method === 'PUT').at(-1).body, { name: 'office' });
  assert.match(await page.getByTestId('main-cluster').innerText(), /acme · losos-edge/);

  await section.getByRole('button', { name: 'Leave' }).click();
  await section.getByText('In no cluster').waitFor();
  await section.getByRole('button', { name: 'Join a cluster' }).click();
  await section.getByLabel('Name').fill('office');
  await section.getByLabel('Gateway address').fill('http://gw-a.local:8443');
  await section.getByLabel('Secret').fill('d'.repeat(64));
  await section.getByRole('button', { name: 'Join a cluster' }).click();
  await section.getByText('Cluster office').waitFor();
  assert.deepEqual(writes.filter((w) => w.method === 'PUT').at(-1).body, {
    name: 'office',
    secret: 'd'.repeat(64),
    peer: 'http://gw-a.local:8443',
  });
  assert.ok(writes.some((w) => w.method === 'DELETE' && w.path === '/manage/api/gateway/cluster'));
  assert.deepEqual(errors, []);
  await context.close();
});

/* What a plain load of the admin page logs: the edge's page, built from the
 * same parts, must not add to it. */
async function baseline() {
  const context = await browser.newContext({ viewport: { width: 1100, height: 900 } });
  const page = await context.newPage();
  await page.route('**/api/**', (route) => json(route, 401, { error: 'unauthorized' }));
  await page.addInitScript(() => {
    window.__violations = [];
    document.addEventListener('securitypolicyviolation', (e) =>
      window.__violations.push(`${e.violatedDirective} ${e.blockedURI}`),
    );
  });
  await page.goto(origin + '/', { waitUntil: 'networkidle' });
  await page.waitForTimeout(300);
  const seen = await violations(page);
  await context.close();
  return seen;
}
const BASELINE = await baseline();

await check('a deep link reloads, and the page adds no policy violation', async () => {
  const { page, context, errors } = await openEdge({ path: '/manage/boxes/office-box' });
  await page.getByTestId('site-cloud').waitFor();
  await page.reload({ waitUntil: 'networkidle' });
  await page.getByTestId('site-cloud').waitFor();
  await page.getByRole('link', { name: 'All boxes' }).click();
  await page.getByTestId('box-office-box').waitFor();
  const extra = (await violations(page)).filter((v) => !BASELINE.includes(v));
  assert.deepEqual(extra, [], `baseline ${JSON.stringify(BASELINE)}`);
  assert.deepEqual(errors, []);
  await context.close();
});

// ── The owner's switch, on the box ──────────────────────────────────────

const SECTIONS = ['storage', 'backup', 'apps', 'market', 'network', 'look', 'lab', 'access'];
const SETTINGS = {
  sharingMyStorage: false,
  hostName: 'office-box',
  gpuEnable: false,
  proxyEnable: true,
  clusterEnable: false,
  shareCompute: false,
  computeWindowStart: '23:00',
  computeWindowEnd: '07:00',
  hardeningApparmor: false,
  hardeningMalloc: false,
  hardeningNosmt: false,
  hardeningUsbguard: false,
};
const EDGE_URL = 'http://192.168.1.2:8080';

async function openBox({ official = false, outside = false } = {}) {
  const context = await browser.newContext({ viewport: { width: 1280, height: 900 }, locale: 'en-US' });
  const page = await context.newPage();
  const posts = [];
  let managedBy = null;
  const edge = () => ({
    reachable: true,
    official,
    edges: [
      {
        name: 'Office edge',
        url: EDGE_URL,
        source: 'lan',
        official,
        onNetwork: !outside,
        ...(official ? { gateway: { state: 'trusted', owner: 'Office IT <it@office.example>', fingerprint: 'AB', image: 'gw' } } : {}),
      },
    ],
    lanSearched: true,
    configuredUrl: null,
    checkedAt: 1,
    path: { url: EDGE_URL, source: 'lan' },
    manage: {
      candidate: outside ? null : { name: 'Office edge', url: EDGE_URL },
      refused: outside ? 'outsideNetwork' : null,
      managedBy,
      active: managedBy !== null,
    },
  });
  await page.route('**/api/**', (route) => json(route, 200, {}));
  await page.route('**/api/setup/claim', (route) => json(route, 200, { claimed: true }));
  await page.route('**/api/me', (route) =>
    json(route, 200, { name: 'eva', displayName: 'Eva', groups: ['losos-admins'], admin: true, spareKey: false, sections: SECTIONS, standalone: true }),
  );
  await page.route('**/api/state', (route) => json(route, 200, { mode: 'local', sharing: false }));
  await page.route('**/api/settings', (route) => json(route, 200, SETTINGS));
  await page.route('**/api/status', (route) => json(route, 200, { state: 'idle', progress: 0, message: '' }));
  await page.route('**/api/edge', (route) => json(route, 200, edge()));
  await page.route('**/api/edge/manage', (route) => {
    const body = JSON.parse(route.request().postData() ?? '{}');
    posts.push(body);
    managedBy = body.enabled ? EDGE_URL : null;
    return json(route, 200, { manage: edge().manage });
  });
  await page.addInitScript(() => window.localStorage.setItem('losos-tour', 'seen'));
  await page.goto(origin + '/settings/mesh', { waitUntil: 'networkidle' });
  return { page, context, posts };
}

/* The switch's input is hidden under its track; a person clicks the track. */
const toggle = (page, name) =>
  page.locator('[data-slot="switch"]').filter({ has: page.getByRole('switch', { name, exact: true }) }).click();

await check("the owner's switch lets the company edge in use manage the box, and takes it back", async () => {
  const { page, context, posts } = await openBox();
  const name = 'Let Office edge manage this box';
  await page.getByRole('switch', { name, exact: true }).waitFor();
  assert.equal(await page.getByRole('switch', { name, exact: true }).isChecked(), false);
  await toggle(page, name);
  // The toast's live region repeats the line, prefixed; the row's own text is exact.
  await page.getByText('Office edge can manage this box now', { exact: true }).first().waitFor();
  await page.getByText(/can change LosOS Cloud, LosOS Git and the apps here/).waitFor();
  assert.equal(await page.getByRole('switch', { name, exact: true }).isChecked(), true);
  await toggle(page, name);
  await page.getByText('No edge manages this box now', { exact: true }).first().waitFor();
  assert.deepEqual(posts, [{ enabled: true }, { enabled: false }]);
  await context.close();
});

await check('a checked gateway on the LAN may manage the box like any other edge there', async () => {
  const { page, context, posts } = await openBox({ official: true });
  const name = 'Let Office edge manage this box';
  await page.getByRole('switch', { name, exact: true }).waitFor();
  assert.equal(await page.getByRole('switch', { name, exact: true }).isDisabled(), false);
  await toggle(page, name);
  await page.getByText('Office edge can manage this box now', { exact: true }).first().waitFor();
  assert.deepEqual(posts, [{ enabled: true }]);
  await context.close();
});

await check('an edge outside the box\'s own network is never offered the switch', async () => {
  const { page, context, posts } = await openBox({ outside: true });
  await page.getByText(/is not on its own network, so it may not manage the box/).waitFor();
  const sw = page.getByRole('switch', { name: 'Let the company edge manage this box', exact: true });
  assert.equal(await sw.isDisabled(), true);
  assert.equal(await sw.isChecked(), false);
  assert.deepEqual(posts, []);
  await context.close();
});

await browser.close();
await closeServer();
finish();
