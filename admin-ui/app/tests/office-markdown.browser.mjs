/* Markdown documents and presentations in LosOS Office
 * (src/cloud/office/markdown/), inside LosOS Cloud.
 *
 * What is checked (a new document is a .paper, tests/paper.browser.mjs): a
 * .md opens at /cloud/office/<id> as LosOS's own editor, never in
 * Collabora, under the Office bar with the name, Markdown and the saved
 * state; the formatting bar writes what Obsidian and GitHub read for each
 * of its buttons, takes a mark off again, shows the cursor's bold and
 * heading as pressed, and the read side draws the result with the notes
 * renderer; a file with `slides: true` is a presentation, with its slides
 * in a strip, the slide the cursor is in beside the text, its text centred
 * and its picture on the right, and it plays full screen with the arrows
 * and Escape; the right-click menu of a .md offers Office; and none of it
 * adds to the policy violations a plain load logs.
 *
 * The shared editor's WebSocket has no box here, so it stays offline and
 * the text is what is typed; the routes are stubbed with page.route(). */

import assert from './assert.mjs';
import { launch, runner, serve, stash } from './harness.mjs';

const { origin, close: closeServer } = await serve({ csp: true });
const browser = await launch();
const { check, finish } = runner();

const json = (route, status, body) =>
  route.fulfill({ status, contentType: 'application/json', body: JSON.stringify(body) });

const PNG = Uint8Array.fromBase64('iVBORw0KGgoAAAANSUhEUgAAAAIAAAACCAYAAABytg0kAAAAFklEQVR4nGNk+M/AwMDAxMDAwMDAAAAiHgMBvU2hBwAAAABJRU5ErkJggg==');
const PNG_FILE = await stash('picture.png', PNG);

const entry = (id, path, mime = 'text/markdown') => ({
  id,
  path,
  name: path.split('/').pop(),
  dir: false,
  size: 1,
  mtime: 1,
  etag: 'x',
  mime,
  favorite: false,
});

/* LosOS Cloud with a folder holding `taken` names already. Every PUT is
 * kept in `made`, with its body. */
async function openCloud(path, { files = [], taken = [], byId = {} } = {}) {
  const context = await browser.newContext({ viewport: { width: 1280, height: 900 }, locale: 'en-US', colorScheme: 'dark' });
  const page = await context.newPage();
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  const made = [];
  let next = 500;
  await page.route('**/cloud/api/**', async (route) => {
    const req = route.request();
    const url = new URL(req.url());
    const p = url.pathname.slice('/cloud/api'.length);
    if (p === '/me') return json(route, 200, { login: 'matus', name: 'Matus', email: '', used: 1e9, free: 9e9, product: 'LosOS Cloud' });
    if (p === '/files') return json(route, 200, { folder: { ...entry(1, ''), dir: true }, entries: files });
    if (p === '/files/content' && req.method() === 'PUT') {
      const at = url.searchParams.get('path');
      if (taken.includes(at)) return json(route, 409, { code: 'exists', error: 'exists' });
      const e = entry(next++, at);
      made.push({ path: at, ifNew: url.searchParams.get('ifNew'), body: req.postData(), entry: e });
      byId[e.id] = e;
      return json(route, 201, e);
    }
    if (p === '/files/content') return route.fulfill({ status: 200, contentType: 'image/png', path: PNG_FILE });
    const m = /^\/files\/by-id\/(\d+)$/.exec(p);
    if (m) return byId[m[1]] ? json(route, 200, byId[m[1]]) : json(route, 404, { code: 'notFound', error: 'no' });
    if (p === '/collab/doc') {
      const id = url.searchParams.get('file');
      return json(route, 200, {
        name: byId[id]?.name ?? 'x',
        owner: 'matus',
        box: 'mattbox.local',
        perm: 'edit',
        remote: false,
        me: { name: 'Matus', address: 'matus@mattbox.local' },
      });
    }
    if (p === '/notes/root') return json(route, 200, { root: null });
    if (p === '/notes/vault') {
      return json(route, 200, {
        root: '',
        name: 'Files',
        folderId: 1,
        notes: [],
        canvases: [],
        files: [{ id: 9, path: 'tomato.png', mime: 'image/png', size: 70 }],
        folders: [],
      });
    }
    if (p === '/office/open') return json(route, 500, { error: 'not Collabora' });
    return json(route, 200, {});
  });
  // The shared editor's socket meets a plain page, so it stays offline.
  await page.addInitScript(() => {
    window.__violations = [];
    document.addEventListener('securitypolicyviolation', (e) => window.__violations.push(`${e.violatedDirective} ${e.blockedURI}`));
  });
  await page.goto(origin + path);
  return { page, context, errors, made };
}

const violations = (page) => page.evaluate(() => [...new Set(window.__violations)].sort());
const text = (page) => page.locator('.cm-content').evaluate((el) => el.cmView?.view?.state.doc.toString() ?? [...el.querySelectorAll('.cm-line')].map((l) => l.textContent).join('\n'));
const bar = (page) => page.getByTestId('md-toolbar');
const tool = (page, name) => bar(page).getByRole('button', { name, exact: true });

