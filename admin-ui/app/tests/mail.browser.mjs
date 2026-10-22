/* LosOS Mail (/mail, a screen of the admin SPA inside its top bar and
 * sidebar) under the admin page's policy, with lososd's /api/mail* stood in by page.route the way
 * backend/src/mail.rs answers. Every page fails on a CSP violation or an
 * uncaught error. Walked once each: the inbox, opening a message (which
 * marks it read), a reply with the quote and threading id, search, a move
 * to the bin, an HTML message in its sandboxed frame (whose script never
 * runs), a box with no mail (404), the spare key (409), and the phone
 * layout. Then OpenPGP: a key made in the page, an encrypted and signed
 * message to Eva that Eva's key (made here in Node) opens, and one from Eva
 * that the page opens, first unlocked and then after asking for the
 * passphrase. nix/tests/mail.nix and nix/tests/mail-pgp.nix run the same
 * page against a real Stalwart. */

import assert from 'node:assert';
import * as openpgp from 'openpgp';
import { launch, MAIL_FRAME_CSP, runner, serve } from './harness.mjs';

const { origin, close: closeServer } = await serve({ csp: true });
const browser = await launch();
const { check, finish } = runner();

const json = (route, status, body) =>
  route.fulfill({ status, contentType: 'application/json', body: JSON.stringify(body) });

const ME = {
  name: 'matus',
  displayName: 'Matus',
  groups: ['losos-admins'],
  admin: true,
  spareKey: false,
  sections: ['storage'],
  shared: ['mail'],
};
const FOLDERS = [
  { id: 'a', name: 'Inbox', role: 'inbox', total: 2, unread: 1 },
  { id: 'b', name: 'Drafts', role: 'drafts', total: 0, unread: 0 },
  { id: 'c', name: 'Sent Items', role: 'sent', total: 0, unread: 0 },
  { id: 'd', name: 'Archive', role: 'archive', total: 0, unread: 0 },
  { id: 'e', name: 'Deleted Items', role: 'trash', total: 0, unread: 0 },
];
const HOME = {
  address: 'matus@mattbox.home.arpa',
  name: 'Matus',
  mailboxes: FOLDERS,
  people: [{ name: 'eva', displayName: 'Eva', address: 'eva@mattbox.home.arpa' }],
};
const EVA = { name: 'Eva', email: 'eva@mattbox.home.arpa' };
const ME_ADDR = { name: 'Matus', email: 'matus@mattbox.home.arpa' };
const LAKE = {
  id: 'm1',
  threadId: 't1',
  subject: 'Photos from the lake',
  from: [EVA],
  to: [ME_ADDR],
  receivedAt: '2026-10-20T17:04:00Z',
  preview: 'I put the photos from Saturday in the shared folder.',
  seen: false,
  flagged: false,
  hasAttachment: false,
  pgp: null,
};
const SALE = {
  id: 'm2',
  threadId: 't2',
  subject: 'Autumn sale',
  from: [{ name: 'Shop', email: 'news@shop.example' }],
  to: [ME_ADDR],
  receivedAt: '2026-10-19T08:00:00Z',
  preview: 'Hello Matus',
  seen: true,
  flagged: false,
  hasAttachment: false,
  pgp: null,
};
const SEALED = {
  id: 'm3',
  threadId: 't3',
  subject: 'The spare key',
  from: [EVA],
  to: [ME_ADDR],
  receivedAt: '2026-10-18T09:30:00Z',
  preview: 'Version: 1',
  seen: true,
  flagged: false,
  hasAttachment: true,
  pgp: 'encrypted',
};

/* OpenPGP: Eva's key is made here; Matus's in the page, and kept in
 * `ownKey` as the box would answer for it. */
