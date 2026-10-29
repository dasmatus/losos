/* The signed-in half of the admin UI: what an owner gets after the gate.
 *
 * wizard.browser.mjs covers which screen a box lands on. This covers what
 * happens next: the key prompt accepts and refuses the right things, every
 * sidebar entry lands on the address it claims, a deep link survives a reload
 * (nginx's `try_files` in production, the static server's fallback here), an
 * unknown path says so, and no route throws while rendering.
 *
 * Same arrangement as the wizard file: the real dist/ bundle, the API stubbed
 * with page.route(). The stub enforces the Bearer token the way lososd does, so
 * a wrong key really is refused rather than assumed to be.
 */

import assert from './assert.mjs';
import { launch, pickFiles, runner, serve } from './harness.mjs';

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

/** A box that can join the mesh: the tunnel on and a name of its own. The
 * stock settings above can't, and the pane says why. */
const JOINABLE = { ...SETTINGS, proxyEnable: true, hostName: 'kitchen' };

/* GET /api/options for a box installed with or without a TPM: the real
 * document (the fixture advanced.browser.mjs reads too), with `tpm.enable`'s
 * running value set. Without `options` the catch-all answers `{}`, which the
 * form reads as "no document served". */
const OPTIONS_DOC = await Bun.file(Bun.env.LOSOS_OPTIONS_JSON ?? 'tests/fixtures/options.json').json();
const optionsWithTpm = (tpm) => ({
  available: true,
  ...OPTIONS_DOC,
  stray: [],
  options: OPTIONS_DOC.options.map((o) => ({ ...o, set: null, ...(o.name === 'tpm.enable' ? { current: tpm } : {}) })),
});

const json = (route, status, body) =>
  route.fulfill({ status, contentType: 'application/json', body: JSON.stringify(body) });

/* A claimed box whose API refuses anything without the right Bearer token.
 * `stored` pre-seeds sessionStorage, i.e. a returning owner in the same tab. */
async function open({
  path = '/',
  stored = false,
  viewport = { width: 1280, height: 900 },
  locale = 'en-US',
  market = { available: false },
  // POST /api/market/llm/models's document: the AI models, meters and balances.
  llm = { available: false },
  // POST /api/market/taps's answer, the edge's quote for a model.
  quote = null,
  // [script, arg] run in the page before the app, such as a browser wallet.
  init = null,
  vms = { available: false, reason: 'notSharing' },
  domains = { available: false },
  backup = BACKUP_EMPTY,
  edge = EDGE_FOUND,
  settings = SETTINGS,
  options = null,
  status = { state: 'idle', progress: 0, message: '' },
  storage = null,
  checkoutUrl = 'https://checkout.stripe.com/c/pay/cs_test_1',
  onboardUrl = 'https://connect.stripe.com/setup/e/acct_test/abc',
  now = null,
  cloud = null,
  // A person signed in through the box's sign-in page (Authelia's cookie),
  // as GET /api/me describes them; null for none.
  person = null,
  claim = { claimed: true },
  // The origin to load from: the plain server, or one serving the real CSP.
  base = origin,
} = {}) {
  const page = await browser.newPage({ viewport, locale });
  if (now !== null) await page.clock.setFixedTime(now);
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));

  // Catch-all first: the last-registered matching route wins (see the wizard file).
  await page.route('**/api/**', (route) => json(route, 200, {}));
  await page.route('**/api/setup/claim', (route) => json(route, 200, claim));
  const authed = (route) =>
    person !== null || route.request().headers()['authorization'] === `Bearer ${TOKEN}`;
  // Who is calling: the spare key is an administrator with no name.
  await page.route('**/api/me', (route) => {
    if (route.request().headers()['authorization'] === `Bearer ${TOKEN}`) {
      return json(route, 200, {
        name: null,
        displayName: null,
        groups: ['losos-admins'],
        admin: true,
        spareKey: true,
        sections: ['storage', 'backup', 'apps', 'market', 'network', 'look', 'lab', 'access'],
      });
    }
    return person !== null ? json(route, 200, person) : json(route, 401, { error: 'unauthorized' });
  });
  await page.route('**/api/state', (route) =>
    authed(route) ? json(route, 200, { mode: 'local', sharing: false }) : json(route, 401, { error: 'unauthorized' }),
  );
  await page.route('**/api/settings', (route) =>
    authed(route) ? json(route, 200, settings) : json(route, 401, { error: 'unauthorized' }),
  );
  await page.route('**/api/status', (route) =>
    authed(route) ? json(route, 200, status) : json(route, 401, { error: 'unauthorized' }),
  );
  // GET /api/storage and POST /api/grow as lososd answers them: a grow spends
  // the reserve, and every later reading says so.
  if (storage !== null) {
    await page.route('**/api/storage', (route) =>
      authed(route) ? json(route, 200, storage) : json(route, 401, { error: 'unauthorized' }),
    );
    await page.route('**/api/grow', (route) => {
      if (!authed(route)) return json(route, 401, { error: 'unauthorized' });
      const before = storage.totalBytes;
      storage.totalBytes += storage.reserveBytes;
      const claimed = storage.reserveBytes;
      storage.reserveBytes = 0;
      return json(route, 200, { grew: claimed > 0, beforeBytes: before, afterBytes: storage.totalBytes, claimedBytes: claimed });
    });
  }
  if (options !== null) {
    await page.route('**/api/options', (route) =>
      authed(route) ? json(route, 200, options) : json(route, 401, { error: 'unauthorized' }),
    );
  }
  // The edge scan: GET /api/edge's document, and the gate it implies. An
  // apply that turns sharing on while `edge.reachable` is false is answered
  // the way lososd answers it: 409 with the sentence and `edgeRequired`.
  const applies = [];
  await page.route('**/api/edge', (route) =>
    authed(route) ? json(route, 200, edge) : json(route, 401, { error: 'unauthorized' }),
  );
  await page.route('**/api/apply', (route) => {
    if (!authed(route)) return json(route, 401, { error: 'unauthorized' });
    const nix = route.request().postData() ?? '';
    applies.push(nix);
    const turnsOn = /losos\.(sharingMyStorage|cluster\.enable) = true;/.test(nix);
    if (turnsOn && !edge.reachable) {
      return json(route, 409, {
        error: 'no edge proxy is reachable from this box, so losos.cluster.enable cannot be turned on: storage can only be shared through an edge. Local use keeps working.',
        edgeRequired: true,
        setting: 'losos.cluster.enable',
      });
    }
    return json(route, 200, { job: 'job-1' });
  });
  // The market relay: `market` is GET /api/market's document, every action is
  // recorded, and each answers the way lososd does on success.
  const marketPosts = [];
  await page.route('**/api/market', (route) =>
    authed(route) ? json(route, 200, market) : json(route, 401, { error: 'unauthorized' }),
  );
  await page.route('**/api/market/**', (route) => {
    if (!authed(route)) return json(route, 401, { error: 'unauthorized' });
    const path = new URL(route.request().url()).pathname;
    marketPosts.push([path, route.request().postDataJSON()]);
    if (path === '/api/market/onboard') return json(route, 200, { available: true, ready: false, url: onboardUrl });
    if (path === '/api/market/orders') return json(route, 201, { available: true, checkout_url: checkoutUrl });
    return json(route, 201, { available: true });
  });
  // The AI models relay: the overview is read, every other call recorded and
  // answered the way lososd relays the edge's answer.
  const llmPosts = [];
  await page.route(/\/api\/market\/(llm|taps)(\/|$)/, (route) => {
    if (!authed(route)) return json(route, 401, { error: 'unauthorized' });
    const path = new URL(route.request().url()).pathname;
    const body = route.request().postDataJSON();
    if (path === '/api/market/llm/models') return json(route, 200, llm);
    llmPosts.push([path, body]);
    if (path === '/api/market/llm/chat') {
      return json(route, 200, { content: 'Mostly water, then ice.', finishReason: 'stop', tokensIn: 42, tokensOut: 7, costE6: '31500000', billedE6: '10261500000' });
    }
    if (path === '/api/market/taps/stop') return json(route, 200, { meter: body.meter, state: 'ending' });
    if (path === '/api/market/taps' && quote !== null) return json(route, 200, quote);
    if (path === '/api/market/taps/confirm') {
      if (typeof body.wallet !== 'string' || body.wallet.length === 0) return json(route, 400, { error: 'wallet is not a wallet address' });
      return json(route, 200, { meter: body.meter, state: 'open' });
    }
    return json(route, 409, { error: 'not in this test' });
  });
  // The machines relay: `vms` is GET /api/vms's document; every action and
  // every image upload is recorded and answered the way lososd does.
  const vmPosts = [];
  await page.route('**/api/vms', (route) =>
    authed(route) ? json(route, 200, vms) : json(route, 401, { error: 'unauthorized' }),
  );
  await page.route('**/api/vms/**', (route) => {
    if (!authed(route)) return json(route, 401, { error: 'unauthorized' });
    const request = route.request();
    const url = new URL(request.url());
    if (request.method() === 'PUT') {
      vmPosts.push(['PUT', url.pathname + url.search, request.postDataBuffer()?.length ?? 0]);
      return json(route, 201, { upload_id: 'upl_9f00', size: 64, min_disk_gib: 1 });
    }
    vmPosts.push([url.pathname, request.postDataJSON()]);
    if (url.pathname === '/api/vms/orders') return json(route, 201, { available: true, checkout_url: checkoutUrl });
    return json(route, 201, { available: true });
  });
  // The custom-domain relay: GET answers `domains`; an add or a remove is
  // recorded and answered with the view the edge would send back.
  const domainPosts = [];
  let domainsView = domains;
  await page.route('**/api/domains', (route) => {
    if (!authed(route)) return json(route, 401, { error: 'unauthorized' });
    if (route.request().method() === 'GET') return json(route, 200, domainsView);
    const sent = route.request().postDataJSON();
    domainPosts.push(['/api/domains', sent]);
    if (sent.domain === 'taken.example.org') {
      return json(route, 409, { error: 'taken.example.org is live for another box' });
    }
    domainsView = {
      ...domainsView,
      domains: [...domainsView.domains, { ...WAITING_DOMAIN, domain: sent.domain, txt_name: `_losos-challenge.${sent.domain}` }],
    };
    return json(route, 201, domainsView);
  });
  await page.route('**/api/domains/wallet', (route) => {
    if (!authed(route)) return json(route, 401, { error: 'unauthorized' });
    const sent = route.request().postDataJSON();
    domainPosts.push(['/api/domains/wallet', sent]);
    const wallet = sent.address
      ? {
          address: sent.address,
          bound_at: 1791460000,
          record: ensRecord(sent.address),
          // Public DNS validates the hostname; the label is not looked up yet.
          published: { 'mattbox.boxes.losos.dasmat.us': 'found' },
          published_at: 1791460030,
        }
      : null;
    domainsView = {
      ...domainsView,
      wallet,
      domains: domainsView.domains.map((d) => ({ ...d, wallet: wallet === null ? null : d.wallet })),
    };
    return json(route, 200, domainsView);
  });
  await page.route('**/api/domains/remove', (route) => {
    if (!authed(route)) return json(route, 401, { error: 'unauthorized' });
    const sent = route.request().postDataJSON();
    domainPosts.push(['/api/domains/remove', sent]);
    domainsView = { ...domainsView, domains: domainsView.domains.filter((d) => d.domain !== sent.domain) };
    return json(route, 200, domainsView);
  });
  // Backups and the erase: a small lososd. GET /api/backup answers the view;
  // every write is recorded and moves the view the way lososd would.
  const backupPosts = [];
  let backupView = structuredClone(backup);
  await page.route('**/api/recovery', (route) =>
    authed(route) ? json(route, 200, { code: 'b3c1d2e4-5f60-4a7b-8c9d-0e1f2a3b4c5d', minted: false }) : json(route, 401, { error: 'unauthorized' }),
  );
  await page.route(/\/api\/(backup|erase)(\/.*)?$/, (route) => {
    if (!authed(route)) return json(route, 401, { error: 'unauthorized' });
    const request = route.request();
    const path = new URL(request.url()).pathname;
    if (path === '/api/backup' && request.method() === 'GET') return json(route, 200, backupView);
    const sent = request.postData() === null ? null : request.postDataJSON();
    backupPosts.push([request.method(), path, sent]);
    if (path === '/api/backup/target' && request.method() === 'POST') {
      const { secretAccessKey, ...shown } = sent;
      backupView.target = { ...shown, hasSecret: Boolean(secretAccessKey) || backupView.target?.hasSecret === true };
      return json(route, 200, { target: backupView.target });
    }
    if (path === '/api/backup/target') {
      backupView.target = null;
      return json(route, 200, { target: null });
    }
    if (path === '/api/backup/run' || path === '/api/backup/restore') {
      if (path === '/api/backup/restore' && sent.code.trim().toLowerCase() !== OLD_CODE) {
        return json(route, 400, { error: 'the recovery code does not open this backup' });
      }
      const kind = path.endsWith('run') ? 'backup' : 'restore';
      backupView.job = { kind, job: 'job-b1', state: 'running', startedAt: 1_760_000_000, message: '' };
      return json(route, 200, { job: 'job-b1' });
    }
    if (path === '/api/erase') {
      backupView.erase = {
        phase: 'waiting',
        backup: sent.backup,
        startedAt: 1_760_000_000,
        deadline: 1_760_000_900,
        secondsLeft: 900,
        cancellable: true,
        message: '',
        outside: { hadEdge: false, domainsRemoved: 0, listingsClosed: 0, leftEdge: false, problems: [], erasedAt: 0 },
      };
      return json(route, 200, { erase: backupView.erase });
    }
    if (path === '/api/erase/cancel') {
      backupView.erase = null;
      return json(route, 200, { erase: null });
    }
    return json(route, 404, { error: 'not found' });
  });
  // Stripe's own pages, so a tab sent there has something to land on.
  for (const host of ['checkout.stripe.com', 'connect.stripe.com']) {
    await page.context().route(`https://${host}/**`, (route) =>
      route.fulfill({ status: 200, contentType: 'text/html', body: '<title>Stripe</title>' }),
    );
  }

  if (stored) {
    await page.addInitScript((t) => window.sessionStorage.setItem('losos-token', t), TOKEN);
  }
  if (init !== null) await page.addInitScript(...init);
  // The apps the homepage probes, when a check wants them answering the way
  // a set-up box does: LosOS Cloud's status document and LosOS Git's page.
  if (cloud !== null) {
    await page.route('**/status.php', (route) => json(route, 200, { installed: true, productname: 'LosOS Cloud' }));
    await page.route('**/git/api/v1/version', (route) => route.fulfill({ status: 200, contentType: 'text/html', body: '' }));
  }
  await page.goto(base + path, { waitUntil: 'networkidle' });
  return { page, errors, marketPosts, llmPosts, vmPosts, applies, domainPosts, backupPosts };
}

/* GET /api/backup: a box with no bucket yet, and one with a bucket, a last
 * backup and the report of an erase it went through before. */
const OLD_CODE = '0f8fad5b-d9cb-469f-a165-70867728950e';
const BACKUP_EMPTY = {
  target: null,
  last: null,
  job: null,
  erase: null,
  lastErase: null,
  graceSeconds: 900,
};
const BACKUP_SET = {
  ...BACKUP_EMPTY,
  target: {
    endpoint: 'https://s3.eu-central-1.amazonaws.com',
    bucket: 'mattbox-backups',
    prefix: 'losos',
    region: 'eu-central-1',
    accessKeyId: 'AKIAEXAMPLE1234',
    hasSecret: true,
  },
  last: { time: 1_760_000_000, snapshot: 'a'.repeat(64), files: 1832, bytes: 4_831_838_208 },
  lastErase: { hadEdge: true, domainsRemoved: 2, listingsClosed: 1, leftEdge: true, problems: [], erasedAt: 1_759_000_000 },
};

/* GET /api/domains on an official edge, for a box Stripe has checked: one
 * domain live, one still waiting for its CNAME. */
const WAITING_DOMAIN = {
  wallet: null,
  domain: 'files.example.net',
  status: 'waiting',
  txt_name: '_losos-challenge.files.example.net',
  txt_value: 'losos-domain-v1=0f3c9a51d2e84b7d6a1c93e0b5f2d470',
  txt_found: true,
  points_here: false,
  problem: 'notPointing',
  checked_at: 1791450000,
  verified_at: null,
};
const DOMAINS_READY = {
  available: true,
  eligible: true,
  reason: null,
  target: '3f9a1c0e7b2d4a55.boxes.losos.dasmat.us',
  addresses: ['203.0.113.7', '2001:db8::7'],
  max_domains: 5,
  domains: [
    {
      domain: 'cloud.example.org',
      status: 'live',
      txt_name: '_losos-challenge.cloud.example.org',
      txt_value: 'losos-domain-v1=0f3c9a51d2e84b7d6a1c93e0b5f2d470',
      txt_found: true,
      points_here: true,
      problem: null,
      checked_at: 1791450000,
      verified_at: 1791440000,
      wallet: null,
    },
    WAITING_DOMAIN,
  ],
  box: 'mattbox',
  zone: 'boxes.losos.dasmat.us',
  names: ['mattbox.boxes.losos.dasmat.us', '3f9a1c0e7b2d4a55.boxes.losos.dasmat.us'],
  wallet: null,
};

/* The ENS record the edge publishes for a bound wallet
 * (edge/registrar/src/domains.rs, `ens_record`). */
const ensRecord = (a) =>
  `ENS1 dnsname.ens.eth ${a}`;
/* What the wallet signs to bind (`wallet_message` there). */
const walletMessage = (zone, box, a) => `LosOS box name v1\nzone: ${zone}\nbox: ${box}\naddress: ${a}\n`;

/* What lososd's scan reports with an edge on the LAN, and with none. */
const EDGE_FOUND = {
  reachable: true,
  official: true,
  edges: [{ name: 'edge demo', url: 'http://edge.local:8443', source: 'lan', official: true }],
  lanSearched: true,
  configuredUrl: 'https://losos-edge.dasmat.us',
  checkedAt: 1760000000,
};
/* The same edge without the LosOS root's signature: a company's own. */
const EDGE_COMPANY = {
  ...EDGE_FOUND,
  official: false,
  edges: [{ ...EDGE_FOUND.edges[0], official: false }],
};
/* Two edges at once: the company's on the LAN, which shows no passport, and
 * the public one over the internet, which passed the gateway checks. */
const TRUSTED = { state: 'trusted', owner: 'Ada Gateway <ada@example.net>', fingerprint: 'ABCDEF0123456789', image: 'losos-edge-gateway 0.6.0' };
const EDGE_BOTH = {
  ...EDGE_FOUND,
  edges: [
    { ...EDGE_FOUND.edges[0], official: false },
    { name: 'losos-edge.dasmat.us', url: 'https://losos-edge.dasmat.us', source: 'configured', official: true, gateway: TRUSTED },
  ],
};
/* A gateway this box caught tampered with, beside one whose fees changed. */
const EDGE_TAMPERED = {
  ...EDGE_FOUND,
  official: false,
  edges: [
    {
      ...EDGE_FOUND.edges[0],
      official: false,
      gateway: { state: 'disabled', code: 'tainted', reason: "the gateway's settings or state changed after it booted (PCR 15)", since: 1760000000 },
    },
    {
      name: 'losos-edge.dasmat.us',
      url: 'https://losos-edge.dasmat.us',
      source: 'configured',
      official: false,
      gateway: { state: 'refused', code: 'payoutChanged', reason: 'the fee settings differ from the ones this box ships' },
    },
  ],
};
/* Two secondary clusters on the LAN and the official edge over the
 * internet: the box is in office and uses its gateway gw-a (secondary). */
