/* LosOS Git's pages (/git/, a page of its own in the same bundle) under the
 * admin page's policy, with LosOS Git's API at /git/api/v1/ stood in by
 * page.route the way backend/src/forge/ answers. Every page fails on a CSP
 * violation or an uncaught error. Walked: the repositories, the code with
 * its README, a pull request's conversation (reviewers with the AI
 * reviewer's progress, checks, the merge box), a line comment added to a
 * review that asks for changes, a reply and a resolve, a squash merge, the
 * AI review switch in the settings, the worktrees, a comment whose Markdown
 * carries script, German, the phone layout, the repository's room in
 * LosOS Chat, and someone of another box signing in with their chat
 * address. nix/tests/git-pages.nix and nix/tests/git-federation.nix run the
 * same pages against real boxes. */

import assert from './assert.mjs';
import { launch, runner, serve } from './harness.mjs';

const { origin, close: closeServer } = await serve({ csp: true });
const browser = await launch();
const { check, finish } = runner();

const json = (route, status, body) =>
  route.fulfill({ status, contentType: 'application/json', body: JSON.stringify(body) });

const HEAD = 'b'.repeat(40);
const BASE = 'a'.repeat(40);
const ME = { id: 1, login: 'notshared', full_name: 'Matus', email: 'm@box', is_admin: true, is_bot: false };
const REPO = {
  id: 7,
  owner: { login: 'notshared' },
  name: 'notes',
  full_name: 'notshared/notes',
  description: 'Notes from the garden',
  private: true,
  visibility: 'private',
  default_branch: 'main',
  empty: false,
  fork: false,
  clone_url: 'https://mattbox.local/git/notshared/notes.git',
  created: '2026-10-20T10:00:00Z',
  updated: '2026-10-21T09:00:00Z',
  permissions: { admin: true, push: true, pull: true },
};
const EVA = {
  id: 9,
  login: 'eva@kitchenbox.local',
  full_name: 'Eva',
  email: '',
  is_admin: false,
  is_bot: false,
  matrix_id: '@eva:kitchenbox.local',
};
const ROOM = '!garden:mattbox.local';
const PATCH = [
  '@@ -1,3 +1,4 @@',
  ' # Notes',
  '-Tomatoes in May.',
  '+Tomatoes in April.',
  '+Beans in May.',
  ' Water every evening.',
  '',
].join('\n');

function model() {
  return {
    pull: {
      number: 3,
      title: 'Plant earlier',
      body: 'Moves the tomatoes a month earlier.\n\n<script>window.__owned = 1</script> [bad](javascript:alert(1))',
      user: { login: 'eva' },
      state: 'open',
      created_at: '2026-10-21T08:00:00Z',
      updated_at: '2026-10-21T08:30:00Z',
      closed_at: null,
      html_url: '/git/notshared/notes/pulls/3',
      pull_request: { head: 'plant-earlier', base: 'main', merged: false, merge_commit_sha: null },
      merged: false,
      merged_by: null,
      merged_at: null,
      merge_commit_sha: null,
      head: { ref: 'plant-earlier', sha: HEAD },
      base: { ref: 'main', sha: BASE },
      merge_base: BASE,
      ahead_by: 1,
      behind_by: 0,
      mergeable: true,
      conflicts: [],
      fast_forward: true,
      reviewers: [
        { login: 'losos-reviewer', is_bot: true, state: 'requested', previous: null, requested: true },
      ],
      worktree: { id: 1, owner: { login: 'eva' }, computer: 'laptop', name: 'garden', head: HEAD, updated_at: '2026-10-21T08:30:00Z' },
      comments: 0,
      review_comments: 1,
      can_merge: true,
      permission: 'admin',
    },
    lines: [
      {
        id: 11,
        review_id: 5,
        in_reply_to: null,
        user: { login: 'eva' },
        path: 'README.md',
        line: 3,
        side: 'new',
        commit_id: HEAD,
        body: 'Beans need warmer soil, so May is right.',
        resolved: false,
        resolved_by: null,
        created_at: '2026-10-21T08:31:00Z',
      },
    ],
    reviews: [
      { id: 5, user: { login: 'eva' }, state: 'commented', body: '', commit_id: HEAD, submitted_at: '2026-10-21T08:31:00Z' },
    ],
    repo: { ...REPO, chat_room: null },
    calls: [],
  };
}

