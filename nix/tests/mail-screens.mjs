// Screens of LosOS Mail, taken in nix/tests/mail.nix's `losos-mail-screens`
// run: Chromium on the laptop, the box's own nginx, signed in through LosOS
// Login as the owner. One call per state, so the test can change the box in
// between (sharing off, sharing on) and the browser keeps its session.
//
//   mail-screens sidebar NN slug "title" none|mail   the admin sidebar
//   mail-screens mail NN                             inbox, a message, a reply
//                                                    typed word by word, Sent
//   mail-screens folders NN                          the folder tree, a folder a
//                                                    filter filed into, a folder's
//                                                    menu, a new folder, the filters
//   mail-screens manifest <test-name>                steps.jsonl -> manifest.json
//
// Every shot is 1280 x 900 CSS px at device scale 2, dark, en-US, reduced
// motion, Europe/Bratislava, taken once the page has settled. Each step
// records where the film may point (bounding boxes in CSS px).

/* One line more at the end of a file the earlier calls of this script began. */
async function append(path, line) {
  const file = Bun.file(path);
  await Bun.write(file, ((await file.exists()) ? await file.text() : '') + line);
}

const { chromium } = await import(`${Bun.env.PLAYWRIGHT_CORE}/index.mjs`);

const HOST = 'mattbox.local';
const BASE = `https://${HOST}`;
const OUT = '/tmp/shots';
const PROFILE = '/tmp/shots-profile';
const NAME = 'matus';
const PASSWORD = Bun.env.MAIL_SCREENS_PASSWORD ?? '';
const VIEWPORT = { width: 1280, height: 900 };
// The caret is the film's to draw.
const HIDE_CARET = '*, *::before, *::after { caret-color: transparent !important; }';

async function browser() {
  const context = await chromium.launchPersistentContext(PROFILE, {
    executablePath: Bun.env.CHROMIUM,
    viewport: VIEWPORT,
    deviceScaleFactor: 2,
    colorScheme: 'dark',
    reducedMotion: 'reduce',
    locale: 'en-US',
    timezoneId: 'Europe/Bratislava',
    ignoreHTTPSErrors: true,
    // Only so the caret style can go in; the page's own policy is
    // otherwise what the box sends.
    bypassCSP: true,
    args: ['--disable-gpu', '--no-sandbox'],
  });
  await context.addInitScript((css) => {
    document.addEventListener('DOMContentLoaded', () => {
      const s = document.createElement('style');
      s.textContent = css;
      document.head.appendChild(s);
    });
  }, HIDE_CARET);
  return context;
}

/** Sign in the way the portal does; the cookie lands in the profile. */
async function signedIn(context) {
  let me = await context.request.get(`${BASE}/api/me`);
  if (me.status() !== 200) {
    const r = await context.request.post(`${BASE}/auth/api/firstfactor`, {
      data: { username: NAME, password: PASSWORD, keepMeLoggedIn: true },
    });
    if (!r.ok()) throw new Error(`sign-in: ${r.status()} ${await r.text()}`);
    me = await context.request.get(`${BASE}/api/me`);
    if (me.status() !== 200) throw new Error(`/api/me: ${me.status()}`);
  }
  return context.pages()[0] ?? (await context.newPage());
}

async function settle(page) {
  await page.waitForLoadState('networkidle');
  await page.evaluate(() => document.fonts.ready.then(() => true));
  // No spinner or skeleton left on screen. Polled from here: the page's
  // own policy would refuse waitForFunction's evaluation.
  for (let i = 0; i < 120; i++) {
    const busy = await page.locator('[data-slot=spinner], [data-slot=skeleton]').count();
    if (busy === 0) break;
    await page.waitForTimeout(500);
  }
  await page.waitForTimeout(600);
}

async function target(locator) {
  const b = await locator.boundingBox();
  if (b === null) throw new Error(`no box for ${locator}`);
  return { x: Math.round(b.x), y: Math.round(b.y), w: Math.round(b.width), h: Math.round(b.height) };
}