const PASSPHRASE = 'blue heron at dawn';
const evaPair = await openpgp.generateKey({
  type: 'ecc',
  curve: 'curve25519Legacy',
  userIDs: [{ name: 'Eva', email: 'eva@mattbox.home.arpa' }],
  format: 'object',
});
const evaKey = evaPair.privateKey;
const found = (key) => ({
  source: 'box',
  fingerprint: key.getFingerprint().toUpperCase(),
  userIds: key.getUserIDs(),
  created: Math.floor(key.getCreationTime().getTime() / 1000),
  publicKey: key.toPublic().armor(),
});
let ownKey = null;
const SECRET = 'The spare key is under the blue pot.';
/* Eva's message to Matus, as lososd's raw endpoint hands it over. */
async function sealedRaw() {
  const matus = await openpgp.readKey({ armoredKey: ownKey.publicKey });
  const inner = `Content-Type: text/plain; charset=utf-8\r\n\r\n${SECRET}\r\n`;
  const armored = await openpgp.encrypt({
    message: await openpgp.createMessage({ binary: new TextEncoder().encode(inner) }),
    encryptionKeys: matus,
    signingKeys: evaKey,
  });
  const b = 'b1';
  return (
    'From: Eva <eva@mattbox.home.arpa>\r\nTo: matus@mattbox.home.arpa\r\nSubject: The spare key\r\nMIME-Version: 1.0\r\n' +
    `Content-Type: multipart/encrypted; protocol="application/pgp-encrypted"; boundary="${b}"\r\n\r\n` +
    `--${b}\r\nContent-Type: application/pgp-encrypted\r\n\r\nVersion: 1\r\n\r\n` +
    `--${b}\r\nContent-Type: application/octet-stream\r\n\r\n${armored.replace(/\n/g, '\r\n')}\r\n--${b}--\r\n`
  );
}
const full = (m, extra) => ({ ...m, cc: [], replyTo: [], mailboxIds: ['a'], text: '', hasHtml: false, attachments: [], ...extra });
const MESSAGES = {
  m1: full(LAKE, { text: 'Hi Matus,\n\nThe one of the boat at sunset is my favourite.\n\nEva' }),
  m2: full(SALE, { hasHtml: true, text: 'Hello Matus' }),
  m3: full(SEALED, {
    text: 'Version: 1',
    attachments: [{ blobId: 'x', name: 'encrypted.asc', type: 'application/octet-stream', size: 900 }],
  }),
};
const SALE_HTML =
  '<!doctype html><meta charset="utf-8"><base target="_blank">' +
  "<p id=hello style='color:red'>Hello <b>Matus</b></p>" +
  '<script>parent.document.title = "framed"</script>';

/* The mail page with a stand-in lososd. `me` and `mail` choose what
 * /api/me and /api/mail answer; `calls` records what the page sent. */