/* `me` is who LosOS Git says is signed in: an account, null for a visitor
 * (401), or a function of the request. */
async function open(path, { locale = 'en-US', viewport = { width: 1280, height: 900 }, me = ME, room = null } = {}) {
  const m = model();
  m.repo.chat_room = room;
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
        { name: 'matus', displayName: 'Matus', pronouns: '', about: '', accent: 'teal', accentChosen: true, picture: null, banner: null },
      ],
    }),
  );
  await page.route('**/api/chat', (route) => json(route, 200, { available: true }));
  await page.route('**/api/chat/git-room', (route) => {
    const body = route.request().postDataJSON();
    m.calls.push({ method: 'POST', path: '/api/chat/git-room', body });
    m.repo.chat_room = ROOM;
    return json(route, 200, { room: ROOM });
  });
  const api = '/git/api/v1';
  await page.route(`**${api}/**`, (route) => {
    const req = route.request();
    const url = new URL(req.url());
    const p = url.pathname.slice(api.length);
    const method = req.method();
    if (method !== 'GET') m.calls.push({ method, path: p, body: req.postDataJSON?.() ?? null });
    const r = '/repos/notshared/notes';
    if (p === '/user') {
      const who = typeof me === 'function' ? me(req) : me;
      return who === null ? json(route, 401, { message: 'Sign in first.' }) : json(route, 200, who);
    }
    if (p === '/matrix/session' && method === 'POST') return json(route, 200, { token: 'chat-tok', login: EVA.login });
    if (p === '/user/repos') return json(route, 200, [REPO, { ...REPO, id: 8, name: 'site', private: false, visibility: 'public', description: '' }]);
    if (p === r && method === 'PATCH') {
      Object.assign(m.repo, req.postDataJSON());
      if (m.repo.chat_room === '') m.repo.chat_room = null;
      return json(route, 200, m.repo);
    }
    if (p === r) return json(route, 200, m.repo);
    if (p === `${r}/collaborators`) return json(route, 200, [{ id: 2, login: 'eva', full_name: 'Eva', email: '', is_admin: false, is_bot: false, permission: 'write' }]);
    if (p === `${r}/ai-review` && method === 'PUT')
      return json(route, 200, { enabled: req.postDataJSON().enabled, available: true, model: 'qwen2.5-coder:1.5b', reviewer: { login: 'losos-reviewer', full_name: 'LosOS Reviewer' }, canChange: true });
    if (p === `${r}/ai-review`) return json(route, 200, { enabled: true, available: true, model: 'qwen2.5-coder:1.5b', reviewer: { login: 'losos-reviewer', full_name: 'LosOS Reviewer' }, canChange: true });
    if (p === `${r}/branches`)
      return json(route, 200, [
        { name: 'main', default: true, commit: { sha: BASE, subject: 'First notes', date: '2026-10-20T10:00:00Z', author: 'Matus' } },
        { name: 'plant-earlier', default: false, commit: { sha: HEAD, subject: 'Plant earlier', date: '2026-10-21T08:00:00Z', author: 'Eva' } },
      ]);
    if (p === `${r}/contents`)
      return json(route, 200, {
        type: 'dir',
        path: '',
        commit: BASE,
        entries: [
          { name: 'README.md', path: 'README.md', type: 'file', sha: 'c'.repeat(40), size: 52 },
          { name: 'beds', path: 'beds', type: 'dir', sha: 'd'.repeat(40), size: 0 },
        ],
      });
    if (p === `${r}/readme`)
      return json(route, 200, {
        type: 'file', name: 'README.md', path: 'README.md', sha: 'c'.repeat(40), commit: BASE, size: 52, binary: false, truncated: false,
        text: '# Notes\n\nTomatoes in **May**.\n\n- water\n- weed\n',
      });
    if (p === `${r}/pulls` && method === 'GET') return json(route, 200, [m.pull]);
    if (p === `${r}/pulls/3` && method === 'GET') return json(route, 200, m.pull);
    if (p === `${r}/pulls/3/commits`)
      return json(route, 200, [{ sha: HEAD, author: { name: 'Eva', email: 'eva@box' }, date: '2026-10-21T08:00:00Z', subject: 'Plant earlier', body: '' }]);
    if (p === `${r}/pulls/3/files`)
      return json(route, 200, [
        { filename: 'README.md', previous_filename: null, status: 'modified', additions: 2, deletions: 1, binary: false, patch: PATCH, too_large: false },
      ]);
    if (p === `${r}/issues/3/comments` && method === 'GET') return json(route, 200, []);
    if (p === `${r}/pulls/3/reviews` && method === 'GET') return json(route, 200, m.reviews);
    if (p === `${r}/pulls/3/reviews` && method === 'POST') {
      const body = req.postDataJSON();
      m.reviews.push({ id: 6, user: { login: 'notshared' }, state: body.event === 'REQUEST_CHANGES' ? 'changes_requested' : 'commented', body: body.body, commit_id: HEAD, submitted_at: '2026-10-21T09:10:00Z' });
      return json(route, 200, m.reviews.at(-1));
    }
    if (p === `${r}/pulls/3/comments` && method === 'GET') return json(route, 200, m.lines);
    if (p === `${r}/pulls/3/comments` && method === 'POST') {
      const body = req.postDataJSON();
      const reply = { ...m.lines[0], id: 12, in_reply_to: body.in_reply_to, user: { login: 'notshared' }, body: body.body };
      m.lines.push(reply);
      return json(route, 200, reply);
    }
    if (p === `${r}/pulls/comments/11` && method === 'PATCH') {
      m.lines[0].resolved = req.postDataJSON().resolved;
      m.lines[0].resolved_by = { login: 'notshared' };
      return json(route, 200, m.lines[0]);
    }
    if (p === `${r}/pulls/3/ai-review`) return json(route, 200, { state: 'reviewing', filesDone: 1, filesTotal: 2, ahead: 0 });
    if (p === `${r}/commits/${HEAD}/statuses`)
      return json(route, 200, {
        state: 'pending',
        statuses: [
          { context: 'mesh-ci/test', state: 'success', description: 'Passed on 2 boxes', targetUrl: null, creator: 'ci', created: '', updated: '' },
          { context: 'LosOS Reviewer', state: 'pending', description: 'Reading the changes', targetUrl: '/git/notshared/notes/pulls/3/files', creator: 'losos-reviewer', created: '', updated: '' },
        ],
      });
    if (p === `${r}/pulls/3/merge` && method === 'POST') {
      m.pull = { ...m.pull, merged: true, state: 'closed', merged_by: { login: 'notshared' }, merged_at: '2026-10-21T09:20:00Z', merge_commit_sha: 'e'.repeat(40) };
      return json(route, 200, { merged: true, sha: 'e'.repeat(40) });
    }
    if (p === `${r}/worktrees`)
      return json(route, 200, [
        { id: 1, owner: { login: 'eva' }, computer: 'laptop', name: 'garden', branch: 'plant-earlier', head: HEAD, branch_exists: true, behind_branch: false, updated_at: '2026-10-21T08:30:00Z' },
        { id: 2, owner: { login: 'notshared' }, computer: 'desk', name: 'old-try', branch: 'gone', head: BASE, branch_exists: false, behind_branch: false, updated_at: '2026-10-19T08:30:00Z' },
      ]);
    return json(route, 404, { message: `no stand-in for ${method} ${p}` });
  });
  await page.goto(`${origin}/git${path}`);
  await page.waitForLoadState('networkidle');
  /* The toast stacks' own <style> at load is refused and harmless (their
   * stylesheet is in the bundle; see tests/mail.browser.mjs). Nothing
   * after that may add a violation. */
  const atLoad = await page.evaluate(() => window.__violations);
  assert.deepEqual(atLoad, [], `CSP violations at load: ${JSON.stringify(atLoad)}`);
  const settle = async () => {
    const violations = (await page.evaluate(() => window.__violations)).slice(atLoad.length);
    assert.deepEqual(violations, [], `CSP violations: ${violations.join(', ')}`);
    assert.deepEqual(errors, [], `page errors: ${errors.join('\n')}`);
  };
  return { page, m, settle };
}