async function shot(page, file, title, action = { kind: 'none' }, targets = {}) {
  await settle(page);
  Bun.spawnSync(['mkdir', '-p', OUT]);
  // A modal's blurred backdrop at device scale 2 is slow to paint in a
  // software-rendered VM, so the default 30 s is not always enough.
  await page.screenshot({ path: `${OUT}/${file}`, timeout: 300_000 });
  const url = page.url().replace('https://', '').replace(':443', '');
  await append(`${OUT}/steps.jsonl`, JSON.stringify({ file, url, title, action, targets }) + '\n');
  console.log('shot', file);
}

async function noToasts(page) {
  for (let i = 0; i < 60; i++) {
    const n = await page.locator('[data-sonner-toast], [data-slot=toast]').count();
    if (n === 0) return;
    await page.waitForTimeout(500);
  }
}

async function sidebar(number, slug, title, expect) {
  const context = await browser();
  const page = await signedIn(context);
  await page.goto(`${BASE}/storage`);
  await page.getByRole('navigation').first().waitFor();
  await settle(page);
  const mail = page.getByRole('link', { name: 'Mail', exact: true });
  const present = (await mail.count()) > 0;
  if (present !== (expect === 'mail')) throw new Error(`Mail link present: ${present}`);
  const targets = present ? { mail: await target(mail.first()) } : {};
  const action = present ? { kind: 'click', target: 'mail' } : { kind: 'none' };
  await shot(page, `${number}-${slug}.png`, title, action, targets);
  await context.close();
}

async function mail(first) {
  let n = Number(first);
  const name = (slug) => `${String(n++).padStart(2, '0')}-${slug}.png`;
  const context = await browser();
  const page = await signedIn(context);
  await page.goto(`${BASE}/mail`);
  const row = page.getByRole('button', { name: /Photos from the lake/ });
  await row.waitFor({ timeout: 120_000 });
  await shot(page, name('inbox'), 'the inbox with mail from Eva and Peter', { kind: 'click', target: 'message' }, {
    message: await target(row),
  });

  await row.click();
  const reply = page.getByRole('button', { name: 'Reply', exact: true });
  await reply.waitFor();
  await page.getByText('the boat at sunset', { exact: false }).first().waitFor();
  await shot(page, name('message'), "Eva's message, opened", { kind: 'click', target: 'reply' }, {
    reply: await target(reply),
  });

  await reply.click();
  const dialog = page.getByRole('dialog');
  const body = dialog.locator('textarea');
  await body.waitFor();
  await body.focus();
  await page.keyboard.press('Control+Home');
  const send = dialog.getByRole('button', { name: 'Send', exact: true });
  await shot(page, name('reply'), 'a reply, ready to write', { kind: 'type', target: 'body' }, { body: await target(body) });
  const words = 'They look great, send the rest when you can.'.split(' ');
  for (const [i, word] of words.entries()) {
    const last = i === words.length - 1;
    await page.keyboard.type(last ? word : `${word} `);
    await shot(
      page,
      name(`typed-${i + 1}`),
      `typing: ${words.slice(0, i + 1).join(' ')}`,
      last ? { kind: 'click', target: 'send' } : { kind: 'type', target: 'body' },
      last ? { send: await target(send) } : { body: await target(body) },
    );
  }

  await send.click();
  await dialog.waitFor({ state: 'detached' });
  await noToasts(page);
  await page.getByRole('navigation', { name: 'Folders' }).getByRole('button', { name: /Sent/ }).click();
  await page.getByRole('button', { name: /Re: Photos from the lake/ }).waitFor({ timeout: 60_000 });
  await shot(page, name('sent'), 'the reply, in Sent');
  await context.close();
}

