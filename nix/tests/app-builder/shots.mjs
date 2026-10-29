// Screens of the app builder, taken by Chromium on a laptop VM driving a real
// box (nix/tests/app-builder.nix). Also usable on its own against any box,
// from a folder where `playwright` resolves:
//
//   BOX=https://mattbox.local OWNER=matus PASSWORD=... OUT=./shots \
//     MODE=after node shots.mjs
//
// MODE=pushed, after a clone pushed a change to the app's repository, takes
// the app's page with the change running.
//
// MODE=after walks Your apps: the empty list, the builder, the request typed
// a word at a time, the box at work (real progress, one frame every few
// seconds, each step the builder takes on screen), the finished app in the
// preview, using it, saving it, the app on its own page and in the list, and
// the builder's licence notice in the widget editor. MODE=before takes the
// same addresses on a box without the feature, for the pull request's Before
// column.
//
// Every shot is 1280 x 900 CSS pixels at twice the density, dark, en-US,
// reduced motion, Europe/Bratislava, with the caret hidden. manifest.json
// beside the PNGs lists each step: the file, the address, what the shot
// shows, what happens next and the boxes (CSS pixels) of what a pointer may
// go to.

import { chromium } from 'playwright';

const join = (...parts) => parts.join('/');

const env = Bun.env;
const BOX = env.BOX ?? 'https://mattbox.local';
const OWNER = env.OWNER ?? 'matus';
const PASSWORD = env.PASSWORD;
const OUT = env.OUT ?? 'shots';
const MODE = env.MODE ?? 'after';
const REQUEST = env.REQUEST ?? 'A chore chart for the kids';
// How long the model may take, in seconds, before the run gives up.
const BUILD_TIMEOUT = Number(env.BUILD_TIMEOUT ?? 1800);
// At most this many frames of the box at work.
const WORK_FRAMES = Number(env.WORK_FRAMES ?? 12);
// The number of the first shot, for a run that picks up where another
// stopped (MODE=pushed).
const FIRST = Number(env.FIRST ?? 1);

Bun.spawnSync(["mkdir", "-p", OUT]);
const steps = [];
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

async function box(locator) {
  const b = await locator.boundingBox().catch(() => null);
  if (!b) return undefined;
  return { x: Math.round(b.x), y: Math.round(b.y), w: Math.round(b.width), h: Math.round(b.height) };
}

async function settle(page) {
  await page.waitForLoadState('networkidle');
  await page.evaluate(() => document.fonts.ready);
  await sleep(400);
}

async function shot(page, slug, title, action = 'none', targets = {}, wait = true) {
  if (wait) await settle(page);
  const name = `${String(steps.length + FIRST).padStart(2, '0')}-${slug}.png`;
  // An emulated laptop at twice the density can take minutes on a busy host.
  await page.screenshot({ path: join(OUT, name), timeout: 300_000 });
  const kept = {};
  for (const [k, v] of Object.entries(targets)) if (v) kept[k] = v;
  steps.push({
    file: name,
    url: page.url().split('://')[1],
    title,
    action: typeof action === 'string' ? { kind: action } : action,
    targets: kept,
  });
  console.log(`shot ${name}: ${title}`);
}

async function signIn(page, then) {
  await page.goto(`${BOX}/auth/?rd=${BOX}${then}`);
  await page.locator('#username-textfield').fill(OWNER);
  await page.locator('#password-textfield').fill(PASSWORD);
  await page.locator('#sign-in-button').click();
  await page.waitForURL(`${BOX}${then}`, { timeout: 60_000 });
}

/** The widget editor's Build with AI tab, where the model's notice is. */
async function widgetTab(page) {
  await page.goto(`${BOX}/`);
  await settle(page);
  await page.getByRole('button', { name: 'Add a widget' }).first().click();
  await page.getByRole('dialog', { name: 'Add a widget' }).getByRole('button', { name: 'Write one' }).click();
  const editor = page.getByRole('dialog', { name: 'Write a widget' });
  await editor.waitFor();
  await editor.getByRole('tab', { name: 'Build with AI' }).click();
  await editor.getByTestId('builder-panel').waitFor({ timeout: 60_000 });
  return editor;
}

const browser = await chromium.launch({ args: ['--ignore-certificate-errors'] });
const context = await browser.newContext({
  viewport: { width: 1280, height: 900 },
  deviceScaleFactor: 2,
  colorScheme: 'dark',
  locale: 'en-US',
  timezoneId: 'Europe/Bratislava',
  reducedMotion: 'reduce',
  ignoreHTTPSErrors: true,
});
await context.addInitScript(() => {
  document.addEventListener('DOMContentLoaded', () => {
    const s = document.createElement('style');
    s.textContent = '*{caret-color:transparent !important}';
    try {
      document.head.appendChild(s);
    } catch {}
  });
});
const page = await context.newPage();
const errors = [];
page.on('pageerror', (e) => errors.push(String(e)));

await signIn(page, '/apps');

