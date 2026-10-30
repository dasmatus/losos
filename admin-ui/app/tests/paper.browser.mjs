/* LosOS Office's own documents and presentations (src/cloud/office/paper/),
 * inside LosOS Cloud: `.paper` and `.slides`, MDX edited as it looks.
 *
 * What is checked: the files bar's New document and New presentation make
 * "Untitled document.paper" (the next free name when it is taken) and
 * "Untitled presentation.slides" with the text the editor itself would
 * write, and open them at /cloud/office/<id> in the editor, never in
 * Collabora; the page draws a document's headings, marks, lists, table,
 * callout and footnote, with the bar above it; the bar's buttons and the
 * Insert menu change the shared text the box keeps, a block nobody touched
 * staying byte for byte; a component's fields come up under the bar when
 * it is selected; Page setup writes the page into the front matter and the
 * page takes its size; two people on one file see each other's typing; a
 * presentation has its slides in a strip and plays full screen; a
 * document with script in it runs none of it and keeps it as written;
 * the file's menu offers Office; and none of it adds to the policy
 * violations a plain load logs.
 *
 * The box's live-editing room is tests/yroom.mjs, reached through the
 * harness's routeWebSocket on the whole context, so a tab an opener opens
 * reaches it too; the HTTP routes are stubbed with page.route(). */

import assert from './assert.mjs';
import { launch, routeWebSocket, runner, serve } from './harness.mjs';
import { yroom } from './yroom.mjs';

const { origin, close: closeServer } = await serve({ csp: true });
const browser = await launch();
const { check, finish } = runner();

const json = (route, status, body) =>
  route.fulfill({ status, contentType: 'application/json', body: JSON.stringify(body) });