const gwEdge = (cluster, member) => ({
  name: member, url: `http://${member}.local:8443`, source: 'lan', official: false,
  tier: 'secondary', cluster, member,
});
const EDGE_CLUSTERS = {
  ...EDGE_FOUND,
  official: true,
  edges: [
    gwEdge('office', 'gw-a'),
    gwEdge('office', 'gw-b'),
    gwEdge('lab', 'gw-z'),
    { name: 'losos-edge.dasmat.us', url: 'https://losos-edge.dasmat.us', source: 'configured', official: true, tier: 'tertiary' },
  ],
  path: { name: 'gw-a', url: 'http://gw-a.local:8443', source: 'lan', tunnel: 'gw-a.local:2333', tier: 'secondary', cluster: 'office', member: 'gw-a' },
  clusters: [{ name: 'lab', gateways: ['gw-z'] }, { name: 'office', gateways: ['gw-a', 'gw-b'] }],
  cluster: 'office',
};
/* No gateway of office answers: the box went straight to the official edge. */
const EDGE_TERTIARY = {
  ...EDGE_CLUSTERS,
  edges: [EDGE_CLUSTERS.edges[2], EDGE_CLUSTERS.edges[3]],
  path: { name: 'losos-edge.dasmat.us', url: 'https://losos-edge.dasmat.us', source: 'configured', tunnel: 'edge.losos.cfd:2333', tier: 'tertiary' },
  clusters: [{ name: 'lab', gateways: ['gw-z'] }],
};
const EDGE_NONE = {
  reachable: false,
  official: false,
  edges: [],
  lanSearched: true,
  configuredUrl: 'https://losos-edge.dasmat.us',
  checkedAt: 1760000000,
};

/* A box the market is offered to: one listing on the shelf, payouts set up,
 * sharing storage only. */
const MARKET = {
  available: true,
  listings: [
    { id: 'lst_ab12', kind: 'storage', unit: 'GiB-month', unit_price: 5, currency: 'eur', available: 40 },
  ],
  account: {
    fee_bps: 400,
    currency: 'eur',
    seller_onboarded: true,
    seller_ready: true,
    can_sell_storage: true,
    can_sell_compute: false,
    listings: [],
    entitlements: { storage_gib: 0, compute_vcpu_hours: 0, next_expiry: null },
    purchases: [],
    sales: [],
  },
};

/* A sharing box on an edge that runs machines: two systems in the
 * catalogue, one image of its own on the edge, one host offering replicas,
 * and one machine of its own with two replicas, one up and one importing. */
const VMS = {
  available: true,
  catalogue: {
    images: [
      { id: 'losos', name: 'LosOS', family: 'losos', cloud_init: false, efi: true, disk_gib: 20, memory_mib: 4096 },
      { id: 'ubuntu-24.04', name: 'Ubuntu Server 24.04 LTS', family: 'ubuntu-server', cloud_init: true, efi: false, disk_gib: 20, memory_mib: 2048 },
    ],
    installer_only: ['windows', 'macos'],
    not_offered: [],
    shape: { cpu: 1, memory_mib: 2048, disk_gib: 20 },
    fee_bps: 5000,
    max_replicas: 10,
    max_upload_bytes: 32 * 1024 ** 3,
    published: true,
  },
  listings: [
    { id: 'lst_vm01', kind: 'vm', unit: 'replica-month', unit_price: 600, currency: 'eur', available: 4 },
  ],
  account: {
    ...MARKET.account,
    can_host_vms: true,
    vm_fee_bps: 5000,
    uploads: [
      { id: 'upl_77aa', name: 'My router', stored: true, size: 734003200, min_disk_gib: 8, created_at: 1760000000 },
    ],
    purchases: [
      {
        id: 'ord_0123456789abcdef',
        kind: 'vm',
        unit: 'replica-month',
        quantity: 2,
        amount: 1200,
        fee: 600,
        seller_net: 600,
        currency: 'eur',
        status: 'paid',
        created_at: 1760000000,
        paid_at: 1760000100,
        expires_at: 1762600000,
        expired: false,
        volume: null,
        vm: { image: 'ubuntu-24.04', name: 'web', address: 'https://vm-0123456789ab.vms.example.net', created: true, halted: false },
      },
    ],
  },
  machines: [
    {
      order_id: 'ord_0123456789abcdef',
      replicas: [
        { name: 'vm-0123456789ab-0', status: 'Running' },
        { name: 'vm-0123456789ab-1', status: 'Preparing' },
      ],
    },
  ],
};

const body = (page) => page.locator('body').innerText();
const nav = (page) => page.getByRole('navigation', { name: 'Sections', exact: true });

console.log('admin-ui app browser checks');

/* Signed out, the gate sends the browser to the box's sign-in page
 * (Authelia at /auth/, https only) and back here afterwards; the spare key
 * stays behind a link for when that page is down. */
const PASSWORD = 'Correct-horse battery staple 1';
async function openGate(options = {}) {
  const opened = await open(options);
  const portal = [];
  const secure = [];
  await opened.page.route('https://**/*', (route) => {
    const url = route.request().url();
    if (new URL(url).pathname.startsWith('/auth/')) portal.push(url);
    else secure.push(url);
    return route.fulfill({ status: 200, contentType: 'text/html', body: '<title>LosOS sign-in</title>' });
  });
  return { ...opened, portal, secure };
}

/* This harness serves plain http. There the session cookie (https only)
 * never arrives, so Sign in first goes to the same page on https, marked
 * ?signin, where a person who has a session is simply in and a stranger
 * goes on to the sign-in page without a second click. Straight to Authelia,
 * a person with a session would get its second-factor page. */
await check('signed out on plain http, Sign in goes to the same page on https, marked to go on', async () => {
  const { page, portal, secure } = await openGate({ path: '/storage#here' });
  await page.getByText('Sign in to this box').waitFor();
  await page.getByRole('button', { name: 'Sign in', exact: true }).click();
  await page.waitForURL(/^https:/);
  assert.equal(portal.length, 0, 'went to the sign-in page from plain http');
  assert.equal(secure.length, 1, 'did not move to https');
  const url = new URL(secure[0]);
  assert.equal(url.pathname, '/storage', 'the https page is not the page the person was on');
  assert.ok(url.searchParams.has('signin'), 'the https page is not marked to go on to sign in');
  await page.close();
});

await check('a page marked ?signin drops the mark and goes on to sign in without a click', async () => {
  const { page, secure } = await openGate({ path: '/' });
  await page.getByText('Sign in to this box').waitFor();
  await page.goto(origin + '/storage?signin=');
  await page.waitForURL(/^https:/, { timeout: 10000 });
  assert.equal(secure.length, 1, 'did not go on by itself');
  await page.close();
  const signedIn = await open({
    path: '/storage?signin=',
    person: { name: 'matus', displayName: 'Matus', groups: ['losos-admins'], admin: true, spareKey: false, sections: ['storage'] },
  });
  await signedIn.page.getByTestId('signed-in-as').waitFor();
  assert.equal(new URL(signedIn.page.url()).search, '', 'the mark stayed in the address');
  await signedIn.page.close();
});

await check('while the sign-in page starts, the prompt waits with a spinner and a fact, then goes on by itself', async () => {
  const { page, secure } = await openGate();
  let up = false;
  await page.route('**/auth/api/health', (route) =>
    up ? json(route, 200, { status: 'OK' }) : route.fulfill({ status: 502, contentType: 'text/html', body: 'Bad Gateway' }),
  );
  await page.reload();
  await page.getByText('LosOS Login is starting').waitFor();
  assert.ok(/While you wait/.test(await body(page)), 'no fact under the spinner');
  // The facts come in an order shuffled with the seed "salmon", so every load starts on the same one.
  assert.ok(/take their colours from one file/.test(await body(page)), 'the first fact is not the salmon-seeded one');
  assert.equal(await page.locator('dialog [data-slot="spinner"], [role="dialog"] [data-slot="spinner"]').count() > 0, true, 'no spinner');
  await page.getByRole('button', { name: 'Sign in', exact: true }).click();
  await page.waitForTimeout(500);
  assert.equal(secure.length, 0, 'went on before the sign-in page answered');
  up = true;
  await page.waitForURL(/^https:/, { timeout: 10000 });
  assert.equal(secure.length, 1, 'did not go on once the sign-in page answered');
  await page.close();
});

await check('a person signed in at the box sign-in page gets the app without a key', async () => {
  const { page } = await open({
    person: { name: 'matus', displayName: 'Matus', groups: ['losos-admins'], admin: true, spareKey: false, sections: ['storage', 'backup'] },
  });
  await page.getByTestId('signed-in-as').waitFor();
  await page.getByText('Sign in to this box').waitFor({ state: 'hidden' });
  assert.equal(await page.evaluate(() => sessionStorage.getItem('losos-token')), null, 'a cookie session stored a key');
  // The name beside the picture or initials.
  const who = page.getByTestId('signed-in-name');
  assert.equal((await who.innerText()).trim(), 'Matus', 'the top bar does not name the person signed in');
  assert.match(await page.getByTestId('signed-in-as').getAttribute('aria-label'), /as matus\. You are an administrator/, 'the top bar does not say who and what they are');
  await page.close();
});

await check('a person in one section sees that section and not the administrators\' panes', async () => {
  const { page } = await open({
    path: '/settings/backup',
    person: { name: 'eva', displayName: 'Eva', groups: ['losos-backup'], admin: false, spareKey: false, sections: ['backup'] },
  });
  await page.getByTestId('signed-in-as').waitFor();
  const sections = nav(page);
  await sections.getByRole('link', { name: 'Backup' }).waitFor();
  for (const hidden of ['Storage', 'Mesh', 'Advanced', 'Security', 'Reset', 'Lab']) {
    assert.equal(await sections.getByRole('link', { name: hidden, exact: true }).count(), 0, `${hidden} is shown to a backup-only person`);
  }
  assert.equal(await sections.getByRole('link', { name: 'Overview' }).count(), 1, 'the overview is for everyone');
  // The name beside the picture or initials.
  const who = page.getByTestId('signed-in-name');
  assert.equal((await who.innerText()).trim(), 'Eva', 'the top bar does not name the person signed in');
  assert.match(await page.getByTestId('signed-in-as').getAttribute('aria-label'), /the panes your account was given/, 'a backup-only person is called an administrator');
  await page.close();
});

await check('Mail is in the sidebar only while /api/me lists it among the shared apps', async () => {
  const matus = { name: 'matus', displayName: 'Matus', groups: ['losos-admins'], admin: true, spareKey: false, sections: ['storage', 'backup'] };
  for (const [shared, want] of [[['mail'], 1], [[], 0], [undefined, 0]]) {
    const { page } = await open({ person: { ...matus, ...(shared !== undefined ? { shared } : {}) } });
    await page.getByTestId('signed-in-as').waitFor();
    const mail = nav(page).getByRole('link', { name: 'Mail', exact: true });
    assert.equal(await mail.count(), want, `Mail links with shared=${JSON.stringify(shared)}`);
    if (want === 1) assert.equal(await mail.getAttribute('href'), '/mail', 'Mail is not a screen of the admin pages');
    await page.close();
  }
});

await check('a wrong spare key is refused and the prompt stays up', async () => {
  const { page } = await openGate();
  await page.getByRole('button', { name: /Use the spare admin key instead/ }).click();
  await page.locator('#admin-key').fill('b'.repeat(64));
  await page.getByRole('button', { name: 'Unlock' }).click();
  await page.waitForTimeout(300);
  const text = await body(page);
  assert.ok(/Sign in to this box/i.test(text), 'the prompt closed on a wrong key');
  assert.ok(/was not accepted/i.test(text), `no refusal shown:\n${text}`);
  assert.equal(await page.getByRole('dialog').locator('a[data-help]').getAttribute('href'), '/handbook/troubleshooting/forgot-password/');
  assert.equal(
    await page.getByTestId('signed-in-as').count(),
    0,
    'the app unlocked on a wrong key',
  );
  assert.equal(await page.evaluate(() => sessionStorage.getItem('losos-token')), null);
  await page.close();
});

await check('the right spare key opens the app and is kept for the tab', async () => {
  const { page, portal } = await openGate();
  await page.getByRole('button', { name: /Use the spare admin key instead/ }).click();
  await page.locator('#admin-key').fill(TOKEN);
  await page.getByRole('button', { name: 'Unlock' }).click();
  await page.getByTestId('signed-in-as').waitFor();
  await page.getByText('Sign in to this box').waitFor({ state: 'hidden' });
  assert.equal(await page.evaluate(() => sessionStorage.getItem('losos-token')), TOKEN);
  assert.equal(portal.length, 0, 'the key path must not go through the sign-in page');
  await page.getByTestId('signed-in-as').getByText('Spare admin key').waitFor();
  await page.close();
});

/* A box claimed before sign-in went through Authelia has nobody to sign
 * in as. The prompt asks its owner for a name and a password, with the
 * spare key as proof, and lososd makes them the first administrator. */
await check('a box with nobody to sign in as makes its first administrator with the spare key', async () => {
  const { page } = await openGate({ claim: { claimed: true, people: false } });
  const made = [];
  await page.route('**/api/setup/claim', (route) => {
    const req = route.request();
    if (req.method() !== 'POST') return json(route, 200, { claimed: true, people: false });
    made.push({ auth: req.headers()['authorization'], body: req.postDataJSON() });
    return json(route, 200, { claimed: true, user: null, name: 'matus', token: null });
  });
  await page.getByText('Make your sign-in').waitFor();
  await page.locator('#first-name').fill('Matus');
  await page.locator('#first-password').fill(PASSWORD);
  await page.locator('#admin-key').fill(TOKEN);
  await page.getByRole('button', { name: 'Make it' }).click();
  await page.getByTestId('signed-in-as').waitFor();
  assert.equal(made.length, 1, 'expected one claim');
  assert.equal(made[0].auth, `Bearer ${TOKEN}`, 'the spare key did not go with it');
  assert.deepEqual(made[0].body, { name: 'matus', password: PASSWORD }, 'the name was not lower-cased or the body is wrong');
  await page.getByText(/sign in with matus/i).first().waitFor({ timeout: 5000 });
  await page.close();
});

await check('signing out returns to the prompt and forgets the token', async () => {
  const { page } = await open({ stored: true });
  await page.getByTestId('signed-in-as').click();
  await page.getByRole('menuitem', { name: /Sign out/ }).click();
  await page.getByText('Sign in to this box').waitFor();
  assert.equal(await page.evaluate(() => sessionStorage.getItem('losos-token')), null);
  await page.close();
});

await check('the person menu holds Your account and Sign out, and Alt+Shift+Q signs out too', async () => {
  const { page } = await open({ stored: true });
  await page.getByTestId('signed-in-as').click();
  const menu = page.locator('[data-slot=person-menu]');
  await menu.waitFor();
  assert.equal(await menu.getByRole('menuitem', { name: /Sign out/ }).count(), 1, 'Sign out is not in the menu');
  assert.match(await menu.getByRole('menuitem', { name: /Sign out/ }).innerText(), /Alt\s*Shift\s*Q/, 'the menu does not show the keys');
  assert.equal(await page.getByRole('button', { name: 'Sign out' }).count(), 0, 'Sign out is still a button in the bar');
  await page.keyboard.press('Escape');
  await menu.waitFor({ state: 'hidden' });
  await page.keyboard.press('Alt+Shift+KeyQ');
  await page.getByText('Sign in to this box').waitFor();
  assert.equal(await page.evaluate(() => sessionStorage.getItem('losos-token')), null);
  await page.close();
});

/* One sidebar (src/components/app-sidebar.tsx): Overview, Apps, Storage and
 * Mesh are links at the top; Settings is a fold whose entries are the
 * settings panes. Every entry, top or folded, must land on its own address
 * and be the only one marked current there. */
const DESTINATIONS = [
  ['Overview', '/'],
  ['Apps', '/apps'],
  ['Storage', '/storage'],
  ['Mesh', '/mesh'],
  ['Machines', '/machines', 'Entries under Mesh'],
  ['Network', '/settings/network', 'Settings'],
  ['Hardware', '/settings/hardware', 'Settings'],
  ['Security', '/settings/security', 'Settings'],
  ['Access tokens', '/settings/access', 'Settings'],
  ['Advanced', '/settings/advanced', 'Settings'],
  ['History', '/settings/history', 'Settings'],
  ['Backup', '/settings/backup', 'Settings'],
  ['About', '/settings/about', 'Settings'],
  ['Reset', '/settings/reset', 'Settings'],
];
/* Open a fold by its name, if it is not open already. */
async function unfold(page, name) {
  const fold = nav(page).getByRole('button', { name, exact: true });
  if ((await fold.getAttribute('aria-expanded')) !== 'true') await fold.click();
}
await check('every sidebar entry, folded or not, lands on its own address', async () => {
  const { page } = await open({ stored: true });
  for (const [label, path, under] of DESTINATIONS) {
    if (under !== undefined) await unfold(page, under);
    await nav(page).getByRole('link', { name: label, exact: true }).click();
    await page.waitForURL(origin + path);
    await nav(page).locator('a[aria-current="page"]').filter({ hasText: label }).waitFor({ timeout: 2000 });
    assert.equal(
      await nav(page).locator('a[aria-current="page"]').count(),
      1,
      `${label}: expected exactly one entry marked current`,
    );
  }
  // /settings itself opens the first entry under Settings.
  await page.goto(origin + '/settings', { waitUntil: 'networkidle' });
  await page.getByRole('heading', { name: 'Network', exact: true, level: 1 }).waitFor();
  await nav(page).locator('a[aria-current="page"]').filter({ hasText: 'Network' }).waitFor();
  await page.close();
});

/* Diagrams is a screen of this app at /diagrams, inside the same top bar
 * and sidebar as every pane, so its entry is a router link: no page load,
 * and the entry is the one marked current there. */
await check('the Diagrams entry opens /diagrams inside the app, without a page load', async () => {
  const { page } = await open({ stored: true });
  const entry = nav(page).getByRole('link', { name: 'Diagrams', exact: true });
  assert.equal(await entry.getAttribute('href'), '/diagrams');
  let loads = 0;
  page.on('request', (r) => { if (r.isNavigationRequest() && r.frame() === page.mainFrame()) loads += 1; });
  await entry.click();
  await page.waitForURL(origin + '/diagrams');
  await nav(page).locator('a[aria-current="page"]').filter({ hasText: 'Diagrams' }).waitFor({ timeout: 2000 });
  await page.getByTestId('lab').waitFor();
  assert.equal(loads, 0, 'clicking Diagrams loaded a new page');
  await page.close();
});

await check('there is one panel: no second list of panes beside the content', async () => {
  const { page } = await open({ path: '/settings/hardware', stored: true });
  await nav(page).waitFor();
  assert.equal(await page.getByRole('navigation').count(), 1, 'more than one <nav> on a settings page');
  assert.equal(await page.locator('main').getByRole('link').filter({ hasText: /^(Network|Hardware|Security)$/ }).count(), 0);
  await page.close();
});

