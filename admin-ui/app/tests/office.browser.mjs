/* Documents (src/screens/Office.tsx) and the Office editor
 * (src/cloud/office/editor.tsx) inside the shell.
 *
 * What is checked: Documents is in the sidebar, after Apps, only on a box
 * whose /office/api/files answers JSON (a box with no Office folder answers
 * with the page itself); the list sits under the shell's own pane header in
 * a card, with each document's folder and a Read only badge; an empty
 * folder shows the empty state and a failed list a retry; a document opens
 * under the shell's top bar with no sidebar, framed from /browser/ with the
 * token and the language, with the editor's light or dark look set to the
 * page's and the page itself white; the LosOS bars drive the document
 * through Collabora's postMessage API and show what its map reports
 * (pressed buttons, the style, saved or not, the zoom, the others in it);
 * a read-only document gets no formatting bar; Back returns to the list;
 * and none of it adds to the Content-Security-Policy violations a plain
 * load logs. In LosOS Cloud, a document's name opens it in Office at
 * /cloud/office/<id> when the box runs Office, and only then.
 *
 * Same arrangement as the other files: the real dist/ bundle under the real
 * policy, the routes stubbed with page.route(). */

import assert from 'node:assert';
import { launch, runner, serve } from './harness.mjs';

const { origin, close: closeServer } = await serve({ csp: true });
const browser = await launch();
const { check, finish } = runner();

const SECTIONS = ['storage', 'backup', 'apps', 'market', 'network', 'look', 'lab', 'access', 'account'];
const FILES = [
  { id: '54726970206275646765742e6f6473', name: 'Trip budget.ods', folder: '', size: 10240, modified: 1792580040000, writable: true },
  { id: '5472697020706c616e2e6f6474', name: 'Trip plan.odt', folder: '', size: 11264, modified: 1792580040000, writable: true },
  { id: '576f726b2f4d656574696e67', name: 'Meeting notes.odt', folder: 'Work', size: 9830, modified: 1792580040000, writable: false },
];

/* Collabora's page as far as the editor bars know it: Frame_Ready, then
 * Document_Loaded once the host is ready; Get_Views answered with Matus and
 * Eva; Send_UNO_Command and Action_Save change the states and fire
 * `commandstatechanged` the way coolwsd does. Every message the page sends
 * is kept in window.got. */
const FAKE_COLLABORA = `<!doctype html><title>editor</title><script>
  const states = { '.uno:Bold': 'true', '.uno:StyleApply': 'Heading 1', '.uno:FontHeight': '24',
    '.uno:LeftPara': 'true', '.uno:Undo': 'disabled', '.uno:ModifiedStatus': 'false' };
  const handlers = {};
  let zoom = 8;
  const fire = (type, e) => (handlers[type] || []).forEach((fn) => fn(e));
  const set = (commandName, state) => { states[commandName] = state; fire('commandstatechanged', { commandName, state }); };
  window.app = { map: {
    stateChangeHandler: { getItemValue: (c) => states[c] },
    on: (t, fn) => (handlers[t] = [...(handlers[t] || []), fn]),
    off: (t, fn) => (handlers[t] = (handlers[t] || []).filter((f) => f !== fn)),
    getZoom: () => zoom,
    getZoomScale: (to, from) => Math.pow(1.2, to - from),
    setZoom: (z) => { zoom = z; fire('zoomend', {}); },
    zoomIn: () => { zoom += 1; fire('zoomend', {}); },
    zoomOut: () => { zoom -= 1; fire('zoomend', {}); },
  }, LOUtil: { getViewIdColor: (id) => (id === 1 ? 0x2a8c3c : 0x000000) } };
  window.got = [];
  const post = (m) => parent.postMessage(JSON.stringify(m), location.origin);
  addEventListener('message', (event) => {
    const m = JSON.parse(event.data);
    window.got.push(m);
    if (m.MessageId === 'Host_PostmessageReady') post({ MessageId: 'App_LoadingStatus', Values: { Status: 'Document_Loaded' } });
    if (m.MessageId === 'Get_Views') post({ MessageId: 'Get_Views_Resp', Values: [
      { ViewId: '0', UserName: 'Matus', Color: 0x0e6e7d, IsCurrentView: true },
      { ViewId: '1', UserName: 'Eva Nováková', Color: 0xc23c2a, IsCurrentView: false },
    ] });
    if (m.MessageId === 'Send_UNO_Command') {
      const c = m.Values.Command;
      if (c === '.uno:Italic') set(c, states[c] === 'true' ? 'false' : 'true');
      if (c === '.uno:StyleApply') set(c, m.Values.Args.Style.value);
      set('.uno:Undo', 'enabled');
      set('.uno:ModifiedStatus', 'true');
    }
    if (m.MessageId === 'Action_Save') set('.uno:ModifiedStatus', 'false');
  });
  post({ MessageId: 'App_LoadingStatus', Values: { Status: 'Frame_Ready' } });
</script>`;

