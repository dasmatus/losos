/* LosOS Mail (/mail/, the fourth page of this bundle) under the admin
 * page's policy, with lososd's /api/mail* stood in by page.route the way
 * backend/src/mail.rs answers. Every page fails on a CSP violation or an
 * uncaught error. Walked once each: the inbox, opening a message (which
 * marks it read), a reply with the quote and threading id, search, a move
 * to the bin, an HTML message in its sandboxed frame (whose script never
 * runs), a box with no mail (404), the spare key (409), and the phone
 * layout. nix/tests/mail.nix runs the same page against a real Stalwart. */

import assert from 'node:assert';
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
};
const full = (m, extra) => ({ ...m, cc: [], replyTo: [], mailboxIds: ['a'], text: '', hasHtml: false, attachments: [], ...extra });
const MESSAGES = {
  m1: full(LAKE, { text: 'Hi Matus,\n\nThe one of the boat at sunset is my favourite.\n\nEva' }),
  m2: full(SALE, { hasHtml: true, text: 'Hello Matus' }),
};
const SALE_HTML =
  '<!doctype html><meta charset="utf-8"><base target="_blank">' +
  "<p id=hello style='color:red'>Hello <b>Matus</b></p>" +
  '<script>parent.document.title = "framed"</script>';

/* The mail page with a stand-in lososd. `me` and `mail` choose what
 * /api/me and /api/mail answer; `calls` records what the page sent. */
async function open({ path = '/mail/', me = ME, mail = 200, viewport = { width: 1280, height: 900 } } = {}) {
  const page = await browser.newPage({ viewport, locale: 'en-US' });
  const errors = [];
  const calls = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  await page.addInitScript(() => {
    window.__violations = [];
    document.addEventListener('securitypolicyviolation', (e) => window.__violations.push(e.violatedDirective));
  });
  await page.route('**/api/**', (route) => json(route, 404, { error: 'not here' }));
  await page.route('**/api/me', (route) => (me ? json(route, 200, me) : json(route, 401, { error: 'unauthorized' })));
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
    const messages = inbox ? [LAKE, SALE].filter((m) => q === '' || /lake/i.test(m.subject) === /lake/i.test(q)) : [];
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
  assert.equal(await page.title(), 'Mail');
  const sale = page.getByRole('button', { name: /Autumn sale/ });
  assert.match(await lake.innerText(), /Eva/);
  assert.ok((await lake.boundingBox()).y < (await sale.boundingBox()).y, 'the newer message is not first');
  assert.equal(await lake.getByLabel('Unread').count(), 1, 'the unread dot is missing');
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
  await page.getByRole('button', { name: /Autumn sale/ }).click();
  await page.getByRole('button', { name: 'Show formatting' }).click();
  const frame = page.locator('iframe');
  await frame.waitFor();
  assert.equal(await frame.getAttribute('sandbox'), 'allow-popups allow-popups-to-escape-sandbox');
  assert.equal(await frame.getAttribute('src'), '/api/mail/messages/m2/html');
  await page.frameLocator('iframe').locator('#hello').waitFor();
  await page.waitForTimeout(300);
  assert.equal(await page.title(), 'Mail', 'the message script reached the page');
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

await check('on a phone the folders, the list and the message take turns', async () => {
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

await browser.close();
await closeServer();
finish();