async function open({ path = '/mail', me = ME, mail = 200, viewport = { width: 1280, height: 900 } } = {}) {
  const page = await browser.newPage({ viewport, locale: 'en-US' });
  const errors = [];
  const calls = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  await page.addInitScript(() => {
    window.__violations = [];
    document.addEventListener('securitypolicyviolation', (e) => window.__violations.push(e.violatedDirective));
  });
  // The rest of the shell (pins, the look, the claim) gets empty answers.
  await page.route('**/api/**', (route) => json(route, 200, {}));
  await page.route('**/api/setup/claim', (route) => json(route, 200, { claimed: true }));
  await page.route('**/api/me', (route) => (me ? json(route, 200, me) : json(route, 401, { error: 'unauthorized' })));
  await page.route('**/api/profiles', (route) =>
    json(route, 200, {
      people: [
        { name: 'eva', displayName: 'Eva', pronouns: '', about: '', accent: 'rose', accentChosen: true, picture: null, banner: null },
      ],
    }),
  );
  await page.route('**/api/mail', (route) => {
    if (mail === 404) return json(route, 404, { error: 'there is no mail on this box' });
    if (mail === 409) return json(route, 409, { error: 'the spare key has no mailbox' });
    return json(route, 200, HOME);
  });
  await page.route('**/api/mail/messages?**', (route) => {
    const url = new URL(route.request().url());
    calls.push({ list: url.searchParams.get('mailbox'), q: url.searchParams.get('q') ?? '' });
    const q = url.searchParams.get('q') ?? '';
    const inbox = url.searchParams.get('mailbox') === 'a';
    const messages = inbox ? [LAKE, SALE, SEALED].filter((m) => q === '' || /lake/i.test(m.subject) === /lake/i.test(q)) : [];
    return json(route, 200, { messages, position: 0, total: messages.length });
  });
  await page.route(/\/api\/mail\/messages\/m\d$/, (route) => {
    const id = route.request().url().split('/').pop();
    const method = route.request().method();
    if (method !== 'GET') calls.push({ [method]: id, body: route.request().postData() });
    if (method === 'GET') {
      if (id === 'm1') LAKE.seen = true;
      return json(route, 200, MESSAGES[id]);
    }
    return json(route, 200, { ok: true });
  });
  await page.route(/\/api\/mail\/messages\/m2\/html$/, (route) =>
    route.fulfill({
      status: 200,
      headers: { 'Content-Type': 'text/html; charset=utf-8', 'Content-Security-Policy': MAIL_FRAME_CSP },
      body: SALE_HTML,
    }),
  );
  await page.route('**/api/mail/keys', async (route) => {
    const method = route.request().method();
    if (method === 'PUT') {
      const { publicKey, lockedKey } = JSON.parse(route.request().postData());
      const key = await openpgp.readKey({ armoredKey: publicKey });
      const locked = await openpgp.readPrivateKey({ armoredKey: lockedKey });
      calls.push({ putKey: { decrypted: locked.isDecrypted() } });
      ownKey = { ...found(key), lockedKey, sealed: 'tpm2', wkd: 'https://mattbox.home.arpa/.well-known/openpgpkey/hu/x' };
      delete ownKey.source;
    }
    if (method === 'DELETE') ownKey = null;
    return json(route, 200, { key: ownKey });
  });
  await page.route('**/api/mail/keys/find?**', (route) => {
    const email = new URL(route.request().url()).searchParams.get('email');
    calls.push({ find: email });
    const keys = email === 'eva@mattbox.home.arpa' ? [found(evaKey)] : email === ME_ADDR.email && ownKey ? [{ ...ownKey, source: 'box' }] : [];
    return json(route, 200, { email, keys });
  });
  await page.route(/\/api\/mail\/messages\/m3\/raw$/, async (route) =>
    route.fulfill({ status: 200, contentType: 'message/rfc822', body: await sealedRaw() }),
  );
  await page.route('**/api/mail/send', (route) => {
    calls.push({ send: JSON.parse(route.request().postData()) });
    return json(route, 200, { ok: true });
  });
  await page.goto(origin + path, { waitUntil: 'networkidle' });
  /* Sonner's refused copy of its own stylesheet is logged at load, as on
   * the admin page (tests/app.browser.mjs); its rules come from the bundle.
   * Nothing after that may add a violation. */
  const atLoad = await page.evaluate(() => window.__violations);
  assert.ok(atLoad.every((d) => d === 'style-src-elem'), `CSP violations at load: ${JSON.stringify(atLoad)}`);
  const done = async () => {
    const violations = (await page.evaluate(() => window.__violations)).slice(atLoad.length);
    assert.deepEqual(violations, [], `CSP violations on the mail page: ${JSON.stringify(violations)}`);
    assert.deepEqual(errors, [], 'uncaught errors on the mail page');
    await page.close();
  };
  return { page, calls, done };
}

await check('the inbox lists the mail, newest first, with the unread one marked', async () => {
  LAKE.seen = false;
  const { page, done } = await open();
  const lake = page.getByRole('button', { name: /Photos from the lake/ });
  await lake.waitFor();
  // A screen of the admin pages: their sidebar, with Mail current.
  await page.getByRole('heading', { level: 1, name: 'Mail' }).waitFor();
  const nav = page.getByRole('link', { name: 'Mail', exact: true });
  assert.equal(await nav.getAttribute('aria-current'), 'page', 'Mail is not current in the sidebar');
  const sale = page.getByRole('button', { name: /Autumn sale/ });
  assert.match(await lake.innerText(), /Eva/);
  assert.ok((await lake.boundingBox()).y < (await sale.boundingBox()).y, 'the newer message is not first');
  assert.equal(await lake.getByLabel('Unread').count(), 1, 'the unread dot is missing');
  // People on the box are drawn with their profile's colour, anyone else
  // by initials alone.
  const eva = lake.locator('[data-slot=avatar]');
  await page.waitForFunction(() => document.querySelector('[data-slot=avatar][data-accent=rose]') !== null);
  assert.equal(await eva.getAttribute('data-accent'), 'rose');
  assert.equal((await eva.innerText()).trim(), 'E');
  assert.equal(await sale.locator('[data-slot=avatar]').getAttribute('data-accent'), null);
  await page.getByRole('button', { name: /Sent/ }).first().waitFor();
  await page.getByRole('button', { name: /Bin/ }).first().waitFor();
  await done();
});

