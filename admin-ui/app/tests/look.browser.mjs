/* The look: a background picture behind the page, and widgets written by
 * hand drawn in a sandboxed frame.
 *
 * Same arrangement as the other files: the real dist/ bundle, the API
 * stubbed with page.route(), the stub enforcing the Bearer token. Two things
 * are specific to this file. The static server serves /widget-frame/ under
 * the frame's own policy (harness.mjs, WIDGET_FRAME_CSP) the way nginx's
 * header map does, so the checks here run the frame under the policy the box
 * will give it; and every check that touches the frame runs under the
 * appliance CSP, because the point of the frame is that the admin page's
 * policy stays strict while the widget's script runs anyway.
 */

import assert from 'node:assert';
import { launch, runner, serve } from './harness.mjs';

const TOKEN = 'a'.repeat(64);
const { origin, close: closeServer } = await serve({ csp: true });
const browser = await launch();
const { check, finish } = runner();

const SETTINGS = {
  sharingMyStorage: false,
  nextcloudMode: 'container',
  forgejoMode: 'container',
  hostName: 'mattbox',
  https: false,
  gpuEnable: false,
  apachePort: 80,
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

const LIMITS = {
  widgets: 24,
  nameChars: 60,
  widgetBytes: 131072,
  files: 12,
  fileNameChars: 40,
  fileKinds: ['html', 'css', 'js', 'mjs', 'json', 'svg', 'txt', 'md'],
  imageBytes: 8388608,
  veil: { min: 20, max: 90 },
};

/* A widget that draws the box's name through the bridge and says which
 * theme it was handed, in three files: its stylesheet and script are linked
 * from index.html. What the checks read back out of the frame. */
const CLOCK = {
  id: 'w1',
  name: 'Box name',
  span: 'half',
  files: [
    {
      name: 'index.html',
      content: [
        '<link rel="stylesheet" href="clock.css">',
        '<div class="big" id="out">waiting</div>',
        '<div id="theme"></div>',
        '<script src="clock.js"></script>',
      ].join('\n'),
    },
    { name: 'clock.css', content: '.big { font-size: 24px; color: var(--accent); }' },
    {
      name: 'clock.js',
      content: [
        'document.getElementById("theme").textContent = "theme:" + losos.theme;',
        'losos.onTheme((t) => { document.getElementById("theme").textContent = "theme:" + t; });',
        'losos.metric("box.settings").then((m) => {',
        '  document.getElementById("out").textContent = "box:" + m.hostName;',
        '}).catch((e) => { document.getElementById("out").textContent = "failed:" + e.message; });',
      ].join('\n'),
    },
  ],
};

const json = (route, status, body) =>
  route.fulfill({ status, contentType: 'application/json', body: JSON.stringify(body) });

/* A tiny PNG (1x1), so an upload is real bytes with the magic lososd sniffs. */
const PNG = Buffer.from(
  'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==',
  'base64',
);

/* A claimed, signed-in box with a look. `look` is what GET /api/look answers
 * with, and every write is recorded in `writes` as [method, path, body]. */
async function open({
  path = '/',
  look = { version: 1, background: { kind: 'none' }, veil: 60, widgets: [], shipped: ['tide', 'grid', 'dusk'], limits: LIMITS },
  board = null,
  viewport = { width: 1280, height: 900 },
  locale = 'en-US',
  builder = null,
} = {}) {
  const page = await browser.newPage({ viewport, locale });
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  await page.addInitScript(() => {
    window.__violations = [];
    document.addEventListener('securitypolicyviolation', (e) =>
      window.__violations.push(`${e.violatedDirective} ${e.blockedURI}`),
    );
  });

  let current = look;
  const writes = [];
  const authed = (route) => route.request().headers()['authorization'] === `Bearer ${TOKEN}`;

  await page.route('**/api/**', (route) => json(route, 200, {}));
  await page.route('**/api/setup/claim', (route) => json(route, 200, { claimed: true }));
  await page.route('**/api/state', (route) =>
    authed(route) ? json(route, 200, { mode: 'local', sharing: false }) : json(route, 401, { error: 'unauthorized' }),
  );
  await page.route('**/api/settings', (route) =>
    authed(route) ? json(route, 200, SETTINGS) : json(route, 401, { error: 'unauthorized' }),
  );
  await page.route('**/api/status', (route) =>
    authed(route) ? json(route, 200, { state: 'idle', progress: 0, message: '' }) : json(route, 401, { error: 'unauthorized' }),
  );
  await page.route('**/api/look', (route) => {
    if (!authed(route)) return json(route, 401, { error: 'unauthorized' });
    if (route.request().method() === 'GET') return json(route, 200, current);
    const patch = route.request().postDataJSON();
    writes.push(['POST', '/api/look', patch]);
    current = {
      ...current,
      ...(patch.background ? { background: patch.background } : {}),
      ...(typeof patch.veil === 'number' ? { veil: patch.veil } : {}),
    };
    return json(route, 200, current);
  });
  await page.route('**/api/look/background**', (route) => {
    const method = route.request().method();
    if (method === 'GET') {
      // The picture itself, served without a token like lososd does.
      return route.fulfill({ status: 200, contentType: 'image/png', body: PNG });
    }
    if (!authed(route)) return json(route, 401, { error: 'unauthorized' });
    if (method === 'PUT') {
      const body = route.request().postDataBuffer();
      writes.push(['PUT', '/api/look/background', body]);
      const version = (current.background.version ?? 0) + 1;
      current = { ...current, background: { kind: 'upload', version, contentType: 'image/png', url: `/api/look/background?v=${version}` } };
      return json(route, 200, current);
    }
    writes.push(['DELETE', '/api/look/background', null]);
    current = { ...current, background: { kind: 'none' } };
    return json(route, 200, current);
  });
  let nextId = 2;
  await page.route('**/api/look/widgets**', (route) => {
    if (!authed(route)) return json(route, 401, { error: 'unauthorized' });
    const method = route.request().method();
    const url = new URL(route.request().url());
    if (method === 'DELETE') {
      const id = url.pathname.split('/').pop();
      writes.push(['DELETE', url.pathname, null]);
      const before = current.widgets.length;
      current = { ...current, widgets: current.widgets.filter((w) => w.id !== id) };
      return json(route, 200, { deleted: current.widgets.length !== before });
    }
    const draft = route.request().postDataJSON();
    writes.push(['POST', '/api/look/widgets', draft]);
    const widget = { id: draft.id ?? `w${nextId++}`, name: draft.name, span: draft.span ?? 'half', files: draft.files };
    current = {
      ...current,
      widgets: current.widgets.some((w) => w.id === widget.id)
        ? current.widgets.map((w) => (w.id === widget.id ? widget : w))
        : [...current.widgets, widget],
    };
    return json(route, 200, { widget });
  });

  // The widget builder on the edge, when the check gives one: a GET for the
  // account, a POST that starts a build, and the build read back by id.
  if (builder !== null) {
    await page.route('**/api/builder**', (route) => {
      if (!authed(route)) return json(route, 401, { error: 'unauthorized' });
      const url = new URL(route.request().url());
      const method = route.request().method();
      if (method === 'POST') writes.push(['POST', url.pathname, route.request().postDataJSON()]);
      return builder(url.pathname, method, route, json);
    });
  }

  // Init scripts run in every frame, the sandboxed widget frame included,
  // where storage throws: that is the sandbox working, not a page error.
  await page.addInitScript((t) => {
    try {
      window.sessionStorage.setItem('losos-token', t);
    } catch {}
  }, TOKEN);
  if (board !== null) {
    await page.addInitScript((b) => {
      try {
        window.localStorage.setItem('losos-widgets', JSON.stringify(b));
      } catch {}
    }, board);
  }
  await page.goto(origin + path, { waitUntil: 'networkidle' });
  return { page, errors, writes, look: () => current };
}

const html = (page) => page.locator('html');
const bgVar = (page) => page.evaluate(() => document.documentElement.style.getPropertyValue('--losos-bg-image'));
const veilVar = (page) => page.evaluate(() => document.documentElement.style.getPropertyValue('--losos-bg-veil'));
const violations = (page) => page.evaluate(() => window.__violations);
const frame = (page) => page.frameLocator('iframe[sandbox="allow-scripts"]').first();

/* The code editor is CodeMirror in a shadow root; its content element is a
 * textbox named after the file, and Playwright's role queries reach into
 * open shadow roots. `typeInto` replaces the whole file with `text`, pasted
 * in one go, then types `typed` key by key: completions open on typing,
 * not on a paste. */
const code = (scope, file) => scope.getByRole('textbox', { name: `Source of ${file}` });
async function typeInto(page, scope, file, text, typed = '') {
  const box = code(scope, file);
  await box.click();
  await page.keyboard.press('ControlOrMeta+a');
  if (text.length > 0) await page.keyboard.insertText(text);
  else await page.keyboard.press('Delete');
  if (typed.length > 0) await page.keyboard.type(typed);
}
const fileButton = (scope, file) => scope.getByRole('button', { name: file, exact: true });

/* Open the editor on a new widget, from the board. */
async function openEditor(page) {
  await page.getByRole('button', { name: 'Add a widget' }).first().click();
  await page.getByRole('dialog', { name: 'Add a widget' }).getByRole('button', { name: 'Write one' }).click();
  const editor = page.getByRole('dialog', { name: 'Write a widget' });
  await editor.waitFor();
  return editor;
}

console.log('admin-ui look checks');

// ── Background ────────────────────────────────────────────────────────────

await check('a plain box has no wallpaper: no class on <html>, no variable, the swatch for Plain checked', async () => {
  const { page, errors } = await open({ path: '/settings/look' });
  const group = page.getByRole('radiogroup', { name: 'Background' });
  await group.waitFor();
  assert.equal(await group.getByRole('radio', { name: 'Plain' }).getAttribute('aria-checked'), 'true');
  assert.equal(await group.getByRole('radio').count(), 5, 'Plain, three shipped pictures, and your own');
  assert.ok(!(await html(page).evaluate((el) => el.classList.contains('has-bg'))));
  assert.equal(await bgVar(page), '');
  assert.deepEqual(errors, []);
  await page.close();
});

await check('picking a shipped picture writes the box and paints it through CSSOM under the strict CSP', async () => {
  const { page, errors, writes } = await open({ path: '/settings/look' });
  const group = page.getByRole('radiogroup', { name: 'Background' });
  const atLoad = (await violations(page)).length;
  await group.getByRole('radio', { name: 'Tide' }).click();
  await page.getByText('Background set.', { exact: true }).first().waitFor();
  assert.deepEqual(writes, [['POST', '/api/look', { background: { kind: 'shipped', name: 'tide' } }]]);
  assert.equal(await group.getByRole('radio', { name: 'Tide' }).getAttribute('aria-checked'), 'true');
  assert.ok(await html(page).evaluate((el) => el.classList.contains('has-bg')), 'has-bg was not set');
  assert.equal(await bgVar(page), 'url("/backgrounds/tide.svg")');
  assert.equal(await veilVar(page), '0.6');
  // The picture is a real background on the shell, not only a variable.
  const painted = await page.locator('.losos-shell').evaluate((el) => getComputedStyle(el).backgroundImage);
  assert.match(painted, /linear-gradient/, `no veil in the shell background: ${painted}`);
  assert.match(painted, /backgrounds\/tide\.svg/, `the picture is not in the shell background: ${painted}`);
  const after = await violations(page);
  assert.equal(after.length, atLoad, `painting the wallpaper added CSP violations: ${after.slice(atLoad).join(', ')}`);
  assert.deepEqual(errors, []);
  await page.close();
});

await check('the veil follows the slider at once and is saved once, a moment later', async () => {
  const { page, writes } = await open({
    path: '/settings/look',
    look: { version: 1, background: { kind: 'shipped', name: 'grid' }, veil: 60, widgets: [], shipped: ['tide', 'grid', 'dusk'], limits: LIMITS },
  });
  const slider = page.getByRole('slider', { name: /^Veil/ });
  await slider.waitFor();
  assert.equal(await veilVar(page), '0.6');
  await slider.focus();
  await page.keyboard.press('ArrowLeft');
  await page.keyboard.press('ArrowLeft');
  assert.equal(await veilVar(page), '0.5', 'the veil did not follow the slider at once');
  await page.getByText('Veil saved.', { exact: true }).first().waitFor();
  assert.deepEqual(writes, [['POST', '/api/look', { veil: 50 }]], 'two movements should be one write');
  await page.close();
});

await check('a picture of your own is uploaded as its bytes and painted from the versioned route', async () => {
  const { page, writes } = await open({ path: '/settings/look' });
  await page.getByRole('radiogroup', { name: 'Background' }).waitFor();
  await page.getByTestId('background-file').setInputFiles({ name: 'wall.png', mimeType: 'image/png', buffer: PNG });
  await page.getByText('Background set.', { exact: true }).first().waitFor();
  assert.equal(writes.length, 1);
  assert.equal(writes[0][0], 'PUT');
  assert.ok(Buffer.compare(writes[0][2], PNG) === 0, 'the bytes sent are not the file');
  assert.equal(await bgVar(page), 'url("/api/look/background?v=1")');
  assert.equal(await page.getByRole('radio', { name: 'Your own picture' }).getAttribute('aria-checked'), 'true');
  await page.getByRole('button', { name: 'Remove the picture' }).click();
  await page.getByText('Background removed.', { exact: true }).first().waitFor();
  assert.deepEqual(writes[1], ['DELETE', '/api/look/background', null]);
  assert.equal(await bgVar(page), '');
  await page.close();
});

await check('a file that is not a picture lososd takes is refused before it is sent', async () => {
  const { page, writes } = await open({ path: '/settings/look' });
  await page.getByRole('radiogroup', { name: 'Background' }).waitFor();
  await page.getByTestId('background-file').setInputFiles({ name: 'wall.svg', mimeType: 'image/svg+xml', buffer: Buffer.from('<svg/>') });
  await page.getByText(/not a picture this box takes/).waitFor();
  assert.deepEqual(writes, []);
  await page.close();
});

await check('the wallpaper leaves with the sign-out', async () => {
  const { page } = await open({
    look: { version: 1, background: { kind: 'shipped', name: 'dusk' }, veil: 40, widgets: [], shipped: ['tide', 'grid', 'dusk'], limits: LIMITS },
  });
  await page.waitForFunction(() => document.documentElement.classList.contains('has-bg'));
  assert.equal(await veilVar(page), '0.4');
  await page.getByRole('button', { name: 'Sign out' }).click();
  await page.waitForFunction(() => !document.documentElement.classList.contains('has-bg'));
  assert.equal(await bgVar(page), '');
  await page.close();
});

// ── Widgets written by hand ───────────────────────────────────────────────

await check('a hand-written widget on the board runs in a sandboxed frame, reaches the box through the bridge, and the page stays clean', async () => {
  const { page, errors } = await open({
    look: { version: 1, background: { kind: 'none' }, veil: 60, widgets: [CLOCK], shipped: ['tide', 'grid', 'dusk'], limits: LIMITS },
    board: { version: 1, widgets: [{ id: 'tile1', source: { kind: 'hand', id: 'w1' } }] },
  });
  const tile = page.getByTestId('hand-tile');
  await tile.waitFor();
  const iframe = tile.locator('iframe');
  assert.equal(await iframe.getAttribute('sandbox'), 'allow-scripts', 'the frame must be sandboxed without allow-same-origin');
  assert.equal(await iframe.getAttribute('src'), '/widget-frame/');
  assert.equal(await iframe.getAttribute('title'), 'Box name (widget)');
  // The widget's own script ran and the bridge answered a reading.
  await frame(page).locator('#out').filter({ hasText: 'box:mattbox' }).waitFor({ timeout: 10000 });
  await frame(page).locator('#theme').filter({ hasText: 'theme:light' }).waitFor();
  // The widget was given the box's palette.
  const accent = await frame(page).locator('#out').evaluate((el) => getComputedStyle(el).color);
  assert.equal(accent, 'rgb(14, 110, 125)', `the accent did not reach the frame: ${accent}`);
  // The frame grew to what the widget drew, from the floor it started at.
  const height = await iframe.evaluate((el) => el.getBoundingClientRect().height);
  assert.ok(height >= 72, `frame height ${height}`);
  // The frame is an opaque origin: it cannot see this page.
  const crossed = await frame(page).locator('body').evaluate(() => {
    try {
      return typeof window.parent.document;
    } catch {
      return 'blocked';
    }
  });
  assert.equal(crossed, 'blocked', 'the frame could reach the admin page');
  const seen = await violations(page);
  assert.ok(!seen.some((v) => v.startsWith('script-src')), `the admin page refused script: ${seen.join(', ')}`);
  assert.deepEqual(errors, []);
  await page.close();
});

await check('the theme toggle reaches the widget through the bridge', async () => {
  const { page } = await open({
    look: { version: 1, background: { kind: 'none' }, veil: 60, widgets: [CLOCK], shipped: ['tide', 'grid', 'dusk'], limits: LIMITS },
    board: { version: 1, widgets: [{ id: 'tile1', source: { kind: 'hand', id: 'w1' } }] },
  });
  await frame(page).locator('#theme').filter({ hasText: 'theme:light' }).waitFor({ timeout: 10000 });
  await page.getByRole('radiogroup', { name: 'Appearance' }).getByRole('radio', { name: 'Dark' }).click();
  await frame(page).locator('#theme').filter({ hasText: 'theme:dark' }).waitFor();
  const accent = await frame(page).locator('#out').evaluate((el) => getComputedStyle(el).color);
  assert.equal(accent, 'rgb(72, 179, 192)', `the dark accent did not reach the frame: ${accent}`);
  await page.close();
});

await check('a widget the box no longer has greys its tile out with a sentence', async () => {
  const { page, errors } = await open({
    board: { version: 1, widgets: [{ id: 'tile1', source: { kind: 'hand', id: 'w9' } }] },
  });
  const tile = page.getByTestId('hand-tile');
  await tile.getByText('This widget is no longer on this box.').waitFor();
  assert.equal(await tile.locator('iframe').count(), 0);
  assert.deepEqual(errors, []);
  await page.close();
});

await check('the gallery writes a widget by hand: the editor previews it in the frame, saving writes the box and puts it on the board', async () => {
  const { page, errors, writes } = await open();
  await page.getByRole('button', { name: 'Add a widget' }).first().click();
  const gallery = page.getByRole('dialog', { name: 'Add a widget' });
  await gallery.waitFor();
  await gallery.getByRole('button', { name: 'Write one' }).click();
  const editor = page.getByRole('dialog', { name: 'Write a widget' });
  await editor.waitFor();
  // The preview is the real frame drawing the template.
  await page.getByTestId('hand-preview').locator('iframe[sandbox="allow-scripts"]').waitFor();
  await editor.getByLabel('Name', { exact: true }).fill('Room left');
  await editor.getByLabel('Width').selectOption('full');
  // A new widget opens as three files, index.html first.
  assert.deepEqual(await editor.getByTestId('hand-file').allTextContents(), ['index.html', 'app.js', 'style.css']);
  await typeInto(page, editor, 'index.html', '<p id="hi">hello from the box</p>');
  await page.getByTestId('hand-preview').frameLocator('iframe').locator('#hi').filter({ hasText: 'hello from the box' }).waitFor({ timeout: 10000 });
  await editor.getByRole('button', { name: 'Save and put on the board' }).click();
  await page.getByText('Room left saved.', { exact: true }).first().waitFor();
  assert.equal(writes.length, 1);
  const [method, path, body] = writes[0];
  assert.deepEqual([method, path, body.name, body.span], ['POST', '/api/look/widgets', 'Room left', 'full']);
  assert.deepEqual(body.files.map((f) => f.name), ['index.html', 'app.js', 'style.css']);
  assert.equal(body.files[0].content, '<p id="hi">hello from the box</p>');
  const tile = page.getByTestId('hand-tile');
  await tile.waitFor();
  await tile.frameLocator('iframe').locator('#hi').waitFor();
  assert.ok((await tile.getAttribute('class')).includes('md:col-span-2'), 'a full-width widget spans both columns');
  // Stored on the board as the box's id, so another tab finds the same widget.
  const stored = await page.evaluate(() => JSON.parse(window.localStorage.getItem('losos-widgets')));
  assert.deepEqual(stored.widgets.map((w) => w.source), [{ kind: 'hand', id: 'w2' }]);
  assert.deepEqual(errors, []);
  await page.close();
});

await check('the code editor completes the bridge, the readings, the colours and the file names, and draws under the admin CSP', async () => {
  const { page, errors } = await open();
  // The bundle's two first-paint style-src-elem reports (advanced.browser.mjs)
  // are counted out; the editor must add none.
  const atLoad = (await violations(page)).length;
  const editor = await openEditor(page);
  await code(editor, 'index.html').waitFor();
  const list = page.locator('.cm-tooltip-autocomplete');
  const options = async () => (await list.locator('li').allTextContents()).map((o) => o.trim());

  await fileButton(editor, 'app.js').click();
  await typeInto(page, editor, 'app.js', '', 'losos.');
  await list.waitFor();
  const members = await options();
  for (const member of ['metric', 'theme', 'onTheme', 'lang', 'palette', 'resize', 'files', 'file', 'asset']) {
    assert.ok(members.some((o) => o.startsWith(member)), `losos.${member} was not offered: ${members.join(' | ')}`);
  }
  await page.keyboard.type('met');
  // CodeMirror ignores Enter for 75 ms after the list changes, so a person
  // typing fast does not pick by accident; wait out the narrowing.
  await list.locator('li').filter({ hasText: 'theme' }).first().waitFor({ state: 'detached' });
  await page.waitForTimeout(150);
  await page.keyboard.press('Enter');
  // Picking metric opens the string, where the readings are offered.
  await list.waitFor();
  assert.ok((await options()).some((o) => o.startsWith('storage.bytes')), 'the readings were not offered');
  await page.keyboard.press('Escape');

  await fileButton(editor, 'style.css').click();
  await typeInto(page, editor, 'style.css', 'p { color: ', 'var(--ac');
  await list.waitFor();
  assert.ok((await options()).some((o) => o.startsWith('--accent')), 'the colour variables were not offered');
  await page.keyboard.press('Escape');

  await fileButton(editor, 'index.html').click();
  await typeInto(page, editor, 'index.html', '', '<script src="');
  await list.waitFor();
  const named = await options();
  assert.ok(named.some((o) => o.startsWith('app.js')) && named.some((o) => o.startsWith('style.css')), `file names: ${named.join(' | ')}`);
  await page.keyboard.press('Escape');

  assert.deepEqual((await violations(page)).slice(atLoad), [], 'the editor broke the admin CSP');
  // Half-typed script in the preview is a syntax error there, which is fine.
  assert.deepEqual(errors.filter((e) => !e.startsWith('SyntaxError')), []);
  await page.close();
});

await check('a widget in several files: a new file, linked styles, a script, an SVG picture and fetched JSON all reach the frame', async () => {
  const { page, errors, writes } = await open();
  const editor = await openEditor(page);
  await editor.getByLabel('Name', { exact: true }).fill('Split');
  // A name the box would refuse is refused here first.
  await editor.getByRole('button', { name: 'Add a file' }).click();
  await editor.getByLabel('New file name').fill('Bad Name.exe');
  await editor.getByRole('button', { name: 'Add', exact: true }).click();
  await editor.getByText('Use lowercase letters, digits, - and _', { exact: false }).waitFor();
  await editor.getByLabel('New file name').fill('data.json');
  await editor.getByRole('button', { name: 'Add', exact: true }).click();
  await typeInto(page, editor, 'data.json', '{ "word": "from json" }');
  await editor.getByRole('button', { name: 'Add a file' }).click();
  await editor.getByLabel('New file name').fill('dot.svg');
  await editor.getByRole('button', { name: 'Add', exact: true }).click();
  await typeInto(page, editor, 'dot.svg', '<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"><circle cx="5" cy="5" r="5"/></svg>');
  await fileButton(editor, 'index.html').click();
  await typeInto(page, editor, 'index.html', [
    '<link rel="stylesheet" href="style.css">',
    '<p class="figure" id="word">…</p>',
    '<img id="dot" src="dot.svg" alt="">',
    '<script src="app.js"></script>',
  ].join('\n'));
  await fileButton(editor, 'app.js').click();
  await typeInto(page, editor, 'app.js', 'fetch("data.json").then((r) => r.json()).then((d) => { document.getElementById("word").textContent = d.word + ":" + losos.files.length; });');
  const preview = page.getByTestId('hand-preview').frameLocator('iframe');
  await preview.locator('#word').filter({ hasText: 'from json:5' }).waitFor({ timeout: 10000 });
  // The stylesheet applied, and the picture loaded from its data: URL.
  const colour = await preview.locator('#word').evaluate((el) => getComputedStyle(el).color);
  assert.equal(colour, 'rgb(14, 110, 125)', `style.css did not apply: ${colour}`);
  await preview.locator('#dot').evaluate((img) => (img.complete ? null : new Promise((r) => img.addEventListener('load', r))));
  assert.equal(await preview.locator('#dot').evaluate((img) => img.naturalWidth), 10);
  // A file other than index.html can be removed.
  await fileButton(editor, 'dot.svg').click();
  await editor.getByRole('button', { name: 'Remove dot.svg' }).click();
  assert.deepEqual(await editor.getByTestId('hand-file').allTextContents(), ['index.html', 'app.js', 'data.json', 'style.css']);
  await editor.getByRole('button', { name: 'Save and put on the board' }).click();
  await page.getByText('Split saved.', { exact: true }).first().waitFor();
  assert.deepEqual(writes[0][2].files.map((f) => f.name), ['index.html', 'app.js', 'data.json', 'style.css']);
  assert.deepEqual(errors, []);
  await page.close();
});

await check('an empty name is refused in the dialog, with nothing sent', async () => {
  const { page, writes } = await open();
  await page.getByRole('button', { name: 'Add a widget' }).first().click();
  await page.getByRole('dialog', { name: 'Add a widget' }).getByRole('button', { name: 'Write one' }).click();
  const editor = page.getByRole('dialog', { name: 'Write a widget' });
  // The disclaimer sits where code gets pasted.
  await editor.getByTestId('hand-paste-note').filter({ hasText: 'Pasting something from the internet? Read it first.' }).waitFor();
  await editor.getByRole('button', { name: 'Save and put on the board' }).click();
  await editor.getByText('Give it a name.').waitFor();
  assert.deepEqual(writes, []);
  await page.close();
});

// ── Built by Claude ───────────────────────────────────────────────────────

const ACCOUNT = {
  available: true,
  currency: 'eur',
  balance: 380,
  packs: [500, 1000, 2000],
  price: { model: 'claude-opus-5-5', input_per_million: 480, output_per_million: 2400, markup_percent: 20, max_build: 360 },
  can_build: true,
  builds: [],
};

const BUILT = [
  { name: 'index.html', content: '<link rel="stylesheet" href="built.css"><p id="built">built by the agent</p>' },
  { name: 'built.css', content: '#built { color: rgb(1, 2, 3); }' },
];

function builderStub() {
  let reads = 0;
  const build = (status) => ({
    id: 'bld_0a1b', status, prompt: 'a big clock', revision: false, created_at: 1, finished_at: status === 'running' ? null : 2,
    charged: status === 'running' ? 0 : 120, fault: null, has_source: status === 'done',
    usage: { input_tokens: 0, output_tokens: 0, cache_creation_input_tokens: 0, cache_read_input_tokens: 0 },
    at_limit: false, source: status === 'done' ? BUILT[0].content : null, files: status === 'done' ? BUILT : null,
    notes: status === 'done' ? 'A big clock.' : null,
  });
  return (path, method, route, json) => {
    if (path === '/api/builder') return json(route, 200, ACCOUNT);
    if (path === '/api/builder/builds' && method === 'POST') return json(route, 200, { available: true, ...build('running') });
    if (path === '/api/builder/builds/bld_0a1b') return json(route, 200, { available: true, ...build(reads++ < 1 ? 'running' : 'done') });
    return json(route, 404, { error: 'not found' });
  };
}

await check('Build with Claude shows the price and balance, and a finished build lands in the editor and its preview', async () => {
  const { page, errors, writes } = await open({ builder: builderStub() });
  // The bundle's two first-paint style-src-elem reports (advanced.browser.mjs)
  // are counted out; the builder must add none.
  const atLoad = (await violations(page)).length;
  await page.getByRole('button', { name: 'Add a widget' }).first().click();
  await page.getByRole('dialog', { name: 'Add a widget' }).getByRole('button', { name: 'Write one' }).click();
  const editor = page.getByRole('dialog', { name: 'Write a widget' });
  await editor.getByRole('tab', { name: 'Build with Claude' }).click();
  const panel = page.getByTestId('builder-panel');
  await panel.waitFor();
  assert.match(await page.getByTestId('builder-price').textContent(), /Claude Opus 5\.5: €4\.80 per million tokens read and €24\.00 per million written\. That is Anthropic's price plus 20%\. One build costs at most €3\.60\./);
  assert.equal(await page.getByTestId('builder-balance').textContent(), 'Balance: €3.80');
  // The template is not a widget worth changing, so there is nothing to tick.
  assert.equal(await panel.getByRole('checkbox').count(), 0);
  await panel.getByLabel('What should the widget show?').fill('a big clock');
  await panel.getByRole('button', { name: 'Build' }).click();
  // Done: back on the Write tab with the agent's files in the editor and the frame.
  const built = page.getByTestId('hand-preview').frameLocator('iframe').locator('#built');
  await built.waitFor({ timeout: 15000 });
  assert.equal(await built.evaluate((el) => getComputedStyle(el).color), 'rgb(1, 2, 3)');
  assert.deepEqual(await editor.getByTestId('hand-file').allTextContents(), ['index.html', 'built.css']);
  assert.equal(await code(editor, 'index.html').textContent(), BUILT[0].content);
  assert.deepEqual(writes, [['POST', '/api/builder/builds', { prompt: 'a big clock', lang: 'en' }]]);
  assert.deepEqual((await violations(page)).slice(atLoad), []);
  assert.deepEqual(errors, []);
  await page.close();
});

await check('a box whose edge offers no builder says so in the tab, in each language', async () => {
  for (const [locale, add, write, title, tab, words] of [
    ['en-US', 'Add a widget', 'Write one', 'Write a widget', 'Build with Claude', 'does not offer the widget builder'],
    ['sk-SK', 'Pridať widget', 'Napísať', 'Napísať widget', 'Vytvoriť s Claude', 'tvorcu widgetov neponúka'],
    ['de-DE', 'Widget hinzufügen', 'Eins schreiben', 'Widget schreiben', 'Mit Claude bauen', 'bietet den Widget-Baukasten nicht an'],
  ]) {
    const { page, errors, writes } = await open({
      locale,
      builder: (_path, _method, route, json) => json(route, 200, { available: false }),
    });
    await page.getByRole('button', { name: add }).first().click();
    await page.getByRole('dialog', { name: add }).getByRole('button', { name: write }).click();
    await page.getByRole('dialog', { name: title }).getByRole('tab', { name: tab }).click();
    await page.getByTestId('builder-unavailable').filter({ hasText: words }).waitFor();
    assert.deepEqual(writes, []);
    assert.deepEqual(errors, []);
    await page.close();
  }
});

await check('the Look pane lists the hand-written widgets and can edit and delete them', async () => {
  const { page, writes, look } = await open({
    path: '/settings/look',
    look: { version: 1, background: { kind: 'none' }, veil: 60, widgets: [CLOCK], shipped: ['tide', 'grid', 'dusk'], limits: LIMITS },
    board: { version: 1, widgets: [{ id: 'tile1', source: { kind: 'hand', id: 'w1' } }] },
  });
  const main = page.locator('main');
  await main.getByText('Box name', { exact: true }).waitFor();
  await main.getByText('1 of 24').waitFor();
  await main.getByRole('button', { name: 'Edit Box name' }).click();
  const editor = page.getByRole('dialog', { name: 'Edit Box name' });
  await editor.waitFor();
  await editor.getByLabel('Name', { exact: true }).fill('Box name, renamed');
  await editor.getByRole('button', { name: 'Save', exact: true }).click();
  await page.getByText('Box name, renamed saved.', { exact: true }).first().waitFor();
  assert.equal(writes[0][2].id, 'w1', 'an edit carries the id');
  assert.equal(look().widgets[0].name, 'Box name, renamed');
  await main.getByRole('button', { name: 'Delete Box name, renamed' }).click();
  await page.getByText('Box name, renamed deleted.', { exact: true }).first().waitFor();
  assert.deepEqual(writes[1], ['DELETE', '/api/look/widgets/w1', null]);
  await main.getByText('None yet.', { exact: false }).waitFor();
  // The tile that showed it is gone from this browser's board too.
  const stored = await page.evaluate(() => JSON.parse(window.localStorage.getItem('losos-widgets')));
  assert.deepEqual(stored.widgets, []);
  await page.close();
});

await check('the frame is served under its own policy and the admin page under the strict one', async () => {
  const { page } = await open();
  const headers = await page.evaluate(async () => ({
    page: (await fetch('/')).headers.get('content-security-policy'),
    frame: (await fetch('/widget-frame/')).headers.get('content-security-policy'),
  }));
  assert.match(headers.page, /script-src 'self'/);
  assert.match(headers.frame, /script-src 'unsafe-inline'/);
  assert.match(headers.frame, /frame-ancestors 'self'/);
  await page.close();
});

await check('the pane reads in Slovak and German', async () => {
  for (const [locale, title] of [['sk-SK', 'Pozadie'], ['de-DE', 'Hintergrund']]) {
    const { page, errors } = await open({ path: '/settings/look', locale });
    await page.getByRole('radiogroup', { name: title }).waitFor();
    assert.deepEqual(errors, []);
    await page.close();
  }
});

await browser.close();
await closeServer();
finish();