/** Opens a Markdown file as LosOS Cloud would, and waits for its editor. */
async function openDoc(name, extra = {}) {
  const byId = { 77: entry(77, name) };
  const opened = await openCloud('/cloud/office/77', { byId, ...extra });
  await opened.page.locator('[data-markdown-office]').waitFor();
  await opened.page.locator('.cm-content').waitFor();
  // Offline is "loaded" enough to type: the bar waits for a live line,
  // so a test without a box lifts the wait by reporting it.
  return opened;
}

async function type(page, value) {
  await page.locator('.cm-content').click();
  await page.keyboard.press('Control+A');
  await page.keyboard.press('Delete');
  await page.keyboard.insertText(value);
}

/** Selects the whole text, as a person does before pressing a button. */
async function selectAll(page) {
  await page.locator('.cm-content').focus();
  await page.keyboard.press('Control+A');
}

/** Puts the cursor at the end of the text. */
async function toEnd(page) {
  await page.locator('.cm-content').focus();
  await page.keyboard.press('Control+End');
}

/** Puts the cursor at the end of the line that reads `line`. */
async function caretOn(page, line) {
  await page.locator('.cm-line', { hasText: line }).first().click();
  await page.keyboard.press('End');
}

let BASELINE = [];
{
  const { page, context } = await openCloud('/cloud/files');
  await page.waitForTimeout(400);
  BASELINE = await violations(page);
  await context.close();
}

console.log('office markdown checks');

await check('the bar writes Obsidian and GitHub Markdown, and the read side draws it', async () => {
  const { page, context, errors } = await openDoc('Plan.md');
  // Offline the editor still takes typing; the bar is live once the editor is.
  await type(page, 'word');
  await page.waitForFunction(() => !document.querySelector('[data-testid=md-toolbar] [data-action=bold]')?.disabled, undefined, { timeout: 5000 }).catch(() => {});
  const cases = [
    ['Bold', '**word**'],
    ['Italic', '*word*'],
    ['Strikethrough', '~~word~~'],
    ['Highlight', '==word=='],
    ['Code', '`word`'],
    ['Subscript', '<sub>word</sub>'],
    ['Superscript', '<sup>word</sup>'],
    ['Key', '<kbd>word</kbd>'],
    ['Hidden comment', '%%word%%'],
    ['Formula in the line', '$word$'],
    ['Bulleted list', '- word'],
    ['Numbered list', '1. word'],
    ['Task list', '- [ ] word'],
    ['Quote', '> word'],
    ['Link to a note', '[[word]]'],
    ['Embed a note or file', '![[word]]'],
    ['Tag', '#word'],
    ['Link', '[word](https://)'],
    ['Picture', '![word](picture.png)'],
  ];
  for (const [name, want] of cases) {
    await type(page, 'word');
    await selectAll(page);
    await tool(page, name).click();
    assert.equal(await text(page), want, name);
  }
  // A mark already there comes off again.
  await type(page, 'word');
  await selectAll(page);
  await tool(page, 'Bold').click();
  assert.equal(await tool(page, 'Bold').getAttribute('aria-pressed'), 'true');
  await tool(page, 'Bold').click();
  assert.equal(await text(page), 'word');
  // Blocks land on lines of their own.
  for (const [name, want] of [
    ['Code block', /^word\n\n```\ncode\n```$/],
    ['Formula on its own', /^word\n\n\$\$\nE = mc\^2\n\$\$$/],
    ['Table', /^word\n\n\| Column 1 \| Column 2 \|\n\| --- \| --- \|\n\| {2}\| {2}\|$/],
    ['Folded section', /^word\n\n<details>\n<summary>More<\/summary>\n\ntext\n\n<\/details>$/],
    ['Line across', /^word\n\n---$/],
    ['Footnote', /^word\[\^1\]\n\n\[\^1\]: The footnote$/],
    ['Block link target', /^word \^[0-9a-f]{6}$/],
    ['Properties', /^---\ntags: \n---\n\nword$/],
  ]) {
    await type(page, 'word');
    await toEnd(page);
    await tool(page, name).click();
    assert.match(await text(page), want, name);
  }
  // Headings from the list, and back to text.
  await type(page, 'word');
  await bar(page).getByLabel('Paragraph style').selectOption('2');
  assert.equal(await text(page), '## word');
  assert.equal(await bar(page).getByLabel('Paragraph style').inputValue(), '2');
  await bar(page).getByLabel('Paragraph style').selectOption('0');
  assert.equal(await text(page), 'word');
  // Callouts, GitHub's alerts and Obsidian's.
  await type(page, 'word');
  await selectAll(page);
  await bar(page).getByLabel('Callout').selectOption('warning');
  assert.equal(await text(page), '> [!warning]\n> word');

  // The read side is the notes renderer.
  await type(page, '# Plan\n\n**bold** ==marked== [[Missing]] #garden\n\n> [!tip] Water\n> Every morning\n\n- [x] done\n\n$x^2$');
  const read = page.getByTestId('md-read');
  await read.locator('h1', { hasText: 'Plan' }).waitFor();
  await read.locator('strong', { hasText: 'bold' }).waitFor();
  await read.locator('mark', { hasText: 'marked' }).waitFor();
  await read.locator('a.wikilink.is-unresolved', { hasText: 'Missing' }).waitFor();
  await read.locator('a.tag', { hasText: '#garden' }).waitFor();
  await read.locator('.callout[data-callout="tip"]').waitFor();
  await read.locator('input.task-box[checked]').waitFor();
  await read.locator('math').first().waitFor();
  // Write and Read take the whole width.
  await page.getByRole('radio', { name: 'Read' }).or(page.locator('[data-layout=read]')).first().click();
  assert.equal(await page.locator('.cm-content').isVisible(), false);
  await page.locator('[data-layout=write]').click();
  assert.equal(await read.count(), 0);
  assert.deepEqual(errors, []);
  await context.close();
});

