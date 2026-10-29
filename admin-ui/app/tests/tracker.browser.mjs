/* LosOS Git's issues (/git/issues) under the admin page's policy, with the
 * tracker's API at /git/api/v1/tracker/ stood in by page.route the way
 * backend/src/forge/tracker/api.rs answers. Every page fails on a CSP
 * violation or an uncaught error. Walked: My issues, the keyboard (j,
 * Enter, s and a state, s on a row with nothing open), the board and a
 * card dragged to another column, the palette, a new issue with c, the
 * inbox, a reader who gets no property chips, a description whose Markdown
 * carries script, German, the phone layout, the pins on the admin page's
 * sidebar, a row's right-click menu and a new issue another app links to
 * (?new=1&title=…). nix/tests/git-issues.nix
 * runs the same pages against a real box. */

import assert from './assert.mjs';
import { launch, runner, serve } from './harness.mjs';

const { origin, close: closeServer } = await serve({ csp: true });
const browser = await launch();
const { check, finish } = runner();

const json = (route, status, body) =>
  route.fulfill({ status, contentType: 'application/json', body: JSON.stringify(body) });

const NOW = '2026-10-22T08:00:00Z';
const STATES = [
  ['Backlog', 'backlog'],
  ['Todo', 'unstarted'],
  ['In Progress', 'started'],
  ['In Review', 'started'],
  ['Done', 'done'],
  ['Canceled', 'canceled'],
].map(([name, category], i) => ({ id: i + 1, projectId: 1, name, category, color: '', position: i }));
const LABELS = [
  { id: 1, name: 'bug', color: '#d1453b', description: '' },
  { id: 2, name: 'docs', color: '#2f7de1', description: '' },
];

function model(permission = 'admin') {
  const project = {
    id: 1,
    key: 'BT',
    name: 'Backup tool',
    description: '',
    color: '',
    lead: 'notshared',
    visibility: 'private',
    archived: false,
    created: NOW,
    updated: NOW,
    repos: [{ owner: 'notshared', name: 'backup-tool', defaultBranch: 'main' }],
    permission,
    states: STATES,
    labels: LABELS,
    cycles: [],
    counts: { total: 3, open: 3, started: 1 },
    members: [],
  };
  const issue = (number, title, state, extra = {}) => ({
    id: number,
    projectId: 1,
    key: `BT-${number}`,
    number,
    title,
    body: '',
    state: STATES[state],
    priority: 0,
    estimate: null,
    assignee: 'notshared',
    creator: 'notshared',
    cycleId: null,
    parentId: null,
    parentKey: null,
    due: null,
    sort: number,
    repo: { owner: 'notshared', name: 'backup-tool', number },
    labels: [],
    comments: 0,
    children: 0,
    childrenClosed: 0,
    created: NOW,
    updated: NOW,
    startedAt: null,
    completedAt: null,
    canceledAt: null,
    url: `/git/issues/BT-${number}`,
    ...extra,
  });
  return {
    project,
    issues: [
      issue(1, 'Restore one folder at a time', 2, { priority: 2, labels: [LABELS[0]] }),
      issue(2, 'Warn when the disk is nearly full', 1, {
        priority: 1,
        body: 'Free space below 10 %.\n\n<script>window.__owned = 1</script> [bad](javascript:alert(1))',
      }),
      issue(3, 'Write the restore guide', 1, { assignee: 'eva', labels: [LABELS[1]] }),
    ],
    calls: [],
    next: 4,
  };
}

function detail(m, i) {
  const { comments, children, ...rest } = i;
  return {
    ...rest,
    project: m.project,
    comments: [],
    children: [],
    activity: [{ id: 1, actor: i.creator, kind: 'created', detail: {}, created: NOW }],
    relations: [],
    links: [],
    subscribers: [i.creator],
    subscribed: true,
    permission: m.project.permission,
  };
}

function matches(i, q) {
  const list = (k) => (q.get(k) ?? '').split(',').filter((v) => v !== '');
  const cat = list('category');
  if (cat.includes('active') && ['done', 'canceled'].includes(i.state.category)) return false;
  const assignee = list('assignee');
  if (assignee.includes('me') && i.assignee !== 'notshared') return false;
  const keys = list('key');
  if (keys.length > 0 && !keys.includes(i.key)) return false;
  const text = q.get('q');
  if (text && !i.title.toLowerCase().includes(text.toLowerCase())) return false;
  return true;
}

function apply(m, i, body) {
  if (body.state !== undefined) i.state = STATES.find((s) => s.id === body.state);
  if (body.priority !== undefined) i.priority = body.priority;
  if (body.sort !== undefined) i.sort = body.sort;
  if (body.assignee !== undefined) i.assignee = body.assignee;
}

