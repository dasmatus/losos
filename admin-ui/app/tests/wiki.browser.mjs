/* LosOS Wiki: the directory and editor at /wikis (a screen of the admin
 * SPA) and a space's published site at /wiki/<owner>/<space>/ (a page of
 * its own), under the admin page's policy, with LosOS Git's wiki API and
 * LosOS Cloud's list of knowledge bases stood in by page.route the way
 * backend/src/forge/wiki.rs and backend/src/cloud/notes/kb.rs answer.
 * Every page fails on a CSP violation or an uncaught error. Walked: the
 * directory with its three kinds, a page with its tree, sidebar, picture
 * and in-page links, an edit and a new page below another, a save someone
 * else beat, a space one may only read, an empty one, the search across
 * every wiki, a new space, the published site with its search and the
 * edit link, a private space for a visitor, German, and the phone layout.
 * nix/tests/wiki.nix runs the same pages against a real box. */

import assert from './assert.mjs';
import { launch, runner, serve, SHELL_ME, shellRoutes, stash } from './harness.mjs';

const { origin, close: closeServer } = await serve({ csp: true });
const browser = await launch();
const { check, finish } = runner();

// WIKI_SHOTS=<dir> keeps a picture of each screen, for a look at the layout.
const shot = (page, name) => (Bun.env.WIKI_SHOTS ? page.screenshot({ path: `${Bun.env.WIKI_SHOTS}/${name}.png` }) : null);

const json = (route, status, body) =>
  route.fulfill({ status, contentType: 'application/json', body: JSON.stringify(body) });

const API = '/git/api/v1';
const WHEN = { sha: 'c'.repeat(40), date: '2026-10-21T09:00:00Z', author: 'Matus', subject: 'Update Home' };
// A 1×1 PNG, the picture a page shows from its wiki.
const PNG = Uint8Array.fromBase64('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==');
const PNG_FILE = await stash('picture.png', PNG);

function model() {
  return {
    spaces: {
      'notshared/garden': {
        owner: 'notshared',
        repo: 'garden',
        description: 'How we look after the garden',
        visibility: 'internal',
        writable: true,
        pages: {
          Home: '# Garden\n\nStart with [[Planting]], then read [the watering guide](Guides/Watering).\n\n![The beds](beds.png)\n',
          Planting: 'Tomatoes go in after the last frost.',
          Guides: 'What we learned.',
          'Guides/Watering': 'Water every evening. See [[../Planting|planting]].',
          _Sidebar: '**Garden**\n\n- [[Home]]\n- [[Planting]]',
        },
      },
      'eva/recipes': {
        owner: 'eva',
        repo: 'recipes',
        description: 'Soups and bread',
        visibility: 'public',
        writable: false,
        pages: { Home: '# Recipes\n\nBean soup, rye bread.' },
      },
      'notshared/plans': { owner: 'notshared', repo: 'plans', description: '', visibility: 'private', writable: true, pages: {} },
    },
    saves: [],
    conflict: false,
  };
}

const listed = (name) => !name.split('/').pop().startsWith('_');
const title = (name) => name.split('/').pop().replaceAll('-', ' ');
const shaOf = (text) => [...text].reduce((h, c) => (h * 31 + c.charCodeAt(0)) >>> 0, 7).toString(16).padStart(40, '0');

function overview(s) {
  const names = Object.keys(s.pages).sort((a, b) => a.toLowerCase().localeCompare(b.toLowerCase()));
  return {
    exists: names.length > 0,
    writable: s.writable,
    clone_url: `https://mattbox.local/git/${s.owner}/${s.repo}.wiki.git`,
    branch: 'main',
    home: names.find((n) => n.toLowerCase() === 'home') ?? names.find(listed) ?? null,
    updated: names.length > 0 ? WHEN : null,
    pages: names.filter(listed).map((n) => ({ name: n, title: title(n), path: `${n}.md`, size: s.pages[n].length })),
  };
}

function pageOf(s, name) {
  const found = Object.keys(s.pages).find((n) => n.toLowerCase() === name.toLowerCase());
  if (found === undefined) return null;
  const text = s.pages[found];
  return {
    name: found,
    title: title(found),
    path: `${found}.md`,
    sha: shaOf(text),
    commit: 'd'.repeat(40),
    text,
    updated: WHEN,
    history: [WHEN, { ...WHEN, sha: 'e'.repeat(40), subject: 'Add Home', date: '2026-10-20T08:00:00Z' }],
    sidebar: s.pages._Sidebar ?? null,
    sidebar_page: s.pages._Sidebar === undefined ? null : '_Sidebar',
    footer: null,
    footer_page: null,
  };
}