const entry = (id, path, mime = 'text/vnd.losos.paper') => ({
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

/* LosOS Cloud for one person, with a folder holding `taken` names already
 * and `files` listed. Every PUT is kept in `made`, with its body. A room
 * named in `room` answers the live-editing socket. */
async function openCloud(path, { files = [], taken = [], byId = {}, room = null, login = 'matus', name = 'Matus', perm = 'edit' } = {}) {
  const context = await browser.newContext({ viewport: { width: 1280, height: 900 }, locale: 'en-US', colorScheme: 'light' });
  const errors = [];
  // Every tab of the context, the ones an opener opens included.
  context.on('page', (p) => {
    p.on('pageerror', (e) => errors.push(String(e)));
    p.on('dialog', (d) => {
      errors.push(`dialog: ${d.message()}`);
      void d.dismiss();
    });
  });
  const page = await context.newPage();
  const made = [];
  let next = 500;
  await context.route('**/cloud/api/**', async (route) => {
    const req = route.request();
    const url = new URL(req.url());
    const p = url.pathname.slice('/cloud/api'.length);
    if (p === '/me') return json(route, 200, { login, name, email: '', used: 1e9, free: 9e9, product: 'LosOS Cloud' });
    if (p === '/files') return json(route, 200, { folder: { ...entry(1, ''), dir: true }, entries: files });
    if (p === '/files/content' && req.method() === 'PUT') {
      const at = url.searchParams.get('path');
      if (taken.includes(at)) return json(route, 409, { code: 'exists', error: 'exists' });
      const e = entry(next++, at, at.endsWith('.slides') ? 'text/vnd.losos.slides' : 'text/vnd.losos.paper');
      made.push({ path: at, ifNew: url.searchParams.get('ifNew'), body: req.postData(), entry: e });
      byId[e.id] = e;
      return json(route, 201, e);
    }
    const m = /^\/files\/by-id\/(\d+)$/.exec(p);
    if (m) return byId[m[1]] ? json(route, 200, byId[m[1]]) : json(route, 404, { code: 'notFound', error: 'no' });
    if (p === '/collab/doc') {
      const id = url.searchParams.get('file');
      return json(route, 200, {
        name: byId[id]?.name ?? 'x',
        owner: 'matus',
        box: 'mattbox.local',
        perm,
        remote: false,
        me: { name, address: `${login}@mattbox.local` },
      });
    }
    if (p === '/office/open') return json(route, 500, { error: 'not Collabora' });
    return json(route, 200, {});
  });
  if (room !== null) await routeWebSocket(context, /\/cloud\/api\/collab\/ws/, (ws) => room.connect(ws, login));
  await context.addInitScript(() => {
    window.__violations = [];
    document.addEventListener('securitypolicyviolation', (e) => window.__violations.push(`${e.violatedDirective} ${e.blockedURI}`));
  });
  await page.goto(origin + path);
  return { page, context, errors, made };
}

const violations = (page) => page.evaluate(() => [...new Set(window.__violations)].sort());
const text = (page) => page.locator('[data-testid=paper-editor] .ProseMirror');
const bar = (page) => page.getByTestId('paper-toolbar');
const action = (page, key) => bar(page).locator(`[data-action="${key}"]`);

/** Opens `name` from a room holding `source`, and waits until the page is
 * live and drawn. */
async function openPaper(name, source, extra = {}) {
  const room = extra.room ?? yroom(source);
  const byId = { 77: entry(77, name, name.endsWith('.slides') ? 'text/vnd.losos.slides' : 'text/vnd.losos.paper') };
  const opened = await openCloud('/cloud/office/77', { byId, room, ...extra });
  await opened.page.locator('[data-paper-office]').waitFor();
  await opened.page.locator('[data-testid=paper-editor][data-ready]').waitFor();
  await opened.page.locator('[data-testid=office-saved][data-connection=live]').waitFor();
  return { ...opened, room };
}

/** Selects `words` in the page, or with `caret` puts the cursor right after
 * them, and waits until the editor has taken that selection in. Focusing the
 * page first and then waiting matters on a slow machine: the editor puts its
 * own selection back a moment after it gets focus, and a keystroke before it
 * reads the new one lands where the old one was. */
async function place(page, words, caret) {
  for (let tries = 0; tries < 10; tries++) {
    const held = await page.evaluate(
      async ([w, caret]) => {
        const frame = () => new Promise((r) => requestAnimationFrame(() => r()));
        const root = document.querySelector('[data-testid=paper-editor] .ProseMirror');
        if (document.activeElement !== root) {
          root.focus();
          await frame();
          await new Promise((r) => setTimeout(r, 50));
        }
        const walk = document.createTreeWalker(root, NodeFilter.SHOW_TEXT);
        for (let n = walk.nextNode(); n !== null; n = walk.nextNode()) {
          const at = n.data.indexOf(w);
          if (at < 0) continue;
          const r = document.createRange();
          r.setStart(n, caret ? at + w.length : at);
          r.setEnd(n, at + w.length);
          const changed = new Promise((done) => document.addEventListener('selectionchange', () => done(), { once: true }));
          const s = getSelection();
          s.removeAllRanges();
          s.addRange(r);
          await changed;
          await frame();
          await new Promise((done) => setTimeout(done, 50));
          // Still ours once the editor has had its turn.
          const now = getSelection();
          return now.rangeCount === 1 && now.getRangeAt(0).startContainer === n && now.toString() === (caret ? '' : w);
        }
        throw new Error(`no "${w}" on the page`);
      },
      [words, caret],
    );
    if (held) return;
  }
  throw new Error(`the editor kept its own selection over "${words}"`);
}

/** Puts the cursor right after `words` in the page. */
const caretAfter = (page, words) => place(page, words, true);

/** Selects `words` in the page, as a person drags over them. */
const select = (page, words) => place(page, words, false);

/** Waits until the room's text passes `test`. */
async function roomHas(room, test, what) {
  const until = Date.now() + 5000;
  while (Date.now() < until) {
    if (test(room.text())) return;
    await new Promise((r) => setTimeout(r, 50));
  }
  assert.fail(`${what}; the room holds:\n${room.text()}`);
}

const SAMPLE = `# Trip plan

Leave **early** and take *water*.

- Tent
- Stove

| Day | Where |
| --- | --- |
| 1 | Lake |

<Callout type="warning" title="Weather">
Check the forecast.
</Callout>

Kept as it was.[^1]

[^1]: The note.
`;

let BASELINE = [];
{
  const { page, context } = await openCloud('/cloud/files');
  await page.waitForTimeout(400);
  BASELINE = await violations(page);
  await context.close();
}

console.log('paper checks');

await check('New document makes the next free Untitled document.paper and opens it in Office', async () => {
  const { page, context, errors, made } = await openCloud('/cloud/files', { taken: ['Untitled document.paper'] });
  await page.getByRole('button', { name: 'New document' }).click();
  await page.waitForURL(/\/cloud\/office\/\d+$/);
  assert.equal(made.length, 1, JSON.stringify(made));
  assert.equal(made[0].path, 'Untitled document 2.paper');
  assert.equal(made[0].ifNew, '1');
  assert.equal(made[0].body, '# Untitled document\n');
  await page.locator('[data-paper-office=paper]').waitFor();
  await page.getByRole('heading', { level: 1, name: 'Untitled document 2.paper' }).waitFor();
  await page.getByTestId('office-bar').getByText('Document', { exact: true }).waitFor();
  assert.equal(await page.locator('iframe').count(), 0, 'never Collabora');
  assert.deepEqual(errors, []);
  await context.close();
});

await check('New presentation starts with a title slide and one more', async () => {
  const { page, context, made } = await openCloud('/cloud/files');
  await page.getByRole('button', { name: 'New presentation' }).click();
  await page.waitForURL(/\/cloud\/office\/\d+$/);
  assert.equal(made[0].path, 'Untitled presentation.slides');
  assert.match(made[0].body, /^<Slide layout="title">\n# Untitled presentation\n\n.+\n<\/Slide>\n\n<Slide layout="content">\n## .+\n\n- .+\n<\/Slide>\n$/);
  await page.locator('[data-paper-office=slides]').waitFor();
  await context.close();
});

await check('the page draws the document as it looks, under the bar', async () => {
  const { page, context, errors, room } = await openPaper('Trip.paper', SAMPLE);
  const pm = text(page);
  await pm.getByRole('heading', { level: 1, name: 'Trip plan' }).waitFor();
  assert.equal(await pm.locator('strong', { hasText: 'early' }).count(), 1);
  assert.equal(await pm.locator('em', { hasText: 'water' }).count(), 1);
  assert.equal(await pm.locator('li').count(), 2);
  assert.equal(await pm.locator('table td', { hasText: 'Lake' }).count(), 1);
  await pm.getByText('Check the forecast.').waitFor();
  assert.ok(await bar(page).isVisible());
  // The bar is above the page, as in a word processor.
  const barBox = await bar(page).boundingBox();
  const pageBox = await page.getByTestId('paper-page').boundingBox();
  assert.ok(barBox.y + barBox.height <= pageBox.y, 'the bar sits above the page');
  // Opening a file changes nothing in it.
  await page.waitForTimeout(300);
  assert.equal(room.text(), SAMPLE);
  assert.deepEqual(errors, []);
  await context.close();
  room.close();
});

await check('the bar writes MDX into the shared text, and leaves the rest byte for byte', async () => {
  const { page, context, errors, room } = await openPaper('Trip.paper', SAMPLE);
  await select(page, 'Kept');
  await action(page, 'strong').click();
  await roomHas(room, (t) => t.includes('**Kept** as it was.'), 'Bold wrote **Kept**');
  assert.equal(await action(page, 'strong').getAttribute('aria-pressed'), 'true');
  // Everything before the changed paragraph is the file's own text.
  const head = SAMPLE.slice(0, SAMPLE.indexOf('Kept as it was.'));
  assert.ok(room.text().startsWith(head), room.text());
  assert.ok(room.text().endsWith('[^1]: The note.\n'), room.text());

  await select(page, 'water');
  await action(page, 'em').click();
  await roomHas(room, (t) => t.includes('Leave **early** and take water.'), 'Italic came off');

  // The style menu makes a heading.
  await caretAfter(page, 'as it was.');
  await page.getByTestId('paper-style').selectOption('2');
  await roomHas(room, (t) => t.includes('## **Kept** as it was.'), 'the paragraph became a heading');

  // Undo takes it back.
  await action(page, 'undo').click();
  await roomHas(room, (t) => t.includes('\n**Kept** as it was.') && !t.includes('## **Kept**'), 'undo took the heading back');
  assert.deepEqual(errors, []);
  await context.close();
  room.close();
});

await check('Insert puts a component in, its fields come up, and the file has it', async () => {
  const { page, context, errors, room } = await openPaper('Trip.paper', SAMPLE);
  const insert = async (id) => {
    await page.getByTestId('paper-insert').click();
    await page.locator(`[data-insert="${id}"]`).click();
  };

  await caretAfter(page, 'as it was.');
  await insert('callout');
  await roomHas(room, (t) => /<Callout type="note">\n[^]*\n<\/Callout>/.test(t), 'a callout was written');
  const fields = page.getByTestId('paper-fields');
  await fields.waitFor();
  assert.equal(await fields.getAttribute('data-target'), 'callout');
  await fields.locator('select[data-field=type], [data-field=type] select').selectOption('tip');
  await roomHas(room, (t) => t.includes('<Callout type="tip">'), 'the kind changed to tip');

  await caretAfter(page, 'as it was.');
  await insert('table');
  await roomHas(room, (t) => /\n\| .* \|\n\| --- \|/.test(t.slice(t.indexOf('as it was.'))), 'a pipe table was written');
  await page.getByTestId('paper-table-tools').waitFor();

  await caretAfter(page, 'as it was.');
  await insert('pageBreak');
  await roomHas(room, (t) => t.includes('<PageBreak />'), 'a page break was written');

  await caretAfter(page, 'as it was.');
  await insert('date');
  await roomHas(room, (t) => /<Date value="\d{4}-\d\d-\d\d" format="long" \/>/.test(t), 'a date was written');

  // What was there before is still the file's own text.
  assert.ok(room.text().startsWith(SAMPLE.slice(0, SAMPLE.indexOf('<Callout'))), room.text());
  assert.deepEqual(errors, []);
  await context.close();
  room.close();
});

await check('equations are MathML and dates words, from losos-mdx, as the HTML without script has them', async () => {
  const doc = 'Half is $\\frac{1}{2}$.\n\n<Equation tex="e = mc^2" />\n\nDue <Date value="2026-10-28" format="long" />.\n';
  const { page, context, errors, room } = await openPaper('Maths.paper', doc);
  const paper = page.getByTestId('paper-editor');
  await paper.locator('.paper-math math mfrac').waitFor();
  await paper.locator('.paper-equation math[display=block] annotation[encoding="application/x-tex"]').waitFor({ state: 'attached' });
  assert.equal(await paper.locator('time.mdx-date').textContent(), '28 October 2026');
  assert.equal(await paper.locator('code.language-math').count(), 0);
  assert.equal(room.text(), doc, 'drawing them changed nothing in the file');
  assert.deepEqual(errors, []);
  await context.close();
  room.close();

  // A document in German has its dates in German, whoever reads it.
  const de = await openPaper('Termin.paper', '---\nlang: de\n---\nAm <Date value="2026-03-04" format="long" />.\n');
  assert.equal(await de.page.getByTestId('paper-editor').locator('time.mdx-date').textContent(), '4. März 2026');
  assert.deepEqual(de.errors, []);
  await de.context.close();
  de.room.close();
});

await check('Page setup writes the page into the front matter and the page takes its size', async () => {
  const { page, context, errors, room } = await openPaper('Trip.paper', SAMPLE);
  assert.equal(await page.getByTestId('paper-page').getAttribute('data-size'), 'A4');
  const portrait = await page.getByTestId('paper-page').boundingBox();
  await action(page, 'pageSetup').click();
  const dialog = page.getByTestId('paper-page-setup');
  await dialog.locator('select[data-field=size]').selectOption('Letter');
  await dialog.locator('select[data-field=orientation]').selectOption('landscape');
  await dialog.locator('[data-action=apply]').click();
  await roomHas(room, (t) => t.startsWith('---\n') && /size: Letter/.test(t) && /orientation: landscape/.test(t), 'the front matter has the page');
  assert.ok(room.text().endsWith(SAMPLE), 'the document under it is unchanged');
  await page.locator('[data-testid=paper-page][data-size=Letter][data-orientation=landscape]').waitFor();
  const landscape = await page.getByTestId('paper-page').boundingBox();
  assert.ok(landscape.width > portrait.width, `${landscape.width} > ${portrait.width}`);
  assert.deepEqual(errors, []);
  await context.close();
  room.close();
});

await check('two people on one document see each other type', async () => {
  const room = yroom(SAMPLE);
  const a = await openPaper('Trip.paper', SAMPLE, { room });
  const b = await openPaper('Trip.paper', SAMPLE, { room, login: 'eva', name: 'Eva' });
  await caretAfter(a.page, 'Stove');
  await a.page.keyboard.press('Enter');
  await a.page.keyboard.insertText('Matches');
  await text(b.page).locator('li', { hasText: 'Matches' }).waitFor();
  await caretAfter(b.page, 'The note.');
  await b.page.keyboard.insertText(' From Eva.');
  await text(a.page).getByText('The note. From Eva.').waitFor();
  await roomHas(room, (t) => t.includes('- Stove\n- Matches\n') && t.includes('[^1]: The note. From Eva.'), 'both changes are in the file');
  await a.page.getByTestId('office-bar').locator('[data-face]').nth(1).waitFor();
  for (const { context, errors } of [a, b]) {
    assert.deepEqual(errors, []);
    await context.close();
  }
  room.close();
});

const DECK = `<Slide layout="title">
# Trip plan

Matus
</Slide>

<Slide layout="content">
## Gear

- Tent
- Stove

<Notes>
Mention the stove.
</Notes>
</Slide>

<Slide layout="content">
## Route
</Slide>
`;

await check('a presentation has its slides in a strip and plays full screen', async () => {
  const { page, context, errors, room } = await openPaper('Trip.slides', DECK);
  const strip = page.getByTestId('paper-filmstrip');
  await strip.waitFor();
  assert.equal(await strip.locator('button').count(), 3);
  // A new slide from the bar lands in the file.
  await caretAfter(page, 'Route');
  await bar(page).locator('[data-action=slide]').click();
  await roomHas(room, (t) => (t.match(/<Slide/g) ?? []).length === 4, 'a fourth slide was written');
  await page.waitForFunction(() => document.querySelectorAll('[data-testid=paper-filmstrip] button').length === 4);

  // Present starts at the slide the cursor is in, the new fourth one.
  await page.getByTestId('paper-present').click();
  const presenter = page.getByTestId('paper-presenter');
  await presenter.waitFor();
  await presenter.getByText('4 / 4').waitFor();
  await page.keyboard.press('Home');
  await presenter.getByRole('heading', { name: 'Trip plan' }).waitFor();
  await page.keyboard.press('ArrowRight');
  await presenter.getByRole('heading', { name: 'Gear' }).waitFor();
  await page.keyboard.press('Escape');
  await presenter.waitFor({ state: 'detached' });
  assert.deepEqual(errors, []);
  await context.close();
  room.close();
});

await check('script in a document runs nowhere and is kept as written', async () => {
  const HOSTILE = `# Hello

<script>window.__ran = 1</script>

<img src="x" onerror="window.__ran = 2" />

{window.__ran = 3}

<Callout type="note" onClick="window.__ran = 4">Boo</Callout>

<a href="javascript:window.__ran = 5">click</a>

export const x = (window.__ran = 6)
`;
  const { page, context, errors, room } = await openPaper('Bad.paper', HOSTILE);
  await text(page).getByRole('heading', { name: 'Hello' }).waitFor();
  await page.waitForTimeout(500);
  // Clicking everything on the page still runs nothing.
  for (const el of await text(page).locator('a, img, .paper-raw').all()) await el.click({ force: true }).catch(() => {});
  await page.waitForTimeout(300);
  assert.equal(await page.evaluate(() => window.__ran), undefined, 'nothing ran');
  assert.equal(await text(page).locator('script, img[onerror], [onclick]').count(), 0);
  assert.ok((await text(page).locator('.paper-raw').count()) >= 3, 'the elements are kept as written');
  assert.equal(room.text(), HOSTILE, 'the file is untouched');
  assert.deepEqual(errors, []);
  await context.close();
  room.close();
});

await check('a .paper opens in Office from its menu, with its text one item away, adding no policy violations', async () => {
  const files = [entry(77, 'Trip.paper'), entry(78, 'Talk.slides', 'text/vnd.losos.slides')];
  const room = yroom(SAMPLE);
  const { page, context, errors } = await openCloud('/cloud/files', { files, byId: { 77: files[0], 78: files[1] }, room });
  const row = page.locator('[data-entry="Trip.paper"]');
  await row.waitFor();
  await row.click({ button: 'right' });
  // Office is the file's own opener; its source stays one item away.
  await page.getByRole('menuitem', { name: 'Edit together' }).waitFor();
  const item = page.getByRole('menuitem', { name: 'Open', exact: true });
  await item.waitFor();
  // Openers open the file in a tab of its own.
  const [tab] = await Promise.all([context.waitForEvent('page'), item.click()]);
  await tab.waitForURL(/\/cloud\/office\/77$/);
  await tab.locator('[data-testid=paper-editor][data-ready]').waitFor();
  await tab.locator('[data-testid=office-saved][data-connection=live]').waitFor();
  await tab.waitForTimeout(400);
  assert.deepEqual(await violations(tab), BASELINE);
  assert.deepEqual(await violations(page), BASELINE);
  assert.deepEqual(errors, []);
  await context.close();
  room.close();
});

await browser.close();
await closeServer();
finish();