const json = (route, status, body) =>
  route.fulfill({ status, contentType: 'application/json', body: JSON.stringify(body) });

/* A box with an administrator signed in through LosOS Login. `files` is
 * what GET /office/api/files answers: a list, null for a box with no Office
 * folder (the page itself), or a status number. */
async function open({ path = '/office/', files = FILES, colorScheme = 'dark' } = {}) {
  const context = await browser.newContext({ viewport: { width: 1280, height: 900 }, locale: 'en-US', colorScheme });
  const page = await context.newPage();
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  const opened = [];

  await page.route('**/api/**', (route) => json(route, 200, {}));
  await page.route('**/api/setup/claim', (route) => json(route, 200, { claimed: true }));
  await page.route('**/api/me', (route) =>
    json(route, 200, {
      name: 'matus',
      displayName: 'Matus',
      groups: ['losos-admins', 'losos-owner'],
      admin: true,
      spareKey: false,
      sections: SECTIONS,
      standalone: true,
    }),
  );
  await page.route('**/api/state', (route) => json(route, 200, { mode: 'local', sharing: false }));
  await page.route('**/api/status', (route) => json(route, 200, { state: 'idle', progress: 0, message: '' }));
  await page.route('**/office/api/files', (route) => {
    if (files === null) {
      return route.fulfill({ status: 200, contentType: 'text/html', body: '<!doctype html><title>LosOS</title>' });
    }
    if (typeof files === 'number') return json(route, files, { error: 'no' });
    return json(route, 200, { files });
  });
  await page.route('**/office/api/open', (route) => {
    const { fileId } = JSON.parse(route.request().postData() ?? '{}');
    opened.push(fileId);
    const file = FILES.find((f) => f.id === fileId);
    return json(route, 200, {
      url: '/browser/abc/cool.html?WOPISrc=http%3A%2F%2F127.0.0.1%3A8093%2Fwopi%2Ffiles%2Fx',
      accessToken: 'token-1',
      accessTokenTtl: 1792616040000,
      writable: file.writable,
      name: file.name,
      https: false,
    });
  });
  // The editor itself is coolwsd's page; here a small stand-in with the
  // same postMessage handshake and the part of `app.map` the page reads.
  await page.route('**/browser/**', (route) =>
    route.fulfill({ status: 200, contentType: 'text/html', body: FAKE_COLLABORA }),
  );

  await page.addInitScript(() => {
    window.localStorage.setItem('losos-tour', 'seen');
    window.__violations = [];
    document.addEventListener('securitypolicyviolation', (e) =>
      window.__violations.push(`${e.violatedDirective} ${e.blockedURI}`),
    );
  });
  await page.goto(origin + path, { waitUntil: 'networkidle' });
  return { page, context, errors, opened };
}

const violations = (page) => page.evaluate(() => [...new Set(window.__violations)].sort());
const nav = (page) => page.getByRole('navigation', { name: 'Sections', exact: true });
const labels = async (page) => (await nav(page).getByRole('link').allInnerTexts()).map((l) => l.trim());

async function baseline() {
  const { page, context } = await open({ path: '/office/', files: null });
  await page.waitForTimeout(300);
  const seen = await violations(page);
  await context.close();
  return seen;
}
const BASELINE = await baseline();

console.log('admin-ui documents checks');

await check('a box with no Office folder has no Documents in the sidebar', async () => {
  const { page, context, errors } = await open({ path: '/office/', files: null });
  await nav(page).getByRole('link', { name: 'Overview', exact: true }).waitFor();
  await page.waitForTimeout(300);
  assert.ok(!(await labels(page)).includes('Documents'), (await labels(page)).join(' | '));
  assert.deepEqual(errors, []);
  await context.close();
});