function hitsFor(m, q, only) {
  const out = [];
  for (const [key, s] of Object.entries(m.spaces)) {
    if (only !== undefined && key !== only) continue;
    for (const [name, text] of Object.entries(s.pages)) {
      if (!listed(name)) continue;
      const line = text.split('\n').find((l) => l.toLowerCase().includes(q.toLowerCase()));
      if (line !== undefined || name.toLowerCase().includes(q.toLowerCase()))
        out.push({ owner: s.owner, repo: s.repo, page: name, title: title(name), line: 1, text: line ?? null, html_url: `/wiki/${key}/${name}` });
    }
  }
  return out;
}

/* A page with LosOS Git and LosOS Cloud stood in. `visitor` is someone not
 * signed in: LosOS Git shows them the public space alone. */
async function open(path, { locale = 'en-US', viewport = { width: 1280, height: 900 }, m = model(), visitor = false, me = SHELL_ME } = {}) {
  const page = await browser.newPage({ viewport, locale });
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  await page.addInitScript(() => {
    window.__violations = [];
    document.addEventListener('securitypolicyviolation', (e) => window.__violations.push(e.violatedDirective));
  });
  await shellRoutes(page, me);
  await page.route('**/kb/api/**', (route) => {
    const url = new URL(route.request().url());
    if (url.pathname === '/kb/api/') {
      const q = url.searchParams.get('q') ?? '';
      return json(route, 200, {
        sites: [{ slug: 'birds', title: 'Bird notes', audience: 'anyone', notes: 2, updated: 1_760_000_000, mine: false }],
        hits: q.toLowerCase().startsWith('wat') ? [{ slug: 'birds', site: 'Bird notes', path: 'Ducks.md', line: 2, text: 'Ducks like the water.' }] : [],
      });
    }
    if (url.pathname === '/kb/api/birds/site')
      return json(route, 200, { slug: 'birds', title: 'Bird notes', audience: 'anyone', vault: { notes: [{ path: 'Ducks.md', title: null }, { path: 'Robins.md', title: 'Robins' }] } });
    return json(route, 404, {});
  });
  await page.route(`**${API}/**`, async (route) => {
    const req = route.request();
    const url = new URL(req.url());
    const p = decodeURIComponent(url.pathname.slice(API.length));
    const seen = Object.entries(m.spaces).filter(([, s]) => !visitor || s.visibility === 'public');
    if (p === '/user') return visitor ? json(route, 401, { message: 'sign in' }) : json(route, 200, { id: 1, login: 'notshared', full_name: 'Matus' });
    if (p === '/wikis') {
      const q = url.searchParams.get('q') ?? '';
      return json(route, 200, {
        // Only the spaces with a page, as the box lists them.
        wikis: seen.filter(([, s]) => Object.keys(s.pages).some(listed)).map(([key, s]) => ({
          owner: s.owner,
          repo: s.repo,
          full_name: key,
          description: s.description,
          visibility: s.visibility,
          writable: !visitor && s.writable,
          pages: Object.keys(s.pages).filter(listed).length,
          home: 'Home',
          updated: WHEN,
          html_url: `/wiki/${key}`,
        })),
        hits: q === '' ? [] : hitsFor(m, q).filter((h) => seen.some(([k]) => k === `${h.owner}/${h.repo}`)),
      });
    }
    if (req.method() === 'POST' && p === '/user/repos') {
      const body = req.postDataJSON();
      m.spaces[`notshared/${body.name}`] = { owner: 'notshared', repo: body.name, description: body.description, visibility: body.visibility, writable: true, pages: {} };
      return json(route, 201, { owner: { login: 'notshared' }, name: body.name, full_name: `notshared/${body.name}` });
    }
    const at = /^\/repos\/([^/]+)\/([^/]+)\/wiki(?:\/(pages|search|raw)(?:\/(.*))?)?$/.exec(p);
    if (at === null) return json(route, 404, { message: 'not found' });
    const key = `${at[1]}/${at[2]}`;
    const s = seen.find(([k]) => k === key)?.[1];
    if (s === undefined) return json(route, 404, { message: 'repository not found' });
    if (at[3] === undefined) return json(route, 200, overview(s));
    if (at[3] === 'search') return json(route, 200, { hits: hitsFor(m, url.searchParams.get('q') ?? '', key) });
    if (at[3] === 'raw') return at[4] === 'beds.png' ? route.fulfill({ status: 200, contentType: 'image/png', path: PNG_FILE }) : json(route, 404, {});
    const name = at[4] ?? '';
    if (req.method() === 'GET') {
      const found = pageOf(s, name);
      return found === null ? json(route, 404, { message: 'page not found' }) : json(route, 200, found);
    }
    if (req.method() === 'PUT') {
      const body = req.postDataJSON();
      m.saves.push({ key, name, ...body });
      if (!s.writable) return json(route, 403, { message: 'no' });
      const old = pageOf(s, name);
      if (m.conflict || (old !== null && body.sha !== old.sha) || (old === null && body.sha !== null))
        return json(route, 409, { message: 'the page changed meanwhile' });
      s.pages[old?.name ?? name] = body.text;
      return json(route, 200, pageOf(s, name));
    }
    if (req.method() === 'DELETE') {
      delete s.pages[name];
      return json(route, 204, null);
    }
    return json(route, 405, {});
  });
  await page.goto(origin + path, { waitUntil: 'networkidle' });
  // The bundle's two first-paint style-src-elem reports (advanced.browser.mjs)
  // are the shell's; anything after them is this page's.
  const atLoad = await page.evaluate(() => window.__violations.length);
  return { page, errors, m, atLoad };
}

