/* The handbook's pictures of the admin pages and the wizard, taken from a
 * built dist/ against a stubbed lososd, so every screen shows the current UI
 * and the current logo without a box.
 *
 *   bun run build && bun tests/handbook-screens.mjs /tmp/handbook-screens [name]
 *
 * Each entry in SHOTS is one PNG: a path, a box state (the stub's answers),
 * a theme and a locale, and optionally what to click before the picture and
 * which part of the page to keep (`crop`). An entry with `langs` is taken in
 * English, Slovak and German, the last two as <name>-sk and <name>-de, for
 * the handbook's translated pages; its clicks name buttons through the UI's
 * own catalogue (src/i18n), so they find them in every language.
 * The stubbed answers are the shapes tests/*.browser.mjs use; nothing here is
 * read by the app in production. A second argument narrows the run to the
 * names that contain it. The PNGs are 2x; the handbook carries them
 * downscaled (handbook/docs/reference/for-developers.md, "Pictures"). */

import { launch, serve } from './harness.mjs';

const [outDir = '/tmp/handbook-screens', only = ''] = Bun.argv.slice(2);

const TOKEN = '7c1e4a9d02f6b38e5d71c0a4e9b2f86d3a5c17e0b94d62f8c3e1a07d5b9f4c26'; // a fixture, not a key
const PASSWORD = 'Correct-horse battery staple 1';
const BOX_ADDRESS = '192.168.1.23';
const AT = 'http://mattbox.local';
const NOW = Date.UTC(2026, 9, 8, 9, 30); // 2026-10-08 09:30Z, so dates in shots are stable

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
const JOINED = { ...SETTINGS, proxyEnable: true, clusterEnable: true, shareCompute: true };

const EDGE_NONE = { reachable: false, official: false, edges: [], lanSearched: true, configuredUrl: 'https://losos-edge.dasmat.us', checkedAt: 1760000000 };
const EDGE_OFFICIAL = {
  ...EDGE_NONE,
  reachable: true,
  official: true,
  edges: [{ name: 'losos-edge.dasmat.us', url: 'https://losos-edge.dasmat.us', source: 'configured', official: true }],
};
const EDGE_COMPANY = {
  ...EDGE_NONE,
  reachable: true,
  edges: [{ name: 'office edge', url: 'http://edge.local:8443', source: 'lan', official: false }],
};
const EDGE_BOTH = { ...EDGE_OFFICIAL, edges: [EDGE_COMPANY.edges[0], EDGE_OFFICIAL.edges[0]] };
/* The company edge in use, and the owner has let it manage the box. */
const EDGE_MANAGED = {
  ...EDGE_COMPANY,
  path: { name: 'office edge', url: 'http://edge.local:8443', source: 'lan', tunnel: 'edge.local:2333' },
  manage: {
    candidate: { name: 'office edge', url: 'http://edge.local:8443' },
    refused: null,
    managedBy: 'http://edge.local:8443',
    active: true,
  },
};
/* What the company edge's manage page lists (src/manage/). */
const MANAGED_BOXES = {
  boxes: [
    { id: 'design-box', hostname: 'design-box', seenAgoSecs: 4, managed: true },
    { id: 'mattbox', hostname: 'mattbox', seenAgoSecs: 9, managed: true },
    { id: 'reception', hostname: 'reception', seenAgoSecs: 420, managed: false },
  ],
};

const STORAGE = { totalBytes: 480 * 1024 ** 3, usedBytes: 131 * 1024 ** 3, lentBytes: 0, reserveBytes: 48 * 1024 ** 3 };
const STORAGE_SHARED = { ...STORAGE, lentBytes: 96 * 1024 ** 3 };
const STORAGE_FULL = { ...STORAGE, usedBytes: 421 * 1024 ** 3 };

const BACKUP_SET = {
  target: {
    endpoint: 'https://s3.eu-central-1.amazonaws.com',
    bucket: 'mattbox-backups',
    prefix: 'losos',
    region: 'eu-central-1',
    accessKeyId: 'AKIAIOSFODNN7EXAMPLE',
    hasSecret: true,
    storageClass: 'glacier',
  },
  last: { time: NOW / 1000 - 6 * 3600, snapshot: '9f2c'.repeat(16), files: 18324, bytes: 47_244_640_256 },
  job: null,
  erase: null,
  lastErase: null,
  graceSeconds: 900,
};
const BACKUP_COUNTDOWN = {
  ...BACKUP_SET,
  erase: {
    phase: 'waiting',
    backup: true,
    startedAt: NOW / 1000 - 400,
    deadline: NOW / 1000 + 731,
    secondsLeft: 731,
    cancellable: true,
    message: '',
    outside: { hadEdge: false, domainsRemoved: 0, listingsClosed: 0, leftEdge: false, problems: [], erasedAt: 0 },
  },
};

const LIMITS = {
  widgets: 24,
  nameChars: 60,
  widgetBytes: 131072,
  files: 12,
  fileNameChars: 40,
  fileKinds: ['html', 'css', 'js', 'mjs', 'json', 'svg', 'txt', 'md'],
  imageBytes: 8388608,
  veil: { min: 20, max: 90 },
};
const HAND_WIDGET = {
  id: 'w1',
  name: 'Greeting',
  span: 'half',
  files: [
    {
      name: 'index.html',
      content: [
        '<link rel="stylesheet" href="style.css">',
        '<div class="big" id="out">…</div>',
        '<div id="free"></div>',
        '<script src="app.js"></script>',
      ].join('\n'),
    },
    {
      name: 'app.js',
      content: [
        'losos.metric("box.settings").then((m) => {',
        '  document.getElementById("out").textContent = "Hello from " + m.hostName;',
        '});',
        'document.getElementById("free").textContent = "Drawn in its own sandbox, in the " + losos.theme + " theme.";',
      ].join('\n'),
    },
    {
      name: 'style.css',
      content: [
        'body { font-family: system-ui, sans-serif; margin: 0; color: var(--ink); }',
        '.big { font-size: 28px; font-weight: 600; color: var(--accent); }',
      ].join('\n'),
    },
  ],
};
const LOOK_PLAIN = { version: 1, background: { kind: 'none' }, veil: 60, widgets: [], shipped: ['tide', 'grid', 'dusk'], limits: LIMITS };
const LOOK_TIDE = { ...LOOK_PLAIN, background: { kind: 'shipped', name: 'tide' }, widgets: [HAND_WIDGET] };

/* GET /api/domains: no official edge in reach (the default, matching
 * EDGE_NONE), an official edge and a box Stripe has not checked, and a
 * checked box with one domain live and one waiting for its CNAME. */