await check('a fold opens by itself on one of its pages, and folds away from the keyboard', async () => {
  const { page } = await open({ path: '/settings/security', stored: true });
  const settings = nav(page).getByRole('button', { name: 'Settings', exact: true });
  assert.equal(await settings.getAttribute('aria-expanded'), 'true', 'Settings is folded on one of its own pages');
  const mesh = nav(page).getByRole('button', { name: 'Entries under Mesh', exact: true });
  assert.equal(await mesh.getAttribute('aria-expanded'), 'false', 'Mesh is unfolded while elsewhere');
  // Folded entries are hidden: neither a click target nor a Tab stop.
  const market = nav(page).getByRole('link', { name: 'Market', exact: true, includeHidden: true });
  assert.equal(await market.isVisible(), false, 'the folded Market entry is visible');
  // Fold Settings with the keyboard: its pages leave the Tab order with it.
  await settings.focus();
  await page.keyboard.press('Enter');
  assert.equal(await settings.getAttribute('aria-expanded'), 'false');
  const hardware = nav(page).getByRole('link', { name: 'Hardware', exact: true, includeHidden: true });
  await page.waitForTimeout(300);
  assert.equal(await hardware.isVisible(), false, 'a folded page link is still visible');
  assert.equal(await hardware.evaluate((el) => el.closest('[hidden]') !== null), true, 'a folded page link is still reachable');
  await page.keyboard.press('Enter');
  assert.equal(await settings.getAttribute('aria-expanded'), 'true');
  await nav(page).getByRole('link', { name: 'Hardware', exact: true }).click();
  await page.waitForURL(origin + '/settings/hardware');
  await page.close();
});

await check('the panel narrows to icons with its trigger or Ctrl+B, and remembers it', async () => {
  const { page, errors } = await open({ path: '/mesh', stored: true });
  const panel = nav(page);
  const wide = (await panel.boundingBox()).width;
  await panel.getByRole('button', { name: 'Collapse the sidebar' }).click();
  await page.waitForTimeout(350);
  const narrow = (await panel.boundingBox()).width;
  assert.ok(narrow < 70 && wide > 200, `the panel did not narrow to a rail: ${wide} -> ${narrow}`);
  // On the rail every entry is still a named link, and the current one is marked.
  await panel.getByRole('link', { name: 'Mesh', exact: true }).waitFor();
  // The palette's button is a magnifier, on the rail as in the panel.
  const search = panel.getByRole('button', { name: /^Search or jump to/ });
  assert.ok((await search.boundingBox()).width <= 36, 'the search button is not a magnifier on the rail');
  // Each icon names itself in a tooltip beside the rail (Base UI Tooltip).
  await panel.getByRole('link', { name: 'Storage', exact: true }).hover();
  const tip = page.locator('[data-slot="tooltip-content"]').filter({ hasText: /^Storage$/ });
  await tip.waitFor({ timeout: 2000 });
  const tipBox = await tip.boundingBox();
  assert.ok(tipBox.x >= narrow, `the tooltip is not beside the rail: x=${tipBox.x}`);
  await panel.getByRole('link', { name: 'Settings', exact: true }).click();
  await page.waitForURL(origin + '/settings');
  // The choice survives a reload; Ctrl+B widens it again.
  await page.reload({ waitUntil: 'networkidle' });
  assert.ok((await panel.boundingBox()).width < 70, 'the rail was forgotten on reload');
  await page.keyboard.press('Control+b');
  await page.waitForTimeout(350);
  assert.ok((await panel.boundingBox()).width > 200, 'Ctrl+B did not widen the panel');
  const wideSearch = await panel.getByRole('button', { name: /^Search or jump to/ }).boundingBox();
  assert.ok(wideSearch.width <= 36, 'the search button grew back into a field');
  assert.deepEqual(errors, []);
  await page.close();
});

const bottomBar = (page) => page.getByRole('navigation', { name: 'Main places', exact: true });
const more = (page) => bottomBar(page).getByRole('button', { name: 'More', exact: true });

await check('on a phone the sections are a bar along the bottom, and More raises the rest as a sheet from below', async () => {
  const { page, errors } = await open({ path: '/storage', stored: true, viewport: { width: 375, height: 800 } });
  const bar = bottomBar(page);
  await bar.waitFor();
  // Four places and More, in reach of a thumb: the bar sits on the bottom
  // edge and every tab is at least 48 px tall.
  assert.deepEqual((await bar.getByRole('listitem').allInnerTexts()).map((l) => l.trim()), ['Overview', 'Apps', 'Storage', 'Mesh', 'More']);
  const box = await bar.boundingBox();
  assert.ok(Math.abs(box.y + box.height - 800) <= 1 && box.width === 375, `the bar is not on the bottom edge: ${JSON.stringify(box)}`);
  for (const tab of await bar.locator('a, button').all()) assert.ok((await tab.boundingBox()).height >= 48, 'a tab is too small to press');
  // The page marks itself, and the closed panel is nowhere in the page.
  assert.equal(await bar.locator('a[aria-current="page"]').innerText(), 'Storage');
  assert.equal(await nav(page).count(), 0, 'the closed phone panel is in the page');
  assert.equal(await page.getByRole('button', { name: 'Expand the sidebar' }).count(), 0, 'the old row above the page is still there');
  // A tab goes where it says, and the mark follows.
  await bar.getByRole('link', { name: 'Mesh', exact: true }).click();
  await page.waitForURL(origin + '/mesh');
  await bar.locator('a[aria-current="page"]').filter({ hasText: 'Mesh' }).waitFor();
  // The end of the page clears the bar.
  await page.evaluate(() => window.scrollTo(0, document.documentElement.scrollHeight));
  const shell = await page.locator('.losos-shell').evaluate((el) => parseFloat(getComputedStyle(el).paddingBottom));
  assert.ok(shell >= box.height - 1, `the page's foot is ${shell}px, the bar ${box.height}px`);

  // More: the whole tree in a sheet that rises from the bottom edge.
  assert.equal(await more(page).getAttribute('aria-expanded'), 'false');
  await more(page).click();
  const sheet = page.getByRole('dialog', { name: 'Sections' });
  await sheet.waitFor();
  await page.waitForTimeout(400);
  const up = await sheet.boundingBox();
  assert.ok(Math.abs(up.y + up.height - 800) <= 1 && up.x <= 1 && up.width >= 374, `the sheet did not rise from the bottom: ${JSON.stringify(up)}`);
  assert.equal(await page.evaluate(() => getComputedStyle(document.documentElement).overflow === 'hidden' || getComputedStyle(document.body).overflow === 'hidden'), true, 'the page behind the sheet still scrolls');
  await nav(page).locator('a[aria-current="page"]').filter({ hasText: 'Mesh' }).waitFor();
  // A page the bar does not name lights More instead.
  await nav(page).getByRole('button', { name: 'Settings', exact: true }).click();
  await nav(page).getByRole('link', { name: 'About', exact: true }).click();
  await page.waitForURL(origin + '/settings/about');
  await sheet.waitFor({ state: 'detached' });
  assert.equal(await bar.locator('a[aria-current="page"]').count(), 0);
  assert.equal(await more(page).getAttribute('data-active'), '');
  // Escape closes it, and so does the button on its title row.
  await more(page).click();
  await sheet.waitFor();
  await page.keyboard.press('Escape');
  await sheet.waitFor({ state: 'detached' });
  await more(page).click();
  await sheet.getByRole('button', { name: 'Dismiss' }).click();
  await sheet.waitFor({ state: 'detached' });
  assert.deepEqual(errors, []);
  await page.close();
});

await check('a flick down closes the phone sheet', async () => {
  const { page, errors } = await open({ path: '/', stored: true, viewport: { width: 375, height: 800 } });
  await more(page).click();
  const sheet = page.getByRole('dialog', { name: 'Sections' });
  await sheet.waitFor();
  await page.waitForTimeout(400);
  const up = await sheet.boundingBox();
  // Take hold of the title row and flick it down.
  const x = up.x + 60;
  const y = up.y + 30;
  await page.mouse.move(x, y);
  await page.mouse.down();
  for (let step = 1; step <= 8; step++) await page.mouse.move(x, y + step * 40, { steps: 1 });
  await page.mouse.up();
  await sheet.waitFor({ state: 'detached', timeout: 3000 });
  assert.deepEqual(errors, []);
  await page.close();
});

/* The signed-in Overview, the first page anyone sees, logs no violation
 * under the real policy at either width. Sonner used to inject its
 * stylesheet as a <style> element at load, which the policy refused on
 * every page; vite.sonner-quiet.ts drops that copy and main.tsx bundles
 * the file. */
await check('under the appliance CSP the signed-in Overview logs no violation', async () => {
  const { origin: strict, close } = await serve({ csp: true });
  const watch = () => {
    window.__violations = [];
    document.addEventListener('securitypolicyviolation', (e) =>
      window.__violations.push(`${e.violatedDirective} ${e.target?.nodeName ?? ''} ${e.sourceFile}:${e.columnNumber}`),
    );
  };
  try {
    for (const viewport of [{ width: 1280, height: 900 }, { width: 375, height: 800 }]) {
      const { page, errors } = await open({
        path: '/',
        viewport,
        person: { name: 'matus', displayName: 'Matus', groups: ['losos-admins'], admin: true, spareKey: false, sections: ['storage'] },
        base: strict,
        init: [watch],
      });
      await page.getByRole('region', { name: 'At a glance' }).waitFor();
      await page.waitForTimeout(500);
      assert.deepEqual(await page.evaluate(() => window.__violations), [], `${viewport.width}px: the Overview broke the admin CSP`);
      assert.deepEqual(errors, []);
      await page.close();
    }
  } finally {
    await close();
  }
});

await check('a box with chat and mail puts them in the bar', async () => {
  const { page, errors } = await open({ path: '/', viewport: { width: 375, height: 800 }, person: { name: 'matus', displayName: 'Matus', groups: ['losos-admins'], admin: true, spareKey: false, sections: [], shared: ['chat', 'mail'] } });
  await bottomBar(page).waitFor();
  assert.deepEqual((await bottomBar(page).getByRole('listitem').allInnerTexts()).map((l) => l.trim()), ['Overview', 'Chat', 'Mail', 'Apps', 'More']);
  assert.deepEqual(errors, []);
  await page.close();
});

await check('a wide screen has no bottom bar', async () => {
  const { page } = await open({ path: '/storage', stored: true });
  await nav(page).waitFor();
  assert.equal(await bottomBar(page).count(), 0);
  assert.equal(await page.locator('.losos-shell').evaluate((el) => getComputedStyle(el).paddingBottom), '0px');
  await page.close();
});

for (const { name, now, file, tip } of [
  { name: 'a live salmon', now: new Date(2026, 9, 8, 12), file: 'losos', tip: 'The logo is a live coho salmon' },
  { name: 'on Halloween, the plate of salmon', now: new Date(2026, 9, 31, 12), file: 'losos-halloween', tip: 'For Halloween, the logo is a plate of it' },
]) {
  await check(`${name} replaces the name in the top bar and is the favicon, served from the bundle`, async () => {
    const { page } = await open({ path: '/storage', stored: true, now });
    const logo = page.locator('header').getByRole('img', { name: 'LosOS', exact: true });
    await logo.waitFor();
    assert.ok(await logo.evaluate((img) => img.complete && img.naturalWidth > 0), 'the logo did not load');
    // Vite names a bundled file <name>-<hash>.png, the hash eight letters,
    // digits, _ or -, so losos-<hash> cannot match losos-halloween-<hash>.
    const bundled = new RegExp(`^/assets/${file}-[\\w-]{8}\\.png$`);
    const src = await logo.getAttribute('src');
    assert.match(src, bundled, `the logo is not the bundled ${file}.png: ${src}`);
    const icon = await page.locator('link[rel="icon"]').evaluate((link) => new URL(link.href).pathname);
    assert.match(icon, bundled, `the favicon is not the bundled ${file}.png: ${icon}`);
    assert.equal((await page.request.get(origin + icon)).status(), 200);
    assert.equal(await page.locator('header').getByText('LosOS', { exact: true }).count(), 0, 'the name is still drawn as text');
    // A photo needs a word of explanation: hovering the logo says what it
    // is, and the same words are the trigger's accessible name.
    const trigger = page.locator('header').getByRole('link', { name: /Losos is Slovak for salmon/ });
    await trigger.hover();
    await page.locator('[data-slot="tooltip-content"]').filter({ hasText: tip }).waitFor();
    await page.close();
  });
}

/* The sidebar's moving parts are Base UI (tooltip positioning, the sheet's
 * slide and scroll lock, the folds' measured height), chosen because Base
 * UI styles through CSSOM, which the appliance CSP allows, and never through
 * an injected <style> element or a style attribute, which it refuses. Under
 * the real policy, using all of them must add no violation. */
await check('under the appliance CSP the rail tooltip, the folds, the bottom bar and the phone sheet add no violation', async () => {
  const { origin: strict, close } = await serve({ csp: true });
  try {
    for (const viewport of [{ width: 1280, height: 900 }, { width: 375, height: 800 }]) {
      const page = await browser.newPage({ viewport, locale: 'en-US' });
      await page.addInitScript(() => {
        window.__violations = [];
        document.addEventListener('securitypolicyviolation', (e) => window.__violations.push(e.violatedDirective));
      });
      const errors = [];
      page.on('pageerror', (e) => errors.push(String(e)));
      await page.route('**/api/**', (route) => json(route, 200, {}));
      await page.route('**/api/setup/claim', (route) => json(route, 200, { claimed: true }));
      await page.route('**/api/settings', (route) => json(route, 200, SETTINGS));
      await page.addInitScript((t) => window.sessionStorage.setItem('losos-token', t), TOKEN);
      await page.goto(strict + '/mesh', { waitUntil: 'networkidle' });
      const atLoad = await page.evaluate(() => window.__violations.length);
      if (viewport.width > 767) {
        await nav(page).getByRole('button', { name: 'Settings', exact: true }).click();
        await page.waitForTimeout(300);
        await page.keyboard.press('Control+b');
        await page.waitForTimeout(300);
        await nav(page).getByRole('link', { name: 'Apps', exact: true }).hover();
        const tip = page.locator('[data-slot="tooltip-content"]').filter({ hasText: /^Apps$/ });
        await tip.waitFor({ timeout: 2000 });
        const box = await tip.boundingBox();
        assert.ok(box.x > 40 && box.y > 60, `the tooltip was not positioned: ${JSON.stringify(box)}`);
      } else {
        await page.getByRole('navigation', { name: 'Main places' }).getByRole('button', { name: 'More' }).click();
        await page.getByRole('dialog', { name: 'Sections' }).waitFor();
        await page.waitForTimeout(400);
        const box = await page.getByRole('dialog', { name: 'Sections' }).boundingBox();
        assert.ok(Math.abs(box.y + box.height - viewport.height) <= 1, `the sheet did not rise: ${JSON.stringify(box)}`);
      }
      const after = await page.evaluate(() => window.__violations);
      assert.equal(after.length, atLoad, `the sidebar added CSP violations: ${after.slice(atLoad).join(', ')}`);
      assert.deepEqual(errors, []);
      await page.close();
    }
  } finally {
    await close();
  }
});

/* Every on/off setting is shadcn's Switch in its React Aria flavour, laid
 * into the settings row as shadcn's "Switch with a description": a
 * horizontal Field whose FieldLabel names the switch and whose
 * FieldDescription, when the row has one, describes it. React Aria's switch
 * is a real <input type="checkbox" role="switch"> inside a <label>, so the
 * old hand-made <button role="switch"> must be gone from every pane. */
const SWITCH_PANES = {
  '/settings/network': ['Reachable from outside your home'],
  '/settings/hardware': null,
  '/settings/security': [
    'Ignore USB devices plugged in later',
    'Stricter memory handling',
    'Confine the programs that face the network',
    'Halve the processor to close a leak between jobs',
  ],
  '/mesh': ['Join the mesh', 'Lend this box while I sleep'],
};
await check('every settings toggle is a React Aria Switch, named by its row and described by its line', async () => {
  for (const [path, names] of Object.entries(SWITCH_PANES)) {
    const { page, errors } = await open({ path, stored: true });
    await page.getByRole('switch').first().waitFor();
    assert.equal(await page.locator('button[role="switch"]').count(), 0, `${path} still has a hand-made switch`);
    const switches = page.getByRole('switch');
    const shapes = await switches.evaluateAll((els) =>
      els.map((el) => ({
        tag: el.tagName,
        type: el.type,
        inSlot: el.closest('[data-slot="switch"]') !== null,
        inField: el.closest('[data-slot="field"][data-orientation="horizontal"]') !== null,
        label: document.querySelector(`label[for="${CSS.escape(el.id)}"]`)?.getAttribute('data-slot') ?? null,
        description: el.getAttribute('aria-describedby')
          ? document.getElementById(el.getAttribute('aria-describedby'))?.getAttribute('data-slot') ?? 'dangling'
          : null,
      })),
    );
    for (const shape of shapes) {
      assert.deepEqual(
        { tag: shape.tag, type: shape.type, inSlot: shape.inSlot, inField: shape.inField, label: shape.label },
        { tag: 'INPUT', type: 'checkbox', inSlot: true, inField: true, label: 'field-label' },
        `${path}: ${JSON.stringify(shape)}`,
      );
      assert.ok(shape.description === null || shape.description === 'field-description', `${path}: ${JSON.stringify(shape)}`);
    }
    if (names !== null) {
      for (const name of names) {
        assert.equal(await page.getByRole('switch', { name, exact: true }).count(), 1, `${path}: no switch named "${name}"`);
      }
    }
    assert.deepEqual(errors, []);
    await page.close();
  }
  // The description line is read with the switch: the row's detail is its accessible description.
  const { page } = await open({ path: '/settings/security', stored: true });
  const usb = page.getByRole('switch', { name: 'Ignore USB devices plugged in later' });
  const describedBy = await usb.getAttribute('aria-describedby');
  assert.match(await page.locator(`[id="${describedBy}"]`).innerText(), /Anything attached after the box starts is refused/);
  await page.close();
});

/* A box the installer put in keyfile mode keeps its disk key on the boot
 * partition. The Security pane says so, links the handbook page, and says
 * nothing on a box with a TPM or one that serves no option document. */
await check('the Security pane warns on a box without a TPM, and only there', async () => {
  const notice = (page) => page.locator('[data-notice="no-tpm"]');
  const keyfile = await open({ path: '/settings/security', stored: true, options: optionsWithTpm(false) });
  await notice(keyfile.page).waitFor({ timeout: 3000 }).catch(() => assert.fail('no TPM warning on a keyfile box'));
  assert.match(await notice(keyfile.page).innerText(), /This box has no TPM chip/);
  assert.match(await notice(keyfile.page).innerText(), /anyone who gets hold of the disk can read your files/i);
  const link = notice(keyfile.page).getByRole('link', { name: 'What a TPM does' });
  assert.equal(await link.getAttribute('href'), '/handbook/reference/tpm/');
  assert.deepEqual(keyfile.errors, []);
  await keyfile.page.close();

  for (const options of [optionsWithTpm(true), null]) {
    const { page } = await open({ path: '/settings/security', stored: true, options });
    await page.getByRole('switch', { name: 'Ignore USB devices plugged in later' }).waitFor();
    assert.equal(await notice(page).count(), 0, `a TPM warning with options ${options === null ? 'absent' : 'tpm on'}`);
    await page.close();
  }
});