if (MODE === 'pushed') {
  await page.goto(`${BOX}/apps/u1`);
  const app = page.frameLocator('[data-testid=user-app] iframe');
  await app.getByText(env.PUSHED_TITLE ?? 'Chores at home').first().waitFor({ timeout: 120_000 });
  await shot(page, 'app-pushed', 'the app running the change pushed from a clone');
} else if (MODE === 'before') {
  await shot(page, 'apps-pane', 'the Apps page');
  await widgetTab(page);
  await shot(page, 'widget-builder', 'Build with AI in the widget editor');
  await page.goto(`${BOX}/apps/new`);
  await shot(page, 'apps-new', 'the address of the app builder');
  await page.goto(`${BOX}/apps/u1`);
  await shot(page, 'apps-page', 'the address of an app');
} else {
  let section = page.getByTestId('your-apps');
  await section.waitFor();
  const make = page.getByTestId('your-apps-make');
  await section.scrollIntoViewIfNeeded();
  await shot(page, 'your-apps-empty', 'Your apps, with none yet', 'click', {
    make: await box(make),
    section: await box(section),
  });

  await make.click();
  await page.waitForURL(`${BOX}/apps/new`);
  const field = page.getByLabel('What should the app do?');
  await field.waitFor({ timeout: 60_000 });
  await shot(page, 'builder-empty', 'the app builder, empty', 'click', {
    field: await box(field),
    preview: await box(page.getByTestId('app-preview')),
  });

  await field.click();
  let typed = '';
  for (const word of REQUEST.split(' ')) {
    typed = typed ? `${typed} ${word}` : word;
    await field.fill(typed);
    await shot(page, `type-${String(typed.length).padStart(2, '0')}`, `typing the request: ${typed}`, 'type', {
      field: await box(field),
    });
  }

  const build = page.getByRole('button', { name: 'Build', exact: true });
  await shot(page, 'request-typed', 'the request, ready to build', 'click', {
    build: await box(build),
    field: await box(field),
  });
  await build.click();

  // The box at work: a frame every few seconds until it is done.
  const start = Date.now();
  let frames = 0;
  for (;;) {
    if ((await page.getByTestId('builder-outcome').count()) > 0) break;
    if (Date.now() - start > BUILD_TIMEOUT * 1000) throw new Error('the build did not finish');
    if ((await page.getByTestId('builder-running').count()) > 0 && frames < WORK_FRAMES) {
      frames += 1;
      await shot(
        page,
        `writing-${String(frames).padStart(2, '0')}`,
        'the AI on the box building the app',
        'none',
        {
          status: await box(page.getByTestId('builder-running')),
          preview: await box(page.getByTestId('app-preview')),
        },
        false,
      );
    }
    await sleep(4000);
  }

  const frame = page.frameLocator('[data-testid=app-preview] iframe');
  await frame.locator('h1').waitFor({ timeout: 60_000 });
  const name = page.getByLabel('Name', { exact: true });
  await shot(page, 'app-written', 'the finished app in the preview', 'none', {
    preview: await box(page.getByTestId('app-preview')),
    name: await box(name),
  });

  // Use it: one chore for one child.
  await frame.getByLabel('Task').fill('Feed the cat');
  await frame.getByLabel('Child').selectOption({ index: 1 });
  await shot(page, 'app-try', 'trying the app in the preview', 'click', {
    add: await box(frame.getByRole('button', { name: 'Add' })),
  });
  await frame.getByRole('button', { name: 'Add' }).click();
  await frame.getByText('Feed the cat').first().waitFor();
  await shot(page, 'app-tried', 'a chore added in the preview', 'click', {
    save: await box(page.getByTestId('app-save')),
  });

  await page.getByTestId('app-save').click();
  await page.waitForURL(`${BOX}/apps/u1`, { timeout: 60_000 });
  const app = page.frameLocator('[data-testid=user-app] iframe');
  await app.locator('h1').waitFor({ timeout: 60_000 });
  await app.getByLabel('Task').fill('Water the plants');
  await app.getByLabel('Child').selectOption({ index: 2 });
  await app.getByRole('button', { name: 'Add' }).click();
  await app.getByText('Water the plants').first().waitFor();
  // The box pushes the saved app to its repository on LosOS Git within
  // seconds; the page links it once the app's record says so.
  const source = page.getByTestId('user-app-repo');
  for (let i = 0; i < 60 && !(await source.isVisible()); i++) {
    await sleep(3000);
    await page.reload();
    await app.locator('h1').waitFor({ timeout: 60_000 });
  }
  await app.getByText('Water the plants').first().waitFor();
  await shot(page, 'app-saved', 'the app on its own page, its data on the box', 'click', {
    source: await box(source),
  });
  await source.click();
  await page.waitForURL(`${BOX}/git/notshared/app-chores`, { timeout: 60_000 });
  await page.getByText('app.jsx').first().waitFor({ timeout: 60_000 });
  await shot(page, 'app-repo', "the app's private repository on LosOS Git");
  await page.goto(`${BOX}/apps/u1`);
  await app.getByText('Water the plants').first().waitFor({ timeout: 60_000 });
  // The same page in the light theme, then a pane the admin UI already had,
  // in both themes, to set the app's buttons, fields and cards beside it.
  await page.emulateMedia({ colorScheme: 'light', reducedMotion: 'reduce' });
  await shot(page, 'app-saved-light', 'the app on its own page, light theme');
  await page.goto(`${BOX}/storage`);
  await shot(page, 'storage-light', 'the Storage page beside it, light theme');
  await page.emulateMedia({ colorScheme: 'dark', reducedMotion: 'reduce' });
  await shot(page, 'storage', 'the Storage page beside it');

  await page.goto(`${BOX}/apps`);
  const row = page.getByTestId('your-app').first();
  await row.waitFor();
  section = page.getByTestId('your-apps');
  await section.scrollIntoViewIfNeeded();
  await shot(page, 'your-apps', 'the app under Your apps', 'none', {
    app: await box(row),
    section: await box(section),
  });

  await widgetTab(page);
  await shot(page, 'widget-builder', "Build with AI in the widget editor, with the model's notice");
}

await Bun.write(
  join(OUT, 'manifest.json'),
  JSON.stringify(
    {
      feature: 'builder',
      viewport: { width: 1280, height: 900 },
      dpr: 2,
      commit: env.COMMIT ?? '',
      test: 'losos-app-builder',
      mode: MODE,
      steps,
    },
    null,
    2,
  ),
);
await browser.close();
if (errors.length) throw new Error(`the page threw: ${errors.join(' | ')}`);
