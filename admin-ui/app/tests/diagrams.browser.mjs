/* LosOS Diagrams' canvas view (/diagrams/), the page's default, under its
 * own policy.
 *
 * The real dist/ bundle and the real core (src/lab/core-pkg), served with
 * LAB_CSP the way nginx serves /diagrams/. Every page fails on a CSP
 * violation and on an uncaught error. Walked once each: cards written in
 * Markdown, shapes, arrows and undo, a template, a JSON Canvas file from
 * Obsidian opened and saved with the keys this page does not know, the SVG
 * and PNG pictures, a diagram opened from LosOS Cloud and saved back with
 * the version it read, the machine cards beside a fake /api/vms, the
 * chrome in Slovak and German, and the phone layout. LosOS Cloud is a fake
 * in memory behind /cloud/api, as backend/src/cloud/api.rs answers.
 */

import assert from 'node:assert';
import { readFile } from 'node:fs/promises';
import { launch, runner, serve, shellRoutes } from './harness.mjs';
import { yroom } from './yroom.mjs';

const TOKEN = 'c'.repeat(64);
const { origin, close: closeServer } = await serve({ csp: true });
const browser = await launch();
const { check, finish } = runner();

const json = (route, status, body, headers = {}) =>
  route.fulfill({ status, contentType: 'application/json', headers, body: JSON.stringify(body) });

const SETTINGS = { hostName: 'mattbox', sharingMyStorage: false, cluster: { enable: false } };
const EDGE = { path: 'none', edges: [] };

async function open({
  viewport = { width: 1440, height: 900 },
  locale = 'en-US',
  signedIn = true,
  scheme = 'light',
  routes = null,
  query = '',
} = {}) {
  const page = await browser.newPage({ viewport, locale, colorScheme: scheme, acceptDownloads: true });
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  await page.addInitScript(() => {
    window.__violations = [];
    document.addEventListener('securitypolicyviolation', (e) =>
      window.__violations.push(`${e.violatedDirective} ${e.blockedURI}`),
    );
  });
  if (signedIn) await page.addInitScript((t) => window.sessionStorage.setItem('losos-token', t), TOKEN);
  const authed = (route) => route.request().headers()['authorization'] === `Bearer ${TOKEN}`;
  await shellRoutes(page);
  await page.route('**/api/settings', (route) => (authed(route) ? json(route, 200, SETTINGS) : json(route, 401, {})));
  await page.route('**/api/edge', (route) => (authed(route) ? json(route, 200, EDGE) : json(route, 401, {})));
  await page.route('**/api/vms', (route) => json(route, 200, { available: false, reason: 'notSharing' }));
  // No LosOS Cloud unless a check stands one up.
  await page.route('**/cloud/api/**', (route) => json(route, 401, { error: 'signIn', code: 'signIn' }));
  if (routes) await routes(page, authed);
  await page.goto(origin + '/diagrams/?canvas=svg' + query);
  await page.getByTestId('lab').waitFor({ timeout: 15000 });
  await page.getByTestId('lab-splash').waitFor({ state: 'detached', timeout: 15000 });
  /* Sonner's refused copy of its own stylesheet is logged at load, as on
   * every admin page (tests/app.browser.mjs); its rules come from the
   * bundle. Nothing after that may add a violation. */
  const atLoad = await page.evaluate(() => window.__violations);
  assert.ok(atLoad.length <= 2 && atLoad.every((v) => v === 'style-src-elem inline'), `CSP violations at load: ${atLoad.join(', ')}`);
  const clean = async () => {
    const violations = (await page.evaluate(() => window.__violations)).slice(atLoad.length);
    assert.deepStrictEqual(violations, [], `CSP violations: ${violations.join(', ')}`);
    assert.deepStrictEqual(errors, [], `page errors: ${errors.join(' | ')}`);
  };
  return { page, clean };
}

const nodes = (page) => page.locator('[data-testid=diagram] [data-node]');
const edges = (page) => page.locator('[data-testid=diagram] [data-edge]');
const card = (page, label) => page.locator(`[data-testid=diagram] [data-node][aria-label="${label}"]`);