await check('opening a message reads it, and Reply quotes it and threads under it', async () => {
  LAKE.seen = false;
  const { page, calls, done } = await open();
  await page.getByRole('button', { name: /Photos from the lake/ }).click();
  await page.getByText('The one of the boat at sunset is my favourite.').waitFor();
  await page.getByRole('button', { name: 'Reply', exact: true }).click();
  const dialog = page.getByRole('dialog');
  await dialog.waitFor();
  // The To field offers the people on the box, so it is a combobox.
  assert.equal(await dialog.getByRole('combobox').first().inputValue(), 'eva@mattbox.home.arpa');
  assert.equal(await dialog.locator('input').nth(1).inputValue(), 'Re: Photos from the lake');
  const body = dialog.locator('textarea');
  assert.match(await body.inputValue(), /wrote:\n> Hi Matus,/);
  await body.press('Control+Home');
  await body.pressSequentially('They look great.');
  await dialog.getByRole('button', { name: 'Send', exact: true }).click();
  await dialog.waitFor({ state: 'detached' });
  const sent = calls.find((c) => c.send)?.send;
  assert.ok(sent, 'nothing was sent');
  assert.deepEqual(sent.to, ['eva@mattbox.home.arpa']);
  assert.equal(sent.inReplyTo, 'm1');
  assert.match(sent.text, /^They look great\.\n\n/);
  await done();
});

await check('search asks lososd with the words typed', async () => {
  const { page, calls, done } = await open();
  await page.getByRole('button', { name: /Autumn sale/ }).waitFor();
  await page.getByRole('searchbox', { name: 'Search mail' }).fill('lake');
  await page.getByRole('searchbox', { name: 'Search mail' }).press('Enter');
  await page.getByText('Results for “lake”').waitFor();
  assert.ok(calls.some((c) => c.q === 'lake'), `no search sent: ${JSON.stringify(calls)}`);
  await page.getByRole('button', { name: /Autumn sale/ }).waitFor({ state: 'detached' });
  await done();
});

await check('delete moves the message to the bin through lososd', async () => {
  const { page, calls, done } = await open();
  await page.getByRole('button', { name: /Autumn sale/ }).click();
  await page.getByRole('button', { name: 'Delete', exact: true }).click();
  await page.getByText('Moved to the bin').waitFor();
  assert.ok(calls.some((c) => c.DELETE === 'm2'), `no delete sent: ${JSON.stringify(calls)}`);
  await done();
});

await check('HTML mail is drawn in a sandboxed frame whose script never runs', async () => {
  const { page, done } = await open();
  const title = await page.title();
  await page.getByRole('button', { name: /Autumn sale/ }).click();
  await page.getByRole('button', { name: 'Show formatting' }).click();
  const frame = page.locator('iframe');
  await frame.waitFor();
  assert.equal(await frame.getAttribute('sandbox'), 'allow-popups allow-popups-to-escape-sandbox');
  assert.equal(await frame.getAttribute('src'), '/api/mail/messages/m2/html');
  await page.frameLocator('iframe').locator('#hello').waitFor();
  await page.waitForTimeout(300);
  assert.equal(await page.title(), title, 'the message script reached the page');
  await page.getByRole('button', { name: 'Show plain text' }).click();
  await frame.waitFor({ state: 'detached' });
  await done();
});

await check('a box with no mail says so, and the spare key is asked to sign in', async () => {
  let { page, done } = await open({ mail: 404 });
  await page.getByText('There is no mail on this box.', { exact: false }).waitFor();
  await done();
  ({ page, done } = await open({ mail: 409, me: { ...ME, name: null, spareKey: true } }));
  await page.getByText('The spare key has no mailbox.', { exact: false }).waitFor();
  await page.getByRole('button', { name: 'Sign in' }).waitFor();
  await done();
});

await check('on a phone the list and the message take turns', async () => {
  const { page, done } = await open({ viewport: { width: 390, height: 844 } });
  const lake = page.getByRole('button', { name: /Photos from the lake/ });
  await lake.click();
  await page.getByText('The one of the boat at sunset is my favourite.').waitFor();
  assert.equal(await lake.isVisible(), false, 'the list stays beside the message on a phone');
  await page.getByRole('button', { name: 'Back to the list' }).click();
  await lake.waitFor();
  const width = await page.evaluate(() => document.documentElement.scrollWidth);
  assert.ok(width <= 390, `the page scrolls sideways: ${width}px`);
  await done();
});