await check('the repositories are listed', async () => {
  const { page, settle } = await open('/');
  await page.getByRole('link', { name: 'notes' }).waitFor();
  await page.getByText('Notes from the garden').waitFor();
  await settle();
  await page.close();
});

await check('the code shows the folder and the README as Markdown', async () => {
  const { page, settle } = await open('/notshared/notes');
  await page.getByRole('link', { name: 'README.md' }).waitFor();
  await page.locator('[data-readme] h2', { hasText: 'Notes' }).waitFor();
  assert.equal(await page.locator('[data-readme] strong').textContent(), 'May');
  await page.getByText('https://mattbox.local/git/notshared/notes.git').waitFor();
  if (Bun.env.GIT_SHOTS) await page.screenshot({ path: `${Bun.env.GIT_SHOTS}/code.png`, fullPage: true });
  await settle();
  await page.close();
});

await check('a pull request: reviewers, the AI reviewer at work, checks, worktree, merge box', async () => {
  const { page, settle } = await open('/notshared/notes/pulls/3');
  await page.locator('[data-pull-title]', { hasText: 'Plant earlier' }).waitFor();
  await page.locator('[data-reviewer="losos-reviewer"]').getByText('Bot').waitFor();
  await page.locator('[data-bot-status="reviewing"]', { hasText: '1 of 2 files' }).waitFor();
  await page.locator('[data-checks]').getByText('mesh-ci/test').waitFor();
  await page.locator('[data-worktree-badge]', { hasText: 'garden on laptop' }).waitFor();
  await page.locator('[data-merge-box="open"]').getByText('Ready to merge').waitFor();
  // The description's script and javascript: link stay text.
  assert.equal(await page.evaluate(() => window.__owned), undefined);
  assert.equal(await page.locator('[data-comment] a', { hasText: 'bad' }).count(), 0);
  await page.getByText('<script>window.__owned = 1</script>', { exact: false }).waitFor();
  await settle();
  await page.close();
});