async function fresh(page) {
  await page.getByTestId('file-menu').click();
  await page.getByTestId('file-new').click();
  await page.getByTestId('diagram-empty').waitFor();
}

/** A point on the canvas, from its top-left corner. */
async function at(page, x, y) {
  const box = await page.getByTestId('diagram-wrap').boundingBox();
  return { x: box.x + x, y: box.y + y };
}

async function download(page, testid) {
  await page.getByTestId('file-menu').click();
  const [d] = await Promise.all([page.waitForEvent('download'), page.getByTestId(testid).click()]);
  return { name: d.suggestedFilename(), path: await d.path() };
}

await check('opens on the diagram under its CSP, the network drawn as cards', async () => {
  const { page, clean } = await open();
  await page.getByTestId('diagram').waitFor();
  assert.strictEqual(await page.getByTestId('view-canvas').getAttribute('aria-current'), 'page');
  assert.match(await page.title(), /LosOS Diagrams/);
  assert.ok((await page.locator('[data-testid=diagram] .dg-node.device').count()) >= 3, 'this box is on the canvas');
  assert.ok((await edges(page).count()) >= 3, 'its cables are arrows');
  await clean();
  await page.close();
});

await check('a double-click writes a card, and its Markdown is drawn, one newline a line break', async () => {
  const { page, clean } = await open();
  await fresh(page);
  const p = await at(page, 300, 260);
  await page.mouse.dblclick(p.x, p.y);
  const editor = page.getByRole('textbox', { name: 'Card text' });
  await editor.waitFor();
  await editor.fill('# Launch plan\n\n- [x] film\n- [ ] post\n\n**Friday** at ==noon==, see [the site](https://losos.dasmat.us)\nthen lunch');
  await page.keyboard.press('Escape');
  const body = page.locator('[data-testid=diagram] [data-node-body]').first();
  await body.locator('h1', { hasText: 'Launch plan' }).waitFor();
  assert.strictEqual(await body.locator('[data-done]').count(), 1, 'one task is done');
  assert.strictEqual(await body.locator('strong').innerText(), 'Friday');
  assert.strictEqual(await body.locator('mark').innerText(), 'noon');
  assert.strictEqual(await body.locator('a').getAttribute('href'), 'https://losos.dasmat.us');
  assert.match(await body.locator('p').last().innerText(), /the site\nthen lunch$/, 'Obsidian shows one newline as a line break');
  assert.strictEqual(await card(page, 'Launch plan').count(), 1, 'the card is named by its first line');
  await clean();
  await page.close();
});

await check('cards from the tray, an arrow between them, undo and redo', async () => {
  const { page, clean } = await open();
  await fresh(page);
  await page.locator('[data-testid=tray-items] [data-card=diamond]').click();
  let p = await at(page, 130, 260);
  await page.mouse.click(p.x, p.y);
  await page.keyboard.type('Ready?');
  await page.keyboard.press('Escape');
  await page.locator('[data-testid=tray-items] [data-card=pill]').click();
  p = await at(page, 400, 260);
  await page.mouse.click(p.x, p.y);
  await page.keyboard.type('Ship it');
  await page.keyboard.press('Escape');
  assert.strictEqual(await nodes(page).count(), 2);

  // select the diamond, then drag from its right handle onto the pill
  await card(page, 'Ready?').click();
  const id = await card(page, 'Ready?').getAttribute('data-node');
  const handle = await page.locator(`[data-handle=right][data-handle-node="${id}"]`).boundingBox();
  const target = await card(page, 'Ship it').boundingBox();
  await page.mouse.move(handle.x + handle.width / 2, handle.y + handle.height / 2);
  await page.mouse.down();
  await page.mouse.move(target.x + target.width / 2, target.y + target.height / 2, { steps: 8 });
  await page.mouse.up();
  await page.waitForFunction(() => document.querySelectorAll('[data-testid=diagram] [data-edge]').length === 1);

  // the arrow's look from the inspector
  await page.locator('[data-testid=diagram] [data-edge]').first().click({ force: true });
  await page.getByTestId('edge-label').fill('yes');
  await page.getByTestId('edge-label').press('Enter');
  await page.locator('[data-testid=diagram] [data-edge]', { hasText: 'yes' }).waitFor();

  await page.getByTestId('undo').click();
  await page.getByTestId('undo').click();
  await page.waitForFunction(() => document.querySelectorAll('[data-testid=diagram] [data-edge]').length === 0);
  await page.getByTestId('redo').click();
  await page.waitForFunction(() => document.querySelectorAll('[data-testid=diagram] [data-edge]').length === 1);
  await page.keyboard.press('Control+z');
  await page.waitForFunction(() => document.querySelectorAll('[data-testid=diagram] [data-edge]').length === 0);
  await clean();
  await page.close();
});