async function clean({ page, errors, atLoad }) {
  assert.deepEqual(errors, [], `page errors: ${errors.join('; ')}`);
  const all = await page.evaluate(() => window.__violations);
  assert.ok(all.slice(0, atLoad).every((d) => d === 'style-src-elem'), `CSP violations at load: ${JSON.stringify(all)}`);
  assert.deepEqual(all.slice(atLoad), [], `CSP violations: ${all.slice(atLoad).join(', ')}`);
}

await check('the directory lists spaces, knowledge bases and the handbook', async () => {
  const w = await open('/wikis');
  const { page, m } = w;
  await page.locator('[data-wiki-entry="space:notshared/garden"]').waitFor();
  await shot(page, 'directory');
  const entries = await page.locator('[data-wiki-entry]').evaluateAll((els) => els.map((e) => e.getAttribute('data-wiki-entry')));
  assert.deepEqual(entries, ['space:notshared/garden', 'space:eva/recipes', 'kb:birds', 'handbook']);
  assert.equal(await page.locator('[data-wiki-entry="space:eva/recipes"] [data-audience]').getAttribute('data-audience'), 'public');
  // The shell's sidebar has its entry.
  assert.ok(await page.locator('a[href="/wikis"]').count());
  // Filtering by name before searching.
  await page.locator('[data-wiki-search]').fill('rec');
  await page.waitForFunction(() => document.querySelectorAll('[data-wiki-entry^="space:"]').length === 1);
  await clean(w);
  await page.close();
});

await check('a page reads with its tree, sidebar, picture and links', async () => {
  const w = await open('/wikis/notshared/garden');
  const { page, m } = w;
  const article = page.locator('[data-wiki-page="Home"]');
  await article.waitFor();
  // The tree nests Guides/Watering below Guides.
  await page.locator('[data-page-tree]').getByText('Watering').waitFor();
  assert.ok(await page.locator('[data-wiki-sidebar]').getByText('Garden').count());
  const img = article.locator('img[alt="The beds"]');
  assert.equal(await img.getAttribute('src'), `${API}/repos/notshared/garden/wiki/raw/beds.png`);
  await page.waitForFunction(() => document.querySelector('img[alt="The beds"]')?.naturalWidth === 1);
  await shot(page, 'space');
  // [[Planting]] opens in place.
  await article.getByRole('link', { name: 'Planting' }).first().click();
  await page.locator('[data-wiki-page="Planting"]').waitFor();
  assert.equal(new URL(page.url()).pathname, '/wikis/notshared/garden/Planting');
  // A link inside a folder starts from the folder.
  await page.goto(`${origin}/wikis/notshared/garden/Guides/Watering`);
  const watering = page.locator('[data-wiki-page="Guides/Watering"]');
  await watering.getByRole('link', { name: 'planting', exact: true }).waitFor();
  assert.equal(await watering.getByRole('link', { name: 'planting', exact: true }).getAttribute('href'), '/wikis/notshared/garden/Planting');
  // The sidebar at the top starts its links from the top, not from the page's folder.
  assert.equal(await watering.locator('[data-wiki-sidebar]').getByRole('link', { name: 'Planting', exact: true }).getAttribute('href'), '/wikis/notshared/garden/Planting');
  assert.ok(await page.locator('[data-wiki-history]').getByText('Add Home').count());
  await clean(w);
  await page.close();
});