await check('a line comment goes into a review that asks for changes', async () => {
  const { page, m, settle } = await open('/notshared/notes/pulls/3/files');
  await page.locator('[data-file-diff="README.md"]').waitFor();
  // The removed line takes a comment on the old side.
  const removed = page.locator('tr[data-row="del"]').first();
  await removed.hover();
  await removed.locator('[data-comment-line]').click();
  await page.locator('[data-composer] textarea').fill('Why a month earlier?');
  await page.getByRole('button', { name: 'Add to review' }).click();
  await page.locator('[data-pending]').waitFor();
  await page.locator('[data-review-panel]').getByText('1 comment waiting').waitFor();
  if (Bun.env.GIT_SHOTS) await page.screenshot({ path: `${Bun.env.GIT_SHOTS}/files.png`, fullPage: true });
  await page.locator('[data-review-panel] [role=radio], [data-review-panel] button', { hasText: 'Request changes' }).first().click();
  await page.locator('[data-submit-review]').click();
  await page.getByText('Review sent', { exact: true }).first().waitFor();
  const sent = m.calls.find((c) => c.path.endsWith('/pulls/3/reviews'));
  assert.equal(sent.body.event, 'REQUEST_CHANGES');
  assert.equal(sent.body.commit_id, HEAD);
  assert.deepEqual(sent.body.comments, [{ path: 'README.md', side: 'old', line: 2, body: 'Why a month earlier?' }]);
  await settle();
  await page.close();
});

await check('a thread takes a reply and is resolved', async () => {
  const { page, m, settle } = await open('/notshared/notes/pulls/3/files');
  const thread = page.locator('[data-thread="11"]');
  await thread.getByText('Beans need warmer soil').waitFor();
  await thread.getByRole('button', { name: 'Reply' }).click();
  await thread.locator('textarea').fill('Fair, keeping May for beans.');
  await thread.getByRole('button', { name: 'Reply' }).click();
  await thread.getByText('Fair, keeping May for beans.').waitFor();
  await thread.getByRole('button', { name: 'Resolve' }).click();
  await page.locator('[data-thread="11"]', { hasText: 'Resolved by notshared' }).waitFor();
  assert.ok(m.calls.some((c) => c.method === 'PATCH' && c.body.resolved === true));
  await settle();
  await page.close();
});