async function open(path, { permission = 'admin', locale = 'en-US', viewport = { width: 1280, height: 900 }, pins = null } = {}) {
  const m = model(permission);
  m.pins = pins === null ? null : [...pins];
  m.puts = [];
  const page = await browser.newPage({ viewport, locale });
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  await page.addInitScript(() => {
    window.__violations = [];
    document.addEventListener('securitypolicyviolation', (e) => window.__violations.push(e.violatedDirective));
  });
  await page.route('**/api/profiles', (route) =>
    json(route, 200, {
      people: [
        { name: 'eva', displayName: 'Eva', pronouns: '', about: '', accent: 'rose', accentChosen: true, picture: null, banner: null },
        { name: 'notshared', displayName: 'Matus', pronouns: '', about: '', accent: 'teal', accentChosen: true, picture: null, banner: null },
      ],
    }),
  );
  // An administrator signed in through LosOS Login, whose sidebar pins the
  // box keeps (GET/PUT /api/pins); without `pins` the page has neither.
  if (m.pins !== null) {
    await page.route('**/api/me', (route) => json(route, 200, { name: 'notshared', admin: true, sections: [], shared: [] }));
    await page.route('**/api/pins', (route) => {
      if (route.request().method() === 'PUT') {
        m.pins = route.request().postDataJSON().pins;
        m.puts.push(m.pins);
      }
      return json(route, 200, { pins: m.pins, limit: 24 });
    });
  }
  const api = '/git/api/v1';
  await page.route(`**${api}/**`, (route) => {
    const req = route.request();
    const url = new URL(req.url());
    const p = url.pathname.slice(api.length);
    const method = req.method();
    const body = method === 'GET' ? null : (req.postDataJSON?.() ?? null);
    if (method !== 'GET') m.calls.push({ method, path: p, body });
    if (p === '/user')
      return json(route, 200, { id: 1, login: 'notshared', full_name: 'Matus', email: '', is_admin: true, is_bot: false });
    if (p === '/user/repos') return json(route, 200, []);
    if (p === '/tracker')
      return json(route, 200, {
        me: 'notshared',
        projects: [m.project],
        views: [],
        unread: 1,
        assigned: m.issues.filter((i) => i.assignee === 'notshared').length,
        people: [
          { login: 'notshared', fullName: 'Matus' },
          { login: 'eva', fullName: 'Eva' },
        ],
        ai: false,
        today: '2026-10-22',
      });
    if (p === '/tracker/issues' && method === 'GET')
      return json(route, 200, { issues: m.issues.filter((i) => matches(i, url.searchParams)) });
    if (p === '/tracker/issues' && method === 'POST') {
      const n = m.next++;
      const i = { ...m.issues[2], id: n, number: n, key: `BT-${n}`, title: body.title, body: body.body ?? '', assignee: null, labels: [], state: STATES[1], url: `/git/issues/BT-${n}` };
      m.issues.push(i);
      return json(route, 201, detail(m, i));
    }
    const one = p.match(/^\/tracker\/issues\/(BT-\d+)$/);
    if (one) {
      const i = m.issues.find((x) => x.key === one[1]);
      if (!i) return json(route, 404, { message: 'no such issue' });
      if (method === 'PATCH') apply(m, i, body);
      return json(route, 200, detail(m, i));
    }
    if (p === '/tracker/projects/BT') return json(route, 200, m.project);
    if (p === '/tracker/inbox')
      return json(route, 200, {
        unread: 1,
        items: [{ id: 1, ticketId: 3, kind: 'assigned', actor: 'eva', detail: {}, created: NOW, read: false, issue: m.issues[0] }],
      });
    if (p === '/tracker/inbox/read') return json(route, 200, { unread: 0 });
    return json(route, 404, { message: `no stand-in for ${method} ${p}` });
  });
  await page.goto(`${origin}/git${path}`);
  await page.waitForLoadState('networkidle');
  /* The toast stacks' own <style> at load is refused and harmless (see
   * tests/git.browser.mjs). Nothing after that may add a violation. */
  const atLoad = await page.evaluate(() => window.__violations);
  assert.deepEqual(atLoad, [], `CSP violations at load: ${JSON.stringify(atLoad)}`);
  const settle = async () => {
    const violations = (await page.evaluate(() => window.__violations)).slice(atLoad.length);
    assert.deepEqual(violations, [], `CSP violations: ${violations.join(', ')}`);
    assert.deepEqual(errors, [], `page errors: ${errors.join('\n')}`);
  };
  return { page, m, settle };
}

await check('My issues lists what is given to me, grouped by status', async () => {
  const { page, settle } = await open('/issues');
  const list = page.locator('[data-issue-list]');
  await list.locator('[data-issue-key="BT-1"]').waitFor();
  await list.locator('[data-issue-key="BT-2"]').waitFor();
  assert.equal(await list.locator('[data-issue-key="BT-3"]').count(), 0, "eva's issue is not mine");
  await settle();
  await page.close();
});

