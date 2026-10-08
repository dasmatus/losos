/* The handbook's pictures of the admin pages and the wizard, taken from a
 * built dist/ against a stubbed lososd, so every screen shows the current UI
 * and the current logo without a box.
 *
 *   npm run build && node tests/handbook-screens.mjs /tmp/handbook-screens [name]
 *
 * Each entry in SHOTS is one PNG: a path, a box state (the stub's answers),
 * a theme and a locale, and optionally what to click before the picture.
 * The stubbed answers are the shapes tests/*.browser.mjs use; nothing here is
 * read by the app in production. A second argument narrows the run to the
 * names that contain it. The PNGs are 2x; the handbook carries them
 * downscaled (handbook/docs/reference/for-developers.md, "Pictures"). */

import { mkdirSync } from 'node:fs';
import { join } from 'node:path';
import { readFile } from 'node:fs/promises';
import { launch, serve } from './harness.mjs';

const [outDir = '/tmp/handbook-screens', only = ''] = process.argv.slice(2);
mkdirSync(outDir, { recursive: true });

const TOKEN = '7c1e4a9d02f6b38e5d71c0a4e9b2f86d3a5c17e0b94d62f8c3e1a07d5b9f4c26'; // a fixture, not a key
const PASSWORD = 'Correct-horse battery staple 1';
const BOX_ADDRESS = '192.168.1.23';
const AT = 'http://mattbox.local';
const NOW = Date.UTC(2026, 9, 8, 9, 30); // 2026-10-08 09:30Z, so dates in shots are stable

const SETTINGS = {
  sharingMyStorage: false,
  nextcloudMode: 'container',
  forgejoMode: 'container',
  hostName: 'mattbox',
  https: true,
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

const STORAGE = { totalBytes: 480 * 1024 ** 3, usedBytes: 131 * 1024 ** 3, lentBytes: 0, reserveBytes: 48 * 1024 ** 3 };
const STORAGE_SHARED = { ...STORAGE, lentBytes: 96 * 1024 ** 3 };
const STORAGE_FULL = { ...STORAGE, usedBytes: 421 * 1024 ** 3 };

const LIMITS = { widgets: 24, nameChars: 60, sourceBytes: 65536, imageBytes: 8388608, veil: { min: 20, max: 90 } };
const HAND_WIDGET = {
  id: 'w1',
  name: 'Greeting',
  span: 'half',
  source: [
    '<style>',
    '  body { font-family: system-ui, sans-serif; margin: 0; color: var(--ink); }',
    '  .big { font-size: 28px; font-weight: 600; color: var(--accent); }',
    '</style>',
    '<div class="big" id="out">…</div>',
    '<div id="free"></div>',
    '<script>',
    '  losos.metric("box.settings").then((m) => {',
    '    document.getElementById("out").textContent = "Hello from " + m.hostName;',
    '  });',
    '  document.getElementById("free").textContent = "Drawn in its own sandbox, in the " + losos.theme + " theme.";',
    '</script>',
  ].join('\n'),
};
const LOOK_PLAIN = { version: 1, background: { kind: 'none' }, veil: 60, widgets: [], shipped: ['tide', 'grid', 'dusk'], limits: LIMITS };
const LOOK_TIDE = { ...LOOK_PLAIN, background: { kind: 'shipped', name: 'tide' }, widgets: [HAND_WIDGET] };

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

const CONFIG = {
  enabled: true,
  repository: {
    owner: 'notshared',
    name: 'losos-config',
    url: '/forgejo/notshared/losos-config',
    clone: 'http://mattbox.local/forgejo/notshared/losos-config.git',
    viaForgejo: true,
  },
  head: { sha: 'b'.repeat(40), branch: 'main' },
  log: [
    { sha: 'b'.repeat(40), when: '2026-10-07T10:12:00+02:00', subject: 'Change hostName, cluster.enable' },
    { sha: 'c'.repeat(40), when: '2026-10-06T18:40:00+02:00', subject: 'Change hardening.usbguard' },
    { sha: 'a'.repeat(40), when: '2026-10-06T03:00:00+02:00', subject: 'losos install' },
  ],
  sync: { state: 'ok', detail: 'In step with LosOS Git.', syncedAt: '2026-10-07T10:12:30+02:00', remoteHead: 'b'.repeat(40) },
};

const RESULTS = {
  sources: ['nixpkgs', 'flathub'],
  results: [
    { id: 'jellyfin', name: 'Jellyfin', version: '10.11.0', summary: 'A media server for your films and music.', source: 'nixpkgs', homepage: 'https://jellyfin.org' },
    { id: 'immich', name: 'Immich', version: '2.4.1', summary: 'Photo and video backup from your phone.', source: 'nixpkgs', homepage: 'https://immich.app' },
    { id: 'vaultwarden', name: 'Vaultwarden', summary: 'A password manager server.', source: 'flathub' },
  ],
};

const DOC = JSON.parse(await readFile('tests/fixtures/options.json', 'utf8'));
const OPTIONS = {
  available: true,
  version: DOC.version,
  options: DOC.options.map((option) => ({ ...option, set: null })),
  stray: [],
  excluded: DOC.excluded,
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
    { job: 'losos-rebuild-2b9e', state: 'failed', at: NOW - 3 * 86400_000, message: 'error: attribute "losos.nextcloud.port" is read-only' },
  ],
};

