// Screens of LosOS Mail, taken in nix/tests/mail.nix's `losos-mail-screens`
// run: Chromium on the laptop, the box's own nginx, signed in through LosOS
// Login as the owner. One call per state, so the test can change the box in
// between (sharing off, sharing on) and the browser keeps its session.
//
//   mail-screens sidebar NN slug "title" none|mail   the admin sidebar
//   mail-screens mail NN                             inbox, a message, a reply
//                                                    typed word by word, Sent
//   mail-screens manifest <test-name>                steps.jsonl -> manifest.json
//
// Every shot is 1280 x 900 CSS px at device scale 2, dark, en-US, reduced
// motion, Europe/Bratislava, taken once the page has settled. Each step
// records where the film may point (bounding boxes in CSS px).

import { appendFileSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';

const { chromium } = await import(`${process.env.PLAYWRIGHT_CORE}/index.mjs`);

const HOST = 'mattbox.local';
const BASE = `https://${HOST}`;
const OUT = '/tmp/shots';
const PROFILE = '/tmp/shots-profile';
const NAME = 'matus';
const PASSWORD = process.env.MAIL_SCREENS_PASSWORD ?? '';
const VIEWPORT = { width: 1280, height: 900 };
// The caret is the film's to draw.
const HIDE_CARET = '*, *::before, *::after { caret-color: transparent !important; }';

async function browser() {
  const context = await chromium.launchPersistentContext(PROFILE, {
    executablePath: process.env.CHROMIUM,
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
  mkdirSync(OUT, { recursive: true });
  // A modal's blurred backdrop at device scale 2 is slow to paint in a
  // software-rendered VM, so the default 30 s is not always enough.
  await page.screenshot({ path: `${OUT}/${file}`, timeout: 300_000 });
  const url = page.url().replace('https://', '').replace(':443', '');
  appendFileSync(`${OUT}/steps.jsonl`, JSON.stringify({ file, url, title, action, targets }) + '\n');
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

function manifest(test) {
  const steps = readFileSync(`${OUT}/steps.jsonl`, 'utf8')
    .split('\n')
    .filter((l) => l !== '')
    .map((l) => JSON.parse(l))
    .sort((a, b) => a.file.localeCompare(b.file));
  const doc = {
    feature: 'mail',
    viewport: VIEWPORT,
    dpr: 2,
    commit: process.env.MAIL_SCREENS_COMMIT ?? '',
    test,
    steps,
  };
  writeFileSync(`${OUT}/manifest.json`, JSON.stringify(doc, null, 2) + '\n');
  rmSync(`${OUT}/steps.jsonl`);
}

const [verb, ...rest] = process.argv.slice(2);
if (verb === 'sidebar') await sidebar(...rest);
else if (verb === 'mail') await mail(...rest);
else if (verb === 'manifest') manifest(...rest);
else {
  console.error(`unknown verb ${verb}`);
  process.exit(2);
}