await check('a squash merge sends the head it showed', async () => {
  const { page, m, settle } = await open('/notshared/notes/pulls/3');
  await page.locator('[data-merge-box="open"]').waitFor();
  await page.locator('[data-merge-box] button', { hasText: 'Squash' }).click();
  await page.locator('[data-merge]').click();
  await page.locator('[data-merge-box="merged"]').waitFor();
  const merge = m.calls.find((c) => c.path.endsWith('/merge'));
  assert.equal(merge.body.style, 'squash');
  assert.equal(merge.body.head_commit_id, HEAD);
  assert.equal(merge.body.delete_branch, true);
  await settle();
  await page.close();
});

await check('the worktrees and how to mark one', async () => {
  const { page, settle } = await open('/notshared/notes/worktrees');
  await page.locator('[data-worktree="garden"]').getByRole('link', { name: '#3' }).waitFor();
  await page.locator('[data-worktree="old-try"]').getByText('Branch deleted').waitFor();
  await page.locator('[data-worktree-howto]').getByText('push.pushOption "worktree=fix-backup"', { exact: false }).waitFor();
  await settle();
  await page.close();
});

await check('the AI review switch in the settings turns LosOS Reviewer off', async () => {
  const { page, m, settle } = await open('/notshared/notes/settings');
  const box = page.locator('[data-ai-review-setting="on"]');
  await box.getByText('qwen2.5-coder:1.5b on this box comments on every pull request', { exact: false }).waitFor();
  if (Bun.env.GIT_SHOTS) await page.screenshot({ path: `${Bun.env.GIT_SHOTS}/settings.png`, fullPage: true });
  await box.getByRole('switch').click({ force: true });
  await page.locator('[data-ai-review-setting="off"]').waitFor();
  await page.getByText('AI review is off', { exact: true }).first().waitFor();
  const put = m.calls.find((c) => c.method === 'PUT' && c.path.endsWith('/ai-review'));
  assert.deepEqual(put.body, { enabled: false });
  await settle();
  await page.close();
});

await check("the room switch makes the repository's room in LosOS Chat and the header links it", async () => {
  const { page, m, settle } = await open('/notshared/notes/settings');
  const off = page.locator('[data-chat-room-setting="off"]');
  await off.getByText('Talk about this repository in LosOS Chat').waitFor();
  await off.getByText('People join when you invite them from chat', { exact: false }).waitFor();
  await off.getByRole('switch').click({ force: true });
  const on = page.locator('[data-chat-room-setting="on"]');
  await on.waitFor();
  await page.getByText('The room is ready', { exact: true }).first().waitFor();
  const made = m.calls.find((c) => c.path === '/api/chat/git-room');
  assert.deepEqual(made.body, { owner: 'notshared', repo: 'notes' });
  const link = `/chat/?join=${encodeURIComponent(ROOM)}&via=mattbox.local`;
  assert.equal(await on.locator('[data-chat-room-open]').getAttribute('href'), link);
  await page.getByRole('link', { name: 'Code' }).first().click();
  await page.locator('[data-readme]').waitFor();
  assert.equal(await page.locator('[data-chat-room]').getAttribute('href'), link);
  await page.getByRole('link', { name: 'Settings' }).first().click();
  await on.getByRole('switch').click({ force: true });
  await page.locator('[data-chat-room-setting="off"]').waitFor();
  const unlinked = m.calls.find((c) => c.method === 'PATCH');
  assert.deepEqual(unlinked.body, { chat_room: '' });
  await settle();
  await page.close();
});