await check('a box with an Office folder lists Documents after Apps, and it opens the list', async () => {
  const { page, context, errors } = await open({ path: '/office/' });
  await nav(page).getByRole('link', { name: 'Documents', exact: true }).waitFor({ timeout: 3000 });
  const all = await labels(page);
  assert.equal(all[all.indexOf('Apps') + 1], 'Documents', all.join(' | '));
  await nav(page).getByRole('link', { name: 'Documents', exact: true }).click();
  await page.getByRole('heading', { level: 1, name: 'Documents' }).waitFor();
  assert.equal(new URL(page.url()).pathname, '/office/');
  assert.equal(
    await nav(page).getByRole('link', { name: 'Documents', exact: true }).getAttribute('aria-current'),
    'page',
  );
  assert.deepEqual(errors, []);
  await context.close();
});

await check('the list shows each document, its folder and whether it is read only, in a card', async () => {
  const { page, context, errors } = await open();
  const table = page.locator('[data-slot="card"] table');
  await table.waitFor();
  const rows = table.locator('tbody tr');
  assert.equal(await rows.count(), 3);
  assert.match(await rows.nth(0).innerText(), /Trip budget\.ods/);
  assert.match(await rows.nth(2).innerText(), /Meeting notes\.odt\s+Work\s+Read only/);
  assert.equal(await rows.nth(1).getByRole('link').getAttribute('href'), `#file=${FILES[1].id}`);
  assert.equal(await page.locator('main[aria-busy="true"], [aria-busy="true"]').count(), 0);
  assert.deepEqual(errors, []);
  await context.close();
});

await check('an empty folder says so, and a failed list offers to try again', async () => {
  let { page, context } = await open({ files: [] });
  await page.getByText('No documents yet').waitFor();
  await context.close();
  ({ page, context } = await open({ files: 503 }));
  await page.getByText('Office is still starting. Try again in a minute.').waitFor();
  await page.getByRole('button', { name: 'Try again' }).waitFor();
  await context.close();
});

await check('a document opens under the top bar, beside the sidebar, in the LosOS look', async () => {
  const { page, context, errors, opened } = await open();
  await page.getByRole('link', { name: /Trip plan\.odt/ }).click();
  const frame = page.locator('main iframe');
  await frame.waitFor();
  assert.deepEqual(opened, [FILES[1].id]);
  assert.equal(new URL(page.url()).hash, `#file=${FILES[1].id}`);
  // The shell's top bar and sidebar stay where they were; the editor takes
  // the rest of the window beside the sidebar.
  await page.getByTestId('signed-in-as').waitFor();
  await nav(page).waitFor();
  await page.getByRole('heading', { level: 1, name: 'Trip plan.odt' }).waitFor();
  const box = await frame.boundingBox();
  const side = await nav(page).boundingBox();
  assert.ok(box.x >= side.x + side.width && box.width >= 900 && box.height > 600, `the editor is ${JSON.stringify(box)}`);

  const src = new URL(await frame.getAttribute('src'), origin);
  assert.equal(src.pathname, '/browser/abc/cool.html');
  assert.equal(src.searchParams.get('access_token'), 'token-1');
  assert.equal(src.searchParams.get('lang'), 'en-GB');
  // The look is the stylesheet the box serves the editor, not a parameter.
  assert.equal(src.searchParams.get('css_variables'), null);

  // The editor's own look follows the page's; the document stays white.
  assert.equal(await page.evaluate(() => localStorage.getItem('darkTheme')), 'true');
  assert.equal(await page.evaluate(() => localStorage.getItem('darkBackgroundForTheme.dark')), 'false');

  await page.getByRole('button', { name: 'Back to files' }).click();
  await page.getByRole('heading', { level: 1, name: 'Documents' }).waitFor();
  await nav(page).waitFor();
  assert.deepEqual(errors, []);
  await context.close();
});

