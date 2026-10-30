/* The bar over a selection (components/quick-actions.tsx): picked text in
 * LosOS Office's Markdown documents and in Notes, cards in LosOS Diagrams.
 *
 * What is checked: picking words in an Office document brings the bar up
 * above them only once the pointer lets go; its buttons write the same
 * Markdown as the Office bar and show pressed what the words already have;
 * Link takes the bar's place with a field and writes the link; Link to a
 * note offers the vault's notes; Alt+F10 moves the focus into the bar and
 * Escape hands it back and hides the bar until the selection changes; Ask
 * AI sends the words to the box's assistant and its answer can replace
 * them; Copy copies. In a note being read the bar copies and links to the
 * section. In Diagrams two cards picked together get a colour, an arrow
 * between them and Delete from the bar, and the arrow keys walk the bar
 * without moving the cards. Nothing adds a policy violation or an error.
 *
 * The shared editor's WebSocket has no box here, so it stays offline and
 * the text is what is typed; the routes are stubbed with page.route(). */

import assert from './assert.mjs';
import { launch, runner, serve, shellRoutes } from './harness.mjs';

const { origin, close: closeServer } = await serve({ csp: true });
const browser = await launch();
const { check, finish } = runner();

const json = (route, status, body) => route.fulfill({ status, contentType: 'application/json', body: JSON.stringify(body) });

const entry = (id, path, mime = 'text/markdown') => ({ id, path, name: path.split('/').pop(), dir: false, size: 1, mtime: 1, etag: 'x', mime, favorite: false });

const NOTES = {
  'Garden.md': '# Garden\n\nThe beds face south.\n\n## Watering\n\nWater the tomatoes every morning before nine.\n',
  'Tomatoes.md': '# Tomatoes\n\nSan Marzano and Black Krim.\n',
};

function vault() {
  return {
    root: 'Notes',
    name: 'Notes',
    folderId: 7,
    notes: Object.entries(NOTES).map(([path, text], i) => ({ id: 100 + i, path, size: text.length, mtime: 1, etag: 'e1', title: null, aliases: [], tags: [], links: [], private: false })),
    canvases: [],
    files: [],
    folders: [],
  };
}