const json = (route, status, body) => route.fulfill({ status, contentType: 'application/json', body: JSON.stringify(body) });

const { origin, close: closeServer } = await serve({ csp: true });
const browser = await launch();

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
  board = BOARD,
  health = true,
  cloudDown = false,
  apply = { job: 'losos-rebuild-7f3a' },
} = {}) {
  const page = await browser.newPage({ viewport, locale, colorScheme: theme, deviceScaleFactor: 2 });
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
  await page.route(`${AT}/**`, async (route) => {
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
  await page.route('**/api/state', (route) => json(route, 200, state));
  await page.route('**/api/settings', (route) => json(route, 200, settings));
  await page.route('**/api/status', (route) => json(route, 200, status));
  await page.route('**/api/edge', (route) => json(route, 200, edge));
  await page.route('**/api/storage', (route) => json(route, 200, storage));
  await page.route('**/api/market', (route) => json(route, 200, market));
  await page.route('**/api/config', (route) => json(route, 200, CONFIG));
  await page.route('**/api/options', (route) => json(route, 200, OPTIONS));
  await page.route('**/api/apply', (route) => json(route, 200, apply));
  await page.route('**/api/recovery', (route) => json(route, 200, { code: '7d1c3a52-9b0e-4f61-8c2d-5a7e9f0b1c3d', minted: true }));
  await page.route('**/api/apps/search**', (route) => json(route, 200, RESULTS));
  await page.route('**/api/look', (route) => {
    if (route.request().method() === 'GET') return json(route, 200, current);
    const patch = route.request().postDataJSON();
    current = { ...current, ...(patch.background ? { background: patch.background } : {}), ...(typeof patch.veil === 'number' ? { veil: patch.veil } : {}) };
    return json(route, 200, current);
  });
  await page.route('**/api/sign-in', (route) => {
    if (cloudDown) return json(route, 503, { error: 'LosOS cloud did not answer', ready: false, waitingFor: 'LosOS cloud did not answer' });
    const { password } = route.request().postDataJSON() ?? {};
    return password === PASSWORD ? json(route, 200, { user: 'notshared', token: TOKEN }) : json(route, 401, { error: 'that password was not accepted' });
  });
  // The app tiles probe LosOS cloud and LosOS Git; both answer.
  await page.route('**/nextcloud/status.php', (route) => json(route, 200, { installed: true, version: '32.0.1.2', productname: 'LosOS cloud' }));
  await page.route('**/forgejo/', (route) => route.fulfill({ status: 200, contentType: 'text/html', body: '<title>LosOS Git</title>' }));
  // The wizard's frame: what Nextcloud's <head> carries once the owner is
  // signed in, over an empty body. The shot that uses it is clipped above
  // the frame, so nothing of the app is drawn, let alone invented.
  await page.route(/\/nextcloud(\/.*)?$/, (route) =>
    route.fulfill({ status: 200, contentType: 'text/html', body: '<!doctype html><html><head data-requesttoken="x" data-user="notshared"></head><body style="margin:0;background:#f4f6f6"></body></html>' }),
  );

  await page.goto(AT + path, { waitUntil: 'networkidle' });
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

const SHOTS = [
  // ── The admin pages ──────────────────────────────────────────────────
  { name: 'overview', path: '/' },
  { name: 'overview-dark', path: '/', theme: 'dark' },
  { name: 'overview-phone', path: '/', viewport: { width: 390, height: 844 } },
  {
    name: 'overview-phone-sheet',
    path: '/',
    viewport: { width: 390, height: 844 },
    act: async (page) => {
      await page.getByRole('button', { name: /sections|menu|sidebar/i }).first().click();
      await settle(page);
    },
  },
  { name: 'overview-not-answering', path: '/', health: false, status: { state: 'idle', progress: 0, message: '' } },
  { name: 'overview-almost-full', path: '/', storage: STORAGE_FULL },
  { name: 'overview-apply-failed', path: '/', status: { state: 'failed', progress: 100, job: '7', message: 'nixos-rebuild exited with status 1' } },
  { name: 'unlock-dialog', path: '/', signedIn: false, board: null },
  {
    name: 'unlock-dialog-cloud-down',
    path: '/',
    signedIn: false,
    cloudDown: true,
    act: async (page) => {
      await page.locator('input#owner-password').fill(PASSWORD);
      await page.getByRole('button', { name: 'Unlock' }).click();
      await settle(page, 800);
    },
  },
  {
    name: 'unlock-spare-key',
    path: '/',
    signedIn: false,
    act: async (page) => {
      await page.getByRole('button', { name: /Use the spare admin key instead/ }).click();
      await settle(page);
    },
  },
  {
    name: 'unlock-refused',
    path: '/',
    signedIn: false,
    act: async (page) => {
      await page.locator('input#owner-password').fill('Wrong-horse battery staple 1');
      await page.getByRole('button', { name: 'Unlock' }).click();
      await settle(page, 800);
    },
  },
  { name: 'apps', path: '/apps' },
  {
    name: 'apps-find-more',
    path: '/apps',
    act: async (page) => {
      await page.getByPlaceholder('Search for an app').fill('media');
      await page.keyboard.press('Enter');
      await settle(page, 1000);
    },
  },
  { name: 'storage', path: '/storage' },
  { name: 'storage-shared', path: '/storage', storage: STORAGE_SHARED, state: { mode: 'mesh', sharing: true }, settings: { ...JOINED, sharingMyStorage: true } },
  {
    name: 'storage-use-reserve',
    path: '/storage',
    act: async (page) => {
      await page.getByRole('button', { name: /Use reserve/ }).click();
      await settle(page);
    },
  },
  { name: 'mesh-none', path: '/mesh' },
  { name: 'mesh-official', path: '/mesh', edge: EDGE_OFFICIAL },
  { name: 'mesh-company', path: '/mesh', edge: EDGE_COMPANY, market: MARKET_UNOFFICIAL },
  { name: 'mesh-two-edges', path: '/mesh', edge: EDGE_BOTH, settings: JOINED },
  {
    name: 'mesh-warning-tooltip',
    path: '/mesh',
    edge: EDGE_COMPANY,
    market: MARKET_UNOFFICIAL,
    act: async (page) => {
      await page.locator('[data-edge-sign="warning"]').first().hover();
      await settle(page, 800);
    },
  },
  { name: 'market-soon', path: '/mesh', edge: EDGE_OFFICIAL },
  { name: 'settings-network', path: '/settings/network' },
  {
    name: 'settings-apply-bar',
    path: '/settings/network',
    act: async (page) => {
      await page.getByRole('textbox').first().fill('kitchenbox');
      await settle(page);
    },
  },
  {
    name: 'settings-apply-invalid',
    path: '/settings/network',
    act: async (page) => {
      await page.getByRole('textbox').first().fill('kitchen box');
      await settle(page);
    },
  },
  { name: 'settings-hardware', path: '/settings/hardware' },
  { name: 'settings-security', path: '/settings/security' },
  { name: 'settings-look', path: '/settings/look', look: LOOK_TIDE },
  { name: 'settings-about', path: '/settings/about' },
  { name: 'settings-reset', path: '/settings/reset' },
  {
    name: 'settings-reset-dialog',
    path: '/settings/reset',
    act: async (page) => {
      await page.getByRole('button', { name: /Reset/ }).last().click();
      await settle(page);
    },
  },
  { name: 'settings-advanced', path: '/settings/advanced' },
  { name: 'settings-history', path: '/settings/history' },
  {
    name: 'settings-search',
    path: '/settings/security',
    act: async (page) => {
      await page.getByPlaceholder(/search|hľadať/i).first().fill('usb');
      await settle(page);
    },
  },
  { name: 'overview-tide', path: '/', look: LOOK_TIDE, board: { ...BOARD, widgets: [{ id: 'h', source: { kind: 'hand', id: 'w1' } }, ...BOARD.widgets.slice(0, 3)] } },
  {
    name: 'widget-gallery',
    path: '/',
    board: { version: 1, widgets: [] },
    act: async (page) => {
      await page.getByRole('button', { name: /Add a widget/ }).first().click();
      await settle(page, 800);
    },
  },
  {
    name: 'widget-build-one',
    path: '/',
    board: { version: 1, widgets: [] },
    act: async (page) => {
      await page.getByRole('button', { name: /Add a widget/ }).first().click();
      await settle(page);
      await page.getByRole('button', { name: /Build one/ }).first().click();
      await settle(page, 800);
    },
  },
  {
    name: 'widget-write-one',
    path: '/',
    look: LOOK_TIDE,
    board: { version: 1, widgets: [] },
    act: async (page) => {
      await page.getByRole('button', { name: /Add a widget/ }).first().click();
      await settle(page);
      await page.getByRole('button', { name: /Write one/ }).first().click();
      await settle(page, 1200);
    },
  },
  { name: 'settings-hardware-sk-dark', path: '/settings/hardware', locale: 'sk-SK', theme: 'dark' },
  { name: 'overview-sk', path: '/', locale: 'sk-SK' },
  { name: 'mesh-two-edges-sk', path: '/mesh', edge: EDGE_BOTH, settings: JOINED, locale: 'sk-SK' },
  { name: 'unlock-dialog-sk', path: '/', signedIn: false, board: null, locale: 'sk-SK' },
  { name: 'overview-de-dark', path: '/', locale: 'de-DE', theme: 'dark' },
  {
    name: 'settings-search-sk',
    path: '/settings/security',
    locale: 'sk-SK',
    act: async (page) => {
      await page.getByPlaceholder(/search|hľadať/i).first().fill('usb');
      await settle(page);
    },
  },
  /* The Market pane is planned (panes.ts), so its route falls back. This one
   * runs only against a throwaway build with `planned: false`, for a
   * picture of the pane as it will open: LOSOS_SHOOT_MARKET=1. */
  { name: 'market-open', path: '/settings/market', edge: EDGE_OFFICIAL, market: MARKET_OPEN, settings: JOINED, needsMarket: true },
  { name: 'market-open-sk', path: '/settings/market', locale: 'sk-SK', edge: EDGE_OFFICIAL, market: MARKET_OPEN, settings: JOINED, needsMarket: true },
  // ── The wizard ───────────────────────────────────────────────────────
  { name: 'wizard-step1-trust', path: '/', claimed: false, signedIn: false, full: true },
  { name: 'wizard-step1-trust-sk', path: '/', claimed: false, signedIn: false, locale: 'sk-SK' },
  { name: 'wizard-step1-trust-phone', path: '/', claimed: false, signedIn: false, viewport: { width: 390, height: 844 } },
  {
    name: 'wizard-step2-waiting',
    path: '/',
    claimed: false,
    signedIn: false,
    ready: false,
    waitingFor: 'LosOS cloud is still installing itself',
    act: async (page) => {
      await page.getByRole('button', { name: /next|continue|skip|pokračovať/i }).first().click();
      await settle(page, 800);
    },
  },
  {
    name: 'wizard-step2-password',
    path: '/',
    claimed: false,
    signedIn: false,
    act: async (page) => {
      await page.getByRole('button', { name: /next|continue|skip|pokračovať/i }).first().click();
      await settle(page);
      const pw = page.locator('input[type="password"]').first();
      await pw.fill('Correct-horse 1');
      await settle(page);
    },
  },
  {
    name: 'wizard-step2-spare-key',
    path: '/',
    claimed: false,
    signedIn: false,
    act: async (page) => {
      await page.getByRole('button', { name: /next|continue|skip|pokračovať/i }).first().click();
      await settle(page);
      const inputs = page.locator('input[type="password"]');
      const n = await inputs.count();
      for (let i = 0; i < n; i++) await inputs.nth(i).fill(PASSWORD);
      await page.getByRole('button', { name: /set the password|set password|continue|next|nastaviť heslo/i }).first().click();
      await settle(page, 1500);
      await hideToasts(page);
    },
    full: true,
  },
  {
    name: 'wizard-step2-spare-key-sk',
    locale: 'sk-SK',
    path: '/',
    claimed: false,
    signedIn: false,
    act: async (page) => {
      await page.getByRole('button', { name: /next|continue|skip|pokračovať/i }).first().click();
      await settle(page);
      const inputs = page.locator('input[type="password"]');
      const n = await inputs.count();
      for (let i = 0; i < n; i++) await inputs.nth(i).fill(PASSWORD);
      await page.getByRole('button', { name: /set the password|set password|continue|next|nastaviť heslo/i }).first().click();
      await settle(page, 1500);
      await hideToasts(page);
    },
    full: true,
  },
  {
    name: 'wizard-step3-sign-in',
    clip: { x: 300, y: 0, width: 680, height: 200 },
    path: '/',
    claimed: false,
    signedIn: false,
    act: async (page) => {
      await page.getByRole('button', { name: /next|continue|skip|pokračovať/i }).first().click();
      await settle(page);
      const inputs = page.locator('input[type="password"]');
      const n = await inputs.count();
      for (let i = 0; i < n; i++) await inputs.nth(i).fill(PASSWORD);
      await page.getByRole('button', { name: /set the password|set password|continue|next|nastaviť heslo/i }).first().click();
      await settle(page, 1200);
      await page.getByRole('button', { name: /next|continue|sign in|pokračovať|prihlásiť/i }).last().click();
      await settle(page, 2500);
      await hideToasts(page);
    },
  },
  {
    name: 'wizard-step3-sign-in-sk',
    locale: 'sk-SK',
    clip: { x: 300, y: 0, width: 680, height: 200 },
    path: '/',
    claimed: false,
    signedIn: false,
    act: async (page) => {
      await page.getByRole('button', { name: /next|continue|skip|pokračovať/i }).first().click();
      await settle(page);
      const inputs = page.locator('input[type="password"]');
      const n = await inputs.count();
      for (let i = 0; i < n; i++) await inputs.nth(i).fill(PASSWORD);
      await page.getByRole('button', { name: /set the password|set password|continue|next|nastaviť heslo/i }).first().click();
      await settle(page, 1200);
      await page.getByRole('button', { name: /next|continue|sign in|pokračovať|prihlásiť/i }).last().click();
      await settle(page, 2500);
      await hideToasts(page);
    },
  },
];

for (const shot of SHOTS) {
  if (only && !shot.name.includes(only)) continue;
  const { name, act, full = false, clip = null, needsMarket = false, ...options } = shot;
  if (needsMarket && !process.env.LOSOS_SHOOT_MARKET) continue;
  const page = await open(options);
  try {
    if (act) await act(page);
    await page.screenshot({ path: join(outDir, `${name}.png`), fullPage: full, ...(clip ? { clip } : {}) });
    console.log('  wrote', name);
  } catch (e) {
    console.log('  FAIL', name, e.message.split('\n')[0]);
  }
  await page.close();
}

await browser.close();
await closeServer();