async function folders(first) {
  let n = Number(first);
  const name = (slug) => `${String(n++).padStart(2, '0')}-${slug}.png`;
  const context = await browser();
  const page = await signedIn(context);
  await page.goto(`${BASE}/mail`);
  // The tree is the wide screen's; the row of folders is hidden at this size.
  const tree = page.getByRole('navigation', { name: 'Folders' }).filter({ has: page.getByRole('heading') });
  const losos = tree.locator('[data-folder="LosOS box"]');
  await losos.waitFor({ timeout: 120_000 });
  await page.getByRole('button', { name: /Photos from the lake/ }).waitFor();
  await shot(page, name('folder-tree'), 'the folders as a tree: LosOS box inside Projects', { kind: 'click', target: 'folder' }, {
    folder: await target(losos),
  });

  await losos.click();
  const filed = page.getByRole('button', { name: /Backup log: Sunday/ });
  await filed.waitFor({ timeout: 60_000 });
  await shot(page, name('filed'), "Peter's backup log, filed here by a filter, read and flagged", { kind: 'none' }, {
    message: await target(filed),
  });

  await losos.click({ button: 'right' });
  const menu = page.locator('[data-slot="context-menu"]').first();
  await menu.waitFor();
  await shot(page, name('folder-menu'), 'a right click on a folder of your own', { kind: 'none' }, { menu: await target(menu) });
  await page.keyboard.press('Escape');
  await menu.waitFor({ state: 'detached' });

  await tree.getByRole('button', { name: 'New folder' }).click();
  const dialog = page.getByRole('dialog');
  const field = dialog.getByLabel('Name');
  await field.waitFor();
  await field.pressSequentially('Receipts');
  const create = dialog.getByRole('button', { name: 'Create' });
  await shot(page, name('new-folder'), 'a new folder, named', { kind: 'click', target: 'create' }, { create: await target(create) });
  await create.click();
  await dialog.waitFor({ state: 'detached' });
  await noToasts(page);
  await page.getByRole('heading', { level: 2, name: 'Receipts' }).waitFor();
  await shot(page, name('new-folder-open'), 'the new folder, open and empty');

  await page.getByRole('button', { name: 'Filters', exact: true }).click();
  const filters = page.getByRole('dialog');
  const backups = filters.locator('[data-filter="Backup reports"]');
  await backups.waitFor();
  await shot(page, name('filters'), 'the filters, in the order they run', { kind: 'click', target: 'edit' }, {
    edit: await target(backups.getByRole('button', { name: 'Edit' })),
  });
  await backups.getByRole('button', { name: 'Edit' }).click();
  const editor = page.getByRole('dialog');
  await editor.getByLabel('Name').waitFor();
  await shot(page, name('filter'), "a filter: Peter's backup mail goes to LosOS box, read and flagged");
  await editor.getByRole('button', { name: 'Cancel' }).click();
  await context.close();
}

async function manifest(test) {
  const steps = (await Bun.file(`${OUT}/steps.jsonl`).text())
    .split('\n')
    .filter((l) => l !== '')
    .map((l) => JSON.parse(l))
    .sort((a, b) => a.file.localeCompare(b.file));
  const doc = {
    feature: 'mail',
    viewport: VIEWPORT,
    dpr: 2,
    commit: Bun.env.MAIL_SCREENS_COMMIT ?? '',
    test,
    steps,
  };
  await Bun.write(`${OUT}/manifest.json`, JSON.stringify(doc, null, 2) + '\n');
  await Bun.$`rm -f ${OUT}/steps.jsonl`;
}

const [verb, ...rest] = Bun.argv.slice(2);
if (verb === 'sidebar') await sidebar(...rest);
else if (verb === 'mail') await mail(...rest);
else if (verb === 'folders') await folders(...rest);
else if (verb === 'manifest') await manifest(...rest);
else {
  // Thrown: an uncaught error is what makes Bun exit non-zero.
  throw new Error(`unknown verb ${verb}`);
}