await check('the LosOS bars drive the document and show what it reports', async () => {
  const { page, context, errors } = await open({ path: `/office/#file=${FILES[1].id}` });
  const bar = page.getByRole('toolbar', { name: 'Formatting' });
  await bar.waitFor();
  const got = () => page.frameLocator('main iframe').locator('body').evaluate(() => window.got);
  const button = (name) => bar.getByRole('button', { name, exact: true });

  // What the document reported when it loaded.
  await page.getByTestId('office-saved').getByText('Saved to the box').waitFor();
  assert.equal(await button('Bold').getAttribute('aria-pressed'), 'true');
  assert.equal(await button('Italic').getAttribute('aria-pressed'), 'false');
  assert.equal(await button('Align left').getAttribute('aria-pressed'), 'true');
  assert.ok(await button('Undo').isDisabled());
  assert.equal(await bar.getByLabel('Paragraph style').inputValue(), 'Heading 1');
  assert.equal(await bar.getByLabel('Font size').inputValue(), '24');
  // Opened at the page's real size.
  await page.getByTestId('office-zoom').getByText('100%').waitFor();
  // Eva is in it too, ringed in her cursor's colour; Matus is not listed.
  const viewers = page.getByTestId('office-viewers');
  await viewers.getByLabel('Eva Nováková').waitFor();
  assert.equal(await viewers.locator('li').count(), 1);
  // The ring is the colour Collabora draws Eva's cursor in, not her author colour.
  const ring = await viewers.getByLabel('Eva Nováková').evaluate((el) => {
    for (let n = el; n; n = n.parentElement) if (n.style.boxShadow) return n.style.boxShadow;
    return '';
  });
  assert.match(ring, /rgb\(42, 140, 60\)/, ring);
  assert.ok(await page.getByRole('button', { name: 'Save' }).isDisabled());

  // A button sends the command and hands the keyboard back.
  await button('Italic').click();
  await page.waitForFunction(() => document.querySelector('[data-command=".uno:Italic"]')?.getAttribute('aria-pressed') === 'true');
  const sent = await got();
  const i = sent.findIndex((m) => m.MessageId === 'Send_UNO_Command' && m.Values.Command === '.uno:Italic');
  assert.ok(i >= 0, JSON.stringify(sent));
  assert.equal(sent[i + 1].MessageId, 'Grab_Focus');
  await page.getByTestId('office-saved').getByText('Changes not saved yet').waitFor();
  assert.ok(!(await button('Undo').isDisabled()));

  // The style list applies a paragraph style.
  await bar.getByLabel('Paragraph style').selectOption('Heading 2');
  await page.waitForFunction(() => document.querySelector('select[aria-label="Paragraph style"]')?.value === 'Heading 2');
  const style = (await got()).find((m) => m.Values?.Command === '.uno:StyleApply');
  assert.deepEqual(style.Values.Args, {
    Style: { type: 'string', value: 'Heading 2' },
    FamilyName: { type: 'string', value: 'ParagraphStyles' },
  });

  // Zoom and Save.
  await page.getByRole('button', { name: 'Zoom in' }).click();
  await page.getByTestId('office-zoom').getByText('120%').waitFor();
  await page.getByRole('button', { name: 'Save' }).click();
  await page.getByTestId('office-saved').getByText('Saved to the box').waitFor();
  await page.getByRole('button', { name: 'Download as PDF' }).click();
  const exported = async () => (await got()).some((m) => m.MessageId === 'Action_Export' && m.Values.Format === 'pdf');
  for (let n = 0; n < 20 && !(await exported()); n++) await page.waitForTimeout(50);
  assert.ok(await exported(), JSON.stringify(await got()));
  assert.deepEqual(errors, []);
  await context.close();
});

await check('in the light theme the editor opens light, on a white page', async () => {
  const { page, context } = await open({ path: `/office/#file=${FILES[1].id}`, colorScheme: 'light' });
  await page.locator('main iframe').waitFor();
  assert.equal(await page.evaluate(() => localStorage.getItem('darkTheme')), 'false');
  assert.equal(await page.evaluate(() => localStorage.getItem('darkBackgroundForTheme.light')), 'false');
  await context.close();
});

await check('a read-only document says so in the editor bar', async () => {
  const { page, context } = await open({ path: `/office/#file=${FILES[2].id}` });
  await page.locator('main iframe').waitFor();
  await page.getByText('Read only', { exact: true }).waitFor();
  // Nothing to format and nothing to save; the zoom and the PDF stay.
  await page.getByRole('button', { name: 'Download as PDF' }).waitFor();
  assert.equal(await page.getByRole('toolbar', { name: 'Formatting' }).count(), 0);
  assert.equal(await page.getByRole('button', { name: 'Save' }).count(), 0);
  await context.close();
});

