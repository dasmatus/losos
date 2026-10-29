// OpenPGP in LosOS Mail, driven in Chromium on the laptop of
// nix/tests/mail-pgp.nix against the box's own nginx, signed in through
// LosOS Login. Each call is one person in their own browser profile, so the
// key they unlock lives only as long as that call, as it does in a tab.
//
//   mail-pgp key <name> <password> <passphrase> <NN|->      make a key
//   mail-pgp send <password> <passphrase> <NN|->            Matus -> Eva, encrypted
//   mail-pgp outside <password> <NN|->                      keys from WKD and a keyserver
//   mail-pgp read <password> <passphrase> <NN|->            Eva opens it
//   mail-pgp sent <password> <passphrase>                   Matus reads his own copy
//   mail-pgp before <eva-password> <matus-password>         the same screens, older pages
//   mail-pgp manifest <test-name>
//
// Shots (NN, or - for none) follow nix/tests/mail-screens.mjs: 1280 x 900 CSS
// px at device scale 2, dark, en-US, reduced motion, Europe/Bratislava, once
// the page has settled, with where the film may point.

/* One line more at the end of a file the earlier calls of this script began. */
async function append(path, line) {
  const file = Bun.file(path);
  await Bun.write(file, ((await file.exists()) ? await file.text() : '') + line);
}

const { chromium } = await import(`${Bun.env.PLAYWRIGHT_CORE}/index.mjs`);

const BASE = 'https://mattbox.local';
const OUT = '/tmp/shots';
const VIEWPORT = { width: 1280, height: 900 };
const HIDE_CARET = '*, *::before, *::after { caret-color: transparent !important; }';
const DOMAIN = 'mattbox.home.arpa';
const SECRET = 'The spare key is under the blue pot by the door.';

function fail(message) {
  throw new Error(message);
}