await check('a visitor goes to their own box to say who they are', async () => {
  const { page, settle } = await open('/notshared/notes', { me: null });
  await page.locator('[data-readme]').waitFor();
  await page.locator('[data-chat-sign-in]').click();
  await page.locator('[data-chat-address]').fill('eva');
  await page.locator('[data-chat-continue]').click();
  await page.getByText('A chat address looks like @you:yourbox.local.').waitFor();
  await page.locator('[data-chat-address]').fill('@eva:kitchenbox.local');
  await settle();
  await page.route('https://kitchenbox.local/**', (route) =>
    route.fulfill({ status: 200, contentType: 'text/html', body: '<p>kitchenbox</p>' }),
  );
  await page.locator('[data-chat-continue]').click();
  await page.waitForURL('https://kitchenbox.local/chat/vouch**');
  const asked = new URL(page.url()).searchParams;
  assert.equal(asked.get('for'), new URL(origin).host);
  assert.equal(asked.get('back'), `${origin}/git/notshared/notes`);
  await page.close();
});

await check('back from their box, someone of another box keeps to their own branches', async () => {
  const { page, m, settle } = await open('/notshared/notes#chat-token=openid-tok&chat-server=kitchenbox.local&chat-name=Eva', {
    me: (req) => (req.headers().authorization === 'token chat-tok' ? EVA : null),
    room: ROOM,
  });
  await page.locator('[data-testid="signed-in-name"]', { hasText: 'Eva' }).waitFor();
  const session = m.calls.find((c) => c.path === '/matrix/session');
  assert.deepEqual(session.body, { access_token: 'openid-tok', matrix_server_name: 'kitchenbox.local', display_name: 'Eva' });
  assert.equal(new URL(page.url()).hash, '');
  assert.equal(await page.evaluate(() => localStorage.getItem('losos-git-chat-token')), 'chat-tok');
  await page.locator('[data-contributor-note]').getByText('eva@kitchenbox.local/', { exact: false }).waitFor();
  assert.equal(
    await page.locator('[data-chat-room]').getAttribute('href'),
    `https://kitchenbox.local/chat/?join=${encodeURIComponent(ROOM)}&via=mattbox.local`,
  );
  assert.equal(await page.locator('[data-chat-sign-in]').count(), 0);
  await settle();
  await page.close();
});

await check('German says du', async () => {
  const { page, settle } = await open('/notshared/notes/pulls/3/files', { locale: 'de-DE' });
  await page.locator('[data-review-panel]').getByText('Dein Review').waitFor();
  await settle();
  await page.close();
});

await check('a phone gets no sideways scroll on the pull request', async () => {
  const { page, settle } = await open('/notshared/notes/pulls/3', { viewport: { width: 390, height: 844 } });
  await page.locator('[data-pull-title]').waitFor();
  const wide = await page.evaluate(() => document.documentElement.scrollWidth - document.documentElement.clientWidth);
  assert.ok(wide <= 0, `the page scrolls sideways by ${wide}px`);
  await settle();
  await page.close();
});

await check('on a phone LosOS Git has a bar along the bottom: the places of the repository, then More for the rest', async () => {
  const { page, settle } = await open('/notshared/notes/pulls/3', { viewport: { width: 390, height: 844 } });
  const bar = page.getByRole('navigation', { name: 'Main places', exact: true });
  await bar.waitFor();
  assert.deepEqual((await bar.getByRole('listitem').allInnerTexts()).map((l) => l.trim()), ['Repositories', 'Code', 'Pull requests', 'Issues', 'More']);
  assert.equal(await bar.locator('a[aria-current="page"]').innerText(), 'Pull requests');
  const box = await bar.boundingBox();
  assert.ok(Math.abs(box.y + box.height - 844) <= 1, `the bar is not on the bottom edge: ${JSON.stringify(box)}`);
  await bar.getByRole('button', { name: 'More', exact: true }).click();
  const sheet = page.getByRole('dialog', { name: 'LosOS Git' });
  await sheet.waitFor();
  await sheet.getByRole('link', { name: 'Branches' }).click();
  await page.waitForURL(/\/git\/notshared\/notes\/branches$/);
  await sheet.waitFor({ state: 'detached' });
  await bar.getByRole('link', { name: 'Repositories', exact: true }).click();
  await page.waitForURL(/\/git\/?$/);
  await bar.getByRole('link', { name: 'Code', exact: true }).waitFor({ state: 'detached' });
  assert.deepEqual((await bar.getByRole('listitem').allInnerTexts()).map((l) => l.trim()), ['Repositories', 'Issues', 'More']);
  await settle();
  await page.close();
});

await browser.close();
await closeServer();
finish();