await check('j and Enter open an issue; s picks a status', async () => {
  const { page, m, settle } = await open('/issues/p/BT');
  await page.locator('[data-issue-list] [data-issue-key="BT-1"]').waitFor();
  await page.keyboard.press('j');
  await page.keyboard.press('Enter');
  await page.waitForURL(/open=BT-/);
  const key = new URL(page.url()).searchParams.get('open');
  await page.locator(`[data-reader="${key}"]`).waitFor();
  await page.keyboard.press('s');
  const menu = page.locator('[data-menu="state"]');
  await menu.waitFor();
  await menu.locator('[data-state-id="5"]').click();
  await page.locator(`[data-reader="${key}"] [data-prop="state"]`, { hasText: 'Done' }).waitFor();
  const patch = m.calls.find((c) => c.method === 'PATCH' && c.path === `/tracker/issues/${key}`);
  assert.equal(patch.body.state, 5);
  await settle();
  await page.close();
});

await check('s on the row in focus, with no issue open, opens the status menu for it', async () => {
  const { page, m, settle } = await open('/issues/p/BT');
  await page.locator('[data-issue-list] [data-issue-key="BT-1"]').waitFor();
  await page.keyboard.press('j');
  await page.keyboard.press('s');
  const menu = page.locator('[data-menu="state"]');
  await menu.waitFor();
  await menu.locator('[data-state-id="6"]').click();
  await page.locator('[data-selection-bar]').waitFor({ state: 'hidden' });
  const patch = m.calls.find((c) => c.method === 'PATCH');
  assert.ok(patch, 'no change was sent');
  assert.equal(patch.body.state, 6);
  await settle();
  await page.close();
});

await check('a card dragged to another column changes its status', async () => {
  const { page, m, settle } = await open('/issues/p/BT?layout=board');
  const card = page.locator('[data-card][data-issue-key="BT-3"]');
  await card.waitFor();
  await card.dragTo(page.locator('[data-column="In Progress"]'));
  await page.locator('[data-column="In Progress"] [data-issue-key="BT-3"]').waitFor();
  const patch = m.calls.find((c) => c.method === 'PATCH' && c.path === '/tracker/issues/BT-3');
  assert.equal(patch.body.state, 3);
  await settle();
  await page.close();
});

await check('the palette finds an issue by its words', async () => {
  const { page, settle } = await open('/issues/p/BT');
  await page.locator('[data-issue-list]').waitFor();
  await page.keyboard.press('Control+k');
  await page.locator('[data-palette]').waitFor();
  await page.keyboard.type('nearly full');
  await page.getByRole('option', { name: /nearly full/ }).waitFor();
  await page.keyboard.press('Enter');
  await page.waitForURL(/open=BT-2/);
  await settle();
  await page.close();
});

await check('c opens a new issue and Ctrl+Enter makes it', async () => {
  const { page, m, settle } = await open('/issues/p/BT');
  await page.locator('[data-issue-list]').waitFor();
  await page.keyboard.press('c');
  await page.getByRole('dialog').waitFor();
  await page.keyboard.type('Rotate the backup key every year');
  await page.keyboard.press('Control+Enter');
  await page.getByText('Rotate the backup key every year').first().waitFor();
  const post = m.calls.find((c) => c.method === 'POST' && c.path === '/tracker/issues');
  assert.equal(post.body.project, 'BT');
  assert.equal(post.body.title, 'Rotate the backup key every year');
  await settle();
  await page.close();
});

await check('the inbox shows what happened', async () => {
  const { page, settle } = await open('/issues/inbox');
  await page.locator('[data-inbox] [data-inbox-item="assigned"]').waitFor();
  await settle();
  await page.close();
});

await check('a reader sees the issue and its description as text, with no property chips', async () => {
  const { page, settle } = await open('/issues/p/BT?open=BT-2', { permission: 'read' });
  const reader = page.locator('[data-reader="BT-2"]');
  await reader.locator('[data-description]').getByText('Free space below 10 %.').waitFor();
  assert.equal(await page.evaluate(() => window.__owned), undefined);
  assert.equal(await reader.locator('[data-description] a', { hasText: 'bad' }).count(), 0);
  await page.keyboard.press('c');
  const dialog = page.getByRole('dialog');
  await dialog.waitFor();
  assert.equal(await dialog.locator('[data-prop]').count(), 0, 'a reader cannot set properties on a new issue');
  await settle();
  await page.close();
});