await check('an edit saves with the page it started from', async () => {
  const w = await open('/wikis/notshared/garden/Planting');
  const { page, m } = w;
  await page.locator('[data-wiki-page="Planting"]').waitFor();
  const before = pageOf(m.spaces['notshared/garden'], 'Planting').sha;
  await page.locator('[data-wiki-edit]').click();
  const editor = page.locator('[data-wiki-editor]');
  await editor.locator('textarea').fill('Tomatoes go in after the last frost, beans a week later.');
  await editor.getByRole('tab', { name: 'Preview' }).click();
  await editor.getByText('beans a week later').waitFor();
  await shot(page, 'editor');
  await editor.getByRole('button', { name: 'Save' }).click();
  await page.locator('[data-wiki-page="Planting"]').getByText('beans a week later').waitFor();
  assert.deepEqual(m.saves.at(-1), { key: 'notshared/garden', name: 'Planting', text: 'Tomatoes go in after the last frost, beans a week later.', sha: before, message: null });
  await clean(w);
  await page.close();
});

await check('a new page below another, and a save someone else beat', async () => {
  const w = await open('/wikis/notshared/garden/Guides');
  const { page, m } = w;
  await page.locator('[data-wiki-page="Guides"]').waitFor();
  await page.getByRole('button', { name: 'New page below this one' }).click();
  const editor = page.locator('[data-wiki-editor]');
  await editor.getByLabel('Title').fill('Pruning in spring');
  await editor.getByText('Saved as Guides/Pruning-in-spring.md').waitFor();
  await editor.locator('textarea').fill('Cut back the roses in March.');
  await editor.getByLabel('What changed').fill('Write down pruning');
  await editor.getByRole('button', { name: 'Save' }).click();
  await page.locator('[data-wiki-page="Guides/Pruning-in-spring"]').waitFor();
  assert.deepEqual(m.saves.at(-1), { key: 'notshared/garden', name: 'Guides/Pruning-in-spring', text: 'Cut back the roses in March.', sha: null, message: 'Write down pruning' });
  await page.locator('[data-page-tree]').getByText('Pruning in spring').waitFor();
  // Someone saved in between: the text stays and the page says so.
  m.conflict = true;
  await page.locator('[data-wiki-edit]').click();
  await editor.locator('textarea').fill('Mine');
  await editor.getByRole('button', { name: 'Save' }).click();
  await editor.getByText('Someone saved this page while you wrote').waitFor();
  assert.equal(await editor.locator('textarea').inputValue(), 'Mine');
  await clean(w);
  await page.close();
});

await check('a space one may only read, and an empty one', async () => {
  const w = await open('/wikis/eva/recipes');
  const { page, m } = w;
  await page.locator('[data-wiki-page="Home"]').getByText('Bean soup').waitFor();
  assert.equal(await page.locator('[data-wiki-edit]').count(), 0);
  assert.equal(await page.getByRole('button', { name: 'New page', exact: true }).count(), 0);
  // A repository with no wiki yet, opened from LosOS Git.
  await page.goto(`${origin}/wikis/notshared/plans/`);
  await page.getByText('No pages yet').waitFor();
  await page.getByRole('button', { name: 'Write the first page' }).click();
  assert.equal(await page.locator('[data-wiki-editor]').getByLabel('Title').inputValue(), 'Home');
  await clean(w);
  await page.close();
});

await check('search looks through every wiki and offers the handbook', async () => {
  const w = await open('/wikis');
  const { page, m } = w;
  await page.locator('[data-wiki-entry="space:notshared/garden"]').waitFor();
  await page.locator('[data-wiki-search]').fill('water');
  await page.locator('[data-wiki-search]').press('Enter');
  const results = page.locator('[data-wiki-results]');
  await results.locator('[data-hit="garden/Guides/Watering"]').waitFor();
  await results.locator('[data-hit="birds/Ducks.md"]').waitFor();
  await shot(page, 'search');
  assert.equal(await results.locator('[data-hit="birds/Ducks.md"]').getAttribute('href'), '/kb/birds/Ducks');
  assert.equal(await results.locator('[data-hit="handbook"]').getAttribute('href'), '/handbook/search?q=water');
  await results.locator('[data-hit="garden/Guides/Watering"]').click();
  await page.locator('[data-wiki-page="Guides/Watering"]').waitFor();
  await clean(w);
  await page.close();
});

await check('the knowledge base and the handbook have an overview', async () => {
  const w = await open('/wikis?kb=birds');
  const { page, m } = w;
  await page.locator('[data-wiki-kb="birds"]').getByText('Robins').waitFor();
  assert.equal(await page.locator('[data-wiki-kb="birds"]').getByRole('link', { name: 'Robins' }).getAttribute('href'), '/kb/birds/Robins');
  await page.locator('[data-wiki-entry="handbook"]').click();
  await page.locator('[data-wiki-handbook]').getByText('Troubleshooting').waitFor();
  assert.equal(await page.locator('[data-wiki-handbook]').getByRole('link', { name: 'Open the handbook' }).getAttribute('href'), '/handbook/');
  await clean(w);
  await page.close();
});