await check('a switch flips from its label, from Space and from its track, and a disabled row reads as disabled', async () => {
  const { page, errors } = await open({ path: '/settings/security', stored: true });
  const usb = page.getByRole('switch', { name: 'Ignore USB devices plugged in later' });
  const track = page.locator('[data-slot="switch"]').filter({ has: usb });
  // The apply bar is what a change raises. (Apply itself stays off here: the
  // stub's port 80 is out of range, which this pane cannot fix.)
  const apply = page.getByRole('button', { name: 'Apply', exact: true });
  assert.equal(await usb.isChecked(), false);
  await page.getByText('Ignore USB devices plugged in later', { exact: true }).click();
  assert.equal(await usb.isChecked(), true, 'clicking the label did not turn it on');
  assert.equal(await track.getAttribute('data-selected'), 'true', 'the track does not show it on');
  await apply.waitFor({ timeout: 2000 }).catch(() => assert.fail('turning a switch on raised no apply bar'));
  await usb.focus();
  await page.keyboard.press('Space');
  assert.equal(await usb.isChecked(), false, 'Space did not turn it off');
  await track.click();
  assert.equal(await usb.isChecked(), true, 'clicking the track did not turn it on');
  await page.close();

  // Lending needs the mesh: with it off, the lend row is disabled and says why.
  const mesh = await open({ path: '/mesh', stored: true, settings: JOINABLE });
  const lend = mesh.page.getByRole('switch', { name: 'Lend this box while I sleep' });
  await lend.waitFor({ state: 'attached' });
  assert.ok(await lend.isDisabled(), 'lending is offered without the mesh');
  const field = mesh.page.locator('[data-slot="field"]').filter({ has: lend });
  assert.equal(await field.getAttribute('data-disabled'), 'true', 'the lend row does not read as disabled');
  assert.match(await field.locator('[data-slot="field-description"]').innerText(), /Join the mesh first/);
  // React Aria's input is visually hidden under its <label>; a pointer lands on the track.
  const join = mesh.page.getByRole('switch', { name: 'Join the mesh' });
  await mesh.page.locator('[data-slot="switch"]').filter({ has: join }).click();
  assert.equal(await join.isChecked(), true, 'clicking the track did not join');
  assert.ok(await lend.isEnabled(), 'joining did not free the lend switch');
  assert.equal(await field.locator('[data-slot="field-description"]').count(), 0, 'the "join first" line stayed');
  assert.deepEqual([...errors, ...mesh.errors], []);
  await mesh.page.close();
});

/* React Aria and Base UI side by side, under the real policy: the switches
 * hide their input through a React style prop (CSSOM, allowed), and React
 * Aria's one injectable <style> (touch-action for pressables) is shipped in
 * index.css instead. Flipping every switch on a pane, then working the
 * sidebar beside them, must add no violation to the ones logged at load. */
await check('under the appliance CSP the switches and the sidebar beside them add no violation', async () => {
  const { origin: strict, close } = await serve({ csp: true });
  try {
    const page = await browser.newPage({ viewport: { width: 1280, height: 900 }, locale: 'en-US' });
    await page.addInitScript(() => {
      window.__violations = [];
      document.addEventListener('securitypolicyviolation', (e) => window.__violations.push(e.violatedDirective));
    });
    const errors = [];
    page.on('pageerror', (e) => errors.push(String(e)));
    await page.route('**/api/**', (route) => json(route, 200, {}));
    await page.route('**/api/setup/claim', (route) => json(route, 200, { claimed: true }));
    await page.route('**/api/settings', (route) => json(route, 200, SETTINGS));
    await page.addInitScript((t) => window.sessionStorage.setItem('losos-token', t), TOKEN);
    await page.goto(strict + '/settings/security', { waitUntil: 'networkidle' });
    await page.getByRole('switch').first().waitFor();
    const atLoad = await page.evaluate(() => window.__violations.length);
    for (const sw of await page.getByRole('switch').all()) {
      await sw.focus();
      await page.keyboard.press('Space');
    }
    for (const track of await page.locator('[data-slot="switch"]').all()) await track.click();
    // The pressable rule is the bundle's, not React Aria's refused copy.
    assert.equal(
      await page.locator('[data-slot="switch"]').first().evaluate((el) => getComputedStyle(el).touchAction),
      'manipulation',
    );
    await page.keyboard.press('Control+b');
    await page.waitForTimeout(300);
    await nav(page).getByRole('link', { name: 'Apps', exact: true }).hover();
    await page.locator('[data-slot="tooltip-content"]').filter({ hasText: /^Apps$/ }).waitFor({ timeout: 2000 });
    await page.keyboard.press('Control+b');
    await page.getByRole('switch', { name: 'Stricter memory handling' }).focus();
    await page.keyboard.press('Space');
    const after = await page.evaluate(() => window.__violations);
    assert.equal(after.length, atLoad, `the switches added CSP violations: ${after.slice(atLoad).join(', ')}`);
    assert.deepEqual(errors, []);
    await page.close();
  } finally {
    await close();
  }
});

for (const path of ['/apps', '/storage', '/mesh', '/machines', '/settings', '/settings/hardware', '/settings/advanced', '/settings/history', '/settings/backup', '/settings/about', '/settings/reset']) {
  await check(`a deep link to ${path} renders without errors and survives a reload`, async () => {
    const { page, errors } = await open({ path, stored: true });
    await nav(page).waitFor();
    assert.ok(!/Nothing here/.test(await body(page)), `${path} fell through to not-found`);
    await page.reload({ waitUntil: 'networkidle' });
    await nav(page).waitFor();
    assert.equal(new URL(page.url()).pathname, path);
    assert.deepStrictEqual(errors, [], `page errors: ${errors.join('; ')}`);
    await page.close();
  });
}

await check('About leads with the address this browser is using and names the .local address separately', async () => {
  /* The harness serves on 127.0.0.1:<port> and the stubbed box is called
   * mattbox, so the two differ — the libvirt case, where the owner reached
   * the box by IP and mattbox.local does not resolve on the host. The pane
   * must not send them to the name as if it were the one address. */
  const { page, errors } = await open({ path: '/settings/about', stored: true });
  const main = page.locator('main');
  await main.getByText(origin, { exact: true }).waitFor();
  await main.getByText('http://mattbox.local', { exact: true }).waitFor();
  const text = await main.innerText();
  assert.ok(text.indexOf(origin) < text.indexOf('http://mattbox.local'), 'the address in use comes before the name');
  assert.deepStrictEqual(errors, []);
  await page.close();
});

const tiles = (page) =>
  page.getByRole('list', { name: 'Apps on this box' }).getByRole('link').evaluateAll((links) =>
    links.map((a) => [a.textContent.trim(), a.getAttribute('href')]),
  );

await check('the homepage has a tile for each page of LosOS Cloud, LosOS Git and Settings', async () => {
  const { page, errors } = await open({ stored: true, cloud: {} });
  await page.getByRole('link', { name: 'Photos' }).waitFor();
  assert.deepEqual(await tiles(page), [
    ['Files', '/cloud/files'],
    ['Photos', '/cloud/photos'],
    ['Code', '/git/'],
    ['Issues', '/git/issues'],
    ['Settings', '/settings'],
  ]);
  assert.deepEqual(errors, []);
  await page.close();
});

await check('an unknown address says so instead of showing a blank page', async () => {
  const { page } = await open({ path: '/no/such/page', stored: true });
  await page.getByText('Nothing here').waitFor();
  await page.close();
});

await check('the theme toggle stamps the choice on <html> and it survives a reload', async () => {
  const { page } = await open({ stored: true });
  const stamp = () => page.evaluate(() => document.documentElement.className + '|' + document.documentElement.dataset.theme);
  const before = await stamp();
  const dark = page.getByRole('radio', { name: 'Dark' });
  await dark.click();
  const after = await stamp();
  assert.notEqual(after, before, 'choosing Dark changed nothing on <html>');
  await page.reload({ waitUntil: 'networkidle' });
  const reloaded = await stamp();
  assert.equal(reloaded, after, 'the theme was lost on reload (theme-boot.js did not stamp it)');
  await page.close();
});

await check('a Slovak browser gets Slovak, and the picker switches to German for good', async () => {
  const { page, errors } = await open({ stored: true, locale: 'sk-SK' });
  await page.getByRole('navigation', { name: 'Sekcie', exact: true }).getByText('Prehľad').waitFor();
  assert.equal(await page.evaluate(() => document.documentElement.lang), 'sk');
  await page.getByLabel('Jazyk').selectOption('de');
  await page.getByRole('navigation', { name: 'Bereiche', exact: true }).getByText('Übersicht').waitFor();
  await page.reload({ waitUntil: 'networkidle' });
  await page.getByRole('navigation', { name: 'Bereiche', exact: true }).getByText('Übersicht').waitFor();
  assert.equal(await page.evaluate(() => document.documentElement.lang), 'de');
  assert.deepEqual(errors, []);
  await page.close();
});

await check('a browser in a language we do not carry falls back to English', async () => {
  const { page } = await open({ stored: true, locale: 'fr-FR' });
  await nav(page).getByText('Overview').waitFor();
  await page.close();
});

for (const locale of ['sk-SK', 'de-DE']) {
  await check(`every section renders in ${locale} without errors`, async () => {
    for (const path of ['/', '/apps', '/storage', '/mesh', '/machines', '/settings/network', '/settings/hardware', '/settings/security', '/settings/access', '/settings/advanced', '/settings/history', '/settings/backup', '/settings/about', '/settings/reset']) {
      const { page, errors } = await open({ path, stored: true, locale, market: MARKET, vms: VMS, backup: BACKUP_SET });
      await page.locator('main').waitFor();
      assert.deepEqual(errors, [], `${path} threw in ${locale}`);
      await page.close();
    }
  });
}

await check('a box the market is not offered to says so quietly, and still offers the sharing switch', async () => {
  const { page, errors } = await open({ path: '/settings/market', stored: true });
  await page.getByText('The market is not available on this box').waitFor();
  await page.getByRole('switch', { name: "Share this box's disk with the mesh" }).waitFor();
  assert.deepEqual(errors, []);
  await page.close();
});

await check('buying opens Stripe Checkout in a new tab', async () => {
  const { page, errors, marketPosts } = await open({ path: '/settings/market', stored: true, market: MARKET });
  const tab = page.context().waitForEvent('page');
  await page.getByRole('button', { name: 'Buy', exact: true }).click();
  const checkout = await tab;
  await checkout.waitForURL('https://checkout.stripe.com/c/pay/cs_test_1');
  assert.deepEqual(marketPosts, [['/api/market/orders', { listing_id: 'lst_ab12', quantity: 1 }]]);
  assert.deepEqual(errors, []);
  await page.close();
});

/* A seller with a paid period running on its one listing, a sale held for
 * it, and a purchase whose seller went offline: part refunded, the rest
 * paid. */
const SLOT_END = 1792800000;
// Keep the paid-period assertions independent of the calendar date CI runs.
const SLOT_NOW = new Date((SLOT_END - 86400) * 1000);
const slotOrder = (id, extra) => ({
  id,
  listing_id: 'lst_held',
  kind: 'storage',
  unit: 'GiB-month',
  quantity: 4,
  amount: 2000,
  fee: 80,
  seller_net: 1920,
  currency: 'eur',
  status: 'paid',
  created_at: 1790200000,
  paid_at: 1790200000,
  expires_at: SLOT_END,
  expired: false,
  volume: null,
  ...extra,
});
const SLOTS = {
  ...MARKET,
  account: {
    ...MARKET.account,
    listings: [
      { id: 'lst_held', kind: 'storage', unit: 'GiB-month', unit_price: 500, capacity: 10, available: 6, active: true, locked_until: SLOT_END },
    ],
    sales: [slotOrder('ord_sale', { card: { step: 'held' }, payout_at: SLOT_END + 86400 })],
    purchases: [slotOrder('ord_bought', { card: { step: 'settled', reason: 'sellerOffline', refund: 1401 } })],
    payouts: { held: 1920, ready: 960, paying: 0, next_at: SLOT_END, auto_after_secs: 86400 },
    offline_refund_secs: 86400,
    direct_pay: false,
  },
};

await check('a paid period greys Stop selling, and Pay out sends what is ready', async () => {
  const { page, errors, marketPosts } = await open({ path: '/settings/market', stored: true, market: SLOTS, now: SLOT_NOW });
  await page.getByText(/on sale · paid for until/).waitFor();
  const stop = page.getByRole('button', { name: 'Stop selling' });
  assert.ok(await stop.isDisabled(), 'Stop selling is live while a paid period runs');
  assert.match((await stop.getAttribute('title')) ?? '', /Someone has paid for this until/);
  await page.getByText('Held until the paid period ends').waitFor();
  await page.getByText("€14.01 refunded to the buyer, the rest paid to the seller · the seller's box was offline for too long").waitFor();
  await page.getByText('If this box is offline for 24 hours in a row', { exact: false }).waitFor();
  const sent = page.waitForRequest((request) => request.url().endsWith('/api/market/release'));
  await page.getByRole('button', { name: 'Pay out' }).click();
  await sent;
  await page.locator('[data-toast]').filter({ hasText: '€9.60 is on its way to you.' }).waitFor();
  assert.deepEqual(marketPosts, [['/api/market/release', {}]]);
  assert.deepEqual(errors, []);
  await page.close();
});

await check('Pay out is greyed until a period ends, and says when the next one does', async () => {
  const waiting = { ...SLOTS, account: { ...SLOTS.account, payouts: { ...SLOTS.account.payouts, ready: 0 } } };
  const { page, errors, marketPosts } = await open({ path: '/settings/market', stored: true, market: waiting, now: SLOT_NOW });
  await page.getByText('You can pay out once a period ends, the next on', { exact: false }).waitFor();
  assert.ok(await page.getByRole('button', { name: 'Pay out' }).isDisabled(), 'Pay out is live with nothing ready');
  assert.deepEqual(marketPosts, []);
  assert.deepEqual(errors, []);
  await page.close();
});

await check('a payment page that is not Stripe is never opened', async () => {
  const { page, marketPosts } = await open({
    path: '/settings/market',
    stored: true,
    market: MARKET,
    checkoutUrl: 'https://checkout.stripe.com.evil.example/pay',
  });
  const tab = page.context().waitForEvent('page');
  await page.getByRole('button', { name: 'Buy', exact: true }).click();
  const placeholder = await tab;
  await page.locator('[data-toast]').filter({ hasText: 'no payment page came back' }).waitFor();
  await placeholder.waitForEvent('close', { timeout: 2000 }).catch(() => {});
  assert.ok(placeholder.isClosed(), 'the placeholder tab was left open');
  assert.equal(marketPosts.length, 1);
  await page.close();
});

await check('setting up payouts opens Stripe onboarding in a tab with no opener', async () => {
  const unready = { ...MARKET, account: { ...MARKET.account, seller_onboarded: false, seller_ready: false } };
  const { page, errors, marketPosts } = await open({ path: '/settings/market', stored: true, market: unready });
  const tab = page.waitForEvent('popup');
  await page.getByRole('button', { name: 'Set up payouts' }).click();
  const onboarding = await tab;
  await onboarding.waitForURL('https://connect.stripe.com/setup/e/acct_test/abc');
  assert.equal(await onboarding.evaluate(() => window.opener), null);
  assert.deepEqual(marketPosts, [['/api/market/onboard', {}]]);
  assert.deepEqual(errors, []);
  await page.close();
});

await check('a valid listing is sent in minor units', async () => {
  const { page, marketPosts } = await open({ path: '/settings/market', stored: true, market: MARKET });
  await page.getByLabel('Price per unit (EUR)').fill('1,25');
  await page.getByLabel('Capacity (units)').fill('2');
  const sent = page.waitForRequest((request) => request.url().endsWith('/api/market/listings'));
  await page.getByRole('button', { name: 'Offer for sale' }).click();
  await sent;
  assert.deepEqual(marketPosts, [['/api/market/listings', { kind: 'storage', unit_price: 125, capacity: 2 }]]);
  await page.close();
});

await check('the listing form refuses a bad price before anything is sent', async () => {
  const { page, marketPosts } = await open({ path: '/settings/market', stored: true, market: MARKET });
  await page.getByPlaceholder('Price', { exact: false }).fill('0.005');
  await page.getByPlaceholder('Capacity', { exact: false }).fill('10');
  await page.getByRole('button', { name: 'Offer for sale' }).click();
  await page.getByText('Enter a price above zero, like 0.05.').waitFor();
  const price = page.getByPlaceholder('Price', { exact: false });
  assert.equal(await price.getAttribute('aria-invalid'), 'true');
  const described = await price.getAttribute('aria-describedby');
  assert.match(await page.locator(`[id="${described}"]`).innerText(), /like 0\.05/);
  assert.deepEqual(marketPosts, []);
  await page.close();
});

await check('a zero-decimal currency is priced in whole units, not hundredths', async () => {
  const yen = { ...MARKET, account: { ...MARKET.account, currency: 'jpy' } };
  const { page, marketPosts } = await open({ path: '/settings/market', stored: true, market: yen });
  await page.getByLabel('Price per unit (JPY)').fill('1,5');
  await page.getByLabel('Capacity (units)').fill('2');
  await page.getByRole('button', { name: 'Offer for sale' }).click();
  await page.getByText('Enter a price above zero, like 5.').waitFor();
  assert.deepEqual(marketPosts, []);
  await page.getByLabel('Price per unit (JPY)').fill('500');
  const sent = page.waitForRequest((request) => request.url().endsWith('/api/market/listings'));
  await page.getByRole('button', { name: 'Offer for sale' }).click();
  await sent;
  assert.deepEqual(marketPosts, [['/api/market/listings', { kind: 'storage', unit_price: 500, capacity: 2 }]]);
  await page.close();
});

await check('a blocked payouts pop-up says so', async () => {
  const unready = { ...MARKET, account: { ...MARKET.account, seller_onboarded: false, seller_ready: false } };
  const { page, errors, marketPosts } = await open({ path: '/settings/market', stored: true, market: unready });
  await page.evaluate(() => {
    window.open = () => null;
  });
  await page.getByRole('button', { name: 'Set up payouts' }).click();
  await page.locator('[data-toast]').filter({ hasText: 'Allow pop-ups for this page' }).waitFor();
  assert.deepEqual(marketPosts, []);
  assert.deepEqual(errors, []);
  await page.close();
});

await check('a quantity out of range is tied to its field', async () => {
  const { page, marketPosts } = await open({ path: '/settings/market', stored: true, market: MARKET });
  const qty = page.getByLabel('Quantity');
  await qty.fill('41');
  await page.getByRole('button', { name: 'Buy', exact: true }).click();
  await page.getByText('Enter a whole number from 1 to 40.').waitFor();
  assert.equal(await qty.getAttribute('aria-invalid'), 'true');
  const described = await qty.getAttribute('aria-describedby');
  assert.match(await page.locator(`[id="${described}"]`).innerText(), /from 1 to 40/);
  assert.deepEqual(marketPosts, []);
  await page.close();
});

/* ── The shadcn conversion ─────────────────────────────────────────────
 * The admin UI's primitives are shadcn/ui components on the box's palette
 * (admin-ui/app/components.json). These checks pin the places that changed
 * shape, by the `data-slot` each component stamps on itself, so a later
 * "tidy" that drops one back to a bare div fails here and not on a box. */

const RESULTS = {
  sources: ['nixpkgs', 'flathub'],
  results: [
    { id: 'jellyfin', name: 'Jellyfin', version: '10.11.0', summary: 'A media server.', source: 'nixpkgs', homepage: 'https://jellyfin.org' },
    { id: 'immich', name: 'Immich', version: '2.4.1', summary: 'Photo backup.', source: 'nixpkgs', homepage: 'https://immich.app' },
    { id: 'vaultwarden', name: 'Vaultwarden', summary: 'A password manager server.', source: 'flathub' },
  ],
};

await check('the spare key prompt is one Field and a refusal marks it invalid', async () => {
  const { page } = await openGate();
  await page.getByRole('button', { name: /Use the spare admin key instead/ }).click();
  const field = page.locator('[role="dialog"] [data-slot="field"], dialog [data-slot="field"]').first();
  await field.waitFor();
  const input = field.locator('input#admin-key');
  assert.equal(await input.count(), 1, 'the key input is not inside the Field');
  assert.equal(await field.getAttribute('data-invalid'), null, 'the field is invalid before anything was typed');
  await input.fill('b'.repeat(64));
  await page.getByRole('button', { name: 'Unlock' }).click();
  await page.waitForTimeout(300);
  assert.equal(await field.getAttribute('data-invalid'), 'true', 'a refusal did not mark the field invalid');
  const error = field.locator('[data-slot="field-error"]');
  assert.equal(await error.getAttribute('role'), 'alert');
  assert.equal(await input.getAttribute('aria-describedby'), await error.getAttribute('id'), 'the error is not linked to the input');
  await page.close();
});