await check("the pin in an issue's top bar and a project's header put them on the sidebar", async () => {
  const { page, m, settle } = await open('/issues/p/BT?open=BT-1', { pins: [] });
  const pin = page.locator('[data-pin="issue:BT-1"]');
  await pin.waitFor();
  assert.equal(await pin.getAttribute('aria-pressed'), 'false');
  await pin.click();
  await page.locator('[data-pin="issue:BT-1"][aria-pressed="true"]').waitFor();
  assert.deepEqual(m.puts.at(-1), [{ kind: 'issue', key: 'BT-1' }]);
  await page.locator('[data-pin="project:BT"]').click();
  await page.locator('[data-pin="project:BT"][aria-pressed="true"]').waitFor();
  assert.deepEqual(m.puts.at(-1), [
    { kind: 'issue', key: 'BT-1' },
    { kind: 'project', key: 'BT' },
  ]);
  await settle();
  await page.close();
});

await check('without a sign-in that may change the pins, there is no pin button', async () => {
  const { page, settle } = await open('/issues/p/BT?open=BT-1');
  await page.locator('[data-reader="BT-1"]').waitFor();
  await page.waitForTimeout(300);
  assert.equal(await page.locator('[data-pin]').count(), 0);
  await settle();
  await page.close();
});

await check('right-clicking a row or a card offers open, copy and pin', async () => {
  const { page, m, settle } = await open('/issues/p/BT', { pins: [] });
  const row = page.locator('[data-issue-list] [data-issue-key="BT-2"]');
  await row.waitFor();
  // The pins have answered once the project's own pin button is drawn.
  await page.locator('[data-pin="project:BT"]').waitFor();
  await row.click({ button: 'right' });
  const menu = page.locator('[data-slot="context-menu"]');
  await menu.waitFor();
  assert.deepEqual(await menu.getByRole('menuitem').allInnerTexts(), [
    'Open',
    'Open in new tab',
    'Copy link',
    'Copy key and title',
    'Pin to sidebar',
  ]);
  await menu.getByRole('menuitem', { name: 'Pin to sidebar' }).click();
  await menu.waitFor({ state: 'detached' });
  assert.deepEqual(m.puts.at(-1), [{ kind: 'issue', key: 'BT-2' }]);
  await row.click({ button: 'right' });
  await menu.getByRole('menuitem', { name: 'Open', exact: true }).click();
  await page.waitForURL(/open=BT-2/);

  await page.goto(`${origin}/git/issues/p/BT?layout=board`, { waitUntil: 'networkidle' });
  await page.locator('[data-pin="project:BT"]').waitFor();
  const card = page.locator('[data-card][data-issue-key="BT-3"]');
  await card.click({ button: 'right' });
  await menu.getByRole('menuitem', { name: 'Pin to sidebar' }).click();
  await menu.waitFor({ state: 'detached' });
  assert.deepEqual(m.puts.at(-1), [
    { kind: 'issue', key: 'BT-2' },
    { kind: 'issue', key: 'BT-3' },
  ]);
  await page.close();
});

await check('a link from another app opens a new issue already filled in, in the project it names', async () => {
  const words = new URLSearchParams({ new: '1', into: 'bt', title: 'The backup mail bounced', body: '> It bounced twice.\n\nFrom Mail' });
  const { page, m, settle } = await open(`/issues?${words}`);
  const dialog = page.locator('[data-create-issue]');
  await dialog.waitFor();
  assert.equal(await dialog.getByRole('textbox', { name: 'Title' }).inputValue(), 'The backup mail bounced');
  assert.equal(await dialog.getByRole('textbox', { name: 'Description' }).inputValue(), '> It bounced twice.\n\nFrom Mail');
  // A reload does not open it again.
  assert.equal(new URL(page.url()).searchParams.get('new'), null);
  await dialog.getByRole('textbox', { name: 'Title' }).click();
  await page.keyboard.press('Control+Enter');
  await page.getByText('The backup mail bounced').first().waitFor();
  const post = m.calls.find((c) => c.method === 'POST' && c.path === '/tracker/issues');
  assert.equal(post.body.project, 'BT');
  assert.equal(post.body.body, '> It bounced twice.\n\nFrom Mail');
  await settle();
  await page.close();
});

await check('the pages speak German', async () => {
  const { page, settle } = await open('/issues', { locale: 'de-DE' });
  await page.getByText('Meine Issues').first().waitFor();
  await settle();
  await page.close();
});

await check('a phone gets no sideways scroll on the list', async () => {
  const { page, settle } = await open('/issues/p/BT', { viewport: { width: 390, height: 844 } });
  await page.locator('[data-issue-list]').waitFor();
  const wide = await page.evaluate(() => document.documentElement.scrollWidth - document.documentElement.clientWidth);
  assert.ok(wide <= 0, `the page scrolls sideways by ${wide}px`);
  await settle();
  await page.close();
});

await browser.close();
await closeServer();
finish();