await check('a template lands where the click is', async () => {
  const { page, clean } = await open();
  await fresh(page);
  await page.getByRole('button', { name: 'Templates', exact: true }).click();
  await page.locator('[data-testid=tray-items] [data-template=flowchart]').click();
  const p = await at(page, 500, 350);
  await page.mouse.click(p.x, p.y);
  await page.waitForFunction(() => document.querySelectorAll('[data-testid=diagram] [data-node]').length >= 4);
  assert.ok((await edges(page).count()) >= 3, 'the flowchart has its arrows');
  assert.strictEqual(await page.getByTestId('diagram-empty').count(), 0);
  await clean();
  await page.close();
});

/* What Obsidian writes, with keys from plugins this page has never heard
 * of, on the document, a card and an arrow. */
const OBSIDIAN = {
  nodes: [
    { id: 'a1', type: 'text', text: '## Inbox\nsort on Monday', x: -200, y: -80, width: 260, height: 120, color: '4', styleAttributes: { shape: 'pill', border: 'dashed' }, 'x-plugin-pin': true },
    { id: 'b2', type: 'link', url: 'https://obsidian.md', x: 160, y: -80, width: 300, height: 120 },
    { id: 'g0', type: 'group', label: 'This week', x: -240, y: -140, width: 760, height: 260, color: '#2a9d8f' },
  ],
  edges: [{ id: 'e1', fromNode: 'a1', fromSide: 'right', toNode: 'b2', toSide: 'left', toEnd: 'arrow', label: 'read', styleAttributes: { path: 'dotted' }, 'x-plugin-weight': 3 }],
  'x-plugin-doc': { version: 2 },
};

await check('an Obsidian Canvas file opens, and saves with every key it came with', async () => {
  const { page, clean } = await open();
  await page.getByTestId('open-file').setInputFiles({ name: 'week.canvas', mimeType: 'application/json', buffer: Buffer.from(JSON.stringify(OBSIDIAN)) });
  await card(page, 'Inbox').waitFor();
  assert.strictEqual(await nodes(page).count(), 3);
  assert.strictEqual(await edges(page).count(), 1);
  await page.locator('[data-testid=diagram] [data-group-label]', { hasText: 'This week' }).waitFor();

  const saved = await download(page, 'file-download');
  assert.strictEqual(saved.name, 'week.canvas');
  const doc = JSON.parse(await readFile(saved.path, 'utf8'));
  const a1 = doc.nodes.find((n) => n.id === 'a1');
  assert.strictEqual(a1['x-plugin-pin'], true, 'a card keeps its unknown key');
  assert.deepStrictEqual(a1.styleAttributes, { shape: 'pill', border: 'dashed' });
  assert.strictEqual(a1.color, '4');
  assert.strictEqual(doc.nodes.find((n) => n.id === 'g0').color, '#2a9d8f');
  const e1 = doc.edges.find((e) => e.id === 'e1');
  assert.strictEqual(e1['x-plugin-weight'], 3, 'an arrow keeps its unknown key');
  assert.strictEqual(e1.label, 'read');
  assert.deepStrictEqual(doc['x-plugin-doc'], { version: 2 }, 'the document keeps its unknown key');
  await clean();
  await page.close();
});