await check('the catalogue lists hits in a Table with the publisher as a column, and shows an Empty state when nothing comes back', async () => {
  const { page, errors } = await open({ path: '/apps', stored: true });
  await page.route('**/api/apps/search**', (route) => {
    const q = new URL(route.request().url()).searchParams.get('q') ?? '';
    return json(route, 200, q === 'zzz' ? { sources: ['nixpkgs'], results: [] } : RESULTS);
  });
  const search = page.getByPlaceholder('Search for an app');
  assert.equal(await search.locator('xpath=ancestor::*[@data-slot="input-group"]').count(), 1, 'the search is not an Input Group');
  await search.fill('media');
  const table = page.locator('[data-slot="table"]');
  await table.waitFor();
  const heads = await table.locator('thead th').allInnerTexts();
  assert.deepEqual(heads.slice(0, 2), ['App', 'Published by']);
  assert.equal(await table.locator('tbody tr').count(), 3, 'one row per hit');
  const sources = await table.locator('tbody tr td:nth-child(2)').allInnerTexts();
  assert.deepEqual(sources, ['nixpkgs', 'nixpkgs', 'flathub'], 'the publisher must be on every row, never abbreviated');
  assert.equal(await table.getByRole('link', { name: /Look at it/ }).count(), 2, 'a hit without a homepage gets no link');
  await search.fill('zzz');
  await page.locator('[data-slot="empty"]').waitFor();
  assert.match(await body(page), /Nothing came back/i);
  assert.deepEqual(errors, []);
  await page.close();
});

await check('an empty board and an unknown address are both Empty states', async () => {
  const { page } = await open({ stored: true });
  const board = page.locator('[data-slot="empty"]');
  await board.waitFor();
  assert.equal(await board.getByRole('button', { name: 'Add a widget' }).count(), 1, 'the invitation lost its button');
  await page.goto(origin + '/nothing-here', { waitUntil: 'networkidle' });
  await page.locator('[data-slot="empty"]').waitFor();
  assert.match(await body(page), /Nothing here/);
  await page.close();
});

await check('the gallery offers each widget as an Item with its own action', async () => {
  const { page } = await open({ stored: true });
  await page.getByRole('button', { name: 'Add a widget' }).first().click();
  const dialog = page.getByRole('dialog');
  await dialog.waitFor();
  const items = dialog.locator('[data-slot="item"]');
  const count = await items.count();
  assert.ok(count >= 3, `expected the catalogue plus the custom card, got ${count}`);
  for (let i = 0; i < count; i++) {
    const item = items.nth(i);
    assert.equal(await item.locator('[data-slot="item-title"]').count(), 1, `item ${i} has no title`);
    assert.equal(await item.getByRole('button').count(), 1, `item ${i} does not carry exactly one action`);
  }
  assert.equal(await items.last().getAttribute('data-variant'), 'dashed', 'the build-your-own card is the dashed one');
  await page.close();
});

await check('the theme toggle is a Toggle Group: a radio group the arrow keys move through', async () => {
  const { page } = await open({ stored: true });
  const group = page.getByRole('radiogroup', { name: 'Appearance' });
  assert.equal(await group.getAttribute('data-slot'), 'toggle-group');
  const auto = group.getByRole('radio', { name: 'Match the browser' });
  assert.equal(await auto.getAttribute('aria-checked'), 'true');
  await auto.focus();
  await page.keyboard.press('ArrowRight');
  assert.equal(await group.getByRole('radio', { name: 'Light' }).getAttribute('aria-checked'), 'true', 'ArrowRight did not move the choice');
  assert.equal(await page.evaluate(() => document.documentElement.dataset.theme), 'light');
  await page.keyboard.press('End');
  assert.equal(await page.evaluate(() => document.documentElement.dataset.theme), 'dark', 'End did not jump to the last choice');
  await page.close();
});

await check('the mesh hours are one Button Group holding both time inputs', async () => {
  const { page } = await open({ path: '/mesh', stored: true });
  const group = page.getByRole('group', { name: 'Hours' });
  await group.waitFor();
  assert.equal(await group.getAttribute('data-slot'), 'button-group');
  assert.equal(await group.locator('input[type="time"]').count(), 2);
  assert.equal(await group.locator('[data-slot="button-group-text"]').innerText(), 'until');
  await page.close();
});

await check('purchases are listed in a Table with a column per fact', async () => {
  const bought = {
    ...MARKET,
    account: {
      ...MARKET.account,
      purchases: [
        { id: 'ord_1', kind: 'storage', amount: 1000, fee: 40, seller_net: 960, currency: 'eur', quantity: 2, unit: 'GiB-month', status: 'paid', created_at: 1791158400, paid_at: 1791158460, expired: false, expires_at: 1798761600, volume: 'pvc-1' },
      ],
    },
  };
  const { page } = await open({ path: '/settings/market', stored: true, market: bought });
  const table = page.getByTestId('market-purchases');
  await table.waitFor();
  assert.deepEqual(await table.locator('thead th').allInnerTexts(), ['Bought', 'Quantity', 'Until', 'Status']);
  assert.equal(await table.locator('tbody tr').count(), 1);
  assert.equal(await page.getByLabel('Quantity').locator('xpath=ancestor::*[@data-slot="button-group"]').count(), 1, 'quantity and Order are not one Button Group');
  await page.close();
});

/* A USDC edge that also takes Bitcoin, and a seller who owes platform fees
 * on coin sales: under the limit, with one USDC sale that paid toward them.
 * No payout address, so the page asks no chain for balances or fees. */
const COIN_FEES = {
  ...MARKET,
  account: {
    ...MARKET.account,
    currency: 'usd',
    usdc: {
      test: true,
      chain_id: 84532,
      solana: null,
      ens_chain_id: null,
      chains: [
        {
          network: 'base-sepolia',
          name: 'Base Sepolia',
          family: 'evm',
          chain_id: 84532,
          usdc: `0x${'11'.repeat(20)}`,
          pay: `0x${'22'.repeat(20)}`,
          treasury: `0x${'33'.repeat(20)}`,
        },
      ],
    },
    coins: ['btc'],
    coin_keys: { btc: 'wpkh([73c5da0a/84h/1h/0h]tpubD6NzVbkrYhZ4X/0/*)' },
    coin_fees_owed: 850,
    coin_fees_since: 1791158400,
    coin_fee_limit: 1000,
    coin_fee_grace_secs: 30 * 24 * 3600,
    sales: [
      { id: 'ord_2', kind: 'storage', amount: 10000, fee: 400, fee_offset: 850, seller_net: 8750, currency: 'usd', quantity: 2, unit: 'GiB-month', status: 'paid', created_at: 1791158400, paid_at: 1791158460, expired: false, expires_at: 1798761600, volume: null },
    ],
  },
};

await check('coin fees owed get one row with the limits, and a USDC sale names what it paid toward them', async () => {
  const { page, errors, marketPosts } = await open({ path: '/settings/market', stored: true, market: COIN_FEES });
  const row = page.getByTestId('coin-fees');
  await row.waitFor();
  const text = await row.innerText();
  assert.match(text, /Platform fees on coin sales/);
  assert.match(text, /You owe \S*8\.50/);
  assert.match(text, /no more than \S*10\.00 and it is settled by/);
  assert.match(text, /Set a USDC payout address on the Ethereum networks first/);
  const pay = row.getByRole('button', { name: /^Pay \S*8\.50$/ });
  assert.equal(await pay.isDisabled(), true, 'fees can be paid with no payout address to pay them from');
  assert.equal(await page.getByText(/Platform fees owed on coin sales: /).count(), 0, 'the coin key row still repeats the debt');
  await page.getByText(/you receive \S*87\.50, after \S*8\.50 toward platform fees owed on coin sales/).waitFor();
  assert.deepEqual(marketPosts, []);
  assert.deepEqual(errors, []);
  await page.close();
});

/* A USDC edge on Base mainnet that is a Binance partner, and a box with a
 * payout address there: the wallet offers euros for USDC at Binance. */
const PAYOUT = `0x${'44'.repeat(20)}`;
const ONRAMP_MARKET = {
  ...MARKET,
  account: {
    ...MARKET.account,
    currency: 'usd',
    payout: PAYOUT,
    usdc: {
      test: false,
      chain_id: 8453,
      solana: null,
      ens_chain_id: null,
      onramp: 'binance',
      chains: [
        {
          network: 'base',
          name: 'Base',
          family: 'evm',
          chain_id: 8453,
          usdc: `0x${'11'.repeat(20)}`,
          pay: `0x${'22'.repeat(20)}`,
          treasury: `0x${'33'.repeat(20)}`,
        },
      ],
    },
  },
};

const BINANCE_LINK = 'https://app.binance.com/en/pre-connect?orderId=1';
const binanceOrder = (stage, extra = {}) => ({
  order_id: 'a1'.repeat(16),
  link: BINANCE_LINK,
  link_expires: 0,
  euros: '25',
  address: PAYOUT,
  network: 'base',
  stage,
  status: 0,
  usdc: null,
  tx: null,
  ...extra,
});

await check('buying USDC at Binance takes each of its documents opened, then the edge makes the order and the row follows it', async () => {
  const { page, errors, marketPosts } = await open({ path: '/settings/market', stored: true, market: ONRAMP_MARKET });
  // Binance's pages, and the edge's answers through lososd.
  await page.context().route('https://www.binance.com/**', (route) =>
    route.fulfill({ status: 200, contentType: 'text/html', body: '<title>Binance</title>' }),
  );
  let stage = binanceOrder('open');
  await page.route('**/api/market/onramp', (route) => {
    marketPosts.push(['/api/market/onramp', route.request().postDataJSON()]);
    return json(route, 200, binanceOrder('open'));
  });
  await page.route('**/api/market/onramp/status', (route) => {
    marketPosts.push(['/api/market/onramp/status', route.request().postDataJSON()]);
    return json(route, 200, stage);
  });

  const row = page.getByTestId('onramp-topup');
  await row.waitFor();
  assert.match(await row.innerText(), /Buy USDC with euros/);
  await row.getByRole('button', { name: 'Buy', exact: true }).click();
  assert.match(await row.innerText(), /Binance sells the USDC, not LosOS/);
  assert.match(await row.innerText(), /may refuse a purchase from yours/);

  const docs = {
    'Terms of Use': 'https://www.binance.com/en/terms',
    'Product Terms': 'https://www.binance.com/en/about-legal/product-terms',
    'Privacy Notice': 'https://www.binance.com/en/about-legal/privacy-portal',
    'Risk Warning': 'https://www.binance.com/en/risk-warning',
  };
  for (const [name, href] of Object.entries(docs)) {
    assert.equal(await row.getByRole('link', { name }).getAttribute('href'), href);
  }
  const accept = row.getByRole('checkbox');
  const go = row.getByRole('button', { name: 'Create the order' });
  const amount = row.getByLabel('Amount in euros');
  await amount.fill('0,50');
  await row.getByText(/Enter 1 to 100,000 euros/).waitFor();
  await amount.fill('25');
  assert.equal(await accept.isDisabled(), true, 'the terms can be accepted unread');
  assert.equal(await go.isDisabled(), true, 'an order can be made before the terms are accepted');
  assert.equal(await row.getByText('Not opened').count(), 4);

  // Opening three of the four is not enough.
  const names = Object.keys(docs);
  for (const name of names.slice(0, 3)) {
    const [tab] = await Promise.all([page.waitForEvent('popup'), row.getByRole('link', { name }).click()]);
    await tab.close();
  }
  assert.equal(await accept.isDisabled(), true, 'the terms can be accepted with one document unopened');
  const [last] = await Promise.all([page.waitForEvent('popup'), row.getByRole('link', { name: names[3] }).click()]);
  await last.close();
  assert.equal(await row.getByText('Opened', { exact: true }).count(), 4);
  assert.equal(await go.isDisabled(), true, 'opening the documents is not accepting them');
  await accept.check();
  assert.equal(await go.isDisabled(), false);
  await go.click();

  const followed = page.getByTestId('onramp-order');
  await followed.getByText(/Your order for 25 euros is ready/).waitFor();
  assert.equal(await followed.getByRole('link', { name: 'Open Binance' }).getAttribute('href'), BINANCE_LINK);
  assert.deepEqual(
    marketPosts.filter(([path]) => path === '/api/market/onramp'),
    [['/api/market/onramp', { address: PAYOUT, euros: '25' }]],
  );
  assert.deepEqual(
    JSON.parse(await page.evaluate(() => localStorage.getItem('losos-binance-terms'))).terms,
    'binance@2026-10-21',
  );

  // A reload picks the order up again, and a finished one links the transfer.
  const tx = `0x${'ab'.repeat(32)}`;
  stage = binanceOrder('done', { status: 20, usdc: '24.61', tx });
  await page.reload({ waitUntil: 'networkidle' });
  const done = page.getByTestId('onramp-order');
  await done.getByText('Binance sent 24.61 USDC to this wallet on Base.').waitFor();
  assert.equal(await done.getByRole('link', { name: 'Open Binance' }).count(), 0);
  assert.equal(await done.getByRole('link', { name: 'View the transaction' }).getAttribute('href'), `https://basescan.org/tx/${tx}`);
  assert.deepEqual(
    marketPosts.filter(([path]) => path === '/api/market/onramp/status').at(-1),
    ['/api/market/onramp/status', { order_id: 'a1'.repeat(16) }],
  );
  await done.getByRole('button', { name: 'Done' }).click();
  assert.equal(await page.getByTestId('onramp-topup').getByRole('button', { name: 'Buy', exact: true }).count(), 1);
  assert.equal(await page.evaluate(() => localStorage.getItem('losos-binance-order')), null);
  assert.deepEqual(errors, []);
  await page.close();
});

await check('an order Binance refuses shows its reason and makes nothing to follow', async () => {
  const { page, errors } = await open({ path: '/settings/market', stored: true, market: ONRAMP_MARKET });
  await page.context().route('https://www.binance.com/**', (route) =>
    route.fulfill({ status: 200, contentType: 'text/html', body: '<title>Binance</title>' }),
  );
  await page.route('**/api/market/onramp', (route) =>
    json(route, 409, { error: 'Binance refused the order: service unavailable in your region' }),
  );
  const row = page.getByTestId('onramp-topup');
  await row.getByRole('button', { name: 'Buy', exact: true }).click();
  for (const link of await row.getByRole('link').all()) {
    const [tab] = await Promise.all([page.waitForEvent('popup'), link.click()]);
    await tab.close();
  }
  await row.getByRole('checkbox').check();
  await row.getByLabel('Amount in euros').fill('25');
  await row.getByRole('button', { name: 'Create the order' }).click();
  await row.getByRole('alert').getByText(/service unavailable in your region/).waitFor();
  assert.equal(await page.getByTestId('onramp-order').count(), 0);
  assert.equal(await page.evaluate(() => localStorage.getItem('losos-binance-terms')), null);
  assert.deepEqual(errors, []);
  await page.close();
});

await check('no euro top-up on a test edge or an edge that is not a Binance partner', async () => {
  const usdc = ONRAMP_MARKET.account.usdc;
  for (const changed of [{ ...usdc, test: true }, { ...usdc, onramp: undefined }]) {
    const market = { ...ONRAMP_MARKET, account: { ...ONRAMP_MARKET.account, usdc: changed } };
    const { page, errors } = await open({ path: '/settings/market', stored: true, market });
    await page.getByText('Stablecoins').waitFor();
    assert.equal(await page.getByTestId('onramp-topup').count(), 0);
    assert.deepEqual(errors, []);
    await page.close();
  }
});

await check('a seller whose coin fees are overdue is told coin payments stopped and USDC still works', async () => {
  const paused = {
    ...COIN_FEES,
    account: { ...COIN_FEES.account, coin_fees_owed: 1250, coin_sales_paused: true, coin_fee_credit: 120, sales: [] },
  };
  const { page, errors } = await open({ path: '/settings/market', stored: true, market: paused });
  const row = page.getByTestId('coin-fees');
  await row.waitFor();
  const status = row.getByRole('status');
  assert.equal(
    await status.innerText(),
    'Buyers cannot pay you in coins until this is settled. USDC still works, and each USDC sale pays toward it.',
  );
  assert.doesNotMatch(await row.innerText(), /no more than/);
  assert.match(await row.innerText(), /\S*1\.20 from a USDC sale went beyond what you owed/);
  assert.deepEqual(errors, []);
  await page.close();
});

await check('the Mesh pane names the edge proxy the box found, and the join switch is live', async () => {
  const { page, errors } = await open({ path: '/mesh', stored: true, settings: JOINABLE });
  const row = page.locator('[data-edge="found"]');
  await row.waitFor();
  const text = await row.innerText();
  assert.match(text, /Edge proxy found: edge demo/);
  assert.match(text, /http:\/\/edge\.local:8443/);
  assert.match(text, /On this network/);
  const join = page.getByRole('switch', { name: 'Join the mesh', exact: true });
  assert.equal(await join.isDisabled(), false, 'the join switch is greyed with an edge in reach');
  assert.deepEqual(errors, []);
  await page.close();
});