/** LosOS Cloud with the stubs Office and Notes need, and the box's AI. */
async function openCloud(path, { assistant = false } = {}) {
  const context = await browser.newContext({ viewport: { width: 1280, height: 900 }, locale: 'en-US', colorScheme: 'light' });
  await context.grantPermissions(['clipboard-read', 'clipboard-write'], { origin });
  const page = await context.newPage();
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  const asked = [];
  await page.route('**/cloud/api/**', async (route) => {
    const req = route.request();
    const url = new URL(req.url());
    const p = url.pathname.slice('/cloud/api'.length);
    if (p === '/me') return json(route, 200, { login: 'matus', name: 'Matus', email: '', used: 1e9, free: 9e9, product: 'LosOS Cloud' });
    if (p === '/files') return json(route, 200, { folder: { ...entry(1, ''), dir: true }, entries: [] });
    if (p === '/files/by-id/77') return json(route, 200, entry(77, 'Notes/Plan.md'));
    if (p === '/collab/doc') {
      return json(route, 200, { name: 'Plan.md', owner: 'matus', box: 'mattbox.local', perm: 'edit', remote: false, me: { name: 'Matus', address: 'matus@mattbox.local' } });
    }
    if (p === '/notes/root') return json(route, 200, { root: 'Notes' });
    if (p === '/notes/vault') return json(route, 200, vault());
    if (p === '/files/content') {
      const rel = (url.searchParams.get('path') ?? '').replace(/^Notes\//, '');
      const text = NOTES[rel];
      if (text === undefined) return json(route, 404, { code: 'notFound', error: 'no such file' });
      return route.fulfill({ status: 200, contentType: 'text/markdown', headers: { etag: '"e1"' }, body: text });
    }
    if (p === '/office/open') return json(route, 500, { error: 'not Collabora' });
    return json(route, 200, {});
  });
  await page.route('**/api/assistant', (route) => json(route, 200, assistant ? { available: true, ready: true, model: 'stand-in' } : { available: false }));
  await page.route('**/api/assistant/asks', (route) => {
    asked.push(JSON.parse(route.request().postData() ?? '{}'));
    return json(route, 200, { id: 'a1', state: 'done', answer: 'Water the plants daily.', cut: false });
  });
  await page.addInitScript(() => {
    window.__violations = [];
    document.addEventListener('securitypolicyviolation', (e) => window.__violations.push(`${e.violatedDirective} ${e.blockedURI}`));
  });
  await page.goto(origin + path);
  return { page, context, errors, asked };
}

const violations = (page) => page.evaluate(() => [...new Set(window.__violations)].sort());
const text = (page) => page.locator('.cm-content').evaluate((el) => el.cmView?.view?.state.doc.toString() ?? [...el.querySelectorAll('.cm-line')].map((l) => l.textContent).join('\n'));
const quick = (page) => page.locator('[data-slot=quick-actions]');
const action = (page, id) => quick(page).locator(`[data-action="${id}"]`);

async function openDoc(options) {
  const opened = await openCloud('/cloud/office/77', options);
  await opened.page.locator('[data-markdown-office]').waitFor();
  await opened.page.locator('.cm-content').waitFor();
  return opened;
}

async function type(page, value) {
  await page.locator('.cm-content').click();
  await page.keyboard.press('Control+A');
  await page.keyboard.press('Delete');
  await page.keyboard.insertText(value);
}

/** Picks the last `n` characters with the keyboard. */
async function pickLast(page, n) {
  await page.keyboard.press('End');
  for (let i = 0; i < n; i++) await page.keyboard.press('Shift+ArrowLeft');
}

let BASELINE = [];
{
  const { page, context } = await openCloud('/cloud/files');
  await page.waitForTimeout(400);
  BASELINE = await violations(page);
  await context.close();
}

console.log('quick actions checks');

await check('picked words in Office bring the bar up above them, and it formats them', async () => {
  const { page, context, errors } = await openDoc();
  await type(page, 'hello world');
  assert.equal(await quick(page).count(), 0, 'no bar without a selection');
  await pickLast(page, 5);
  await quick(page).waitFor();
  // On the first line there is no room above without covering Office's own
  // toolbar, so the bar goes below the words and the toolbar stays usable.
  let bar = await quick(page).boundingBox();
  const first = await page.locator('.cm-line').first().boundingBox();
  const tools = await page.locator('[data-testid=md-toolbar]').boundingBox();
  assert.ok(bar.y >= first.y + first.height - 2, `the bar sits below the first line (${bar.y} vs ${first.y + first.height})`);
  assert.ok(bar.y >= tools.y + tools.height, `the bar leaves the toolbar uncovered (${bar.y} vs ${tools.y + tools.height})`);
  await page.keyboard.press('End');
  await page.keyboard.press('Enter');
  await page.keyboard.press('Enter');
  await page.keyboard.insertText('hello world');
  await pickLast(page, 5);
  await quick(page).waitFor();
  // Above the words, inside the window.
  await page.waitForTimeout(250);
  bar = await quick(page).boundingBox();
  const line = await page.locator('.cm-line').nth(2).boundingBox();
  assert.ok(bar.y + bar.height <= line.y + 2, `the bar sits above the line (${bar.y + bar.height} vs ${line.y})`);
  await action(page, 'bold').click();
  assert.equal(await text(page), 'hello world\n\nhello **world**');
  assert.equal(await action(page, 'bold').getAttribute('aria-pressed'), 'true', 'bold shows pressed');
  await action(page, 'bold').click();
  assert.equal(await text(page), 'hello world\n\nhello world', 'a second press takes it off');
  await action(page, 'highlight').click();
  assert.equal(await text(page), 'hello world\n\nhello ==world==');
  // Typing over the words takes the bar away.
  await page.keyboard.insertText('there');
  await quick(page).waitFor({ state: 'detached' });
  assert.deepEqual(errors, []);
  await context.close();
});

await check('the bar waits for the pointer to let go', async () => {
  const { page, context } = await openDoc();
  await type(page, 'drag across these words');
  const box = await page.locator('.cm-line').first().boundingBox();
  await page.mouse.move(box.x + 40, box.y + box.height / 2);
  await page.mouse.down();
  await page.mouse.move(box.x + 160, box.y + box.height / 2, { steps: 6 });
  await page.waitForTimeout(200);
  assert.equal(await quick(page).count(), 0, 'no bar while the button is down');
  await page.mouse.up();
  await quick(page).waitFor();
  await context.close();
});

await check("Link and Link to a note take the bar's place with a field", async () => {
  const { page, context } = await openDoc();
  await type(page, 'see the site');
  await pickLast(page, 4);
  await action(page, 'link').click();
  const field = quick(page).getByRole('textbox', { name: 'Link' });
  await field.waitFor();
  assert.equal(await page.evaluate(() => document.activeElement?.getAttribute('aria-label')), 'Link', 'the field has the focus');
  await field.fill('example.org/seeds');
  await page.keyboard.press('Enter');
  assert.equal(await text(page), 'see the [site](https://example.org/seeds)');

  await type(page, 'grow tomatoes');
  await pickLast(page, 8);
  await action(page, 'wikilink').click();
  const find = quick(page).getByRole('combobox', { name: 'Link to a note' });
  await find.waitFor();
  await find.fill('tom');
  await quick(page).getByRole('option', { name: /Tomatoes/ }).waitFor();
  await page.keyboard.press('Enter');
  assert.equal(await text(page), 'grow [[Tomatoes|tomatoes]]');
  await context.close();
});

await check('Alt+F10 reaches the bar, the arrows walk it, Escape goes back and hides it', async () => {
  const { page, context } = await openDoc();
  await type(page, 'keyboard only');
  await pickLast(page, 4);
  await quick(page).waitFor();
  await page.keyboard.press('Alt+F10');
  const focused = () => page.evaluate(() => document.activeElement?.getAttribute('data-action'));
  assert.equal(await focused(), 'bold');
  await page.keyboard.press('ArrowRight');
  assert.equal(await focused(), 'italic');
  await page.keyboard.press('Enter');
  assert.equal(await text(page), 'keyboard *only*');
  // The press handed the keyboard back to the text; into the bar again.
  await page.keyboard.press('Alt+F10');
  assert.equal(await focused(), 'bold');
  await page.keyboard.press('Escape');
  await quick(page).waitFor({ state: 'detached' });
  assert.equal(await page.evaluate(() => document.activeElement?.tagName), 'DIV', 'the editor has the focus again');
  // A new selection brings it back.
  await page.keyboard.press('Shift+ArrowLeft');
  await quick(page).waitFor();
  await context.close();
});

await check('Ask AI sends the picked words to the box and the answer replaces them', async () => {
  const { page, context, asked } = await openDoc({ assistant: true });
  await type(page, 'Note: water tomatoes each morning');
  await page.keyboard.press('End');
  for (let i = 0; i < 'water tomatoes each morning'.length; i++) await page.keyboard.press('Shift+ArrowLeft');
  await action(page, 'quick-ask-ai').click();
  const dialog = page.locator('[data-slot=ask-ai]');
  await dialog.waitFor();
  await dialog.getByRole('button', { name: 'Summarise' }).click();
  await dialog.locator('[data-slot=ai-answer]', { hasText: 'Water the plants daily.' }).waitFor();
  assert.equal(asked[0].sources[0].text, 'water tomatoes each morning');
  assert.equal(asked[0].sources[0].title, 'Plan.md');
  await dialog.getByRole('button', { name: 'Replace selection' }).click();
  assert.equal(await text(page), 'Note: Water the plants daily.');
  await context.close();
});

await check('in a note being read the bar copies and links to the section', async () => {
  const { page, context, errors } = await openCloud('/cloud/notes?vault=Notes&note=Garden.md');
  const para = page.locator('p', { hasText: 'every morning' }).first();
  await para.waitFor();
  await para.selectText();
  await quick(page).waitFor();
  assert.equal(await action(page, 'bold').count(), 0, 'nothing that edits while reading');
  await action(page, 'copy-link').click();
  const copied = await page.evaluate(() => navigator.clipboard.readText());
  assert.match(copied, /\/cloud\/notes\?vault=Notes&note=Garden\.md#watering$/);
  await action(page, 'copy').click();
  assert.match(await page.evaluate(() => navigator.clipboard.readText()), /Water the tomatoes every morning/);
  assert.deepEqual(errors, []);
  await context.close();
});

await check('the bar adds no policy violation', async () => {
  const { page, context } = await openDoc();
  await type(page, 'hello world');
  await pickLast(page, 5);
  await quick(page).waitFor();
  await action(page, 'link').hover();
  await page.waitForTimeout(400);
  assert.deepEqual(await violations(page), BASELINE);
  await context.close();
});

// ── Diagrams ──────────────────────────────────────────────────────────────

const TOKEN = 'c'.repeat(64);

async function openDiagrams() {
  const page = await browser.newPage({ viewport: { width: 1440, height: 900 }, locale: 'en-US' });
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  await page.addInitScript((t) => window.sessionStorage.setItem('losos-token', t), TOKEN);
  await shellRoutes(page);
  await page.route('**/api/settings', (route) => json(route, 200, { hostName: 'mattbox', sharingMyStorage: false, cluster: { enable: false } }));
  await page.route('**/api/edge', (route) => json(route, 200, { path: 'none', edges: [] }));
  await page.route('**/api/vms', (route) => json(route, 200, { available: false, reason: 'notSharing' }));
  await page.route('**/api/assistant', (route) => json(route, 200, { available: false }));
  await page.route('**/cloud/api/**', (route) => json(route, 401, { error: 'signIn', code: 'signIn' }));
  await page.goto(origin + '/diagrams/?canvas=svg');
  await page.getByTestId('lab').waitFor({ timeout: 15000 });
  await page.getByTestId('lab-splash').waitFor({ state: 'detached', timeout: 15000 });
  await page.getByTestId('file-menu').click();
  await page.getByTestId('file-new').click();
  await page.getByTestId('diagram-empty').waitFor();
  return { page, errors };
}

async function addCard(page, x, y, words) {
  const box = await page.getByTestId('diagram-wrap').boundingBox();
  await page.mouse.dblclick(box.x + x, box.y + y);
  const editor = page.getByRole('textbox', { name: 'Card text' });
  await editor.waitFor();
  await editor.fill(words);
  await page.keyboard.press('Escape');
  await editor.waitFor({ state: 'detached' });
}

await check('two cards picked in Diagrams get a colour, an arrow and Delete from the bar', async () => {
  const { page, errors } = await openDiagrams();
  await addCard(page, 300, 300, 'Seeds');
  await addCard(page, 700, 300, 'Seedlings');
  const card = (label) => page.locator(`[data-testid=diagram] [data-node][aria-label="${label}"]`);
  await card('Seeds').click();
  await card('Seedlings').click({ modifiers: ['Shift'] });
  await quick(page).waitFor();
  const bar = await quick(page).boundingBox();
  const a = await card('Seeds').boundingBox();
  assert.ok(bar.y + bar.height <= a.y, 'the bar sits above the cards');

  await action(page, 'color').click();
  await page.locator('[data-quick-popup] [data-option="4"]').click();
  await page.waitForFunction(() => document.querySelectorAll('[data-testid=diagram] .dg-node.colored').length === 2);

  // The arrows walk the bar and leave the cards where they are.
  const before = await card('Seeds').boundingBox();
  await page.keyboard.press('Alt+F10');
  await page.keyboard.press('ArrowRight');
  const after = await card('Seeds').boundingBox();
  assert.equal(after.x, before.x, 'the arrow key moved the focus, not the cards');
  await page.keyboard.press('Escape');
  await quick(page).waitFor({ state: 'detached' });
  assert.equal(await page.evaluate(() => document.activeElement?.getAttribute('data-testid')), 'diagram', 'the canvas has the keys again');

  await card('Seeds').click();
  await card('Seedlings').click({ modifiers: ['Shift'] });
  await action(page, 'connect').click();
  await page.waitForFunction(() => document.querySelectorAll('[data-testid=diagram] [data-edge]').length === 1);

  await card('Seeds').click();
  await card('Seedlings').click({ modifiers: ['Shift'] });
  await action(page, 'delete').click();
  await page.getByTestId('diagram-empty').waitFor();
  await quick(page).waitFor({ state: 'detached' });
  assert.deepEqual(errors, []);
  await page.close();
});

await browser.close();
await closeServer();
finish();