await check('Documents and the editor add nothing to the policy violations a plain load logs', async () => {
  const { page, context } = await open();
  await page.getByRole('link', { name: /Trip plan\.odt/ }).click();
  await page.locator('main iframe').waitFor();
  await page.waitForTimeout(300);
  assert.deepEqual(await violations(page), BASELINE);
  await context.close();
});

/* LosOS Cloud's web app with one Word file and one text file; `office` is
 * whether the box runs Office (its extensions call answers or 404s). */
async function openCloud(path, { office = true } = {}) {
  const context = await browser.newContext({ viewport: { width: 1280, height: 900 }, locale: 'en-US', colorScheme: 'dark' });
  const page = await context.newPage();
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  const opened = [];
  const entry = (id, name, mime) => ({ id, path: name, name, dir: false, size: 1, mtime: 1, etag: 'x', mime, favorite: false });
  await page.route('**/cloud/api/**', (route) => {
    const req = route.request();
    const p = new URL(req.url()).pathname.slice('/cloud/api'.length);
    if (p === '/me') return json(route, 200, { login: 'matus', name: 'Matus', email: '', used: 1e9, free: 9e9, product: 'LosOS Cloud' });
    if (p === '/files') {
      return json(route, 200, {
        folder: { ...entry(1, '', ''), dir: true },
        entries: [
          entry(42, 'Trip plan.docx', 'application/vnd.openxmlformats-officedocument.wordprocessingml.document'),
          entry(43, 'todo.txt', 'text/plain'),
        ],
      });
    }
    if (p === '/office/extensions') {
      return office ? json(route, 200, { extensions: [{ extension: 'docx', edit: true }] }) : json(route, 404, { code: 'notFound' });
    }
    if (p === '/office/open') {
      opened.push(JSON.parse(req.postData() ?? '{}').fileId);
      return json(route, 200, {
        url: '/browser/abc/cool.html?WOPISrc=http%3A%2F%2F127.0.0.1%3A8093%2Fwopi%2Ffiles%2F42',
        accessToken: 'token-2',
        accessTokenTtl: 1792616040000,
        writable: true,
        name: 'Trip plan.docx',
        https: false,
      });
    }
    return json(route, 200, {});
  });
  await page.route('**/browser/**', (route) => route.fulfill({ status: 200, contentType: 'text/html', body: FAKE_COLLABORA }));
  await page.goto(origin + path);
  return { page, context, errors, opened };
}

await check('in LosOS Cloud a document opens in Office, in the same frame as the box\'s pages', async () => {
  const { page, context, errors, opened } = await openCloud('/cloud/files');
  const doc = page.getByRole('link', { name: 'Trip plan.docx' }).first();
  await doc.waitFor();
  await page.waitForFunction(() =>
    [...document.querySelectorAll('a')].some((a) => a.getAttribute('href') === '/cloud/office/42'),
  );
  await page.goto(`${origin}/cloud/office/42`);
  await page.getByRole('toolbar', { name: 'Formatting' }).waitFor();
  await page.getByRole('heading', { level: 1, name: 'Trip plan.docx' }).waitFor();
  assert.deepEqual(opened, ['42']);
  const box = await page.locator('main iframe').boundingBox();
  const side = await nav(page).boundingBox();
  assert.ok(box.x >= side.x + side.width && box.width >= 900 && box.height > 600, JSON.stringify(box));
  await page.getByRole('button', { name: 'Back to files' }).click();
  await page.waitForURL(/\/cloud\/files/);
  assert.deepEqual(errors, []);
  await context.close();
});

await check('a box without Office leaves documents to the browser', async () => {
  const { page, context } = await openCloud('/cloud/files', { office: false });
  await page.getByRole('link', { name: 'Trip plan.docx' }).first().waitFor();
  await page.waitForTimeout(300);
  const hrefs = await page.locator('a').evaluateAll((as) => as.map((a) => a.getAttribute('href')));
  assert.ok(!hrefs.some((h) => h?.startsWith('/cloud/office/')), hrefs.join(' '));
  await context.close();
});

await browser.close();
await closeServer();
finish();