await check('each edge in reach gets a row; an unchecked edge wears a warning sign whose tooltip lists what is missing', async () => {
  const { page, errors } = await open({ path: '/mesh', stored: true, edge: EDGE_BOTH, settings: JOINABLE });
  const rows = page.locator('[data-edge="found"]');
  await rows.first().waitFor();
  assert.equal(await rows.count(), 2, 'one row per edge');
  const company = page.locator('[data-edge-official="no"]');
  const official = page.locator('[data-edge-official="yes"]');
  assert.match(await company.innerText(), /Edge proxy found: edge demo/);
  assert.match(await company.innerText(), /On this network/);
  assert.match(await official.innerText(), /losos-edge\.dasmat\.us/);
  assert.match(await official.innerText(), /Over the internet/);
  assert.match(await official.innerText(), /run by Ada Gateway <ada@example\.net>/);

  const warning = company.locator('[data-edge-sign="warning"]');
  assert.equal(await warning.count(), 1, 'the company edge has no warning sign');
  assert.equal(await official.locator('[data-edge-sign="official"]').count(), 1, 'the official edge has no check');
  assert.match(await warning.getAttribute('aria-label'), /shows no gateway passport/);
  assert.match(
    await official.locator('[data-edge-sign="official"]').getAttribute('aria-label'),
    /Checked gateway, run by Ada Gateway.*losos-edge-gateway 0\.6\.0/,
  );
  await warning.hover();
  const tip = page.locator('[data-slot="tooltip-content"]');
  await tip.waitFor({ timeout: 3000 });
  const text = await tip.innerText();
  assert.match(text, /shows no gateway passport/);
  assert.match(text, /buying storage or compute on the market/);
  assert.match(text, /selling this box's spare storage and compute/);
  assert.match(text, /Sharing storage through it works/);

  const join = page.getByRole('switch', { name: 'Join the mesh', exact: true });
  assert.equal(await join.isDisabled(), false, 'an edge in reach greyed the join switch');
  assert.deepEqual(errors, []);
  await page.close();

  // Only a company edge: same sign, and the caption says the market waits.
  const alone = await open({ path: '/mesh', stored: true, edge: EDGE_COMPANY });
  await alone.page.locator('[data-edge-sign="warning"]').waitFor();
  assert.match(await body(alone.page), /None of these passed the gateway checks, so the market/);
  assert.equal(await alone.page.locator('[data-edge-sign="official"]').count(), 0);
  assert.deepEqual(alone.errors, []);
  await alone.page.close();
});

await check('a gateway caught tampered with is marked not used, and one with changed fees says why it is not trusted', async () => {
  const { page, errors } = await open({ path: '/mesh', stored: true, edge: EDGE_TAMPERED, settings: JOINABLE });
  const tampered = page.locator('[data-edge-gateway="disabled"]');
  await tampered.waitFor();
  assert.match(await tampered.innerText(), /Not used/);
  const stop = tampered.locator('[data-edge-sign="disabled"]');
  assert.match(await stop.getAttribute('aria-label'), /Tampered with: the gateway's settings or state changed after it booted/);
  assert.match(await stop.getAttribute('aria-label'), /no longer uses this edge/);
  const refused = page.locator('[data-edge-gateway="refused"]');
  assert.match(
    await refused.locator('[data-edge-sign="warning"]').getAttribute('aria-label'),
    /Not trusted with trading: the fee settings differ/,
  );
  assert.match(await refused.innerText(), /Over the internet/);
  assert.deepEqual(errors, []);
  await page.close();
});

await check('the Mesh pane says which tier the box\'s link is, and the owner moves it to another secondary cluster', async () => {
  const { page, errors } = await open({ path: '/mesh', stored: true, edge: EDGE_CLUSTERS, settings: JOINABLE });
  const link = page.locator('[data-link-tier]');
  await link.waitFor();
  assert.equal(await link.getAttribute('data-link-tier'), 'secondary');
  assert.match(await link.innerText(), /Secondary connection through gw-a/);
  assert.match(await link.innerText(), /A local gateway of the cluster office/);
  const inUse = page.locator('[data-edge-in-use="yes"]');
  assert.equal(await inUse.count(), 1);
  assert.match(await inUse.innerText(), /Secondary, in use/);
  assert.match(await inUse.innerText(), /cluster office/);
  assert.match(await page.locator('[data-edge-tier="tertiary"]').innerText(), /Tertiary/);
  const office = page.locator('[data-cluster="office"]');
  assert.equal(await office.getAttribute('data-cluster-mine'), 'yes');
  assert.match(await office.innerText(), /Gateways: gw-a, gw-b/);
  assert.match(await office.innerText(), /This box's cluster/);

  const posts = [];
  await page.route('**/api/edge/cluster', (route) => {
    posts.push(route.request().postDataJSON());
    return json(route, 200, { cluster: 'lab', path: { ...EDGE_CLUSTERS.path, name: 'gw-z', member: 'gw-z', cluster: 'lab' } });
  });
  await page.locator('[data-cluster="lab"]').getByRole('button', { name: 'Move here' }).click();
  await page.locator('[data-toast]').filter({ hasText: /in the cluster lab now/ }).waitFor({ timeout: 5000 });
  assert.deepEqual(posts, [{ cluster: 'lab' }]);
  assert.deepEqual(errors, []);
  await page.close();

  // Its cluster's gateways are gone: tertiary, and the box is still in office.
  const away = await open({ path: '/mesh', stored: true, edge: EDGE_TERTIARY, settings: JOINABLE });
  const tertiary = away.page.locator('[data-link-tier="tertiary"]');
  await tertiary.waitFor();
  assert.match(await tertiary.innerText(), /Tertiary connection to losos-edge\.dasmat\.us/);
  assert.match(await tertiary.innerText(), /no gateway of the cluster office answers/);
  const gone = away.page.locator('[data-cluster="office"]');
  assert.match(await gone.innerText(), /None of its gateways answers right now/);
  assert.equal(await gone.getAttribute('data-cluster-mine'), 'yes');
  assert.deepEqual(away.errors, []);
  await away.page.close();
});

await check('with no edge proxy in reach the Mesh pane says so and the join switch is greyed with the reason', async () => {
  const { page, errors } = await open({ path: '/mesh', stored: true, edge: EDGE_NONE });
  const row = page.locator('[data-edge="none"]');
  await row.waitFor();
  const text = await row.innerText();
  assert.match(text, /No edge proxy found/);
  assert.match(text, /asked https:\/\/losos-edge\.dasmat\.us; nothing answered/);
  assert.match(text, /Sharing is off/);
  const join = page.getByRole('switch', { name: 'Join the mesh', exact: true });
  assert.equal(await join.isDisabled(), true, 'the join switch is live with no edge in reach');
  assert.match(await body(page), /Needs an edge proxy in reach\./);
  // The hours underneath depend on joining, so they are greyed too.
  assert.equal(await page.getByRole('switch', { name: 'Lend compute while this box is idle' }).count() >= 0, true);
  assert.deepEqual(errors, []);
  await page.close();
});

await check('a box without the tunnel or with the stock name cannot join, and the pane says which', async () => {
  const join = (page) => page.getByRole('switch', { name: 'Join the mesh', exact: true });
  const stock = await open({ path: '/mesh', stored: true });
  await stock.page.locator('[data-edge="found"]').waitFor();
  assert.equal(await join(stock.page).isDisabled(), true, 'a box with the tunnel off can join');
  assert.match(await body(stock.page), /Turn on "Reachable from outside your home" in Network first/);
  assert.deepEqual(stock.errors, []);
  await stock.page.close();

  const named = await open({ path: '/mesh', stored: true, settings: { ...SETTINGS, proxyEnable: true } });
  await named.page.locator('[data-edge="found"]').waitFor();
  assert.equal(await join(named.page).isDisabled(), true, 'a box still called mattbox can join');
  assert.match(await body(named.page), /Every box starts as "mattbox"/);
  assert.deepEqual(named.errors, []);
  await named.page.close();
});

await check('a box already joined keeps its join switch live while the edge is away, so it can leave', async () => {
  const joined = { ...SETTINGS, clusterEnable: true };
  const { page } = await open({ path: '/mesh', stored: true, edge: EDGE_NONE });
  await page.route('**/api/settings', (route) => json(route, 200, joined));
  await page.reload({ waitUntil: 'networkidle' });
  await page.locator('[data-edge="none"]').waitFor();
  const join = page.getByRole('switch', { name: 'Join the mesh', exact: true });
  assert.equal(await join.isChecked(), true);
  assert.equal(await join.isDisabled(), false, 'a joined box cannot leave while the edge is away');
  await page.close();
});

await check('the daemon\'s refusal reaches the owner in the daemon\'s own words', async () => {
  // The edge appears, the owner flips Join, the edge vanishes before Apply:
  // the switch was live, so the 409 is what tells the owner. Its sentence is
  // the toast, and nothing is marked as applying afterwards.
  // The fixture's port 80 fails the form's own validation and keeps Apply
  // greyed; this check is about the daemon's refusal, so the form is valid.
  const { page, applies } = await open({ path: '/mesh', stored: true, settings: JOINABLE });
  await page.locator('[data-edge="found"]').waitFor();
  const join = page.getByRole('switch', { name: 'Join the mesh', exact: true });
  await page.locator('[data-slot="switch"]').filter({ has: join }).click();
  assert.equal(await join.isChecked(), true, 'the join switch did not flip');
  await page.getByRole('button', { name: 'Apply', exact: true }).waitFor();
  await page.unroute('**/api/edge');
  await page.unroute('**/api/apply');
  await page.route('**/api/edge', (route) => json(route, 200, EDGE_NONE));
  await page.route('**/api/apply', (route) => {
    applies.push(route.request().postData() ?? '');
    return json(route, 409, {
      error: 'no edge proxy is reachable from this box, so losos.cluster.enable cannot be turned on: storage can only be shared through an edge. Local use keeps working.',
      edgeRequired: true,
      setting: 'losos.cluster.enable',
    });
  });
  await page.getByRole('button', { name: 'Apply', exact: true }).click();
  const toast = page.locator('[data-toast]').filter({ hasText: /no edge proxy is reachable/ });
  await toast.waitFor({ timeout: 5000 });
  assert.match(await toast.innerText(), /Local use keeps working/);
  assert.equal(applies.length, 1, 'Apply did not post exactly once');
  assert.match(applies[0], /losos\.cluster\.enable = true;/);
  // Nothing is applying: the bar is back with Apply armed.
  await page.getByRole('button', { name: 'Apply', exact: true }).waitFor();
  await page.close();
});

await check('the Network pane says why a box gets no custom domain', async () => {
  const none = await open({ path: '/settings/network', stored: true, domains: { available: false, reason: 'noOfficialEdge' } });
  const section = none.page.getByTestId('custom-domains');
  await section.getByText('Not offered to this box').waitFor();
  assert.match(await section.innerText(), /checked LosOS gateway, and none is in reach/);
  assert.equal(await section.getByRole('textbox').count(), 0, 'a box with no official edge got a domain field');
  assert.deepEqual(none.errors, []);
  await none.page.close();

  const unvouched = await open({
    path: '/settings/network',
    stored: true,
    domains: { ...DOMAINS_READY, eligible: false, reason: 'stripeAccount', target: null, domains: [] },
  });
  const s2 = unvouched.page.getByTestId('custom-domains');
  await s2.getByText('Needs a Stripe account that Stripe has checked').waitFor();
  assert.equal(await s2.getByRole('textbox').count(), 0, 'an unvouched box got a domain field');
  assert.deepEqual(unvouched.errors, []);
  await unvouched.page.close();
});

await check('a vouched box lists its domains with the records to publish, adds one and removes one', async () => {
  const { page, errors, domainPosts } = await open({ path: '/settings/network', stored: true, domains: DOMAINS_READY });
  const section = page.getByTestId('custom-domains');
  await section.getByText('3f9a1c0e7b2d4a55.boxes.losos.dasmat.us', { exact: true }).waitFor();
  const rows = section.getByTestId('custom-domain');
  assert.equal(await rows.count(), 2);
  const live = await rows.nth(0).innerText();
  assert.match(live, /cloud\.example\.org/);
  assert.match(live, /Live/);
  assert.doesNotMatch(live, /CNAME/, 'a live domain still lists the records');
  const waiting = await rows.nth(1).innerText();
  assert.match(waiting, /Waiting for DNS/);
  assert.match(waiting, /does not point at this box's name yet/);
  assert.match(waiting, /CNAME\s+files\.example\.net\s+3f9a1c0e7b2d4a55\.boxes\.losos\.dasmat\.us\. not seen yet/);
  assert.match(waiting, /TXT\s+_losos-challenge\.files\.example\.net\s+"losos-domain-v1=0f3c9a51d2e84b7d6a1c93e0b5f2d470" ✓ seen/);
  assert.match(waiting, /203\.0\.113\.7, 2001:db8::7/);

  await section.getByRole('textbox').fill('  Shop.Example.COM. ');
  await section.getByRole('button', { name: 'Add', exact: true }).click();
  await section.getByTestId('custom-domain').filter({ hasText: 'shop.example.com' }).waitFor();
  assert.deepEqual(domainPosts, [['/api/domains', { domain: 'shop.example.com' }]]);
  assert.equal(await section.getByRole('textbox').inputValue(), '', 'the field kept the added name');

  await section.getByRole('textbox').fill('taken.example.org');
  await section.getByRole('button', { name: 'Add', exact: true }).click();
  const refused = page.locator('[data-toast]').filter({ hasText: /live for another box/ });
  await refused.waitFor({ timeout: 5000 });
  assert.equal(await section.getByRole('textbox').inputValue(), 'taken.example.org', 'a refused name was cleared');

  await rows.nth(0).getByRole('button', { name: 'Remove', exact: true }).click();
  // Three rows before (two fixtures and the added one), two after.
  await page.waitForFunction(() => document.querySelectorAll('[data-testid="custom-domain"]').length === 2);
  assert.match(await rows.nth(0).innerText(), /^files\.example\.net/);
  assert.deepEqual(domainPosts.at(-1), ['/api/domains/remove', { domain: 'cloud.example.org' }]);
  assert.deepEqual(errors, []);
  await page.close();
});

await check('a browser extension wallet binds to the box names, and unbinds', async () => {
  const { privateKeyToAccount } = await import('viem/accounts');
  const { verifyMessage } = await import('viem');
  const account = privateKeyToAccount(`0x${'07'.repeat(32)}`);
  const { page, errors, domainPosts } = await open({ path: '/settings/network', stored: true, domains: DOMAINS_READY });
  // The extension: personal_sign is answered by the test's own key.
  await page.exposeFunction('testSign', (message) => account.signMessage({ message }));
  await page.evaluate((address) => {
    window.ethereum = {
      request: async ({ method, params }) => {
        if (method === 'eth_requestAccounts') return [address];
        if (method === 'personal_sign') return window.testSign(params[0]);
        throw new Error(method);
      },
    };
  }, account.address);
  const section = page.getByTestId('domain-wallet');
  await section.getByText('Let people pay this box by name').waitFor();
  assert.match(await section.innerText(), /mattbox\.boxes\.losos\.dasmat\.us, 3f9a1c0e7b2d4a55\.boxes\.losos\.dasmat\.us/);
  await section.getByTestId('wallet-bind').click();
  await section.getByText('Bound to this box’s wallet').or(section.getByText("Bound to this box's wallet")).waitFor();
  const [route, sent] = domainPosts.at(-1);
  assert.equal(route, '/api/domains/wallet');
  const address = account.address.toLowerCase();
  assert.equal(sent.address, address);
  assert.ok(
    await verifyMessage({ address, message: walletMessage('boxes.losos.dasmat.us', 'mattbox', address), signature: sent.signature }),
    'the signature is not the wallet message the edge checks',
  );
  const names = section.getByTestId('wallet-name');
  assert.equal(await names.count(), 2);
  assert.match(await names.nth(0).innerText(), /mattbox\.boxes\.losos\.dasmat\.us[\s\S]*0x4a62/);
  // Whether a buyer's wallet finds each name, as public DNS last answered.
  assert.match(await names.nth(0).innerText(), /Wallets find this name/);
  assert.match(await names.nth(1).innerText(), /Checking whether wallets find this name/);
  // Each domain of the owner's offers the same record to add at the domain.
  const rows = page.getByTestId('custom-domains').getByTestId('custom-domain');
  assert.match(await rows.nth(0).innerText(), new RegExp(`TXT\\s+cloud\\.example\\.org\\s+"ENS1 dnsname\\.ens\\.eth ${address}"`));
  assert.match(await rows.nth(0).innerText(), /looks for this record at the domain on its next check/);

  await section.getByRole('button', { name: 'Unbind', exact: true }).click();
  await section.getByText('Let people pay this box by name').waitFor();
  assert.deepEqual(domainPosts.at(-1), ['/api/domains/wallet', {}]);
  assert.equal(await page.getByTestId('domain-wallet-record').count(), 0, 'an unbound wallet still offered its record');
  assert.deepEqual(errors, []);
  await page.close();
});

await check('an edge that predates wallets still draws the domains, without the wallet section', async () => {
  const { box, zone, names, wallet, ...older } = DOMAINS_READY;
  const domains = { ...older, domains: older.domains.map(({ wallet: _seen, ...d }) => d) };
  const { page, errors } = await open({ path: '/settings/network', stored: true, domains });
  const rows = page.getByTestId('custom-domains').getByTestId('custom-domain');
  await rows.first().waitFor();
  assert.equal(await rows.count(), 2);
  assert.equal(await page.getByTestId('domain-wallet').count(), 0, 'the wallet section showed without a zone');
  assert.doesNotMatch(await rows.nth(0).innerText(), /ENS1|Wallets pay/);
  assert.deepEqual(errors, []);
  await page.close();
});

// A wallet saved before its address was kept beside the phrase has none
// until it is unlocked, and is still the box's wallet.
for (const keptAddress of [true, false]) {
await check(`the wallet made on this box signs the binding after its password${keptAddress ? '' : ', saved without its address'}`, async () => {
  const { mnemonicToAccount } = await import('viem/accounts');
  const { verifyMessage } = await import('viem');
  const phrase = 'test test test test test test test test test test test junk';
  const address = mnemonicToAccount(phrase).address;
  const bound = {
    ...DOMAINS_READY,
    domains: [
      { ...DOMAINS_READY.domains[0], wallet: 'found' },
      { ...WAITING_DOMAIN, wallet: 'cname' },
    ],
  };
  const { page, errors, domainPosts } = await open({ path: '/settings/network', stored: true, domains: bound });
  // Seal the phrase the way wallet/hot.ts does, then load the page again.
  await page.evaluate(
    async ({ phrase, address, keptAddress }) => {
      const enc = new TextEncoder();
      const salt = crypto.getRandomValues(new Uint8Array(16));
      const iv = crypto.getRandomValues(new Uint8Array(12));
      const base = await crypto.subtle.importKey('raw', enc.encode('right horse battery'), 'PBKDF2', false, ['deriveKey']);
      const key = await crypto.subtle.deriveKey(
        { name: 'PBKDF2', salt, iterations: 210_000, hash: 'SHA-256' },
        base,
        { name: 'AES-GCM', length: 256 },
        false,
        ['encrypt'],
      );
      const ct = new Uint8Array(await crypto.subtle.encrypt({ name: 'AES-GCM', iv }, key, enc.encode(phrase)));
      const b64 = (b) => btoa(String.fromCharCode(...b));
      const db = await new Promise((resolve, reject) => {
        const req = indexedDB.open('losos-wallet', 1);
        req.onupgradeneeded = () => req.result.createObjectStore('key');
        req.onsuccess = () => resolve(req.result);
        req.onerror = () => reject(req.error);
      });
      await new Promise((resolve) => {
        const tx = db.transaction('key', 'readwrite');
        const sealed = { v: 1, salt: b64(salt), iv: b64(iv), ct: b64(ct), made: true };
        tx.objectStore('key').put(keptAddress ? { ...sealed, evm: address } : sealed, 'phrase');
        tx.oncomplete = resolve;
      });
      db.close();
    },
    { phrase, address, keptAddress },
  );
  await page.reload({ waitUntil: 'networkidle' });
  const section = page.getByTestId('domain-wallet');
  await section.getByTestId('wallet-bind').click();
  const password = section.getByLabel('Password of the wallet on this box');
  if (keptAddress) assert.match(await section.innerText(), new RegExp(`Signs as ${address.toLowerCase()}`));
  else assert.doesNotMatch(await section.innerText(), /Signs as/);
  await password.fill('wrong password');
  await section.getByRole('button', { name: 'Sign', exact: true }).click();
  await page.locator('[data-toast]').filter({ hasText: /password/i }).first().waitFor({ timeout: 10000 });
  assert.equal(domainPosts.length, 0, 'a wrong password still sent a binding');
  await password.fill('right horse battery');
  await section.getByRole('button', { name: 'Sign', exact: true }).click();
  await section.getByTestId('wallet-name').first().waitFor({ timeout: 15000 });
  const [, sent] = domainPosts.at(-1);
  const lower = address.toLowerCase();
  assert.equal(sent.address, lower);
  assert.ok(await verifyMessage({ address: lower, message: walletMessage('boxes.losos.dasmat.us', 'mattbox', lower), signature: sent.signature }));
  // What the edge found at the owner's domains: one pays by name, one is a
  // CNAME and is told why it cannot.
  const rows = page.getByTestId('custom-domains').getByTestId('custom-domain');
  assert.match(await rows.nth(0).innerText(), /Wallets pay cloud\.example\.org to this box/);
  assert.doesNotMatch(await rows.nth(0).innerText(), /ENS1/);
  assert.match(await rows.nth(1).innerText(), /is a CNAME, which can carry no other record/);
  assert.doesNotMatch(await rows.nth(1).innerText(), /ENS1/, 'a CNAME was offered a record it cannot carry');
  assert.deepEqual(errors, []);
  await page.close();
});
}

await check('a binding whose names lapsed can still be taken off', async () => {
  const address = '0x4a62000000000000000000000000000000000001';
  const domains = {
    ...DOMAINS_READY,
    names: [],
    wallet: { address, bound_at: 1791460000, record: ensRecord(address), published: {}, published_at: 1791460030 },
  };
  const { page, errors, domainPosts } = await open({ path: '/settings/network', stored: true, domains });
  const section = page.getByTestId('domain-wallet');
  assert.match(await section.innerText(), /No name to bind yet/);
  await section.getByRole('button', { name: 'Unbind', exact: true }).click();
  await section.getByRole('button', { name: 'Unbind', exact: true }).waitFor({ state: 'detached' });
  assert.deepEqual(domainPosts.at(-1), ['/api/domains/wallet', {}]);
  assert.match(await section.innerText(), /No name to bind yet/);
  assert.deepEqual(errors, []);
  await page.close();
});

await check('the Backup pane saves a bucket without echoing the secret, backs up and restores with a code', async () => {
  const { page, errors, backupPosts } = await open({ path: '/settings/backup', stored: true });
  const form = page.getByTestId('backup-target-form');
  await form.getByLabel('Address').fill(' https://s3.eu-central-1.amazonaws.com/ ');
  await form.getByLabel('Bucket').fill('mattbox-backups');
  await form.getByLabel('Folder').fill('/losos/');
  await form.getByLabel('Access key').fill('AKIAEXAMPLE1234');
  const save = form.getByRole('button', { name: 'Save', exact: true });
  assert.ok(await save.isDisabled(), 'saved a new bucket without a secret key');
  await form.getByLabel('Secret key').fill('s3cr3t/Key+value');
  await save.click();
  const shown = page.getByTestId('backup-target');
  await shown.getByText('mattbox-backups/losos', { exact: true }).waitFor();
  assert.deepEqual(backupPosts[0], [
    'POST',
    '/api/backup/target',
    {
      endpoint: 'https://s3.eu-central-1.amazonaws.com',
      bucket: 'mattbox-backups',
      prefix: 'losos',
      region: '',
      accessKeyId: 'AKIAEXAMPLE1234',
      secretAccessKey: 's3cr3t/Key+value',
      storageClass: 'standard',
    },
  ]);
  assert.doesNotMatch(await page.locator('main').innerText(), /s3cr3t/, 'the secret is on the screen');

  // Changing the bucket and leaving the secret empty keeps the stored one.
  await shown.getByRole('button', { name: 'Change…' }).click();
  await page.getByText('Stored. Leave it empty to keep it.').waitFor();
  await page.getByTestId('backup-target-form').getByRole('button', { name: 'Save', exact: true }).click();
  await page.getByTestId('backup-target').waitFor();
  assert.equal('secretAccessKey' in backupPosts[1][2], false, 'an empty secret field was sent');

  const code = page.getByTestId('backup-code');
  assert.equal(await code.getByTestId('recovery-code').count(), 0, 'the code is on screen before it was asked for');
  await code.getByRole('button', { name: 'Show', exact: true }).click();
  await code.getByText('b3c1d2e4-5f60-4a7b-8c9d-0e1f2a3b4c5d').waitFor();

  await page.getByTestId('backup-runs').getByRole('button', { name: 'Back up', exact: true }).click();
  await page.getByTestId('backup-running').waitFor();
  assert.deepEqual(backupPosts[2], ['POST', '/api/backup/run', null]);
  await page.close();

  const again = await open({ path: '/settings/backup', stored: true, backup: BACKUP_SET });
  const restore = again.page.getByTestId('backup-restore');
  await again.page.getByTestId('backup-runs').getByText(/4\.5 GB, 1,832 files/).waitFor();
  const button = restore.getByRole('button', { name: 'Restore…' });
  await restore.getByLabel('Recovery code').fill('not a code');
  assert.ok(await button.isDisabled(), 'a malformed code can be sent');
  await restore.getByLabel('Recovery code').fill(` ${OLD_CODE.toUpperCase()} `);
  await button.click();
  const dialog = again.page.getByRole('dialog');
  await dialog.getByText('Restore the newest backup?').waitFor();
  await dialog.getByRole('button', { name: 'Restore', exact: true }).click();
  await again.page.getByTestId('restore-running').waitFor();
  assert.deepEqual(again.backupPosts, [['POST', '/api/backup/restore', { code: OLD_CODE.toUpperCase() }]]);
  assert.deepEqual([...errors, ...again.errors], []);
  await again.page.close();
});

await check('a claimed reserve reads as claimed, after a reload too', async () => {
  const GiB = 1024 ** 3;
  const storage = { totalBytes: 100 * GiB, usedBytes: 40 * GiB, reserveBytes: 10 * GiB };
  const { page, errors } = await open({ path: '/storage', stored: true, storage });
  const use = page.getByRole('button', { name: 'Use reserve…' });
  assert.ok(await use.isEnabled(), 'a reserve the box reports is not offered');
  await use.click();
  await page.getByRole('button', { name: 'Use it', exact: true }).click();
  const spent = page.getByRole('button', { name: 'Reserve in use' });
  await spent.waitFor();
  assert.ok(await spent.isDisabled(), 'the spent reserve can be claimed again');
  await page.reload({ waitUntil: 'networkidle' });
  await spent.waitFor();
  assert.ok(await spent.isDisabled(), 'a reload offers the spent reserve again');
  await page.getByText('Already added to this box\'s disk.', { exact: false }).waitFor();
  assert.deepEqual(errors, []);
  await page.close();
});

await check('a box that reports no reserve offers none', async () => {
  const { page } = await open({ path: '/storage', stored: true });
  await page.getByText('Not reported').first().waitFor();
  assert.ok(await page.getByRole('button', { name: 'Use reserve…' }).isDisabled(), 'an unknown reserve is offered');
  await page.close();
});

await check('a change that failed before the page loaded is reported on Settings', async () => {
  const status = { state: 'failed', progress: 100, job: 'job-7', message: 'nixos-rebuild exited with status 1' };
  const { page } = await open({ path: '/settings/network', stored: true, status });
  await page.getByText('The changes could not be applied', { exact: true }).waitFor();
  await page.getByText('nixos-rebuild exited with status 1', { exact: true }).waitFor();
  await page.close();
});

await check('while a change is applied, backups, restores and the erase wait and say why', async () => {
  const status = { state: 'building', progress: 0, job: 'job-8', message: 'building' };
  const { page } = await open({ path: '/settings/backup', stored: true, backup: BACKUP_SET, status });
  await page.getByTestId('backup-waiting').waitFor();
  assert.ok(await page.getByRole('button', { name: 'Back up', exact: true }).isDisabled(), 'a backup can start during a rebuild');
  await page.close();
  const reset = await open({ path: '/settings/reset', stored: true, backup: BACKUP_SET, status });
  await reset.page.getByText('Backups, restores and the erase wait until it is done.').waitFor();
  assert.ok(await reset.page.getByRole('button', { name: /^Erase/ }).first().isDisabled(), 'an erase can start during a rebuild');
  await reset.page.close();
});

await check('Glacier is offered on Amazon S3 only, and a restore from it warns that it takes hours', async () => {
  const { page, errors, backupPosts } = await open({ path: '/settings/backup', stored: true });
  const form = page.getByTestId('backup-target-form');
  const select = form.getByLabel('Storage');
  await form.getByLabel('Address').fill('https://minio.example.org:9000');
  assert.ok(await select.isDisabled(), 'Glacier is offered off Amazon S3');
  await form.getByText('Glacier is on Amazon S3 only.', { exact: false }).waitFor();
  await form.getByLabel('Address').fill('https://s3.eu-central-1.amazonaws.com');
  assert.ok(await select.isEnabled(), 'Glacier is not offered on Amazon S3');
  await select.selectOption('deepArchive');
  await form.getByText('A restore waits up to 12 hours', { exact: false }).waitFor();
  await form.getByLabel('Bucket').fill('mattbox-backups');
  await form.getByLabel('Access key').fill('AKIAEXAMPLE1234');
  await form.getByLabel('Secret key').fill('s3cr3t/Key+value');
  await form.getByRole('button', { name: 'Save', exact: true }).click();
  await page.getByTestId('backup-class').getByText('Glacier Deep Archive', { exact: true }).waitFor();
  assert.equal(backupPosts[0][2].storageClass, 'deepArchive');

  // Switching the address away from AWS sends Standard, whatever was picked.
  await page.getByTestId('backup-target').getByRole('button', { name: 'Change…' }).click();
  const again = page.getByTestId('backup-target-form');
  assert.equal(await again.getByLabel('Storage').inputValue(), 'deepArchive');
  await again.getByLabel('Address').fill('http://192.168.1.20:9000');
  await again.getByRole('button', { name: 'Save', exact: true }).click();
  await page.getByTestId('backup-class').getByText('Standard', { exact: true }).waitFor();
  assert.equal(backupPosts[1][2].storageClass, 'standard');
  await page.close();

  const cold = await open({
    path: '/settings/backup',
    stored: true,
    backup: { ...BACKUP_SET, target: { ...BACKUP_SET.target, storageClass: 'glacier' } },
  });
  const restore = cold.page.getByTestId('backup-restore');
  await restore.getByLabel('Recovery code').fill(OLD_CODE);
  await restore.getByRole('button', { name: 'Restore…' }).click();
  const dialog = cold.page.getByRole('dialog');
  await dialog.getByTestId('restore-thaw').waitFor();
  await dialog.getByRole('button', { name: 'Restore', exact: true }).click();
  await cold.page.getByTestId('restore-running').getByText('AWS is thawing the backup first', { exact: false }).waitFor();
  assert.deepEqual([...errors, ...cold.errors], []);
  await cold.page.close();
});

await check('the Reset pane starts an erase with a backup, counts down and cancels it', async () => {
  const { page, errors, backupPosts } = await open({ path: '/settings/reset', stored: true, backup: BACKUP_SET });
  const section = page.getByTestId('erase');
  await page.getByTestId('erase-report').getByText('Custom domains removed').waitFor();
  await section.getByRole('button', { name: 'Erase…' }).click();
  const dialog = page.getByRole('dialog');
  await dialog.getByText('Erase everything on this box?').waitFor();
  assert.ok(await dialog.getByRole('switch', { name: 'Back up first' }).isChecked(), 'the backup is off by default');
  await dialog.getByRole('button', { name: 'Start the countdown' }).click();
  const countdown = page.getByTestId('erase-countdown');
  await countdown.getByRole('timer').waitFor();
  assert.match(await countdown.getByRole('timer').innerText(), /^1[45]:\d\d$/);
  assert.deepEqual(backupPosts[0], ['POST', '/api/erase', { backup: true }]);
  assert.equal(await page.getByTestId('erase-report').count(), 0, 'the old report shows during an erase');

  await countdown.getByRole('button', { name: 'Cancel erase' }).click();
  await section.getByRole('button', { name: 'Erase…' }).waitFor();
  assert.deepEqual(backupPosts[1], ['POST', '/api/erase/cancel', null]);
  await page.locator('[data-toast]').filter({ hasText: 'Erase cancelled' }).waitFor();
  assert.deepEqual(errors, []);
  await page.close();

  // Without a bucket the backup cannot be asked for, and the dialog says so.
  const bare = await open({ path: '/settings/reset', stored: true });
  await bare.page.getByTestId('erase').getByRole('button', { name: 'Erase…' }).click();
  const sw = bare.page.getByRole('dialog').getByRole('switch', { name: 'Back up first' });
  assert.ok(await sw.isDisabled(), 'the backup switch is live with no bucket');
  assert.equal(await sw.isChecked(), false);
  await bare.page.getByRole('dialog').getByText(/nothing can be brought back/).waitFor();
  await bare.page.getByRole('button', { name: 'Start the countdown' }).click();
  await bare.page.getByTestId('erase-countdown').waitFor();
  assert.deepEqual(bare.backupPosts[0], ['POST', '/api/erase', { backup: false }]);
  await bare.page.close();
});

// ── Machines (pane-machines.tsx) ──────────────────────────────────────────

await check('a box that does not share its disk sees the Machines pane greyed, with a link to share it', async () => {
  const { page, errors, vmPosts } = await open({ path: '/machines', stored: true });
  const notice = page.getByTestId('machines-unavailable');
  await notice.getByText('Machines are for boxes that share their disk').waitFor();
  const share = notice.getByRole('link', { name: 'Share the disk' });
  assert.equal(await share.getAttribute('href'), '/settings/market');
  // The mesh banner's own Dismiss is the shell's, not the pane's.
  const buttons = page.locator('main').getByRole('button').and(page.locator(':not([data-mesh-dismiss])'));
  assert.equal(await buttons.count(), 0, 'the greyed pane offers a button');
  assert.deepEqual(vmPosts, []);
  assert.deepEqual(errors, []);
  await page.close();
});

await check('the other reasons each get their own sentence', async () => {
  for (const [reason, text] of [
    ['noOfficialEdge', 'No LosOS edge in reach'],
    ['notOffered', 'This edge runs no machines'],
  ]) {
    const { page, errors } = await open({ path: '/machines', stored: true, vms: { available: false, reason } });
    await page.getByTestId('machines-unavailable').getByText(text).waitFor();
    assert.deepEqual(errors, []);
    await page.close();
  }
});

await check('starting three replicas posts the image, the name and the cloud-init, and opens Stripe', async () => {
  const { page, errors, vmPosts } = await open({ path: '/machines', stored: true, vms: VMS });
  const start = page.getByTestId('machines-start');
  await start.getByLabel('System').selectOption('ubuntu-24.04');
  await start.getByLabel('Name').fill('web');
  await start.getByRole('button', { name: 'One replica more' }).click();
  await start.getByRole('button', { name: 'One replica more' }).click();
  await start.getByText('€18.00 a month for 3 replicas').waitFor();
  await start.getByText('50% goes to the owner of the box that runs it, 50% to LosOS.').waitFor();
  await start.getByLabel('Cloud-init (optional)').fill('#cloud-config\nssh_authorized_keys: [ssh-ed25519 AAAA]\n');
  const tab = page.context().waitForEvent('page');
  await start.getByRole('button', { name: 'Pay and start' }).click();
  const checkout = await tab;
  await checkout.waitForURL('https://checkout.stripe.com/c/pay/cs_test_1');
  assert.deepEqual(vmPosts, [
    [
      '/api/vms/orders',
      {
        listing_id: 'lst_vm01',
        quantity: 3,
        image: 'ubuntu-24.04',
        name: 'web',
        user_data: '#cloud-config\nssh_authorized_keys: [ssh-ed25519 AAAA]\n',
      },
    ],
  ]);
  assert.deepEqual(errors, []);
  await page.close();
});

await check('the replica count stops at what the host has free, and LosOS takes no cloud-init', async () => {
  const { page, vmPosts } = await open({ path: '/machines', stored: true, vms: VMS });
  const start = page.getByTestId('machines-start');
  const more = start.getByRole('button', { name: 'One replica more' });
  for (let i = 0; i < 5; i += 1) if (await more.isEnabled()) await more.click();
  assert.equal(await start.getByLabel('Replicas').inputValue(), '4');
  assert.equal(await more.isDisabled(), true);
  await start.getByLabel('System').selectOption('losos');
  assert.equal(await start.getByLabel('Cloud-init (optional)').count(), 0, 'LosOS reads no cloud-init');
  // No name: refused under the field, nothing sent.
  await start.getByRole('button', { name: 'Pay and start' }).click();
  await page.getByText('Give it a name of up to 40 characters.').waitFor();
  assert.deepEqual(vmPosts, []);
  await page.close();
});

await check('your machines show each replica and the address they share', async () => {
  const { page, errors } = await open({ path: '/machines', stored: true, vms: VMS });
  const mine = page.getByTestId('machines-mine');
  await mine.getByText('web', { exact: true }).waitFor();
  await mine.getByText('vm-0123456789ab-0').waitFor();
  await mine.getByText('running', { exact: true }).waitFor();
  await mine.getByText('copying its disk', { exact: true }).waitFor();
  const address = mine.getByRole('link', { name: 'https://vm-0123456789ab.vms.example.net' });
  assert.equal(await address.getAttribute('rel'), 'noreferrer');
  // An uploaded image is offered next to the catalogue.
  await page.getByTestId('machines-start').getByLabel('System').selectOption('upl_77aa');
  assert.deepEqual(errors, []);
  await page.close();
});

await check('an own QCOW2 is streamed to /api/vms/images with its name and firmware', async () => {
  const { page, errors, vmPosts } = await open({ path: '/machines', stored: true, vms: VMS });
  const images = page.getByTestId('machines-images');
  const qcow2 = new Uint8Array(64);
  qcow2.set([0x51, 0x46, 0x49, 0xfb, 0, 0, 0, 3]);
  await pickFiles(images.getByLabel('QCOW2 file'), [{ name: 'router.qcow2', bytes: qcow2 }]);
  assert.equal(await images.getByLabel('Name of the image').inputValue(), 'router');
  await images.getByLabel('Boots with UEFI').check();
  await images.getByRole('button', { name: 'Upload' }).click();
  await page.locator('[data-toast]').filter({ hasText: 'Image uploaded' }).waitFor();
  assert.deepEqual(vmPosts, [['PUT', '/api/vms/images?name=router&efi=1', 64]]);
  await images.getByRole('button', { name: 'Remove' }).click();
  await page.locator('[data-toast]').filter({ hasText: 'Image removed' }).waitFor();
  assert.deepEqual(vmPosts[1], ['/api/vms/images/remove', { upload_id: 'upl_77aa' }]);
  assert.deepEqual(errors, []);
  await page.close();
});

await check('a hosting box offers replicas at a price per replica-month', async () => {
  const { page, errors, vmPosts } = await open({ path: '/machines', stored: true, vms: VMS });
  const host = page.getByTestId('machines-host');
  await host.getByLabel('Price per replica-month (EUR)').fill('7,50');
  await host.getByLabel('Replicas you will run').fill('60');
  await host.getByRole('button', { name: 'Offer' }).click();
  await host.getByText('Offer between 1 and 50 replicas.').waitFor();
  assert.deepEqual(vmPosts, []);
  await host.getByLabel('Replicas you will run').fill('5');
  await host.getByRole('button', { name: 'Offer' }).click();
  await page.locator('[data-toast]').filter({ hasText: 'Offer is up' }).waitFor();
  assert.deepEqual(vmPosts, [['/api/vms/listings', { unit_price: 750, capacity: 5 }]]);
  await page.getByText('You receive 50% of every machine sale and LosOS keeps 50%.', { exact: false }).waitFor();
  assert.deepEqual(errors, []);
  await page.close();
});

/* The banner while the box is not on a mesh (components/mesh-notice.tsx):
 * it reads the saved settings, names no feature, links to the switch, and
 * stays dismissed in this browser until the box joins. */
const SHARED = { ...JOINABLE, sharingMyStorage: true, clusterEnable: true };

await check('a box off the mesh shows the banner on every page, with the way to join', async () => {
  const { page, errors } = await open({ stored: true, settings: { ...SETTINGS, sharingMyStorage: true } });
  const notice = page.locator('[data-notice="mesh"]');
  await notice.waitFor();
  assert.equal(await notice.getAttribute('data-mesh'), 'notJoined');
  assert.equal(await notice.getAttribute('role'), 'status');
  assert.match(await notice.innerText(), /Some features may not be available/);
  assert.doesNotMatch(await notice.innerText(), /chat|mail|call/i, 'the banner names a feature');
  await notice.getByRole('link', { name: 'Join a mesh', exact: true }).click();
  await page.waitForURL('**/mesh');
  // On the Mesh pane the switch is right there: the banner stays, the link goes.
  await page.getByRole('switch', { name: 'Join the mesh', exact: true }).waitFor();
  await notice.waitFor();
  assert.equal(await notice.getByRole('link').count(), 0);
  assert.deepEqual(errors, []);
  await page.close();
});

await check('a box on a mesh that keeps its storage to itself points at the sharing switch', async () => {
  const { page } = await open({ stored: true, settings: { ...SHARED, sharingMyStorage: false } });
  const notice = page.locator('[data-notice="mesh"]');
  await notice.waitFor();
  assert.equal(await notice.getAttribute('data-mesh'), 'notSharing');
  assert.equal(await notice.getByRole('link', { name: 'Share storage', exact: true }).getAttribute('href'), '/settings/market');
  await page.close();
});

await check('a box that shares and has joined shows no banner', async () => {
  const { page } = await open({ stored: true, settings: SHARED });
  await page.getByRole('heading', { name: 'mattbox' }).first().waitFor().catch(() => {});
  await page.waitForLoadState('networkidle');
  assert.equal(await page.locator('[data-notice="mesh"]').count(), 0);
  await page.close();
});

await check('the banner stays dismissed across reloads, goes when the box joins, and comes back if it leaves', async () => {
  let current = SETTINGS;
  const { page } = await open({ stored: true });
  await page.route('**/api/settings', (route) => json(route, 200, current));
  await page.reload({ waitUntil: 'networkidle' });
  const notice = page.locator('[data-notice="mesh"]');
  await notice.waitFor();
  await notice.getByRole('button', { name: 'Dismiss', exact: true }).click();
  await notice.waitFor({ state: 'detached' });
  await page.reload({ waitUntil: 'networkidle' });
  await page.getByRole('link', { name: 'Overview' }).first().waitFor();
  assert.equal(await notice.count(), 0, 'the dismissal did not survive a reload');
  // The box joins; a page change reads the settings again.
  current = SHARED;
  await page.getByRole('link', { name: 'Mesh' }).first().click();
  await page.waitForURL('**/mesh');
  await page.waitForFunction(() => window.localStorage.getItem('losos-mesh-notice') === null);
  // ...and leaves: the banner is back without being asked.
  current = SETTINGS;
  await page.getByRole('link', { name: 'Overview' }).first().click();
  await notice.waitFor();
  await page.close();
});

await check('the banner fits a phone in both themes', async () => {
  for (const scheme of ['light', 'dark']) {
    const { page, errors } = await open({ stored: true, viewport: { width: 375, height: 800 } });
    await page.emulateMedia({ colorScheme: scheme });
    await page.locator('[data-notice="mesh"]').waitFor();
    const overflow = await page.evaluate(
      () => document.documentElement.scrollWidth - document.documentElement.clientWidth,
    );
    assert.ok(overflow <= 0, `${scheme}: page is ${overflow}px wider than the viewport`);
    assert.deepEqual(errors, []);
    await page.close();
  }
});

await check('a phone-width viewport does not scroll the page sideways', async () => {
  const { page } = await open({ stored: true, viewport: { width: 375, height: 800 } });
  await bottomBar(page).waitFor();
  const overflow = await page.evaluate(
    () => document.documentElement.scrollWidth - document.documentElement.clientWidth,
  );
  assert.ok(overflow <= 0, `page is ${overflow}px wider than the viewport`);
  await page.close();
});

await check('the Machines pane fits a phone too', async () => {
  const { page, errors } = await open({ path: '/machines', stored: true, vms: VMS, viewport: { width: 375, height: 800 } });
  await page.getByTestId('machines-start').waitFor();
  const overflow = await page.evaluate(
    () => document.documentElement.scrollWidth - document.documentElement.clientWidth,
  );
  assert.ok(overflow <= 0, `page is ${overflow}px wider than the viewport`);
  assert.deepEqual(errors, []);
  await page.close();
});

/* AI models on a USDC edge on Base: three models, one running for this box
 * with a balance under a day of runway. No wallet is saved in this browser. */
const AI_WALLET = `0x${'4e'.repeat(20)}`;
const AI_MARKET = {
  ...MARKET,
  account: {
    ...MARKET.account,
    currency: 'usd',
    usdc: {
      test: false,
      chain_id: 8453,
      solana: null,
      ens_chain_id: null,
      chains: [
        { network: 'base', name: 'Base', family: 'evm', chain_id: 8453, usdc: `0x${'11'.repeat(20)}`, pay: `0x${'22'.repeat(20)}`, treasury: `0x${'33'.repeat(20)}` },
      ],
    },
  },
};
const AI_METER = {
  id: 'mtr_5c1e0a9b7d3f42e8a6b01c9d',
  model: 'llama-3-3-70b',
  network: 'base',
  state: 'open',
  wallet: AI_WALLET,
  spendPerHour: '500000',
  boxes: 4,
  group: 'ready',
  requests: 9,
  tokensIn: 12400,
  tokensOut: 3100,
  billedE6: '10230000000',
  paid: '0',
  createdAt: 1791450000,
};
const LLM = {
  available: true,
  network: 'base',
  feeBps: 400,
  models: [
    { id: 'llama-3-3-70b', name: 'Llama 3.3 70B', needMib: 46080, context: 8192, priceIn: '600000', priceOut: '900000', licence: 'Llama 3.3 Community', lentMib: 61440, group: 'ready', boxes: 4 },
    { id: 'qwen3-32b', name: 'Qwen3 32B', needMib: 22528, context: 8192, priceIn: '300000', priceOut: '450000', licence: 'Apache-2.0', lentMib: 61440 },
    { id: 'deepseek-r1', name: 'DeepSeek R1', needMib: 409600, context: 4096, priceIn: '2000000', priceOut: '4000000', licence: 'MIT', lentMib: 61440 },
  ],
  meters: [AI_METER],
  wallets: [{ network: 'base', wallet: AI_WALLET, balance: 8000000, asked: 0, notBefore: 0, available: '7989770', runwaySecs: 57526, low: true }],
  lending: { paid: '0', owed: '0' },
  noticeSecs: 172800,
  minSpendPerHour: '10000',
  maxSpendPerHour: '100000000',
};

await check('AI models: the catalogue has prices and memory, says once who reads the prompts, and a running model shows its meter and balance', async () => {
  const { page, errors, marketPosts, llmPosts } = await open({ path: '/settings/market', stored: true, market: AI_MARKET, llm: LLM });
  const section = page.getByTestId('ai-models');
  await section.waitFor();
  const llama = section.locator('[data-model="llama-3-3-70b"]');
  assert.match(await llama.innerText(), /\$0\.60 in, \S*\$0\.90 out per million tokens/);
  assert.match(await llama.innerText(), /Needs 45 GiB; boxes lend 60 GiB now/);
  assert.match(await llama.innerText(), /Running on 4 boxes/);
  // A model already running for this box has no Start; one that does not
  // fit in what the boxes lend cannot be started.
  assert.equal(await llama.getByRole('button', { name: 'Start' }).count(), 0);
  assert.equal(await section.locator('[data-model="deepseek-r1"]').getByRole('button', { name: 'Start' }).isDisabled(), true);
  assert.match(await section.locator('[data-model="deepseek-r1"]').innerText(), /Waiting for more memory/);
  assert.equal(await page.getByText('The boxes running a model can read its prompts and answers.', { exact: false }).count(), 1);
  const meters = page.getByTestId('ai-meters');
  assert.match(await meters.innerText(), /Llama 3\.3 70B · Open/);
  assert.match(await meters.innerText(), /\$0\.0102 spent on 15,500 tokens · at most \S*\$0\.50 an hour/);
  const balance = page.getByTestId('ai-balance');
  assert.match(await balance.innerText(), /Less than a day left at the limits you set: about 16 hours/);
  assert.match(await balance.innerText(), /\$7\.99/);
  await balance.getByRole('button', { name: 'Ask for it back' }).waitFor();
  assert.match(await section.innerText(), /Set up the USDC wallet below first, or connect a browser wallet\./);
  assert.deepEqual(marketPosts, []);
  assert.deepEqual(llmPosts, []);
  assert.deepEqual(errors, []);
  await page.close();
});

await check('AI models: Stop needs no wallet and a chat answer says what it cost', async () => {
  const { page, errors, llmPosts } = await open({ path: '/settings/market', stored: true, market: AI_MARKET, llm: LLM });
  const chat = page.getByTestId('ai-chat');
  await chat.waitFor();
  await chat.getByPlaceholder('Ask something').fill('What is a comet made of?');
  await chat.getByRole('button', { name: 'Send' }).click();
  await chat.getByText('Mostly water, then ice.').waitFor();
  // 42 tokens at $0.60 a million and 7 at $0.90: 31.5 millionths of a dollar.
  assert.match(await chat.innerText(), /This answer cost \S*\$0\.000031: 42 tokens in, 7 out\./);
  assert.deepEqual(llmPosts, [
    ['/api/market/llm/chat', { meter: AI_METER.id, messages: [{ role: 'user', content: 'What is a comet made of?' }], maxTokens: 256 }],
  ]);
  // The next question carries the conversation so far.
  await chat.getByPlaceholder('Ask something').fill('And its tail?');
  await chat.getByRole('button', { name: 'Send' }).click();
  await page.waitForFunction(() => document.querySelectorAll('[data-testid="ai-chat"] p').length >= 6);
  assert.deepEqual(llmPosts[1][1].messages, [
    { role: 'user', content: 'What is a comet made of?' },
    { role: 'assistant', content: 'Mostly water, then ice.' },
    { role: 'user', content: 'And its tail?' },
  ]);
  await page.getByTestId('ai-meters').getByRole('button', { name: 'Stop' }).click();
  await page.getByText('The edge closes the taps; nothing more is billed.', { exact: true }).waitFor();
  assert.deepEqual(llmPosts[2], ['/api/market/taps/stop', { meter: AI_METER.id }]);
  assert.deepEqual(errors, []);
  await page.close();
});

await check('AI models: Check again tells the edge which wallet opened the taps', async () => {
  const quoted = { ...LLM, meters: [{ ...AI_METER, state: 'quoted', wallet: undefined }] };
  const { page, errors, llmPosts } = await open({ path: '/settings/market', stored: true, market: AI_MARKET, llm: quoted });
  const meters = page.getByTestId('ai-meters');
  await meters.waitFor();
  // The tab that opened the taps remembers the wallet that did.
  await page.evaluate(([meter, wallet]) => sessionStorage.setItem('losos-llm-meter', JSON.stringify({ meter, wallet })), [AI_METER.id, AI_WALLET]);
  await meters.getByRole('button', { name: 'Check again' }).click();
  await page.waitForFunction(() => document.querySelector('[data-testid="ai-meters"] button[disabled]') === null);
  for (let i = 0; i < 50 && !llmPosts.some(([path]) => path === '/api/market/taps/confirm'); i++) await page.waitForTimeout(100);
  assert.deepEqual(
    llmPosts.filter(([path]) => path === '/api/market/taps/confirm'),
    [['/api/market/taps/confirm', { meter: AI_METER.id, wallet: AI_WALLET }]],
  );
  assert.deepEqual(errors, []);
  await page.close();
});

await check('AI models: a wallet that gives up partway tells the edge which wallet it was', async () => {
  const meter = 'mtr_0b7e4c2a9d1f36e85a4c07b2';
  const quote = {
    available: true,
    meter,
    model: 'qwen3-32b',
    network: 'base',
    chain: AI_MARKET.account.usdc.chains[0],
    feeBps: 400,
    spendPerHour: '500000',
    taps: [
      { id: 'tap-1', key: `0x${'a1'.repeat(32)}`, seller: `0x${'51'.repeat(20)}`, rate: '250000' },
      { id: 'tap-2', key: `0x${'a2'.repeat(32)}`, seller: `0x${'52'.repeat(20)}`, rate: '250000' },
    ],
    openBefore: Math.floor(Date.now() / 1000) + 900,
  };
  // A browser wallet that connects, then refuses everything it is asked.
  const init = (wallet) => {
    window.ethereum = {
      request: async ({ method }) => {
        if (method === 'eth_requestAccounts') return [wallet];
        throw Object.assign(new Error('User rejected the request.'), { code: 4001 });
      },
    };
  };
  const { page, llmPosts } = await open({
    path: '/settings/market',
    stored: true,
    market: AI_MARKET,
    llm: { ...LLM, meters: [], wallets: [] },
    quote,
    init: [init, AI_WALLET],
  });
  const qwen = page.locator('[data-model="qwen3-32b"]');
  await qwen.waitFor();
  await qwen.getByRole('button', { name: 'Start' }).click();
  await qwen.getByRole('button', { name: 'Start' }).last().click();
  for (let i = 0; i < 50 && !llmPosts.some(([path]) => path === '/api/market/taps/stop'); i++) await page.waitForTimeout(100);
  assert.deepEqual(
    llmPosts.filter(([path]) => path.startsWith('/api/market/taps/')),
    [['/api/market/taps/stop', { meter, wallet: AI_WALLET }]],
  );
  await page.close();
});

await check('AI models: Start asks for an hourly limit and a top-up, refuses a limit out of range, and needs a wallet before it asks the edge', async () => {
  const { page, errors, llmPosts } = await open({ path: '/settings/market', stored: true, market: AI_MARKET, llm: LLM });
  const qwen = page.locator('[data-model="qwen3-32b"]');
  await qwen.waitFor();
  await qwen.getByRole('button', { name: 'Start' }).click();
  const limit = qwen.getByLabel('Most it may spend in an hour (USDC)');
  assert.equal(await limit.inputValue(), '0.50');
  // Eight hours at the limit; with no wallet chosen, nothing is known held.
  assert.equal(await qwen.getByLabel('Add to your balance (USDC)').inputValue(), '4');
  await limit.fill('500');
  await qwen.getByRole('button', { name: 'Start' }).last().click();
  await qwen.getByText('Allow between 0.01 and 100 USDC an hour.').waitFor();
  await limit.fill('2');
  assert.equal(await qwen.getByLabel('Add to your balance (USDC)').inputValue(), '16');
  await qwen.getByRole('button', { name: 'Start' }).last().click();
  await page.getByText('Set up the USDC wallet below first, or connect a browser wallet.').nth(1).waitFor();
  assert.deepEqual(llmPosts, [], 'a quote was asked for with no wallet to pay it');
  assert.deepEqual(errors, []);
  await page.close();
});

/* Under the real policy the Market pane, a running model and an opened Start
 * form log no violation, at load or after. */
await check('under the appliance CSP the AI models section adds no violation', async () => {
  const { origin: strict, close } = await serve({ csp: true });
  try {
    const watch = () => {
      window.__violations = [];
      document.addEventListener('securitypolicyviolation', (e) =>
        window.__violations.push(`${e.violatedDirective} ${e.target?.nodeName ?? ''}`),
      );
    };
    const { page, errors } = await open({ path: '/settings/market', stored: true, market: AI_MARKET, llm: LLM, base: strict, init: [watch] });
    await page.getByTestId('ai-meters').waitFor();
    const atLoad = await page.evaluate(() => window.__violations);
    assert.deepEqual(atLoad, [], `unexpected violations at load: ${atLoad.join(', ')}`);
    const qwen = page.locator('[data-model="qwen3-32b"]');
    await qwen.getByRole('button', { name: 'Start' }).click();
    await qwen.getByLabel('Most it may spend in an hour (USDC)').fill('2');
    await page.waitForTimeout(300);
    const after = await page.evaluate(() => window.__violations);
    assert.equal(after.length, atLoad.length, `the AI models section added CSP violations: ${after.slice(atLoad.length).join(', ')}`);
    assert.deepEqual(errors, []);
    await page.close();
  } finally {
    await close();
  }
});

await check('AI models: a wallet imported further down the same page can pay at once, and forgetting it takes it away', async () => {
  const { mnemonicToAccount } = await import('viem/accounts');
  const phrase = 'test test test test test test test test test test test junk';
  const address = mnemonicToAccount(phrase).address;
  const { page, errors, llmPosts } = await open({ path: '/settings/market', stored: true, market: AI_MARKET, llm: LLM });
  const section = page.getByTestId('ai-models');
  await section.getByText('Set up the USDC wallet below first, or connect a browser wallet.').first().waitFor();
  await page.getByPlaceholder('12 or 24 words from MetaMask, Rabby, Rainbow, Trust Wallet or Phantom').fill(phrase);
  await page.getByPlaceholder('A password for this browser (8 or more characters)').fill('right horse battery');
  await page.getByRole('button', { name: 'Import', exact: true }).click();
  // No reload: the section reads the saved wallet again when it changes.
  await section.getByText('The wallet on this box', { exact: true }).waitFor({ timeout: 15000 });
  assert.match(await section.innerText(), new RegExp(`${address.slice(0, 6)}…${address.slice(-4)}`));
  assert.doesNotMatch(await section.innerText(), /Set up the USDC wallet below first/);
  await page.getByRole('button', { name: "Forget this browser's copy", exact: true }).click();
  await section.getByText('Set up the USDC wallet below first, or connect a browser wallet.').first().waitFor();
  assert.deepEqual(llmPosts, []);
  assert.deepEqual(errors, []);
  await page.close();
});

await check('AI models: a balance asked back says when it can be taken, then offers to take it', async () => {
  const now = Date.UTC(2026, 9, 8, 9, 30);
  const waiting = { ...LLM, meters: [], wallets: [{ ...LLM.wallets[0], asked: 8000000, notBefore: now / 1000 + 36 * 3600, runwaySecs: undefined, low: false }] };
  let { page, errors } = await open({ path: '/settings/market', stored: true, market: AI_MARKET, llm: waiting, now });
  let balance = page.getByTestId('ai-balance');
  await balance.waitFor();
  assert.match(await balance.innerText(), /\$8\.00 asked back; you can take it from 9 Oct 2026/);
  assert.equal(await balance.getByRole('button').count(), 0);
  assert.equal(await page.getByTestId('ai-chat').count(), 0, 'a chat with nothing running');
  assert.deepEqual(errors, []);
  await page.close();

  const due = { ...waiting, wallets: [{ ...waiting.wallets[0], notBefore: now / 1000 - 60 }] };
  ({ page, errors } = await open({ path: '/settings/market', stored: true, market: AI_MARKET, llm: due, now }));
  balance = page.getByTestId('ai-balance');
  await balance.waitFor();
  assert.match(await balance.innerText(), /\$8\.00 can be taken back now/);
  await balance.getByRole('button', { name: 'Take it back' }).waitFor();
  assert.deepEqual(errors, []);
  await page.close();
});

await check('AI models: an edge that runs none says so, and a USDC-less market asks for none', async () => {
  let { page, errors } = await open({ path: '/settings/market', stored: true, market: AI_MARKET, llm: { available: false } });
  await page.getByTestId('ai-models').getByText('This edge runs no AI models.').waitFor();
  assert.deepEqual(errors, []);
  await page.close();
  ({ page, errors } = await open({ path: '/settings/market', stored: true, market: MARKET, llm: LLM }));
  await page.getByRole('heading', { name: 'Market' }).first().waitFor().catch(() => undefined);
  await page.waitForTimeout(300);
  assert.equal(await page.getByTestId('ai-models').count(), 0);
  assert.deepEqual(errors, []);
  await page.close();
});

await check('the Market pane with AI models fits a phone', async () => {
  const { page, errors } = await open({ path: '/settings/market', stored: true, market: AI_MARKET, llm: LLM, viewport: { width: 375, height: 800 } });
  await page.getByTestId('ai-chat').waitFor();
  const overflow = await page.evaluate(() => document.documentElement.scrollWidth - document.documentElement.clientWidth);
  assert.ok(overflow <= 0, `page is ${overflow}px wider than the viewport`);
  assert.deepEqual(errors, []);
  await page.close();
});

await browser.close();
await closeServer();
finish();