async function browser(name) {
  const context = await chromium.launchPersistentContext(`/tmp/profile-${name}`, {
    viewport: VIEWPORT,
    deviceScaleFactor: 2,
    colorScheme: 'dark',
    reducedMotion: 'reduce',
    locale: 'en-US',
    timezoneId: 'Europe/Bratislava',
    ignoreHTTPSErrors: true,
    // Only so the caret style can go in.
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

async function signedIn(context, name, password) {
  let me = await context.request.get(`${BASE}/api/me`);
  if (me.status() !== 200) {
    const r = await context.request.post(`${BASE}/auth/api/firstfactor`, {
      data: { username: name, password, keepMeLoggedIn: true },
    });
    if (!r.ok()) fail(`sign-in ${name}: ${r.status()} ${await r.text()}`);
    me = await context.request.get(`${BASE}/api/me`);
    if (me.status() !== 200) fail(`/api/me: ${me.status()}`);
  }
  const page = context.pages()[0] ?? (await context.newPage());
  page.on('pageerror', (e) => fail(`page error: ${e}`));
  return page;
}

async function settle(page) {
  await page.waitForLoadState('networkidle');
  await page.evaluate(() => document.fonts.ready.then(() => true));
  for (let i = 0; i < 120; i++) {
    const busy = await page.locator('[data-slot=spinner], [data-slot=skeleton]').count();
    if (busy === 0) break;
    await page.waitForTimeout(500);
  }
  await page.waitForTimeout(600);
}

async function noToasts(page) {
  for (let i = 0; i < 60; i++) {
    const n = await page.locator('[data-sonner-toast], [data-slot=toast]').count();
    if (n === 0) return;
    await page.waitForTimeout(500);
  }
}

async function target(locator) {
  const b = await locator.boundingBox();
  if (b === null) fail(`no box for ${locator}`);
  return { x: Math.round(b.x), y: Math.round(b.y), w: Math.round(b.width), h: Math.round(b.height) };
}

/* Numbered shots, or none when the first number is "-". */
function shooter(first) {
  let n = first === '-' ? null : Number(first);
  return async (page, slug, title, action = { kind: 'none' }, targets = {}) => {
    if (n === null) return;
    await settle(page);
    Bun.spawnSync(['mkdir', '-p', OUT]);
    const file = `${String(n++).padStart(2, '0')}-${slug}.png`;
    await page.screenshot({ path: `${OUT}/${file}`, timeout: 300_000 });
    const url = page.url().replace('https://', '').replace(':443', '');
    await append(`${OUT}/steps.jsonl`, JSON.stringify({ file, url, title, action, targets }) + '\n');
    console.log('shot', file);
  };
}

/* Wait for a condition polled from here: the page's policy refuses
 * Playwright's own evaluation in waitForFunction. */
async function until(what, test, timeout = 120_000) {
  const deadline = Date.now() + timeout;
  while (Date.now() < deadline) {
    if (await test()) return;
    await new Promise((r) => setTimeout(r, 500));
  }
  fail(`timed out: ${what}`);
}

async function key(name, password, passphrase, first) {
  const shot = shooter(first);
  const context = await browser(name);
  const page = await signedIn(context, name, password);
  await page.goto(`${BASE}/mail`);
  const button = page.getByTestId('mail-encryption');
  await button.waitFor({ timeout: 120_000 });
  await page.getByRole('navigation', { name: 'Folders' }).waitFor();
  await shot(page, 'inbox', 'the inbox, with Encryption beside Write', { kind: 'click', target: 'encryption' }, {
    encryption: await target(button),
  });
  await button.click();
  const dialog = page.getByRole('dialog');
  await dialog.getByText(`A new key for ${name}@${DOMAIN}`, { exact: false }).waitFor();
  const pass = dialog.getByLabel('Passphrase');
  const again = dialog.getByLabel('The same again');
  await shot(page, 'make-key', 'Encryption: a new key for this address', { kind: 'type', target: 'passphrase' }, {
    passphrase: await target(pass),
  });
  await pass.fill(passphrase);
  await again.fill(passphrase);
  const make = dialog.getByRole('button', { name: 'Make the key' });
  await shot(page, 'passphrase', 'the passphrase, twice', { kind: 'click', target: 'make' }, { make: await target(make) });
  await make.click();
  const fingerprint = dialog.getByTestId('mail-key-fingerprint');
  await fingerprint.waitFor({ timeout: 180_000 });
  await noToasts(page);
  await dialog.getByText('sealed again', { exact: false }).waitFor();
  await shot(page, 'key', 'the key: its fingerprint, sealed on the box, published for others', { kind: 'none' }, {
    fingerprint: await target(fingerprint),
  });
  console.log('fingerprint', (await fingerprint.innerText()).replace(/\s/g, ''));
  await context.close();
}

async function compose(page, to) {
  await page.getByRole('button', { name: 'Write' }).click();
  const dialog = page.getByRole('dialog');
  const field = dialog.getByRole('combobox').first();
  await field.fill(to);
  return dialog;
}

async function unlockWith(page, passphrase, shot, slug, title) {
  const dialog = page.getByRole('dialog', { name: 'Unlock your key' });
  await dialog.waitFor();
  const field = dialog.getByLabel('Passphrase');
  await shot(page, slug, title, { kind: 'type', target: 'passphrase' }, { passphrase: await target(field) });
  await field.fill(passphrase);
  await dialog.getByRole('button', { name: 'Unlock' }).click();
  await dialog.waitFor({ state: 'detached', timeout: 60_000 });
}

async function send(password, passphrase, first) {
  const shot = shooter(first);
  const context = await browser('matus');
  const page = await signedIn(context, 'matus', password);
  await page.goto(`${BASE}/mail`);
  await page.getByTestId('mail-encryption').waitFor({ timeout: 120_000 });
  const dialog = await compose(page, `eva@${DOMAIN}`);
  const chip = dialog.locator('[data-testid=mail-recipient-key][data-state=found]');
  await chip.waitFor({ timeout: 60_000 });
  if (!/key on this box/.test(await chip.innerText())) fail(`chip: ${await chip.innerText()}`);
  const encrypt = dialog.locator('#mail-encrypt');
  await until('encrypt on by itself', () => encrypt.isChecked());
  if (!(await dialog.locator('#mail-sign').isChecked())) fail('sign is off');
  await dialog.getByLabel('Subject').fill('The spare key');
  const body = dialog.locator('textarea');
  await body.focus();
  const words = SECRET.split(' ');
  for (const [i, word] of words.entries()) {
    await page.keyboard.type(i === words.length - 1 ? word : `${word} `);
  }
  const sendButton = dialog.getByRole('button', { name: 'Send', exact: true });
  await shot(page, 'compose', 'to Eva: her key is on this box, so it encrypts and signs', { kind: 'click', target: 'send' }, {
    recipient: await target(chip),
    encrypt: await target(encrypt),
    send: await target(sendButton),
  });
  await sendButton.click();
  await unlockWith(page, passphrase, shot, 'unlock-to-sign', 'signing asks for the passphrase once per tab');
  await dialog.waitFor({ state: 'detached', timeout: 120_000 });
  await noToasts(page);
  await context.close();
}

async function outside(password, first) {
  const shot = shooter(first);
  const context = await browser('matus');
  const page = await signedIn(context, 'matus', password);
  await page.goto(`${BASE}/mail`);
  await page.getByTestId('mail-encryption').waitFor({ timeout: 120_000 });
  const dialog = await compose(page, 'friend@example.org, stranger@elsewhere.example');
  const chips = dialog.locator('[data-testid=mail-recipient-key][data-state=found]');
  await until('two keys found', async () => (await chips.count()) === 2, 90_000);
  const texts = await chips.allInnerTexts();
  if (!texts.some((t) => /friend@example\.org.*key from their mail domain/s.test(t))) fail(`chips: ${texts}`);
  if (!texts.some((t) => /stranger@elsewhere\.example.*key from keys\.openpgp\.org/s.test(t))) fail(`chips: ${texts}`);
  await shot(page, 'outside-keys', 'keys for people elsewhere: from their mail domain and from keys.openpgp.org', { kind: 'none' }, {
    wkd: await target(chips.first()),
    keyserver: await target(chips.nth(1)),
  });
  await dialog.getByRole('button', { name: 'Discard' }).click();
  await context.close();
}

async function read(password, passphrase, first) {
  const shot = shooter(first);
  const context = await browser('eva');
  const page = await signedIn(context, 'eva', password);
  await page.goto(`${BASE}/mail`);
  const row = page.getByRole('button', { name: /The spare key/ });
  await row.waitFor({ timeout: 120_000 });
  await row.getByLabel('Encrypted message').waitFor();
  await shot(page, 'eva-inbox', "Eva's inbox: the message is encrypted", { kind: 'click', target: 'message' }, {
    message: await target(row),
  });
  await row.click();
  await page.getByText('Unlock your key to read it.').waitFor({ timeout: 60_000 });
  if ((await page.getByText(SECRET).count()) !== 0) fail('the text shows before the key is unlocked');
  const unlock = page.getByRole('button', { name: 'Unlock', exact: true });
  await shot(page, 'locked', 'locked until Eva unlocks her key', { kind: 'click', target: 'unlock' }, {
    unlock: await target(unlock),
  });
  await unlock.click();
  await unlockWith(page, passphrase, shot, 'unlock-to-read', 'her passphrase, in her browser');
  const opened = page.getByTestId('mail-opened');
  await opened.getByText(SECRET).waitFor({ timeout: 120_000 });
  const note = page.getByTestId('mail-security');
  if ((await note.getAttribute('data-state')) !== 'ok') fail(`note: ${await note.getAttribute('data-state')}`);
  const said = await note.innerText();
  if (!said.includes(`Encrypted, and signed by matus@${DOMAIN}.`)) fail(`note: ${said}`);
  await shot(page, 'opened', 'opened: encrypted, and signed by Matus', { kind: 'none' }, {
    note: await target(note),
    text: await target(opened),
  });
  await context.close();
}

async function sent(password, passphrase) {
  const context = await browser('matus');
  const page = await signedIn(context, 'matus', password);
  await page.goto(`${BASE}/mail`);
  await page.getByRole('navigation', { name: 'Folders' }).getByRole('button', { name: /Sent/ }).click();
  const row = page.getByRole('button', { name: /The spare key/ });
  await row.waitFor({ timeout: 120_000 });
  await row.click();
  await page.getByRole('button', { name: 'Unlock', exact: true }).click();
  await unlockWith(page, passphrase, shooter('-'));
  await page.getByTestId('mail-opened').getByText(SECRET).waitFor({ timeout: 120_000 });
  await context.close();
}

/* The pull request's "before" pictures, on the admin pages from before
 * OpenPGP: Eva's inbox and the message, and Matus writing to her. Same
 * window and state as 08-eva-inbox, 11-opened and 06-compose. */
async function before(evaPassword, matusPassword) {
  const dir = `${OUT}-before`;
  Bun.spawnSync(['mkdir', '-p', dir]);
  const shot = async (page, slug) => {
    await settle(page);
    await page.screenshot({ path: `${dir}/${slug}.png`, timeout: 300_000 });
    console.log('before', slug);
  };
  const eva = await browser('eva-before');
  let page = await signedIn(eva, 'eva', evaPassword);
  await page.goto(`${BASE}/mail`);
  const row = page.getByRole('button', { name: /The spare key/ });
  await row.waitFor({ timeout: 120_000 });
  await shot(page, 'eva-inbox');
  await row.click();
  await page.getByRole('button', { name: 'Reply', exact: true }).waitFor({ timeout: 60_000 });
  await shot(page, 'opened');
  await eva.close();

  const matus = await browser('matus-before');
  page = await signedIn(matus, 'matus', matusPassword);
  await page.goto(`${BASE}/mail`);
  await page.getByRole('navigation', { name: 'Folders' }).first().waitFor({ timeout: 120_000 });
  const dialog = await compose(page, `eva@${DOMAIN}`);
  await dialog.getByLabel('Subject').fill('The spare key');
  await dialog.locator('textarea').fill(SECRET);
  await shot(page, 'compose');
  await matus.close();
}

async function manifest(test) {
  const steps = (await Bun.file(`${OUT}/steps.jsonl`).text())
    .split('\n')
    .filter((l) => l !== '')
    .map((l) => JSON.parse(l))
    .sort((a, b) => a.file.localeCompare(b.file));
  await Bun.write(
    `${OUT}/manifest.json`,
    JSON.stringify({ feature: 'mail-pgp', viewport: VIEWPORT, dpr: 2, commit: Bun.env.MAIL_SCREENS_COMMIT ?? '', test, steps }, null, 2) + '\n',
  );
  await Bun.$`rm -f ${OUT}/steps.jsonl`;
}

const [verb, ...rest] = Bun.argv.slice(2);
const verbs = { key, send, outside, read, sent, before, manifest };
if (!(verb in verbs)) {
  // Thrown: an uncaught error is what makes Bun exit non-zero.
  throw new Error(`unknown verb ${verb}`);
}
await verbs[verb](...rest);