const DOMAINS_OFF = { available: false, reason: 'noOfficialEdge' };
const DOMAINS_UNVOUCHED = {
  available: true,
  eligible: false,
  reason: 'stripeAccount',
  target: null,
  addresses: ['203.0.113.7', '2001:db8::7'],
  max_domains: 5,
  domains: [],
};
const DOMAINS_READY = {
  ...DOMAINS_UNVOUCHED,
  eligible: true,
  reason: null,
  target: '3f9a1c0e7b2d4a55.boxes.losos.dasmat.us',
  domains: [
    {
      domain: 'cloud.novak-family.eu',
      status: 'live',
      txt_name: '_losos-challenge.cloud.novak-family.eu',
      txt_value: 'losos-domain-v1=6b0d2f9e41c7a83d5e12f04b9a7c6d18',
      txt_found: true,
      points_here: true,
      problem: null,
      checked_at: 1791451800,
      verified_at: 1791448200,
    },
    {
      domain: 'git.novak-family.eu',
      status: 'waiting',
      txt_name: '_losos-challenge.git.novak-family.eu',
      txt_value: 'losos-domain-v1=6b0d2f9e41c7a83d5e12f04b9a7c6d18',
      txt_found: true,
      points_here: false,
      problem: 'notPointing',
      checked_at: 1791451800,
      verified_at: null,
    },
  ],
};

const MARKET_OFF = { available: false };
const MARKET_UNOFFICIAL = { available: false, reason: 'noOfficialEdge' };
const MARKET_OPEN = {
  available: true,
  listings: [
    { id: 'lst_ab12', kind: 'storage', unit: 'GiB-month', unit_price: 12, currency: 'eur', available: 400 },
    { id: 'lst_cd34', kind: 'compute', unit: 'vCPU-hour', unit_price: 90, currency: 'eur', available: 120 },
  ],
  account: {
    fee_bps: 400,
    currency: 'eur',
    seller_onboarded: false,
    seller_ready: false,
    can_sell_storage: true,
    can_sell_compute: false,
    listings: [],
    entitlements: { storage_gib: 0, compute_vcpu_hours: 0, next_expiry: null },
    purchases: [],
    sales: [],
  },
};

/* A seller a month into selling: one listing a buyer has paid for until
 * 30 October, that buyer's sale held, an earlier sale ready to pay out, and
 * one purchase of its own whose seller went offline, partly refunded. */
const SLOT_END = Date.UTC(2026, 9, 30, 9, 30) / 1000;
const slotOrder = (id, extra) => ({
  id,
  listing_id: 'lst_held',
  kind: 'storage',
  unit: 'GiB-month',
  quantity: 40,
  amount: 2000,
  fee: 80,
  seller_net: 1920,
  currency: 'eur',
  status: 'paid',
  created_at: SLOT_END - 30 * 86400,
  paid_at: SLOT_END - 30 * 86400,
  expires_at: SLOT_END,
  expired: false,
  volume: null,
  ...extra,
});
const MARKET_SLOTS = {
  ...MARKET_OPEN,
  account: {
    ...MARKET_OPEN.account,
    seller_onboarded: true,
    seller_ready: true,
    listings: [
      { id: 'lst_held', kind: 'storage', unit: 'GiB-month', unit_price: 50, capacity: 100, available: 60, active: true, locked_until: SLOT_END },
    ],
    sales: [
      slotOrder('ord_sale1', { card: { step: 'held' }, payout_at: SLOT_END + 86400 }),
      slotOrder('ord_sale2', { quantity: 20, amount: 1000, fee: 40, seller_net: 960, expires_at: NOW / 1000 - 3 * 86400, card: { step: 'held' } }),
    ],
    purchases: [
      slotOrder('ord_bought', { kind: 'compute', unit: 'vCPU-hour', quantity: 20, card: { step: 'settled', reason: 'sellerOffline', refund: 1401 } }),
    ],
    payouts: { held: 1920, ready: 960, paying: 0, next_at: SLOT_END, auto_after_secs: 86400 },
    offline_refund_secs: 86400,
    direct_pay: false,
  },
};

/* A box on an official edge that settles in USDC on Base and Solana and
 * runs AI models on the memory boxes lend: one model running for this box
 * for three hours, its balance under a day of runway, and this box lending
 * 16 GiB to the group that serves it. */
const USDC_MAINNET = {
  test: false,
  chain_id: 8453,
  solana: 'solana',
  ens_chain_id: null,
  chains: [
    {
      network: 'base',
      name: 'Base',
      family: 'evm',
      chain_id: 8453,
      usdc: '0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913',
      pay: `0x${'7a'.repeat(20)}`,
      treasury: `0x${'3c'.repeat(20)}`,
    },
    {
      network: 'solana',
      name: 'Solana',
      family: 'solana',
      chain_id: null,
      usdc: 'EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v',
      pay: 'Esc7oWpF3x1QfXo4pYbJjLW1kY5nqH8TzCtqGm2RA9dV',
      treasury: 'Trs9vH4dQ1mWb6KxJ3cNfR8sLpZ2gYtE5uAoXk7nB1q',
    },
  ],
};
const MARKET_AI = {
  ...MARKET_OPEN,
  account: { ...MARKET_OPEN.account, currency: 'usd', seller_onboarded: true, seller_ready: true, usdc: USDC_MAINNET },
};
const LLM_STARTED = NOW / 1000 - 3 * 3600;
const LLM = {
  available: true,
  network: 'base',
  feeBps: 400,
  models: [
    { id: 'llama-3-3-70b', name: 'Llama 3.3 70B', needMib: 46080, context: 8192, priceIn: '600000', priceOut: '900000', licence: 'Llama 3.3 Community', lentMib: 61440, group: 'ready', boxes: 4 },
    { id: 'qwen3-32b', name: 'Qwen3 32B', needMib: 22528, context: 8192, priceIn: '300000', priceOut: '450000', licence: 'Apache-2.0', lentMib: 61440 },
    { id: 'deepseek-r1', name: 'DeepSeek R1', needMib: 409600, context: 4096, priceIn: '2000000', priceOut: '4000000', licence: 'MIT', lentMib: 61440 },
  ],
  meters: [
    {
      id: 'mtr_5c1e0a9b7d3f42e8a6b01c9d',
      model: 'llama-3-3-70b',
      network: 'base',
      state: 'open',
      wallet: `0x${'4e'.repeat(20)}`,
      spendPerHour: '500000',
      boxes: 4,
      group: 'ready',
      requests: 9,
      tokensIn: 12400,
      tokensOut: 3100,
      billedE6: '10230000000',
      paid: '0',
      createdAt: LLM_STARTED,
    },
  ],
  wallets: [
    { network: 'base', wallet: `0x${'4e'.repeat(20)}`, balance: 8000000, asked: 0, notBefore: 0, available: '7989770', runwaySecs: 57526, low: true },
  ],
  lending: { paid: '4210000', owed: '380000', model: 'llama-3-3-70b', group: 'ready', lentMib: 16384 },
  noticeSecs: 172800,
  minSpendPerHour: '10000',
  maxSpendPerHour: '100000000',
};
const LENDING = { 'inference.lend.enable': 'true', 'inference.lend.memoryMiB': '16384' };

