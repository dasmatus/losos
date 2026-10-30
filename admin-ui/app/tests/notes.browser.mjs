/* Notes in LosOS Cloud (src/cloud/notes/, src/notes/) and a knowledge base
 * (src/kb/), on the kitchen garden vault in tests/fixtures/vault/.
 *
 * What is checked: the renderer (losos-mdx in WebAssembly) draws a .note
 * as MDX, its components beside Obsidian's syntax, and runs nothing a
 * .note carries (no script, handler, expression or javascript: link, each
 * named among the note's problems); a .md converts to a .note that reads
 * the same; and it draws GitHub's Markdown (tables, task
 * lists, strikethrough, footnotes, autolinks, alerts) and Obsidian's
 * (wikilinks with aliases, headings and blocks, embeds of a section and of
 * a picture, callouts that fold, tags, ==highlights==, %%comments%%, maths,
 * properties); links resolve as Obsidian resolves them and a missing one
 * makes the note; backlinks, tags, search, the mind map and the graph work;
 * a ticked box writes its line back with the version it read; a folder's
 * right click offers "Publish as knowledge base" only when the box says
 * knowledge bases are available, and the dialog publishes; a knowledge base
 * reads anonymously, leaves private notes out, sends a reader of a box-only
 * site to LosOS Login and says when there is none; and none of it raises a
 * Content-Security-Policy violation under the admin policy.
 *
 * The real dist/ bundle under the real policy, the box's API stubbed with
 * page.route(). The index the box would list is made here from the
 * fixture with a small scan; the box's own scanner has its tests in
 * backend/src/cloud/notes/scan.rs, and nix/tests/notes.nix runs both
 * halves together. */

import assert from './assert.mjs';
import { launch, runner, serve } from './harness.mjs';

const { origin, close: closeServer } = await serve({ csp: true });
const browser = await launch();
const { check, finish } = runner();

const FIXTURE = 'tests/fixtures/vault';

const files = new Map();
for await (const rel of new Bun.Glob('**/*').scan({ cwd: FIXTURE, dot: true })) {
  files.set(rel, await Bun.file(`${FIXTURE}/${rel}`).text());
}
files.set(
  'Garden plan.canvas',
  JSON.stringify({
    nodes: [
      { id: 'g', type: 'group', label: 'Summer', x: 0, y: 0, width: 900, height: 500 },
      { id: 'a', type: 'text', text: '## Beds\nRotate every year. See [[Compost]].', x: 20, y: 40, width: 300, height: 200 },
      { id: 'b', type: 'file', file: 'Tomatoes.md', x: 400, y: 40, width: 300, height: 200 },
      { id: 'c', type: 'file', file: 'attachments/tomato.svg', x: 20, y: 300, width: 200, height: 160 },
      { id: 'd', type: 'link', url: 'https://example.org/seeds', x: 400, y: 300, width: 300, height: 100 },
    ],
    edges: [{ id: 'e', fromNode: 'a', toNode: 'b' }],
  }),
);