await check('the picture comes out as SVG and as PNG', async () => {
  const { page, clean } = await open();
  await page.getByTestId('open-file').setInputFiles({ name: 'week.canvas', mimeType: 'application/json', buffer: Buffer.from(JSON.stringify(OBSIDIAN)) });
  await card(page, 'Inbox').waitFor();
  const svg = await download(page, 'export-svg');
  assert.strictEqual(svg.name, 'week.svg');
  const text = await readFile(svg.path, 'utf8');
  assert.match(text, /^<svg[^>]+xmlns="http:\/\/www.w3.org\/2000\/svg"/);
  assert.match(text, /Inbox/);
  assert.doesNotMatch(text, /var\(--/, 'colours are resolved, not tokens');
  const png = await download(page, 'export-png');
  assert.strictEqual(png.name, 'week.png');
  const bytes = await readFile(png.path);
  assert.strictEqual(bytes.subarray(1, 4).toString(), 'PNG');
  assert.ok(bytes.length > 2000, `a real picture: ${bytes.length} bytes`);
  await clean();
  await page.close();
});

/* LosOS Cloud in memory: a folder with a diagram and a note, ETags that
 * change on every write, ifMatch and ifNew as the real API checks them. */
function fakeCloud() {
  const files = new Map([
    ['Diagrams/plan.canvas', { text: JSON.stringify(OBSIDIAN), etag: 'v1' }],
    ['Diagrams/notes.md', { text: '# notes', etag: 'n1' }],
  ]);
  const puts = [];
  let n = 1;
  const ids = new Map([...files.keys()].map((p, i) => [p, 7 + i]));
  const entry = (path) => ({ id: ids.get(path) ?? 99, path, name: path.slice(path.lastIndexOf('/') + 1), dir: false, size: files.get(path).text.length, mtime: 0, etag: files.get(path).etag });
  const routes = async (page) => {
    await page.route('**/cloud/api/me', (route) => json(route, 200, { login: 'matus', name: 'Matus' }));
    await page.route('**/cloud/api/files?*', (route) => {
      const path = new URL(route.request().url()).searchParams.get('path') ?? '';
      const entries =
        path === ''
          ? [{ path: 'Diagrams', name: 'Diagrams', dir: true, size: 0, mtime: 0, etag: 'd' }]
          : [...files.keys()].filter((p) => p.startsWith(path + '/')).map(entry);
      return json(route, 200, { folder: { path }, entries });
    });
    await page.route('**/cloud/api/files/content?*', (route) => {
      const u = new URL(route.request().url()).searchParams;
      const path = u.get('path');
      if (route.request().method() === 'GET') {
        const f = files.get(path);
        return f ? route.fulfill({ status: 200, contentType: 'application/json', headers: { ETag: `"${f.etag}"` }, body: f.text }) : json(route, 404, { code: 'notFound' });
      }
      puts.push({ path, ifMatch: u.get('ifMatch'), ifNew: u.get('ifNew') });
      const there = files.get(path);
      if (u.get('ifNew') && there) return json(route, 412, { code: 'exists' });
      if (u.get('ifMatch') && (!there || there.etag !== u.get('ifMatch'))) return json(route, 412, { code: 'changed' });
      files.set(path, { text: route.request().postData(), etag: 'v' + ++n });
      return json(route, 200, entry(path));
    });
  };
  return { files, puts, routes };
}

await check('LosOS Cloud: a diagram opens from ?file=, saves back on its version, and a change elsewhere is caught', async () => {
  const cloud = fakeCloud();
  const { page, clean } = await open({ routes: cloud.routes, query: '&file=' + encodeURIComponent('Diagrams/plan.canvas') });
  await card(page, 'Inbox').waitFor();
  assert.strictEqual(await page.getByTestId('diagram-name').inputValue(), 'plan');

  // a new card, then Ctrl+S writes it back over the version it read
  await page.locator('[data-testid=tray-items] [data-card=text]').click();
  const p = await at(page, 260, 420);
  await page.mouse.click(p.x, p.y);
  await page.keyboard.type('Added on the box');
  await page.keyboard.press('Escape');
  await page.keyboard.press('Control+s');
  await page.getByText('Saved plan in LosOS Cloud (Diagrams/plan.canvas).', { exact: true }).waitFor();
  assert.deepStrictEqual(cloud.puts[0], { path: 'Diagrams/plan.canvas', ifMatch: 'v1', ifNew: null });
  const written = JSON.parse(cloud.files.get('Diagrams/plan.canvas').text);
  assert.ok(written.nodes.some((n) => n.text === 'Added on the box'));
  assert.strictEqual(written['x-plugin-doc'].version, 2);

  // someone else saves it; this tab's next save is refused, not silent
  cloud.files.get('Diagrams/plan.canvas').etag = 'elsewhere';
  await page.keyboard.press('Control+s');
  await page.getByText(/changed in LosOS Cloud since/i).waitFor();
  assert.strictEqual(cloud.files.get('Diagrams/plan.canvas').etag, 'elsewhere', 'nothing was overwritten');
  await clean();
  await page.close();
});

await check('LosOS Cloud: the dialog lists folders and diagrams only, and a new name is a new file', async () => {
  const cloud = fakeCloud();
  const { page, clean } = await open({ routes: cloud.routes });
  await page.getByTestId('file-menu').click();
  await page.getByTestId('cloud-open').click();
  const dialog = page.getByRole('dialog', { name: 'Open from LosOS Cloud' });
  await dialog.getByRole('button', { name: 'Diagrams', exact: true }).click();
  await dialog.getByRole('button', { name: 'plan.canvas' }).waitFor();
  assert.strictEqual(await dialog.getByRole('button', { name: 'notes.md' }).count(), 0, 'a note is not a diagram');
  await dialog.getByRole('button', { name: 'plan.canvas' }).click();
  await card(page, 'Inbox').waitFor();

  await page.getByTestId('file-menu').click();
  await page.getByRole('menuitem', { name: 'Save to LosOS Cloud as…' }).click();
  const save = page.getByRole('dialog', { name: 'Save to LosOS Cloud' });
  await save.getByRole('textbox').fill('copy');
  await save.getByTestId('cloud-save').click();
  await save.waitFor({ state: 'detached' });
  assert.deepStrictEqual(cloud.puts.at(-1), { path: 'Diagrams/copy.canvas', ifMatch: null, ifNew: '1' });
  assert.ok(cloud.files.has('Diagrams/copy.canvas'));
  await clean();
  await page.close();
});

await check('edited together: two people on one file see each other, their changes merge, undo is each one\'s own', async () => {
  const cloud = fakeCloud();
  const room = yroom(cloud.files.get('Diagrams/plan.canvas').text);
  // The box writes the room's text back to the file, as room.rs does.
  room.doc.on('update', () => (cloud.files.get('Diagrams/plan.canvas').text = room.text()));
  const person = (name, login) => async (page) => {
    await cloud.routes(page);
    await page.route('**/cloud/api/collab/doc?*', (route) =>
      json(route, 200, { name: 'plan.canvas', owner: 'matus', box: 'mattbox.local', perm: 'edit', remote: false, me: { name, address: `${login}@mattbox.local` } }),
    );
    await page.routeWebSocket(/\/cloud\/api\/collab\/ws/, (ws) => room.connect(ws, login));
  };
  const query = '&file=' + encodeURIComponent('Diagrams/plan.canvas');
  const a = await open({ routes: person('Matus', 'matus'), query });
  const b = await open({ routes: person('Eva', 'eva'), query });
  for (const { page } of [a, b]) {
    await card(page, 'Inbox').waitFor();
    await page.locator('[data-testid=live-people] [data-face]').nth(1).waitFor();
  }

  const write = async (page, x, y, words) => {
    await page.locator('[data-testid=tray-items] [data-card=text]').click();
    const p = await at(page, x, y);
    await page.mouse.click(p.x, p.y);
    await page.keyboard.type(words);
    await page.keyboard.press('Escape');
  };
  // One writes, the other sees it; the file follows.
  await write(a.page, 80, 450, 'From Matus');
  await card(b.page, 'From Matus').waitFor();
  assert.ok(JSON.parse(cloud.files.get('Diagrams/plan.canvas').text).nodes.some((n) => n.text === 'From Matus'));

  // Eva's pointer and selection, in her colour, on Matus's canvas.
  await card(b.page, 'Inbox').click();
  const p = await at(b.page, 400, 400);
  await b.page.mouse.move(p.x, p.y);
  const eva = a.page.locator('[data-testid=peers] [data-peer="eva@mattbox.local"]');
  await eva.getByText('Eva').waitFor();
  await eva.locator('.dg-peer-sel').waitFor();
  assert.ok(['teal', 'blue', 'violet', 'rose', 'amber', 'green'].includes(await eva.getAttribute('data-accent')));

  // Both change the file at the same moment: Eva's messages are held
  // until both cards are down, then everything is let through.
  room.hold('eva');
  await write(a.page, 230, 450, 'Card A');
  await write(b.page, 400, 450, 'Card B');
  room.release('eva');
  for (const { page } of [a, b]) {
    await card(page, 'Card A').waitFor();
    await card(page, 'Card B').waitFor();
  }
  // The two copies settle on one file that parses.
  await a.page.waitForTimeout(1200);
  const doc = JSON.parse(room.text());
  for (const words of ['From Matus', 'Card A', 'Card B', '## Inbox\nsort on Monday']) assert.ok(doc.nodes.some((n) => n.text === words), words);
  assert.strictEqual(new Set(doc.nodes.map((n) => n.id)).size, doc.nodes.length);
  assert.strictEqual(doc['x-plugin-doc'].version, 2, 'a key this page does not know stays');

  // Undo takes back Matus's last card, not Eva's.
  await a.page.getByTestId('undo').click();
  await card(b.page, 'Card A').waitFor({ state: 'detached' });
  assert.strictEqual(await card(a.page, 'Card A').count(), 0);
  assert.strictEqual(await card(a.page, 'Card B').count(), 1);

  // Saving says where the changes already went.
  await a.page.keyboard.press('Control+s');
  await a.page.getByText('Changes go to LosOS Cloud as you make them (Diagrams/plan.canvas).', { exact: true }).waitFor();
  assert.strictEqual(cloud.puts.length, 0, 'nothing was PUT over the room');

  for (const { page, clean } of [a, b]) {
    await clean();
    await page.close();
  }
  room.close();
});

await check('a diagram shared from another box opens in its room; a view-only share takes no change', async () => {
  const room = yroom(JSON.stringify(OBSIDIAN, null, 2));
  const before = room.text();
  const routes = async (page) => {
    await page.route('**/cloud/api/me', (route) => json(route, 200, { login: 'eva', name: 'Eva' }));
    await page.route('**/cloud/api/collab/doc?*', (route) => {
      const u = new URL(route.request().url()).searchParams;
      assert.deepStrictEqual([u.get('share'), u.get('file')], ['sh_1', '7']);
      return json(route, 200, {
        name: 'plan.canvas', owner: 'Matus', ownerAddress: 'matus@mattbox.local', box: 'mattbox.local',
        perm: 'read', remote: true, linked: true, me: { name: 'Eva', address: 'eva@kitchenbox.local' },
      });
    });
    await page.routeWebSocket(/\/cloud\/api\/collab\/ws\?.*share=sh_1/, (ws) => room.connect(ws, 'eva'));
  };
  const { page, clean } = await open({ routes, query: '&share=sh_1&fileId=7' });
  await card(page, 'Inbox').waitFor();
  await page.getByText('Shared by Matus. The file stays on mattbox.local.').waitFor();
  await page.getByTestId('live-people').getByText('View only').waitFor();

  await page.locator('[data-testid=tray-items] [data-card=text]').click();
  const p = await at(page, 260, 420);
  await page.mouse.click(p.x, p.y);
  await page.getByText('You can view this diagram but not change it.', { exact: true }).waitFor();
  assert.strictEqual(room.text(), before, 'the room kept its text');
  assert.strictEqual(await nodes(page).count(), 3, 'the card went again');
  await clean();
  await page.close();
  room.close();
});

const VMS = {
  available: true,
  catalogue: { images: [{ id: 'debian-13', name: 'Debian 13' }, { id: 'nixos-26', name: 'NixOS 26.11' }] },
  listings: [],
  account: {
    purchases: [
      { id: 'ord_1', kind: 'vm', status: 'paid', vm: { image: 'debian-13', name: 'web', address: '10.42.0.7', created: 0, halted: false } },
    ],
    uploads: [],
  },
  machines: [{ order_id: 'ord_1', replicas: [{ name: 'web-0', status: 'Running' }, { name: 'web-1', status: 'Starting' }] }],
};

await check('Machines: an ordered machine is a live card, a planned one links to the order form', async () => {
  const { page, clean } = await open({
    signedIn: true,
    routes: async (p, authed) => {
      await p.route('**/api/vms', (route) => (authed(route) ? json(route, 200, VMS) : json(route, 401, {})));
    },
  });
  await fresh(page);
  await page.getByRole('button', { name: 'Machines', exact: true }).click();
  await page.locator('[data-testid=tray-items] [data-machine=ord_1]').click();
  let p = await at(page, 130, 260);
  await page.mouse.click(p.x, p.y);
  const replicas = page.getByTestId('machine-replicas');
  await replicas.waitFor();
  assert.match(await replicas.innerText(), /web-0[\s\S]*Running/);
  assert.match(await replicas.innerText(), /web-1[\s\S]*Starting/);
  assert.match(await page.locator('[data-testid=diagram] .dg-node.machine').first().textContent(), /web/);

  await page.locator('[data-testid=tray-items] [data-machine-image=nixos-26]').click();
  p = await at(page, 400, 260);
  await page.mouse.click(p.x, p.y);
  const order = page.getByTestId('machine-order');
  await order.waitFor();
  assert.match(await order.getAttribute('href'), /^\/machines\?image=nixos-26/);
  await clean();
  await page.close();
});

await check('Machines: a box that does not share says why', async () => {
  const { page, clean } = await open();
  await page.getByRole('button', { name: 'Machines', exact: true }).click();
  const why = page.getByTestId('machines-unavailable');
  await why.waitFor();
  assert.match(await why.innerText(), /shar/i);
  await clean();
  await page.close();
});

await check('the chrome speaks Slovak and German', async () => {
  for (const [locale, file, cards] of [
    ['sk-SK', 'Súbor', 'Karty'],
    ['de-DE', 'Datei', 'Karten'],
  ]) {
    const { page, clean } = await open({ locale });
    assert.match(await page.getByTestId('file-menu').innerText(), new RegExp(file));
    await page.getByRole('button', { name: cards, exact: true }).waitFor();
    await clean();
    await page.close();
  }
});

await check('phone width: no sideways scroll, the canvas still takes a card', async () => {
  const { page, clean } = await open({ viewport: { width: 390, height: 844 } });
  const wide = await page.evaluate(() => document.documentElement.scrollWidth - window.innerWidth);
  assert.ok(wide <= 0, `the page scrolls sideways by ${wide}px`);
  await fresh(page);
  const p = await at(page, 150, 200);
  await page.mouse.dblclick(p.x, p.y);
  await page.getByRole('textbox', { name: 'Card text' }).fill('On the phone');
  await page.keyboard.press('Escape');
  await card(page, 'On the phone').waitFor();
  await clean();
  await page.close();
});

await check('dark theme: cards follow the house tokens', async () => {
  const { page, clean } = await open({ scheme: 'dark' });
  await page.getByTestId('open-file').setInputFiles({ name: 'week.canvas', mimeType: 'application/json', buffer: Buffer.from(JSON.stringify(OBSIDIAN)) });
  await card(page, 'Inbox').waitFor();
  const fill = await page.locator('[data-testid=diagram] [data-node="b2"] .dg-shape').evaluate((el) => getComputedStyle(el).fill);
  const bg = await page.evaluate(() => getComputedStyle(document.body).backgroundColor);
  const lum = (c) => c.match(/\d+/g).slice(0, 3).reduce((a, b) => a + Number(b), 0);
  assert.ok(lum(fill) < 300 && lum(bg) < 300, `dark card ${fill} on ${bg}`);
  await clean();
  await page.close();
});

await browser.close();
closeServer();
finish();