const CONFIG = {
  enabled: true,
  repository: {
    owner: 'notshared',
    name: 'losos-config',
    url: '/git/notshared/losos-config',
    clone: 'http://mattbox.local/git/notshared/losos-config.git',
    viaServer: true,
  },
  head: { sha: 'b'.repeat(40), branch: 'main' },
  log: [
    { sha: 'b'.repeat(40), when: '2026-10-07T10:12:00+02:00', subject: 'Change hostName, cluster.enable' },
    { sha: 'c'.repeat(40), when: '2026-10-06T18:40:00+02:00', subject: 'Change hardening.usbguard' },
    { sha: 'a'.repeat(40), when: '2026-10-06T03:00:00+02:00', subject: 'losos install' },
  ],
  sync: { state: 'ok', detail: 'In step with LosOS Git.', syncedAt: '2026-10-07T10:12:30+02:00', remoteHead: 'b'.repeat(40) },
  upstream: {
    url: 'https://github.com/dasmatus/losos.git',
    branch: 'main',
    report: { state: 'ok', detail: 'Took dasmatus/losos main at a088a5cb03; the box builds it at the next update.', syncedAt: '2026-10-07T03:12:00+02:00', remoteHead: 'a088a5cb03e9d1d0d770e6e1fd4bbfe17d5d5143' },
  },
};

const RESULTS = {
  sources: ['Artifact Hub'],
  results: [
    { id: 'jellyfin', name: 'jellyfin', version: '2.0.0', summary: 'A media server for your films and music.', source: 'utkuozdemir', homepage: 'https://jellyfin.org', chart: { repo: 'https://utkuozdemir.org/helm-charts', name: 'jellyfin', version: '2.0.0' } },
    { id: 'immich', name: 'immich', version: '0.9.3', summary: 'Photo and video backup from your phone.', source: 'immich', homepage: 'https://immich.app', chart: { repo: 'https://immich-app.github.io/immich-charts', name: 'immich', version: '0.9.3' } },
    { id: 'vaultwarden', name: 'vaultwarden', version: '0.31.8', summary: 'A password manager server.', source: 'guerzon', homepage: 'https://github.com/dani-garcia/vaultwarden', chart: { repo: 'https://guerzon.github.io/vaultwarden', name: 'vaultwarden', version: '0.31.8' } },
  ],
};

/* One app installed from the search, and what the box reads out of a chart
 * for the install dialog. */