await check('a new space is a repository with a first page', async () => {
  const w = await open('/wikis');
  const { page, m } = w;
  await page.getByRole('button', { name: 'New space' }).click();
  const dialog = page.getByRole('dialog');
  await dialog.getByLabel('Name').fill('Team handbook');
  await dialog.getByText('Repository: Team-handbook').waitFor();
  await dialog.getByRole('button', { name: 'Make the space' }).click();
  await page.locator('[data-wiki-page="Home"]').getByText('Team handbook').waitFor();
  assert.equal(new URL(page.url()).pathname, '/wikis/notshared/Team-handbook/');
  assert.ok(m.spaces['notshared/Team-handbook'].pages.Home.startsWith('# Team handbook'));
  await clean(w);
  await page.close();
});

await check('the published site reads a space and links its editor', async () => {
  const w = await open('/wiki/notshared/garden/');
  const { page, m } = w;
  await page.locator('[data-wiki-page="Home"]').waitFor();
  await shot(page, 'site');
  assert.equal(await page.locator('[data-wiki-site-title]').textContent(), 'garden');
  await page.locator('[data-wiki-page="Home"]').getByRole('link', { name: 'the watering guide' }).click();
  await page.locator('[data-wiki-page="Guides/Watering"]').waitFor();
  assert.equal(new URL(page.url()).pathname, '/wiki/notshared/garden/Guides/Watering');
  assert.equal(await page.locator('[data-wiki-site-edit]').getAttribute('href'), '/wikis/notshared/garden/Guides/Watering');
  await page.locator('[data-wiki-site-search]').fill('frost');
  await page.locator('[data-wiki-site-search]').press('Enter');
  await page.locator('[data-wiki-site-results] [data-hit="Planting"]').click();
  await page.locator('[data-wiki-page="Planting"]').waitFor();
  // Back walks the history the page made.
  await page.goBack();
  await page.locator('[data-wiki-page="Guides/Watering"]').waitFor();
  // /wiki/ lists the spaces with pages.
  await page.goto(`${origin}/wiki/`);
  await page.locator('[data-wiki-directory] [data-wiki-space="eva/recipes"]').waitFor();
  assert.equal(await page.locator('[data-wiki-directory] [data-wiki-space]').count(), 2);
  await shot(page, 'site-directory');
  // A repository with no wiki yet: whoever may write is sent to write one.
  await page.goto(`${origin}/wiki/notshared/plans/`);
  await page.getByText('No pages yet').waitFor();
  assert.equal(await page.locator('[data-wiki-site-edit]').getAttribute('href'), '/wikis/notshared/plans/');
  await clean(w);
  await page.close();
});

await check('a visitor reads the public space and is offered to sign in elsewhere', async () => {
  const w = await open('/wiki/eva/recipes/', { visitor: true });
  const { page, m } = w;
  await page.locator('[data-wiki-page="Home"]').getByText('Bean soup').waitFor();
  assert.equal(await page.locator('[data-wiki-site-edit]').count(), 0);
  await page.goto(`${origin}/wiki/notshared/garden/`);
  await page.locator('[data-wiki-missing]').getByText('open only to people signed in').waitFor();
  assert.ok(await page.locator('[data-wiki-missing]').getByRole('button', { name: 'Sign in' }).count());
  await clean(w);
  await page.close();
});

await check('German, and the phone layout one column at a time', async () => {
  const w = await open('/wikis', { locale: 'de-DE', viewport: { width: 390, height: 844 } });
  const { page, m } = w;
  await page.getByText('Alle Wikis').first().waitFor();
  await page.locator('[data-wiki-entry="space:notshared/garden"]').click();
  await page.locator('[data-page-tree]').getByText('Planting').click();
  await page.locator('[data-wiki-page="Planting"]').waitFor();
  await shot(page, 'phone');
  assert.equal(await page.locator('[data-page-tree]').isVisible(), false);
  const width = await page.evaluate(() => document.documentElement.scrollWidth);
  assert.ok(width <= 390, `the page scrolls sideways: ${width}px`);
  await page.goto(`${origin}/wiki/notshared/garden/`);
  await page.locator('[data-wiki-site-pages]').click();
  await page.locator('[data-page-tree]').getByText('Watering').click();
  await page.locator('[data-wiki-page="Guides/Watering"]').waitFor();
  await clean(w);
  await page.close();
});

await browser.close();
await closeServer();
finish();