/* A small stand-in for backend/src/cloud/notes/scan.rs. */
function scan(path, text) {
  if (path.endsWith('.canvas')) {
    const data = JSON.parse(text);
    const links = [];
    for (const n of data.nodes) {
      if (n.type === 'file') links.push({ target: n.file, embed: false });
      if (n.type === 'text') for (const m of n.text.matchAll(/\[\[([^\]|#]+)/g)) links.push({ target: m[1], embed: false });
    }
    return { title: null, aliases: [], tags: [], links, private: false };
  }
  const front = /^---\n([\s\S]*?)\n---\n/.exec(text);
  const yaml = front?.[1] ?? '';
  const body = front === null ? text : text.slice(front[0].length);
  const list = (key) => {
    const m = new RegExp(`^${key}:\\s*(.*)$`, 'm').exec(yaml);
    if (m === null) return [];
    return m[1].replace(/^\[|\]$/g, '').split(',').map((s) => s.trim()).filter(Boolean);
  };
  const tags = new Set(list('tags'));
  const prose = body.replace(/```[\s\S]*?```/g, '').replace(/`[^`]*`/g, '');
  for (const m of prose.matchAll(/(?:^|\s)#([\p{L}_\-/][\p{L}\p{N}_\-/]*)/gu)) tags.add(m[1]);
  const links = [...prose.matchAll(/(!?)\[\[([^\]|#]+)/g)].map((m) => ({ target: m[2], embed: m[1] === '!' }));
  return {
    title: null,
    aliases: list('aliases'),
    tags: [...tags],
    links,
    private: /^private:\s*true\s*$/m.test(yaml),
  };
}

let nextId = 100;
const ids = new Map();
const idOf = (p) => {
  if (!ids.has(p)) ids.set(p, nextId++);
  return ids.get(p);
};
const etags = new Map();
const etagOf = (p) => etags.get(p) ?? 'e1';

function index({ root, name, published = false }) {
  const notes = [];
  const canvases = [];
  const attachments = [];
  const folders = new Set();
  for (const [p, text] of files) {
    if (p.includes('/')) folders.add(p.slice(0, p.lastIndexOf('/')));
    const row = { id: idOf(p), path: p, size: text.length, mtime: 1, etag: etagOf(p) };
    if (p.endsWith('.md') || p.endsWith('.note')) {
      const s = scan(p, text);
      if (published && s.private) continue;
      notes.push({ ...row, ...s });
    } else if (p.endsWith('.canvas')) canvases.push({ ...row, ...scan(p, text) });
    else attachments.push({ id: idOf(p), path: p, mime: 'image/svg+xml', size: text.length });
  }
  return { root, name, folderId: 7, notes, canvases, files: attachments, folders: [...folders].sort() };
}

function search(q) {
  const words = q.toLowerCase().split(/\s+/).filter(Boolean);
  const hits = [];
  for (const [p, text] of files) {
    if (!p.endsWith('.md') && !p.endsWith('.note')) continue;
    text.split('\n').forEach((line, i) => {
      if (words.every((w) => line.toLowerCase().includes(w))) hits.push({ path: p, line: i, text: line.trim() });
    });
  }
  return hits;
}

const json = (route, status, body) => route.fulfill({ status, contentType: 'application/json', body: JSON.stringify(body) });

async function open(path, { kb = { available: true, sites: [] }, kbSite = 'anyone', viewport = { width: 1440, height: 1000 }, vault = 'garden' } = {}) {
  const context = await browser.newContext({ viewport, locale: 'en-US' });
  const page = await context.newPage();
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  await page.addInitScript(() => {
    window.__violations = [];
    document.addEventListener('securitypolicyviolation', (e) =>
      window.__violations.push(`${e.violatedDirective} ${e.blockedURI}`),
    );
  });
  const writes = [];
  const posts = [];
  const moves = [];
  // 'garden' is the fixture; 'new' is a box that has no Notes folder yet,
  // which the page makes, and then lists as an empty vault.
  let made = vault === 'garden';
  await page.route('**/cloud/api/**', async (route) => {
    const req = route.request();
    const url = new URL(req.url());
    const p = url.pathname.slice('/cloud/api'.length);
    if (p === '/me') return json(route, 200, { login: 'matus', name: 'Matus', email: '', used: 1e9, free: 9e9, product: 'LosOS Cloud' });
    if (p === '/notes/vault') {
      if (url.searchParams.get('path') !== 'Notes' || !made) return json(route, 404, { code: 'notFound', error: 'no such folder' });
      if (vault === 'new') return json(route, 200, { root: 'Notes', name: 'Notes', folderId: 7, notes: [], canvases: [], files: [], folders: [] });
      return json(route, 200, index({ root: 'Notes', name: 'Notes' }));
    }
    if (p === '/files/folder' && req.method() === 'POST') {
      made = true;
      return json(route, 201, { id: 7, path: 'Notes', name: 'Notes', dir: true, size: 0, mtime: 1, etag: 'x', mime: '', favorite: false });
    }
    if (p === '/notes/search') return json(route, 200, { hits: search(url.searchParams.get('q') ?? '') });
    if (p === '/notes/root') return json(route, 200, { root: 'Notes' });
    if (p === '/files/content') {
      const rel = (url.searchParams.get('path') ?? '').replace(/^Notes\//, '');
      if (req.method() === 'PUT') {
        writes.push({ path: rel, body: req.postData() ?? '', ifMatch: url.searchParams.get('ifMatch'), ifNew: url.searchParams.get('ifNew') });
        files.set(rel, req.postData() ?? '');
        etags.set(rel, `e${writes.length + 1}`);
        return json(route, 201, {});
      }
      const text = files.get(rel);
      if (text === undefined) return json(route, 404, { code: 'notFound', error: 'no such file' });
      return route.fulfill({ status: 200, contentType: rel.endsWith('.svg') ? 'image/svg+xml' : 'text/markdown', headers: { etag: `"${etagOf(rel)}"` }, body: text });
    }
    if (p === '/files/move' && req.method() === 'POST') {
      const body = JSON.parse(req.postData() ?? '{}');
      moves.push(body);
      const from = body.from.replace(/^Notes\//, '');
      const to = body.to.replace(/^Notes\//, '');
      if (files.has(to) && !body.overwrite) return json(route, 409, { code: 'exists', error: 'already there' });
      files.set(to, files.get(from));
      files.delete(from);
      return json(route, 200, { id: idOf(to), path: body.to, name: to, dir: false, size: 0, mtime: 1, etag: 'x', mime: 'text/markdown', favorite: false });
    }
    if (p === '/kb' && req.method() === 'GET') return json(route, 200, kb);
    if (p === '/kb' && req.method() === 'POST') {
      const body = JSON.parse(req.postData() ?? '{}');
      posts.push(body);
      const site = { id: 1, folderId: 7, folder: body.path, slug: 'kitchen-garden', title: body.title, audience: body.audience, created: 1 };
      kb.sites = [site];
      return json(route, 201, site);
    }
    if (p === '/trash') return json(route, 200, { entries: [], days: 30 });
    if (p === '/files') {
      const entry = (name, dir) => ({ id: dir ? 7 : 8, path: name, name, dir, size: 0, mtime: 1, etag: 'x', mime: dir ? '' : 'text/plain', favorite: false });
      return json(route, 200, { folder: entry('', true), entries: [entry('Notes', true), entry('todo.txt', false)] });
    }
    return json(route, 200, {});
  });
  await page.route('**/kb/api/**', async (route) => {
    const url = new URL(route.request().url());
    const [, , , slug, call] = url.pathname.split('/');
    if (slug !== 'kitchen-garden') return json(route, 404, { code: 'notFound', error: 'no such site' });
    if (kbSite === 'box') return json(route, 401, { code: 'signIn', error: 'sign in' });
    const published = index({ root: '', name: 'Kitchen garden', published: true });
    if (call === 'site') return json(route, 200, { slug, title: 'Kitchen garden', audience: 'anyone', vault: published });
    if (call === 'search') return json(route, 200, { hits: search(url.searchParams.get('q') ?? '') });
    const rel = url.searchParams.get('path') ?? '';
    const listed = [...published.notes, ...published.canvases, ...published.files].some((n) => n.path === rel);
    if (!listed) return json(route, 404, { code: 'notFound', error: 'not published' });
    return route.fulfill({ status: 200, contentType: rel.endsWith('.svg') ? 'image/svg+xml' : 'text/markdown', body: files.get(rel) });
  });
  await page.route('**/auth/**', (route) => route.fulfill({ status: 200, contentType: 'text/html', body: '<p>LosOS Login</p>' }));
  await page.goto(origin + path);
  return { page, context, errors, writes, posts, moves };
}

const violations = (page) => page.evaluate(() => [...new Set(window.__violations)].sort());

const ALL_VIOLATIONS = new Set();

/* What a plain load of the file list logs, which should be nothing: Notes
 * and the knowledge base must not add to it. */
const BASELINE = await (async () => {
  const { page, context } = await open('/cloud/files');
  await page.waitForSelector('[data-entry="Notes"]');
  await page.waitForTimeout(300);
  const seen = await violations(page);
  await context.close();
  return seen;
})();

await check('a new box makes its Notes folder and offers the first note', async () => {
  const { page, context, errors } = await open('/cloud/notes', { vault: 'new' });
  await page.waitForSelector('[data-notes-empty]', { timeout: 10000 });
  assert.equal(await page.locator('[data-notes-empty] [data-new-note]').count(), 1);
  assert.deepEqual(errors, []);
  await context.close();
});

await check('a note reads with GitHub and Obsidian Markdown', async () => {
  const { page, context, errors } = await open('/cloud/notes?vault=Notes');
  try {
    await page.waitForSelector('[data-note="Home.md"] .note-body h1');
    assert.equal(await page.textContent('[data-note-title]'), 'Home');
    // Properties from the front matter.
    assert.match(await page.textContent('[data-properties]'), /season\s*2026/);
    // In the order the note gives them.
    assert.deepEqual(await page.locator('[data-properties] dt').allTextContents(), ['title', 'tags', 'aliases', 'season']);
    // A callout with its own title, and wikilinks that resolve.
    assert.equal(await page.textContent('.callout[data-callout="tip"] .callout-title'), 'Where to begin');
    // One dot before a callout's title, whichever way it was written.
    const dots = (sel) => page.$eval(sel, (t) => t.querySelectorAll('.callout-icon').length + (getComputedStyle(t, '::before').content === 'none' ? 0 : 1));
    assert.equal(await dots('.callout[data-callout="tip"] .callout-title'), 1);
    assert.equal(await page.getAttribute('a.wikilink:text("Tomatoes")', 'data-note-path'), 'Tomatoes.md');
    assert.equal(await page.getAttribute('a.wikilink:text("Aphids")', 'data-note-path'), 'Pests/Aphids.md');
    // [[Beans|runner beans]] has no note: shown as one still to write.
    assert.equal(await page.getAttribute('a.is-unresolved', 'data-note-target'), 'Beans');
    assert.equal(await page.textContent('a.is-unresolved'), 'runner beans');
    // GFM: a task list, a table, an autolink.
    assert.equal(await page.locator('.note-body input.task-box').count(), 4);
    assert.equal(await page.locator('.note-body input.task-box:checked').count(), 1);
    assert.equal(await page.locator('.note-body table tr').count(), 5);
    assert.equal(await page.getAttribute('.note-body a[href="https://www.rhs.org.uk"]', 'target'), '_blank');
    // GitHub's www. autolink, and a file name that is not a web address.
    assert.equal(await page.getAttribute('.note-body a[href="https://www.seedcooperative.org.uk"]', 'target'), '_blank');
    assert.equal(await page.locator('.note-body a:text("Home.md")').count(), 0);
    assert.equal(await page.getAttribute('.note-body a.tag', 'data-tag'), 'pests');
    // Backlinks: Home links out, nothing links in.
    assert.match(await page.textContent('[data-side="outgoing"]'), /Tomatoes/);
    assert.match(await page.textContent('[data-side="missing"]'), /Beans/);
    assert.deepEqual(errors, []);
    for (const v of await violations(page)) ALL_VIOLATIONS.add(v);
  } finally {
    await context.close();
  }
});

await check('links, embeds, folds, footnotes and backlinks', async () => {
  const { page, context, errors } = await open('/cloud/notes?vault=Notes');
  try {
    await page.click('.note-body a.wikilink:text("Tomatoes")');
    await page.waitForSelector('[data-note="Tomatoes.md"] .note-body h1');
    assert.match(page.url(), /note=Tomatoes\.md/);
    // A link to a heading with an alias.
    const compost = page.locator('a.wikilink:text("hot compost")');
    assert.equal(await compost.getAttribute('data-note-path'), 'Compost.md');
    assert.equal(await compost.getAttribute('data-sub'), 'Hot compost');
    // ![[Planting calendar#April]]: only that section, filled in.
    const embed = page.locator('[data-embed-of="Planting calendar.md"]');
    await embed.waitFor();
    const embedded = await embed.textContent();
    assert.match(embedded, /Direct sow carrots/);
    assert.doesNotMatch(embedded, /chillies/);
    // A folded callout is a <details>, closed.
    assert.equal(await page.locator('details.callout[data-callout="warning"]').count(), 1);
    assert.equal(await page.locator('details.callout[open]').count(), 0);
    // Strikethrough, footnote, the picture from the attachments folder.
    assert.equal(await page.textContent('.note-body del'), '12 kg');
    assert.equal(await page.locator('.note-body .footnotes li').count(), 1);
    assert.match(await page.getAttribute('.note-body img', 'src'), /files\/content\?path=Notes%2Fattachments%2Ftomato\.svg/);
    // Who links here.
    const back = await page.textContent('[data-side="backlinks"]');
    for (const name of ['Home', 'Compost', 'Planting calendar', 'Aphids', 'Slugs']) assert.match(back, new RegExp(name));
    // The outline jumps to a heading.
    await page.click('[data-side="outline"] button:text("Problems")');
    await page.waitForFunction(() => location.hash === '#problems');
    assert.deepEqual(errors, []);
    for (const v of await violations(page)) ALL_VIOLATIONS.add(v);
  } finally {
    await context.close();
  }
});

await check('maths, code, alerts, block ids, highlights and comments', async () => {
  const { page, context, errors } = await open('/cloud/notes?vault=Notes&note=Compost.md');
  try {
    await page.waitForSelector('[data-note="Compost.md"] .note-body h1');
    assert.ok((await page.locator('.note-body math').count()) >= 2, 'inline and block maths');
    assert.equal(await page.locator('.note-body .math-block').count(), 1);
    assert.ok((await page.locator('.note-body pre code .hljs-comment').count()) >= 1, 'code is coloured');
    assert.equal(await page.locator('.callout[data-callout="note"]').count(), 1, '> [!NOTE] is a callout');
    assert.equal(await page.locator('.note-body [id="^ratio"]').count(), 1);
    await page.goto(`${origin}/cloud/notes?vault=Notes&note=Planting%20calendar.md`);
    await page.waitForSelector('[data-note="Planting calendar.md"] .note-body h1');
    assert.doesNotMatch(await page.textContent('.note-body'), /shade from the shed/);
    await page.goto(`${origin}/cloud/notes?vault=Notes&note=Pests%2FAphids.md`);
    await page.waitForSelector('[data-note="Pests/Aphids.md"] .note-body mark');
    assert.equal(await page.textContent('.note-body mark'), 'Squash them early');
    assert.equal(await page.textContent('[data-note-folder]'), 'Notes / Pests');
    assert.deepEqual(errors, []);
    for (const v of await violations(page)) ALL_VIOLATIONS.add(v);
  } finally {
    await context.close();
  }
});

await check('a .note reads as MDX, its components beside Obsidian\'s syntax', async () => {
  const { page, context, errors, writes } = await open('/cloud/notes?vault=Notes&note=Harvest.note');
  try {
    await page.waitForSelector('[data-note="Harvest.note"] .note-body h1');
    assert.equal(await page.textContent('[data-note-title]'), 'Harvest');
    assert.match(await page.textContent('[data-properties]'), /season\s*2026/);
    // <Callout>, <Tabs> and <Details>, drawn by the crate.
    assert.equal(await page.textContent('aside.callout[data-callout="tip"] .callout-title'), 'Pick in the morning');
    assert.equal(await page.$eval('aside.callout .callout-title', (t) => getComputedStyle(t, '::before').content === 'none' ? 0 : 1), 1, 'a <Callout> has its dot');
    assert.deepEqual(await page.locator('.mdx-tabs .mdx-tab').evaluateAll((els) => els.map((e) => e.dataset.label)), ['Roots', 'Fruit']);
    assert.equal(await page.locator('details.mdx-details table tr').count(), 3);
    // Obsidian's syntax inside and around them.
    assert.equal(await page.getAttribute('a.wikilink:text("the varieties")', 'data-note-path'), 'Tomatoes.md');
    assert.equal(await page.getAttribute('a.wikilink:text("the varieties")', 'data-sub'), 'Varieties');
    assert.equal(await page.getAttribute('aside.callout a.tag', 'data-tag'), 'harvest');
    assert.equal(await page.textContent('.note-body mark'), 'tomatoes');
    assert.equal(await page.locator('[data-problems]').count(), 0);
    assert.equal(await page.locator('.note-body input.task-box:checked').count(), 1);
    // A box below the front matter writes its own line of the file.
    const written = page.waitForResponse((r) => r.request().method() === 'PUT' && r.url().includes('/files/content'));
    await page.locator('.note-body input.task-box').nth(1).click();
    await written;
    assert.equal(writes[0].path, 'Harvest.note');
    assert.equal(writes[0].body.split('\n')[38], '- [x] Dry the bean seed for next year');
    // The outline is the note's headings.
    assert.match(await page.textContent('[data-side="outline"]'), /Stores[\s\S]*Still to do/);
    assert.deepEqual(errors, []);
    for (const v of await violations(page)) ALL_VIOLATIONS.add(v);
  } finally {
    files.set('Harvest.note', await Bun.file(`${FIXTURE}/Harvest.note`).text());
    etags.delete('Harvest.note');
    await context.close();
  }
});

await check('a .note runs nothing it carries and says what it refused', async () => {
  files.set(
    'Unsafe.note',
    [
      '# Unsafe',
      '',
      'import x from "./x.js"',
      '',
      'export const y = 1',
      '',
      '<script>window.__ran = "script"</script>',
      '',
      'Sum {window.__ran = "expression"} here.',
      '',
      '<Callout type="tip" onClick="window.__ran = \'handler\'">Hi</Callout>',
      '',
      '<Figure src="javascript:window.__ran=1" alt="x" />',
      '',
      '[click](javascript:window.__ran="link") and <Losos />',
      '',
      '<losos:wikilink link="Home">forged</losos:wikilink>',
    ].join('\n'),
  );
  files.set('Broken.note', '# Broken\n\n<Callout type="tip">\nnever closed\n');
  const { page, context, errors } = await open('/cloud/notes?vault=Notes&note=Unsafe.note');
  try {
    await page.waitForSelector('[data-note="Unsafe.note"] .note-body h1');
    await page.waitForSelector('[data-problems]');
    await page.click('.note-body a:text("click")').catch(() => {});
    await page.waitForTimeout(300);
    assert.equal(await page.evaluate(() => window.__ran), undefined, 'nothing ran');
    assert.equal(await page.locator('.note-body script, .note-body [onclick], .note-body a[href^="javascript"], .note-body img[src^="javascript"]').count(), 0);
    assert.equal(await page.locator('.note-body a[data-note-path]').count(), 0, 'no forged wikilink');
    const problems = await page.textContent('[data-problems]');
    for (const what of [/import/i, /export/i, /script/i, /\{/, /onClick/, /javascript/, /Losos/, /reserved/]) assert.match(problems, what);
    // A .note that is not MDX at all is still drawn, as Markdown, and says why.
    await page.goto(`${origin}/cloud/notes?vault=Notes&note=Broken.note`);
    await page.waitForSelector('[data-note="Broken.note"] .note-body h1');
    assert.match(await page.textContent('[data-problems]'), /Line \d/);
    assert.match(await page.textContent('.note-body'), /never closed/);
    assert.deepEqual(errors, []);
    for (const v of await violations(page)) ALL_VIOLATIONS.add(v);
  } finally {
    files.delete('Unsafe.note');
    files.delete('Broken.note');
    await context.close();
  }
});

await check('a .md note converts to a .note that reads the same', async () => {
  const before = files.get('Compost.md');
  const { page, context, writes, moves } = await open('/cloud/notes?vault=Notes&note=Compost.md');
  try {
    await page.waitForSelector('[data-note="Compost.md"] .note-body h1');
    const headings = await page.locator('.note-body :is(h1, h2, h3)').allTextContents();
    const maths = await page.locator('.note-body math').count();
    await page.click('[data-convert-note]');
    await page.waitForFunction(() => location.search.includes('note=Compost.note'));
    await page.waitForSelector('[data-note="Compost.note"] .note-body h1');
    // Written over the .md with the version it read, then moved.
    assert.ok(writes.every((w) => w.path === 'Compost.md' && w.ifMatch === 'e1'), JSON.stringify(writes));
    assert.deepEqual(moves.map((m) => [m.from, m.to]), [['Notes/Compost.md', 'Notes/Compost.note']]);
    assert.equal(await page.locator('[data-convert-note]').count(), 0, 'a .note has nothing to convert');
    assert.deepEqual(await page.locator('.note-body :is(h1, h2, h3)').allTextContents(), headings);
    assert.equal(await page.locator('.note-body math').count(), maths);
    assert.equal(await page.locator('[data-problems]').count(), 0);
  } finally {
    files.delete('Compost.note');
    files.set('Compost.md', before);
    etags.delete('Compost.md');
    await context.close();
  }
});

await check('a ticked box writes its line back with the version it read', async () => {
  const before = files.get('Home.md');
  const { page, context, writes } = await open('/cloud/notes?vault=Notes');
  try {
    await page.waitForSelector('.note-body input.task-box');
    // The box ticks at once; the write follows a fresh read of the file.
    const written = page.waitForResponse((r) => r.request().method() === 'PUT' && r.url().includes('/files/content'));
    await page.locator('.note-body input.task-box').nth(1).click();
    await page.waitForFunction(() => document.querySelectorAll('.note-body input.task-box:checked').length === 2);
    await written;
    assert.equal(writes.length, 1);
    assert.equal(writes[0].path, 'Home.md');
    assert.equal(writes[0].ifMatch, 'e1');
    assert.match(writes[0].body, /^- \[x\] Pot on the tomato seedlings$/m);
    // Nothing else in the file moved.
    assert.equal(writes[0].body.replace('- [x] Pot on', '- [ ] Pot on'), before);
  } finally {
    files.set('Home.md', before);
    etags.delete('Home.md');
    await context.close();
  }
});

await check('a link to a note not written yet makes it', async () => {
  const { page, context, writes } = await open('/cloud/notes?vault=Notes');
  try {
    await page.click('a.is-unresolved');
    // A new note is a .note.
    await page.waitForFunction(() => location.search.includes('note=Beans.note') && location.search.includes('view=edit'));
    assert.equal(writes.length, 1);
    assert.equal(writes[0].path, 'Beans.note');
    assert.equal(writes[0].ifNew, '1');
  } finally {
    files.delete('Beans.note');
    await context.close();
  }
});

await check('tags, search, the mind map and the graph', async () => {
  const { page, context, errors } = await open('/cloud/notes?vault=Notes&note=Tomatoes.md');
  try {
    await page.waitForSelector('[data-note="Tomatoes.md"] .note-body h1');
    await page.click('[data-tag-chip="pests"]');
    await page.waitForSelector('[data-tag-view="pests"]');
    const tagged = await page.textContent('[data-tag-view="pests"]');
    for (const name of ['Aphids', 'Slugs', 'Home']) assert.match(tagged, new RegExp(name));
    await page.fill('[data-notes-search]', 'ladybirds');
    await page.waitForSelector('[data-search-hits] a');
    assert.match(await page.textContent('[data-search-hits]'), /Aphids/);
    await page.fill('[data-notes-search]', '');

    await page.goto(`${origin}/cloud/notes?vault=Notes&note=Tomatoes.md&view=map`);
    await page.waitForSelector('[data-mindmap] svg');
    const map = await page.textContent('[data-mindmap]');
    for (const word of ['Tomatoes', 'Varieties', 'Sungold', 'Sowing', 'Problems']) assert.match(map, new RegExp(word));
    await page.click('[data-mindmap] [role="link"][aria-label="Problems"]');
    await page.waitForFunction(() => !location.search.includes('view=map') && location.hash === '#problems');

    await page.click('[data-graph-button]');
    await page.waitForSelector('[data-vault-graph] [data-graph]');
    const nodes = Number(await page.getAttribute('[data-vault-graph] [data-graph]', 'data-nodes'));
    assert.ok(nodes >= 8, `the graph has the vault's notes (${nodes})`);
    await page.click('[data-graph-option="tags"]');
    await page.waitForFunction((n) => Number(document.querySelector('[data-vault-graph] [data-graph]')?.getAttribute('data-nodes')) > n, nodes);
    // The list behind the canvas opens a note from the keyboard.
    await page.focus('[data-vault-graph] [data-graph] ul button:text("Compost")');
    await page.keyboard.press('Enter');
    await page.waitForSelector('[data-note="Compost.md"] .note-body h1');
    assert.deepEqual(errors, []);
    for (const v of await violations(page)) ALL_VIOLATIONS.add(v);
  } finally {
    await context.close();
  }
});

await check('a canvas reads as its cards in order', async () => {
  const { page, context, errors } = await open('/cloud/notes?vault=Notes&note=Garden%20plan.canvas');
  try {
    await page.waitForSelector('[data-canvas]');
    assert.equal(await page.textContent('[data-canvas] h2'), 'Summer');
    const kinds = await page.$$eval('[data-canvas-card]', (els) => els.map((e) => e.getAttribute('data-canvas-card')));
    assert.deepEqual(kinds, ['text', 'note', 'image', 'link']);
    await page.click('[data-canvas-card="note"]');
    await page.waitForSelector('[data-note="Tomatoes.md"] .note-body h1');
    assert.deepEqual(errors, []);
  } finally {
    await context.close();
  }
});

await check('a folder publishes as a knowledge base from its right click', async () => {
  const { page, context, posts } = await open('/cloud/files', { kb: { available: true, sites: [] } });
  try {
    await page.waitForSelector('[data-entry="Notes"]');
    await page.click('[data-entry="Notes"]', { button: 'right' });
    await page.waitForSelector('[data-slot="context-menu"]');
    const items = await page.$$eval('[data-slot="context-menu"] [data-item^="folder-"]', (els) => els.map((e) => e.getAttribute('data-item').slice('folder-'.length)));
    assert.ok(items.includes('notes-open') && items.includes('kb-publish'), items.join(','));
    // The folder's own items sit beside the file menu's, sharing among them.
    assert.ok(await page.isVisible('[data-slot="context-menu"] [data-item="share"]'), 'no Share item');
    await page.click('[data-slot="context-menu"] [data-item="folder-kb-publish"]');
    await page.waitForSelector('[data-publish-dialog] [data-publish]');
    await page.fill('[data-publish-dialog] input', 'Kitchen garden');
    await page.click('[data-publish-dialog] [data-publish]');
    await page.waitForSelector('[data-site-url]');
    assert.deepEqual(posts, [{ path: 'Notes', title: 'Kitchen garden', audience: 'anyone' }]);
    assert.match(await page.textContent('[data-site-url]'), /\/kb\/kitchen-garden\/$/);
    for (const v of await violations(page)) ALL_VIOLATIONS.add(v);
  } finally {
    await context.close();
  }
});

await check('off the mesh, the right click offers no knowledge base', async () => {
  const { page, context } = await open('/cloud/files', { kb: { available: false, sites: [] } });
  try {
    await page.waitForSelector('[data-entry="Notes"]');
    await page.click('[data-entry="Notes"]', { button: 'right' });
    await page.waitForSelector('[data-slot="context-menu"]');
    const items = await page.$$eval('[data-slot="context-menu"] [data-item^="folder-"]', (els) => els.map((e) => e.getAttribute('data-item').slice('folder-'.length)));
    assert.ok(items.includes('notes-open'), items.join(','));
    assert.ok(!items.includes('kb-publish'), items.join(','));
  } finally {
    await context.close();
  }
});

await check('a knowledge base reads without signing in and leaves private notes out', async () => {
  const { page, context, errors } = await open('/kb/kitchen-garden/');
  try {
    await page.waitForSelector('[data-note="Home.md"] .note-body h1');
    assert.equal(await page.textContent('[data-kb-title]'), 'Kitchen garden');
    const tree = await page.textContent('[data-vault-tree]');
    assert.match(tree, /Tomatoes/);
    assert.doesNotMatch(tree, /Private diary/);
    await page.click('.note-body a.wikilink:text("Tomatoes")');
    await page.waitForSelector('[data-note="Tomatoes.md"] .note-body h1');
    assert.equal(new URL(page.url()).pathname, '/kb/kitchen-garden/Tomatoes');
    assert.equal(await page.getAttribute('.note-body img', 'src'), '/kb/api/kitchen-garden/file?path=attachments%2Ftomato.svg');
    // A deep link loads, and so does a note in a folder.
    await page.goto(`${origin}/kb/kitchen-garden/Pests/Aphids`);
    await page.waitForSelector('[data-note="Pests/Aphids.md"] .note-body mark');
    await page.click('[data-view="graph"]');
    await page.waitForSelector('[data-vault-graph] [data-graph]');
    const nodes = Number(await page.getAttribute('[data-vault-graph] [data-graph]', 'data-nodes'));
    assert.equal(nodes, 8, 'seven public notes and the canvas');
    // A .note on the site is MDX too.
    await page.goto(`${origin}/kb/kitchen-garden/Harvest`);
    await page.waitForSelector('[data-note="Harvest.note"] aside.callout');
    assert.equal(await page.locator('[data-note="Harvest.note"] .mdx-tab').count(), 2);
    assert.deepEqual(errors, []);
    for (const v of await violations(page)) ALL_VIOLATIONS.add(v);
  } finally {
    await context.close();
  }
});

await check("a box-only site sends its reader to LosOS Login, and a missing one says so", async () => {
  const signIn = await open('/kb/kitchen-garden/Tomatoes', { kbSite: 'box' });
  try {
    await signIn.page.waitForURL(/\/auth\/\?rd=/);
    assert.equal(decodeURIComponent(new URL(signIn.page.url()).searchParams.get('rd') ?? ''), `${origin}/kb/kitchen-garden/Tomatoes`);
  } finally {
    await signIn.context.close();
  }
  const gone = await open('/kb/no-such-site/');
  try {
    await gone.page.waitForSelector('[data-kb-missing]');
    assert.match(await gone.page.textContent('[data-kb-missing]'), /No knowledge base here/);
  } finally {
    await gone.context.close();
  }
});

await check('no Content-Security-Policy violation under the admin policy', async () => {
  const extra = [...ALL_VIOLATIONS].filter((v) => !BASELINE.includes(v));
  assert.deepEqual(extra, [], `baseline ${JSON.stringify(BASELINE)}`);
});

/* The places of LosOS Cloud on a phone: a bar along the bottom, as on the
 * admin pages, with More for the rest. Notes is not a tab: a vault is a
 * page of its own, without the places, at every width. */
await check('on a phone LosOS Cloud has a bar along the bottom and More for the other places', async () => {
  const { page, errors } = await open('/cloud/files', { viewport: { width: 390, height: 844 } });
  const bar = page.getByRole('navigation', { name: 'Main places', exact: true });
  await bar.waitFor();
  assert.deepEqual((await bar.getByRole('listitem').allInnerTexts()).map((l) => l.trim()), ['Files', 'Recent', 'Favorites', 'Photos', 'More']);
  assert.equal(await bar.locator('a[aria-current="page"]').innerText(), 'Files');
  await bar.getByRole('button', { name: 'More', exact: true }).click();
  const sheet = page.getByRole('dialog', { name: 'Places' });
  await sheet.waitFor();
  await sheet.getByRole('link', { name: 'Trash' }).click();
  await page.waitForURL(/\/cloud\/trash$/);
  await sheet.waitFor({ state: 'detached' });
  assert.equal(await bar.locator('a[aria-current="page"]').count(), 0);
  assert.equal(await bar.getByRole('button', { name: 'More', exact: true }).getAttribute('data-active'), '');
  await page.goto(origin + '/cloud/notes');
  await page.waitForLoadState('networkidle');
  assert.equal(await bar.count(), 0, 'a vault got the places bar');
  assert.deepEqual(errors, []);
  await page.close();
});

await browser.close();
await closeServer();
finish();