await check('OpenPGP: make a key, then send Eva mail only her key opens', async () => {
  ownKey = null;
  const { page, calls, done } = await open();
  await page.getByRole('button', { name: 'Encryption' }).click();
  const dialog = page.getByRole('dialog');
  await dialog.getByText('A new key for matus@mattbox.home.arpa', { exact: false }).waitFor();
  await dialog.getByLabel('Passphrase').fill(PASSPHRASE);
  await dialog.getByLabel('The same again').fill(PASSPHRASE);
  await dialog.getByRole('button', { name: 'Make the key' }).click();
  await dialog.getByTestId('mail-key-fingerprint').waitFor({ timeout: 30_000 });
  assert.deepEqual(calls.find((c) => c.putKey)?.putKey, { decrypted: false }, 'the private key left the page unlocked');
  assert.equal((await dialog.getByTestId('mail-key-fingerprint').innerText()).replace(/\s/g, ''), ownKey.fingerprint);
  await dialog.getByText('sealed again to this box\'s TPM', { exact: false }).waitFor();
  await dialog.getByRole('button', { name: 'Close' }).click();
  await dialog.waitFor({ state: 'detached' });

  await page.getByRole('button', { name: 'Write' }).click();
  const compose = page.getByRole('dialog');
  await compose.getByRole('combobox').first().fill('eva@mattbox.home.arpa');
  await compose.locator('[data-testid=mail-recipient-key][data-state=found]').waitFor();
  assert.match(await compose.getByTestId('mail-recipient-key').innerText(), /key on this box/);
  // Every recipient has a key, so it encrypts by itself, and signs.
  await page.waitForFunction(() => document.querySelector('#mail-encrypt')?.checked === true);
  assert.equal(await compose.locator('#mail-sign').isChecked(), true);
  await compose.getByText('The subject is not encrypted.', { exact: false }).waitFor();
  await compose.getByLabel('Subject').fill('Lake photos');
  await compose.locator('textarea').fill('The boat one, at sunset.');
  await compose.getByRole('button', { name: 'Send', exact: true }).click();
  await compose.waitFor({ state: 'detached' });
  const sent = calls.find((c) => c.send)?.send;
  assert.equal(sent.text, '');
  assert.equal(sent.pgp.mime, 'encrypted');
  assert.ok(!sent.pgp.armored.includes('sunset'));
  const result = await openpgp.decrypt({
    message: await openpgp.readMessage({ armoredMessage: sent.pgp.armored }),
    decryptionKeys: evaKey,
    verificationKeys: await openpgp.readKey({ armoredKey: ownKey.publicKey }),
    format: 'binary',
  });
  assert.match(new TextDecoder().decode(result.data), /The boat one, at sunset\./);
  await result.signatures[0].verified;
  await done();
});

await check('OpenPGP: Eva\'s encrypted mail opens after the passphrase, signed by her', async () => {
  const { page, done } = await open();
  const row = page.getByRole('button', { name: /The spare key/ });
  await row.getByLabel('Encrypted message').waitFor();
  assert.match(await row.innerText(), /Encrypted message/);
  assert.doesNotMatch(await row.innerText(), /Version: 1/);
  await row.click();
  // A new page: the key is locked again.
  await page.getByText('Unlock your key to read it.').waitFor();
  assert.equal(await page.getByText(SECRET).count(), 0);
  await page.getByRole('button', { name: 'Unlock', exact: true }).click();
  const unlock = page.getByRole('dialog');
  await unlock.getByLabel('Passphrase').fill('not the passphrase');
  await unlock.getByRole('button', { name: 'Unlock' }).click();
  await unlock.getByText('That passphrase does not open the key.').waitFor();
  await unlock.getByLabel('Passphrase').fill(PASSPHRASE);
  await unlock.getByRole('button', { name: 'Unlock' }).click();
  await page.getByTestId('mail-opened').getByText(SECRET).waitFor({ timeout: 30_000 });
  const note = page.getByTestId('mail-security');
  assert.equal(await note.getAttribute('data-state'), 'ok');
  assert.match(await note.innerText(), /Encrypted, and signed by eva@mattbox\.home\.arpa\./);
  // The encrypted part is not offered as an attachment.
  assert.equal(await page.getByText('encrypted.asc').count(), 0);
  // Reply quotes what it said, not the armour.
  await page.getByRole('button', { name: 'Reply', exact: true }).click();
  assert.match(await page.getByRole('dialog').locator('textarea').inputValue(), /> The spare key is under the blue pot\./);
  await done();
});

await browser.close();
await closeServer();
finish();