const APPS = {
  available: true,
  sharedAvailable: false,
  apps: [
    {
      release: 'vaultwarden',
      chart: RESULTS.results[2].chart,
      runAs: 'notshared',
      values: {},
      valuesYaml: null,
      frontPort: 30000,
      appPort: 8080,
      phase: 'running',
      message: null,
      updatedAt: 1791451800,
    },
  ],
};
const CHART = {
  chart: RESULTS.results[0].chart,
  release: 'jellyfin',
  valuesYaml: 'image:\n  repository: docker.io/jellyfin/jellyfin\n  tag: ""\nservice:\n  type: ClusterIP\n  port: 8096\npersistence:\n  config:\n    enabled: true\n    size: 5Gi\n  media:\n    enabled: true\n    size: 25Gi\nmetrics:\n  enabled: false\n',
  values: {
    image: { repository: 'docker.io/jellyfin/jellyfin', tag: '' },
    service: { type: 'ClusterIP', port: 8096 },
    persistence: { config: { enabled: true, size: '5Gi' }, media: { enabled: true, size: '25Gi' } },
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

/* The UI's message catalogue, so a click can name a button in the language
 * the shot is taken in: T(lang)(key, vars) is what t(key, vars) draws. */
const { MESSAGES } = await import('../src/i18n/index.ts');
const T = (lang) => (key, vars = {}) => {
  const text = MESSAGES[key]?.[lang];
  if (typeof text !== 'string') throw new Error(`no ${lang} string for ${key}`);
  return text.replace(/\{(\w+)\}/g, (_, name) => String(vars[name] ?? ''));
};
const LOCALES = { en: 'en-US', sk: 'sk-SK', de: 'de-DE' };

const DOC = await Bun.file('tests/fixtures/options.json').json();
const options = (overrides = {}) => ({
  available: true,
  version: DOC.version,
  options: DOC.options.map((option) => ({ ...option, set: overrides[option.name] ?? null })),
  stray: [],
  excluded: DOC.excluded,
});
/* GET /api/site: the settings of LosOS Cloud and LosOS Git only. */
const SITE_OPTION = /^(cloud\.trashDays|git\.site\..+)$/;
const site = (overrides = {}) => {
  const doc = options(overrides);
  return { ...doc, options: doc.options.filter((o) => SITE_OPTION.test(o.name)), excluded: {} };
};

/* Six pins, as an owner would have them after a week: two pages of LosOS
 * Cloud, a file, a folder, a repository and an app installed from the search. */
const PINS = [
  { kind: 'tile', app: 'files' },
  { kind: 'tile', app: 'photos' },
  { kind: 'file', id: 42, path: '/Documents/plan.pdf', folder: false },
  { kind: 'file', id: 11, path: '/Photos/Holidays 2026', folder: true },
  { kind: 'repo', owner: 'notshared', name: 'site' },
  { kind: 'app', release: 'jellyfin' },
];
const FOLDERS = {
  '/': [
    { name: 'Documents', path: '/Documents', id: 10, folder: true, size: 4096 },
    { name: 'Photos', path: '/Photos', id: 11, folder: true, size: 900000 },
    { name: 'notes.md', path: '/notes.md', id: 12, folder: false, size: 812 },
  ],
  '/Documents': [
    { name: 'Tax', path: '/Documents/Tax', id: 20, folder: true, size: 1000 },
    { name: 'budget.ods', path: '/Documents/budget.ods', id: 43, folder: false, size: 9000 },
    { name: 'plan.pdf', path: '/Documents/plan.pdf', id: 42, folder: false, size: 120000 },
    { name: 'recipes.md', path: '/Documents/recipes.md', id: 44, folder: false, size: 3100 },
  ],
};
const REPOS = {
  available: true,
  repos: [
    { owner: 'notshared', name: 'losos-config', private: true, description: "This box's NixOS configuration." },
    { owner: 'notshared', name: 'site', private: false, description: 'My web page' },
  ],
};

/* A board the owner arranged: the five built-ins, as the gallery adds them. */
const BOARD = {
  version: 1,
  widgets: ['uptime', 'disk', 'apps', 'mesh', 'rebuilds'].map((id, i) => ({ id: `b${i}`, source: { kind: 'builtin', id } })),
};

/* A year of the uptime journal: answering nearly every day, two quiet spells. */
function journal() {
  const days = {};
  for (let i = 371; i >= 0; i--) {
    const d = new Date(NOW - i * 86400_000);
    const key = d.toISOString().slice(0, 10);
    const quiet = (i >= 200 && i <= 201) || i === 90;
    const u = quiet ? 3 : 60 + ((i * 7) % 15);
    days[key] = { u, d: quiet ? 40 : (i % 11 === 0 ? 1 : 0), o: quiet ? 1 : 0, l: quiet ? 9 * 3600 : 0 };
  }
  return { v: 1, days, lastUp: true, outageStart: null };
}

const REBUILDS = {
  v: 1,
  jobs: [
    { job: 'losos-rebuild-7f3a', state: 'done', at: NOW - 2 * 3600_000, message: 'Rebuilt and switched.' },
    { job: 'losos-rebuild-61c0', state: 'done', at: NOW - 26 * 3600_000, message: 'Rebuilt and switched.' },
    { job: 'losos-rebuild-2b9e', state: 'failed', at: NOW - 3 * 86400_000, message: 'error: attribute "losos.cloud.trashDays" must be between 1 and 365' },
  ],
};

const json = (route, status, body) => route.fulfill({ status, contentType: 'application/json', body: JSON.stringify(body) });

const { origin, close: closeServer } = await serve({ csp: true });
const browser = await launch();

/* The Users pane: the owner signed in on a box that signs people in itself,
 * and the two people they added. */
const SECTION_NAMES = ['storage', 'backup', 'apps', 'market', 'network', 'look', 'lab', 'access', 'account'];
const SET_AT = Date.UTC(2026, 7, 20) / 1000;
const ME_OWNER = {
  name: 'matus',
  displayName: 'Matus',
  groups: ['losos-admins', 'losos-owner'],
  admin: true,
  spareKey: false,
  sections: SECTION_NAMES,
  standalone: true,
};
const PEOPLE = {
  standalone: true,
  you: 'matus',
  sections: SECTION_NAMES,
  people: [
    { name: 'eva', displayName: 'Eva Novak', email: 'eva@mattbox.local', groups: ['losos-storage', 'losos-backup', 'losos-account', 'losos-two-factor'], disabled: false, twoFactor: true, passwordSetAt: SET_AT },
    { name: 'matus', displayName: 'Matus', email: 'matus@mattbox.local', groups: ['losos-admins', 'losos-owner', 'losos-two-factor'], disabled: false, twoFactor: true, passwordSetAt: SET_AT },
    { name: 'oma', displayName: 'Oma', email: 'oma@mattbox.local', groups: ['losos-viewers', 'losos-account', 'losos-locked'], disabled: false, twoFactor: false, passwordSetAt: Date.UTC(2026, 2, 2) / 1000, locked: ['password'] },
  ],
  rules: { twoFactor: false, passwordDays: 180 },
};
/* The owner's own account, as Your account shows it. */
const ACCOUNT = {
  name: 'matus',
  displayName: 'Matus',
  email: 'matus@mattbox.local',
  factors: { app: true, passkeys: ['Laptop'] },
  passwordSetAt: SET_AT,
  passwordExpiresAt: SET_AT + 180 * 24 * 60 * 60,
  rules: PEOPLE.rules,
  locked: [],
};

/* One page wired to a box in a given state. */
async function open({
  path = '/',
  theme = 'light',
  locale = 'en-US',
  viewport = { width: 1280, height: 900 },
  claimed = true,
  signedIn = true,
  ready = true,
  waitingFor = null,
  tls = true,
  settings = SETTINGS,
  state = { mode: 'local', sharing: false },
  status = { state: 'idle', progress: 0, message: '' },
  edge = EDGE_NONE,
  storage = STORAGE,
  look = LOOK_PLAIN,
  market = MARKET_OFF,
  llm = { available: false },
  domains = DOMAINS_OFF,
  backup = BACKUP_SET,
  board = BOARD,
  health = true,
  apply = { job: 'losos-rebuild-7f3a' },
  reducedMotion = 'no-preference',
  overrides = {},
  pins = [],
  // Who GET /api/me says is signed in; null leaves it to the catch-all,
  // which reads as the spare key on a box that predates the field.
  me = null,
  // The address the page is served at. LosOS Login's cookie is https only,
  // so the wizard's last step is taken at the https one.
  at = AT,
  // LosOS Login answers: the wizard's sign-in takes the password, and from
  // then on GET /api/me without the key is Matus by the session.
  login = false,
} = {}) {
  const page = await browser.newPage({ viewport, locale, colorScheme: theme, deviceScaleFactor: 2, reducedMotion });
  await page.clock.setFixedTime(NOW);
  await page.addInitScript(
    ({ token, board, journal, theme, lang }) => {
      if (token) window.sessionStorage.setItem('losos-token', token);
      if (board) window.localStorage.setItem('losos-widgets', JSON.stringify(board));
      if (journal) window.localStorage.setItem('losos-uptime-journal', JSON.stringify(journal));
      window.localStorage.setItem('losos-theme', theme);
      if (lang) window.localStorage.setItem('losos-lang', lang);
    },
    { token: signedIn && claimed ? TOKEN : null, board: signedIn ? board : null, journal: journal(), theme, lang: locale.slice(0, 2) },
  );

  let current = look;
  // The page is served as if from the box's own name, so the address bar and
  // the About pane read as they do on a LAN, not as 127.0.0.1:port. Registered
  // FIRST: Playwright uses the last matching route, so every stub below wins.
  await page.route(`${at}/**`, async (route) => {
    const url = new URL(route.request().url());
    const response = await route.fetch({ url: origin + url.pathname + url.search });
    await route.fulfill({ response });
  });
  await page.route('**/api/**', (route) => json(route, 200, {}));
  await page.route('**/api/health', (route) => (health ? json(route, 200, { ok: true }) : route.abort()));
  await page.route('**/api/setup/claim', (route) => {
    if (route.request().method() === 'POST') {
      return json(route, claimed ? 409 : 200, claimed ? { error: 'this box has already been set up' } : { claimed: true, user: 'notshared', token: TOKEN });
    }
    return json(route, 200, { claimed, ready, waitingFor: ready ? null : waitingFor });
  });
  await page.route('**/setup/state.json', (route) =>
    json(route, 200, {
      hostName: 'mattbox',
      fqdn: 'mattbox.local',
      address: BOX_ADDRESS,
      tls,
      certificate: tls
        ? {
            url: '/setup/losos-ca.crt',
            fingerprint: 'sha256:' + '4f'.repeat(32),
            fingerprintDisplay: Array.from({ length: 32 }, (_, i) => ['4F', '9A', 'C2', '17', 'E8', '3B', 'D0', '6C'][i % 8]).join(':'),
            expires: '2028-10-06T00:00:00Z',
            install: { sh: '/setup/trust.sh', ps1: '/setup/trust.ps1' },
          }
        : null,
    }),
  );
  await page.route('**/api/state', (route) => {
    // A key that is not the box's is refused, as lososd refuses it.
    const auth = route.request().headers()['authorization'];
    return auth !== undefined && auth !== `Bearer ${TOKEN}` ? json(route, 401, { error: 'unauthorized' }) : json(route, 200, state);
  });
  if (me !== null) await page.route('**/api/me', (route) => json(route, 200, me));
  if (login) {
    let session = false;
    await page.route('**/auth/api/health', (route) => json(route, 200, { status: 'OK' }));
    await page.route('**/auth/api/firstfactor', (route) => {
      session = route.request().postDataJSON().password === PASSWORD;
      return json(route, session ? 200 : 401, { status: session ? 'OK' : 'KO' });
    });
    await page.route('**/api/me', (route) => {
      if (route.request().headers()['authorization'] !== undefined) {
        return json(route, 200, { name: null, displayName: null, groups: [], admin: true, spareKey: true, sections: [], standalone: true });
      }
      if (!session) return json(route, 401, { error: 'unauthorized' });
      return json(route, 200, { name: 'matus', displayName: 'Matus', groups: ['losos-admins', 'losos-owner'], admin: true, spareKey: false, sections: [], standalone: true, shared: [] });
    });
  }
  await page.route('**/api/people', (route) => json(route, 200, PEOPLE));
  await page.route('**/api/account', (route) => json(route, 200, ACCOUNT));
  await page.route('**/api/settings', (route) => json(route, 200, settings));
  await page.route('**/api/status', (route) => json(route, 200, status));
  await page.route('**/api/edge', (route) => json(route, 200, edge));
  await page.route('**/api/storage', (route) => json(route, 200, storage));
  await page.route('**/api/market', (route) => json(route, 200, market));
  await page.route('**/api/market/llm/models', (route) => json(route, 200, llm));
  await page.route('**/api/domains', (route) => json(route, 200, domains));
  await page.route('**/api/backup', (route) => json(route, 200, backup));
  await page.route('**/api/config', (route) => json(route, 200, CONFIG));
  await page.route('**/api/options', (route) => json(route, 200, options(overrides)));
  await page.route('**/api/site', (route) => json(route, 200, site(overrides)));
  await page.route('**/manage/api/boxes', (route) => json(route, 200, MANAGED_BOXES));
  let pinned = pins;
  await page.route('**/api/pins', (route) => {
    if (route.request().method() === 'PUT') pinned = route.request().postDataJSON().pins;
    return json(route, 200, { pins: pinned, limit: 24 });
  });
  await page.route('**/api/cloud/files?**', (route) => {
    const at = new URL(route.request().url()).searchParams.get('path') ?? '/';
    return json(route, 200, { available: true, path: at, query: null, entries: FOLDERS[at] ?? [], more: false });
  });
  await page.route('**/api/git/repos', (route) => json(route, 200, REPOS));
  await page.route('**/api/apply', (route) => json(route, 200, apply));
  await page.route('**/api/recovery', (route) => json(route, 200, { code: '7d1c3a52-9b0e-4f61-8c2d-5a7e9f0b1c3d', minted: true }));
  await page.route('**/api/apps', (route) => json(route, 200, APPS));
  await page.route('**/api/apps/search**', (route) => json(route, 200, RESULTS));
  await page.route('**/api/apps/chart**', (route) => json(route, 200, CHART));
  await page.route('**/api/look', (route) => {
    if (route.request().method() === 'GET') return json(route, 200, current);
    const patch = route.request().postDataJSON();
    current = { ...current, ...(patch.background ? { background: patch.background } : {}), ...(typeof patch.veil === 'number' ? { veil: patch.veil } : {}) };
    return json(route, 200, current);
  });
  // The app tiles probe LosOS Cloud and LosOS Git; both answer.
  await page.route('**/status.php', (route) => json(route, 200, { installed: true, productname: 'LosOS Cloud' }));
  await page.route('**/git/api/v1/version', (route) => route.fulfill({ status: 200, contentType: 'text/html', body: '<title>LosOS Git</title>' }));

  await page.goto(at + path, { waitUntil: 'networkidle' });
  await page.waitForTimeout(600);
  return page;
}

const settle = (page, ms = 500) => page.waitForTimeout(ms);
/* Notifications are drawn over the corner of a shot; hide them through the
 * CSSOM (the admin CSP refuses an injected <style>, as it should). */
const hideToasts = (page) =>
  page.evaluate(() => {
    for (const el of document.querySelectorAll('[data-sonner-toaster], [data-toast], ol[tabindex="-1"]')) el.style.display = 'none';
  });

/* The wizard's steps, each from a fresh page: Continue past the certificate,
 * then the password, then on to signing in. */
const wizardToPassword = async (page, t) => {
  await page.getByRole('button', { name: t('wizard.nav.continue') }).first().click();
  await settle(page);
};
const wizardSetPassword = async (page, t) => {
  await wizardToPassword(page, t);
  await page.locator('input[name="username"]').fill('matus');
  const inputs = page.locator('input[type="password"]');
  const n = await inputs.count();
  for (let i = 0; i < n; i++) await inputs.nth(i).fill(PASSWORD);
  await page.getByRole('button', { name: t('wizard.signin.set') }).first().click();
  await settle(page, 1500);
  await hideToasts(page);
};
const openPins = async (page, t) => {
  await page.getByRole('group', { name: t('pins.title') }).getByRole('button', { name: t('pins.edit') }).click();
  await settle(page);
};

/* The Overview's apps and disk card: the status line, the tiles and the disk
 * bar, without the board under it. */
const OVERVIEW_CARD = 'main [data-slot="card"]';

const SHOTS = [
  // ── The admin pages ──────────────────────────────────────────────────
  { name: 'overview', path: '/', langs: true },
  { name: 'overview-dark', path: '/', theme: 'dark', langs: true },
  { name: 'overview-phone', path: '/', viewport: { width: 390, height: 844 }, langs: true },
  {
    name: 'overview-phone-sheet',
    path: '/',
    viewport: { width: 390, height: 844 },
    langs: true,
    act: async (page) => {
      // More, the last tab of the bottom bar, raises every section as a sheet.
      await page.locator('[data-slot="bottom-nav"] button[aria-haspopup="dialog"]').click();
      await settle(page);
    },
  },
  { name: 'overview-not-answering', path: '/', health: false, crop: { down: OVERVIEW_CARD }, langs: true },
  { name: 'overview-almost-full', path: '/', storage: STORAGE_FULL, crop: { down: OVERVIEW_CARD }, langs: true },
  { name: 'overview-apply-failed', path: '/', status: { state: 'failed', progress: 100, job: '7', message: 'nixos-rebuild exited with status 1' }, crop: { down: OVERVIEW_CARD }, langs: true },
  { name: 'unlock-dialog', path: '/', signedIn: false, board: null, crop: 'dialog', langs: true },
  {
    name: 'unlock-spare-key',
    path: '/',
    signedIn: false,
    crop: 'dialog',
    langs: true,
    act: async (page, t) => {
      await page.getByRole('button', { name: t('shell.signIn.useKey') }).click();
      await settle(page);
    },
  },
  {
    name: 'unlock-refused',
    path: '/',
    signedIn: false,
    crop: 'dialog',
    langs: true,
    act: async (page, t) => {
      await page.getByRole('button', { name: t('shell.signIn.useKey') }).click();
      await page.locator('input#admin-key').fill('b'.repeat(64));
      await page.getByRole('button', { name: t('shell.signIn.unlock'), exact: true }).click();
      await settle(page, 800);
    },
  },
  { name: 'apps', path: '/apps', langs: true },
  {
    name: 'apps-find-more',
    path: '/apps',
    langs: true,
    act: async (page, t) => {
      const search = page.getByPlaceholder(t('panes.apps.searchPlaceholder'));
      await search.fill('media');
      await page.keyboard.press('Enter');
      await settle(page, 1000);
      // The results with the installed apps above them, not the app settings.
      await search.evaluate((el) => {
        el.closest('section').previousElementSibling.scrollIntoView({ block: 'start' });
        window.scrollBy(0, -72);
      });
      await settle(page);
    },
  },
  {
    name: 'apps-install',
    path: '/apps',
    langs: true,
    act: async (page, t) => {
      await page.getByPlaceholder(t('panes.apps.searchPlaceholder')).fill('media');
      await page.keyboard.press('Enter');
      await settle(page, 1000);
      await page.getByRole('button', { name: t('install.buttonLabel', { name: 'jellyfin' }) }).click();
      await page.locator('dialog[open] summary', { hasText: 'service' }).click();
      await settle(page, 800);
    },
  },
  /* The box-wide settings of both apps, with the trash emptied after two
   * weeks. */
  {
    name: 'apps-site-settings',
    path: '/apps',
    langs: true,
    act: async (page) => {
      for (const id of ['accordion-cloud', 'accordion-git']) await page.getByTestId(id).getByRole('button').first().click();
      await settle(page, 600);
      await page.evaluate(() => window.scrollTo(0, 0));
    },
    overrides: { 'cloud.trashDays': '14' },
    crop: { selector: '[data-testid="site-cloud"], [data-testid="site-git"]', pad: 8 },
  },
  { name: 'storage', path: '/storage', crop: 'fit', langs: true },
  { name: 'storage-shared', path: '/storage', storage: STORAGE_SHARED, state: { mode: 'mesh', sharing: true }, settings: { ...JOINED, sharingMyStorage: true }, crop: 'fit', langs: true },
  {
    name: 'storage-use-reserve',
    path: '/storage',
    crop: 'dialog',
    langs: true,
    act: async (page, t) => {
      await page.getByRole('button', { name: t('panes.storage.useReserve') }).click();
      await settle(page);
    },
  },
  { name: 'mesh-none', path: '/mesh', crop: 'fit', langs: true },
  { name: 'mesh-official', path: '/mesh', edge: EDGE_OFFICIAL, crop: 'fit', langs: true },
  { name: 'mesh-company', path: '/mesh', edge: EDGE_COMPANY, market: MARKET_UNOFFICIAL, crop: 'fit', langs: true },
  { name: 'mesh-two-edges', path: '/mesh', edge: EDGE_BOTH, settings: JOINED, crop: 'fit', langs: true },
  {
    name: 'mesh-warning-tooltip',
    path: '/mesh',
    edge: EDGE_COMPANY,
    market: MARKET_UNOFFICIAL,
    crop: { selector: 'main', pad: 8, through: 'main section' },
    langs: true,
    act: async (page) => {
      await page.locator('[data-edge-sign="warning"]').first().hover();
      await settle(page, 800);
    },
  },
  { name: 'market-soon', path: '/mesh', edge: EDGE_OFFICIAL },
  {
    name: 'mesh-manage',
    path: '/mesh',
    edge: EDGE_MANAGED,
    crop: { selector: 'section:has([data-option="edge-manage"])', pad: 16 },
    langs: true,
  },
  // ── A company edge's manage page ─────────────────────────────────────
  { name: 'edge-manage-boxes', path: '/manage/', crop: 'fit', langs: true },
  { name: 'edge-manage-box', path: '/manage/boxes/mattbox', full: true, crop: 'fit', langs: true },
  { name: 'settings-network', path: '/settings/network', crop: 'fit', langs: true },
  { name: 'settings-users', path: '/settings/users', me: ME_OWNER, full: true, crop: 'fit', langs: true },
  { name: 'settings-account', path: '/settings/account', me: ME_OWNER, full: true, crop: 'fit', langs: true },
  { name: 'settings-network-full', path: '/settings/network', full: true },
  { name: 'settings-domain-stripe', path: '/settings/network', full: true, edge: EDGE_OFFICIAL, domains: DOMAINS_UNVOUCHED },
  {
    name: 'settings-domain-ready',
    path: '/settings/network',
    full: true,
    edge: EDGE_OFFICIAL,
    settings: { ...SETTINGS, proxyEnable: true },
    domains: DOMAINS_READY,
  },
  {
    name: 'custom-domain',
    path: '/settings/network',
    edge: EDGE_OFFICIAL,
    settings: { ...SETTINGS, proxyEnable: true },
    domains: DOMAINS_READY,
    crop: { selector: '[data-testid="custom-domains"]', pad: 12 },
    langs: true,
  },
  {
    name: 'settings-apply-bar',
    path: '/settings/network',
    crop: 'fit',
    langs: true,
    act: async (page) => {
      await page.getByRole('textbox').first().fill('kitchenbox');
      await settle(page);
    },
  },
  {
    name: 'settings-apply-invalid',
    path: '/settings/network',
    crop: 'fit',
    langs: true,
    act: async (page) => {
      await page.getByRole('textbox').first().fill('kitchen box');
      await settle(page);
    },
  },
  { name: 'settings-hardware', path: '/settings/hardware' },
  { name: 'settings-security', path: '/settings/security', crop: 'fit', langs: true },
  { name: 'settings-look', path: '/settings/look', look: LOOK_TIDE, langs: true },
  { name: 'settings-about', path: '/settings/about', langs: true },
  /* The first-run tour, one shot per step, over the overview it opens on.
   * Under reduced motion each scene shows its composed still, so the
   * pictures do not depend on timing. `tour` is the first step again, in
   * every language, for the first-run page. */
  ...[1, 2, 3, 4, 5, 6].flatMap((step) =>
    [['', 'light'], ['-dark', 'dark'], ...(step === 1 ? [['-phone', 'light'], ['', 'light', 'tour']] : [])].map(([suffix, theme, name]) => ({
      name: name ?? `tour-${step}${suffix}`,
      path: '/',
      theme,
      ...(suffix === '-phone' ? { viewport: { width: 390, height: 844 } } : {}),
      ...(name ? { langs: true } : {}),
      reducedMotion: 'reduce',
      act: async (page, t) => {
        // As the wizard's Finish leaves it: the tour pending over the overview.
        await page.evaluate(() => window.localStorage.setItem('losos-tour', 'pending'));
        await page.reload({ waitUntil: 'networkidle' });
        const tour = page.getByTestId('tour');
        await tour.waitFor();
        if (step > 1) await tour.getByRole('button', { name: t('tour.step', { n: step }) }).click();
        await settle(page, 700);
        await hideToasts(page);
      },
    })),
  ),
  { name: 'settings-reset', path: '/settings/reset', langs: true },
  {
    name: 'settings-reset-dialog',
    path: '/settings/reset',
    crop: 'dialog',
    langs: true,
    act: async (page, t) => {
      await page.getByRole('button', { name: t('panes.reset.button') }).click();
      await settle(page);
    },
  },
  { name: 'settings-backup', path: '/settings/backup', langs: true },
  {
    name: 'settings-backup-glacier',
    path: '/settings/backup',
    langs: true,
    act: async (page, t) => {
      await page.getByTestId('backup-target').getByRole('button', { name: t('panes.backup.target.change') }).click();
      await page.getByTestId('backup-target-form').getByLabel(t('panes.backup.target.class')).selectOption('deepArchive');
      await settle(page);
    },
  },
  {
    name: 'settings-erase-dialog',
    path: '/settings/reset',
    langs: true,
    act: async (page, t) => {
      await page.getByRole('button', { name: t('panes.erase.button') }).click();
      await settle(page);
    },
  },
  { name: 'settings-erase-countdown', path: '/settings/reset', backup: BACKUP_COUNTDOWN, langs: true },
  { name: 'settings-advanced', path: '/settings/advanced', langs: true },
  { name: 'settings-history', path: '/settings/history', crop: 'fit', langs: true },
  {
    name: 'settings-search',
    path: '/settings/security',
    act: async (page, t) => {
      await page.getByRole('button', { name: t('palette.open', { shortcut: '' }).split('(')[0].trim() }).first().click();
      await page.getByPlaceholder(t('palette.placeholder')).fill('usb');
      await settle(page);
    },
  },
  { name: 'overview-tide', path: '/', look: LOOK_TIDE, board: { ...BOARD, widgets: [{ id: 'h', source: { kind: 'hand', id: 'w1' } }, ...BOARD.widgets.slice(0, 3)] }, langs: true },
  {
    name: 'widget-gallery',
    path: '/',
    board: { version: 1, widgets: [] },
    langs: true,
    act: async (page, t) => {
      await page.getByRole('button', { name: t('widgets.board.add') }).first().click();
      await settle(page, 800);
    },
  },
  {
    name: 'widget-build-one',
    path: '/',
    board: { version: 1, widgets: [] },
    langs: true,
    act: async (page, t) => {
      await page.getByRole('button', { name: t('widgets.board.add') }).first().click();
      await settle(page);
      await page.getByRole('button', { name: t('widgets.gallery.buildOne') }).first().click();
      await settle(page, 800);
    },
  },
  {
    name: 'widget-write-one',
    path: '/',
    look: LOOK_TIDE,
    board: { version: 1, widgets: [] },
    langs: true,
    act: async (page, t) => {
      await page.getByRole('button', { name: t('widgets.board.add') }).first().click();
      await settle(page);
      await page.getByRole('button', { name: t('look.gallery.writeOne') }).first().click();
      await settle(page, 1200);
    },
  },
  /* Pins: the sidebar with six, and the list open on a LosOS Cloud folder. */
  { name: 'pins-overview', path: '/', pins: PINS, langs: true },
  {
    name: 'pins-dialog',
    path: '/',
    pins: PINS,
    langs: true,
    act: async (page, t) => {
      await openPins(page, t);
      const dialog = page.locator('dialog[open]');
      await dialog.getByRole('tab', { name: t('pins.tab.files') }).click();
      await dialog.getByRole('button', { name: t('pins.files.open', { name: 'Documents' }) }).click();
      await settle(page, 800);
    },
  },
  { name: 'settings-hardware-sk-dark', path: '/settings/hardware', lang: 'sk', theme: 'dark' },
  { name: 'overview-de-dark', path: '/', lang: 'de', theme: 'dark' },
  { name: 'market-open', path: '/settings/market', edge: EDGE_OFFICIAL, market: MARKET_OPEN, settings: JOINED, langs: true },
  { name: 'market-slots', path: '/settings/market', edge: EDGE_OFFICIAL, market: MARKET_SLOTS, settings: JOINED, full: true },
  { name: 'market-ai', path: '/settings/market', edge: EDGE_OFFICIAL, market: MARKET_AI, llm: LLM, settings: JOINED, overrides: LENDING, full: true },
  {
    name: 'market-lend',
    path: '/settings/market',
    edge: EDGE_OFFICIAL,
    market: MARKET_AI,
    llm: LLM,
    settings: JOINED,
    overrides: LENDING,
    crop: { selector: '[data-testid="lend-memory"]', pad: 24 },
  },
  // ── The wizard ───────────────────────────────────────────────────────
  { name: 'wizard-step1-trust', path: '/', claimed: false, signedIn: false, full: true, crop: 'column', langs: true },
  { name: 'wizard-step1-trust-phone', path: '/', claimed: false, signedIn: false, viewport: { width: 390, height: 844 } },
  {
    name: 'wizard-step2-waiting',
    path: '/',
    claimed: false,
    signedIn: false,
    ready: false,
    waitingFor: 'LosOS Login is still starting',
    crop: 'column',
    langs: true,
    act: async (page, t) => {
      await wizardToPassword(page, t);
      await settle(page, 300);
    },
  },
  {
    name: 'wizard-step2-password',
    path: '/',
    claimed: false,
    signedIn: false,
    crop: 'column',
    langs: true,
    act: async (page, t) => {
      await wizardToPassword(page, t);
      await page.locator('input[name="username"]').fill('matus');
      await page.locator('input[type="password"]').first().fill('Correct-horse 1');
      // Typing scrolls the field into view; the shot starts at the top.
      await page.evaluate(() => window.scrollTo(0, 0));
      await settle(page);
    },
  },
  { name: 'wizard-step2-spare-key', path: '/', claimed: false, signedIn: false, full: true, crop: 'column', langs: true, act: wizardSetPassword },
  {
    // The last step: signed in, the desktop behind the card.
    name: 'wizard-step3-desktop',
    path: '/',
    claimed: false,
    signedIn: false,
    at: 'https://mattbox.local',
    login: true,
    reducedMotion: 'reduce',
    langs: true,
    act: async (page, t) => {
      await wizardSetPassword(page, t);
      await page.getByRole('button', { name: t('wizard.nav.continue') }).last().click();
      await page.getByTestId('desktop-ready').waitFor({ timeout: 10_000 });
      await settle(page, 1500);
      await hideToasts(page);
    },
  },
];

/* What part of the page a shot keeps, in CSS pixels:
 *   'fit'     the whole width, down to the lowest thing drawn, so a short
 *             pane carries no empty band under it
 *   'column'  the width of the centred column (the wizard), every row
 *   'dialog'  the open dialog and a margin of the page around it
 *   { selector, pad, through }   the box around every match, cut off under
 *             the first match of `through` when given
 *   { down }  the whole width, from the top to the bottom of the first match
 *   { clip }  a fixed rectangle */
async function region(page, crop, full) {
  if (!crop) return null;
  const view = page.viewportSize();
  if (crop.clip) return crop.clip;
  const pad = (box, by) => {
    const x = Math.max(0, Math.floor(box.x - by));
    const y = Math.max(0, Math.floor(box.y - by));
    return { x, y, width: Math.min(view.width, Math.ceil(box.x + box.width + by)) - x, height: Math.ceil(box.y + box.height + by) - y };
  };
  if (crop === 'dialog') return pad(await page.locator('dialog[open]').last().boundingBox(), 48);
  const drawn = await page.evaluate(() => {
    let left = Infinity;
    let right = 0;
    let bottom = 0;
    for (const el of document.body.querySelectorAll('*')) {
      const r = el.getBoundingClientRect();
      if (r.width < 2 || r.height < 2 || getComputedStyle(el).visibility === 'hidden') continue;
      // Wrappers as tall or as wide as the window say nothing about where
      // the page ends.
      if (r.height < window.innerHeight - 1) bottom = Math.max(bottom, r.bottom + window.scrollY);
      if (r.width < window.innerWidth - 1) {
        left = Math.min(left, r.left);
        right = Math.max(right, r.right);
      }
    }
    return { left, right, bottom, height: document.documentElement.scrollHeight };
  });
  if (crop.down) {
    const box = await page.locator(crop.down).first().boundingBox();
    return { x: 0, y: 0, width: view.width, height: Math.ceil(box.y + box.height) + 14 };
  }
  if (crop === 'fit') return { x: 0, y: 0, width: view.width, height: Math.min(Math.ceil(drawn.bottom) + 32, full ? drawn.height : view.height) };
  if (crop === 'column') {
    const x = Math.max(0, Math.floor(drawn.left) - 32);
    return { x, y: 0, width: Math.min(view.width, Math.ceil(drawn.right) + 32) - x, height: full ? drawn.height : view.height };
  }
  const boxes = (await Promise.all((await page.locator(crop.selector).all()).map((l) => l.boundingBox()))).filter(Boolean);
  const x = Math.min(...boxes.map((b) => b.x));
  const y = Math.min(...boxes.map((b) => b.y));
  let end = Math.max(...boxes.map((b) => b.y + b.height));
  if (crop.through) {
    const stop = await page.locator(crop.through).first().boundingBox();
    if (stop) end = Math.min(end, stop.y + stop.height);
  }
  const right = Math.max(...boxes.map((b) => b.x + b.width));
  return pad({ x, y, width: right - x, height: end - y }, crop.pad ?? 0);
}

for (const shot of SHOTS) {
  const { name, act, full = false, crop = null, langs = false, lang: only1 = 'en', perLang = () => ({}), ...options } = shot;
  for (const lang of langs ? ['en', 'sk', 'de'] : [only1]) {
    const file = langs && lang !== 'en' ? `${name}-${lang}` : name;
    if (only && !file.includes(only)) continue;
    const page = await open({ ...options, ...perLang(lang), locale: LOCALES[lang] });
    try {
      if (act) await act(page, T(lang));
      const clip = await region(page, crop, full);
      await page.screenshot({ path: `${outDir}/${file}.png`, fullPage: full || (clip ? clip.y + clip.height > page.viewportSize().height : false), ...(clip ? { clip } : {}) });
      console.log('  wrote', file);
    } catch (e) {
      console.log('  FAIL', file, e.message.split('\n')[0]);
    }
    await page.close();
  }
}

await browser.close();
await closeServer();