const DECK = '---\nslides: true\n---\n\n# Kickoff\n\nMatus, October\n\n---\n\n## The garden\n\n- Beds\n- Water\n\n![tomatoes](tomato.png)\n\n---\n\n## Thanks';

await check('a presentation lays out its slides: text centred, the picture on the right', async () => {
  const { page, context, errors } = await openDoc('Kickoff.md');
  await type(page, DECK);
  await page.locator('[data-markdown-office][data-deck]').waitFor();
  await page.getByTestId('office-bar').getByText('Slides', { exact: true }).waitFor();
  const strip = page.getByTestId('md-strip');
  assert.equal(await strip.locator(':scope > li').count(), 3);
  // The stage follows the cursor: put it on the second slide.
  await caretOn(page, '## The garden');
  const stage = page.getByTestId('md-stage');
  await stage.locator('h2', { hasText: 'The garden' }).waitFor();
  await strip.locator(':scope > li').nth(1).locator('button[aria-current=true]').waitFor();
  const textBox = await stage.locator('.md-slide-text').boundingBox();
  const picBox = await stage.locator('.md-slide-pictures img').boundingBox();
  assert.ok(picBox.x > textBox.x + textBox.width - 1, `picture at ${picBox.x}, text ends at ${textBox.x + textBox.width}`);
  assert.equal(await stage.locator('.md-slide-text img').count(), 0);
  assert.equal(await stage.locator('.md-slide-text').evaluate((el) => getComputedStyle(el).textAlign), 'center');
  // The heading sits in the middle of its column.
  const h = await stage.locator('.md-slide-text h2').boundingBox();
  const mid = textBox.x + textBox.width / 2;
  assert.ok(Math.abs(h.x + h.width / 2 - mid) < 4, `heading centre ${h.x + h.width / 2}, column centre ${mid}`);
  // A slide without a picture is one centred column.
  await strip.locator(':scope > li').nth(0).locator('button').click();
  await stage.locator('h1', { hasText: 'Kickoff' }).waitFor();
  assert.equal(await stage.locator('.md-slide-inner[data-pictures]').count(), 0);
  // The bar adds a slide.
  await tool(page, 'New slide').click();
  await page.waitForFunction(() => document.querySelectorAll('[data-testid=md-strip] > li').length === 4);
  assert.deepEqual(errors, []);
  await context.close();
});

await check('a presentation plays full screen, and the keys move through it', async () => {
  const { page, context } = await openDoc('Kickoff.md');
  await type(page, DECK);
  await page.locator('[data-markdown-office][data-deck]').waitFor();
  await caretOn(page, '# Kickoff');
  await page.getByTestId('md-present').click();
  const show = page.getByTestId('md-presenter');
  await show.waitFor();
  await show.getByTestId('md-present-count').getByText('1 / 3').waitFor();
  await page.keyboard.press('ArrowRight');
  await show.getByTestId('md-present-count').getByText('2 / 3').waitFor();
  await show.locator('.md-slide-pictures img').waitFor();
  await page.keyboard.press('End');
  await show.getByTestId('md-present-count').getByText('3 / 3').waitFor();
  await page.keyboard.press('Escape');
  await show.waitFor({ state: 'detached' });
  await context.close();
});

await check('a Markdown file offers Office in its menu, and adds no policy violations', async () => {
  const files = [entry(78, 'Plan.md'), entry(79, 'todo.txt', 'text/plain')];
  const { page, context } = await openCloud('/cloud/files', { files, byId: { 78: files[0] } });
  const row = page.locator('[data-entry="Plan.md"]');
  await row.waitFor();
  await row.click({ button: 'right' });
  const item = page.getByRole('menuitem', { name: 'Open in LosOS Office' });
  await item.waitFor();
  // Openers open the file in a tab of its own.
  const [tab] = await Promise.all([context.waitForEvent('page'), item.click()]);
  await tab.waitForURL(/\/cloud\/office\/78$/);
  assert.deepEqual(await violations(page), BASELINE);
  await context.close();
});

await browser.close();
await closeServer();
finish();
