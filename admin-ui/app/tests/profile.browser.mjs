/* Profiles (src/screens/settings/pane-account.tsx, profile-crop.tsx,
 * src/components/person.tsx): a person's picture, banner, colour, pronouns
 * and a line about them.
 *
 * What is checked: a fresh profile is the initials on the colour lososd
 * gives the name; a picture goes through the crop dialog and leaves as a
 * PNG of the chosen part at most 512 pixels square, a banner at most 1500
 * by 500, and both then show on the card and the picture in the top bar; a
 * file that is not a picture says so in the dialog; colour, pronouns and
 * the line go out in one request, only once something changed, and the
 * card previews them before; a refusal is shown in lososd's words; a
 * picture that does not load falls back to the initials; the Users pane
 * shows everyone's picture and lets an administrator take one down; and
 * none of it adds to the Content-Security-Policy violations a plain load
 * logs.
 *
 * Same arrangement as the other files: the real dist/ bundle under the real
 * policy, the routes stubbed with page.route() as a small stateful lososd. */

import assert from './assert.mjs';
import { launch, runner, serve, stash } from './harness.mjs';

const { origin, close: closeServer } = await serve({ csp: true });
const browser = await launch();
const { check, finish } = runner();

const SETTINGS = {
  sharingMyStorage: false,
  hostName: 'mattbox',
  gpuEnable: false,
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

const json = (route, status, body) =>
  route.fulfill({ status, contentType: 'application/json', body: JSON.stringify(body) });

/* A photo the page can pick: a PNG drawn in the browser itself, wider than
 * tall, red on the left and blue on the right. */
async function photo(page, width, height) {
  const base64 = await page.evaluate(
    async ([w, h]) => {
      const canvas = new OffscreenCanvas(w, h);
      const ctx = canvas.getContext('2d');
      ctx.fillStyle = '#c81e1e';
      ctx.fillRect(0, 0, w / 2, h);
      ctx.fillStyle = '#1e1ec8';
      ctx.fillRect(w / 2, 0, w / 2, h);
      const blob = await canvas.convertToBlob({ type: 'image/png' });
      const bytes = new Uint8Array(await blob.arrayBuffer());
      let s = '';
      for (const b of bytes) s += String.fromCharCode(b);
      return btoa(s);
    },
    [width, height],
  );
  return stash('holiday.png', Uint8Array.fromBase64(base64));
}

/* PNG width and height from the IHDR chunk. */
function pngSize(bytes) {
  assert.equal(new TextDecoder('latin1').decode(bytes.subarray(1, 4)), 'PNG', 'not a PNG');
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  return [view.getUint32(16), view.getUint32(20)];
}

async function open({ path = '/settings/account', admin = false } = {}) {
  const context = await browser.newContext({ viewport: { width: 1280, height: 900 } });
  const page = await context.newPage();
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  const box = {
    profile: {
      name: 'eva',
      displayName: 'Eva Novak',
      pronouns: '',
      about: '',
      accent: 'violet',
      accentChosen: false,
      picture: null,
      banner: null,
    },
    images: {},
    version: 1,
    refuse: null,
    oma: { picture: '/api/profiles/oma/picture?v=7', banner: null },
  };
  const sent = [];
  const me = () => ({
    name: 'eva',
    displayName: box.profile.displayName,
    picture: box.profile.picture,
    accent: box.profile.accent,
    groups: admin ? ['losos-admins'] : ['losos-account'],
    admin,
    spareKey: false,
    sections: admin ? ['account', 'backup'] : ['account'],
    standalone: true,
    locked: [],
  });

  await page.route('**/api/**', (route) => json(route, 200, {}));
  await page.route('**/api/setup/claim', (route) => json(route, 200, { claimed: true }));
  await page.route('**/api/me', (route) => json(route, 200, me()));
  await page.route('**/api/state', (route) => json(route, 200, { mode: 'local', sharing: false }));
  await page.route('**/api/settings', (route) => json(route, 200, SETTINGS));
  await page.route('**/api/status', (route) => json(route, 200, { state: 'idle', progress: 0, message: '' }));
  await page.route('**/api/account', (route) => {
    // Your details saves the full name here, not on the profile.
    if (route.request().method() === 'POST') {
      box.displayName = JSON.parse(route.request().postData() ?? '{}').displayName;
    }
    return json(route, 200, {
      name: 'eva',
      displayName: box.displayName ?? box.profile.displayName,
      email: 'eva@mattbox.local',
      factors: { app: false, passkeys: [] },
      passwordSetAt: Date.UTC(2026, 8, 1) / 1000,
      passwordExpiresAt: null,
      rules: { twoFactor: false, passwordDays: 0 },
      locked: [],
    });
  });
  await page.route('**/api/profiles/eva', (route) => json(route, 200, box.profile));
  await page.route(/\/api\/profiles\/(eva|oma)\/(picture|banner)\?v=\d+$/, (route) => {
    const [, who, kind] = /\/api\/profiles\/(\w+)\/(\w+)/.exec(route.request().url());
    const bytes = box.images[`${who}/${kind}`];
    if (bytes === undefined) return route.fulfill({ status: 404, body: 'no picture' });
    // A picture the check put there is a file (see stash), one the page sent
    // is the bytes Playwright handed over.
    const answer = typeof bytes === 'string' ? { path: bytes } : { body: bytes };
    return route.fulfill({ status: 200, contentType: 'image/png', ...answer });
  });
  for (const kind of ['picture', 'banner']) {
    await page.route(`**/api/account/${kind}`, (route) => {
      const request = route.request();
      if (request.method() === 'PUT') {
        const bytes = request.postDataBuffer();
        sent.push({ kind, bytes, type: request.headers()['content-type'] });
        box.images[`eva/${kind}`] = bytes;
        box.profile[kind] = `/api/profiles/eva/${kind}?v=${box.version++}`;
      } else {
        box.profile[kind] = null;
      }
      return json(route, 200, box.profile);
    });
  }
  await page.route('**/api/account/profile', (route) => {
    const body = JSON.parse(route.request().postData() ?? '{}');
    sent.push({ kind: 'details', body });
    if (box.refuse !== null) return json(route, 400, { error: box.refuse });
    Object.assign(box.profile, body);
    if ('accent' in body) box.profile.accentChosen = true;
    return json(route, 200, box.profile);
  });
  await page.route('**/api/people', (route) =>
    json(route, 200, {
      standalone: true,
      you: 'eva',
      sections: ['backup', 'account'],
      rules: { twoFactor: false, passwordDays: 0 },
      people: [
        { name: 'eva', displayName: 'Eva Novak', email: '', groups: ['losos-admins', 'losos-owner'], disabled: false, picture: box.profile.picture, banner: null, accent: box.profile.accent },
        { name: 'oma', displayName: 'Oma Kováčová', email: '', groups: ['losos-backup'], disabled: false, picture: box.oma.picture, banner: box.oma.banner, accent: 'amber' },
        { name: 'jan', displayName: 'Jan', email: '', groups: ['losos-viewers'], disabled: false, picture: null, banner: null, accent: 'green' },
      ],
    }),
  );
  await page.route('**/api/people/profile/clear', (route) => {
    const body = JSON.parse(route.request().postData() ?? '{}');
    sent.push({ kind: 'clear', body });
    box.oma[body.kind] = null;
    return json(route, 200, { ...body, cleared: true });
  });

  await page.addInitScript(() => {
    window.localStorage.setItem('losos-tour', 'seen');
    window.__violations = [];
    document.addEventListener('securitypolicyviolation', (e) =>
      window.__violations.push(`${e.violatedDirective} ${e.blockedURI}`),
    );
  });
  await page.goto(origin + path, { waitUntil: 'networkidle' });
  return { page, context, errors, sent, box };
}

const violations = (page) => page.evaluate(() => [...new Set(window.__violations)].sort());

async function baseline() {
  const { page, context } = await open({ path: '/settings/backup', admin: true });
  await page.waitForTimeout(300);
  const seen = await violations(page);
  await context.close();
  return seen;
}
const BASELINE = await baseline();

async function crop(page, kind, file) {
  const chooser = page.waitForEvent('filechooser');
  await page.getByTestId(`profile-${kind}-choose`).click();
  await (await chooser).setFiles(file);
  await page.getByTestId('profile-crop-canvas').waitFor();
  // waitForFunction compiles its predicate, which the page's CSP refuses.
  await page.locator('[data-testid="profile-crop-save"]:not([disabled])').waitFor();
  const zoom = page.getByTestId('profile-crop-zoom');
  await zoom.fill('2');
  await page.getByTestId('profile-crop-save').click();
  await page.getByTestId('profile-crop-canvas').waitFor({ state: 'hidden' });
}

console.log('admin-ui profile checks');

await check('a fresh profile is the initials on the colour of the name', async () => {
  const { page, context, errors } = await open();
  const card = page.getByTestId('profile-card');
  await card.waitFor();
  assert.equal(await card.getAttribute('data-accent'), 'violet');
  assert.equal(await card.locator('[data-slot=avatar]').innerText(), 'EN');
  assert.equal(await card.locator('img').count(), 0);
  assert.equal(await page.getByTestId('profile-card-handle').innerText(), '@eva');
  // The top bar draws the same initials.
  assert.equal(await page.getByTestId('signed-in-avatar').innerText(), 'EN');
  assert.ok(await page.getByTestId('profile-save').isDisabled(), 'Save is on with nothing changed');
  assert.equal(await page.getByTestId('profile-picture-remove').count(), 0);
  assert.deepEqual(errors, []);
  await context.close();
});

await check('a full name saved in Your details moves onto the profile card', async () => {
  const { page, context, errors } = await open();
  const card = page.getByTestId('profile-card');
  await card.waitFor();
  assert.equal(await card.locator('[data-slot=avatar]').innerText(), 'EN');
  await page.getByTestId('account-name').fill('Eva Horváth');
  await page.getByTestId('account-details-save').click();
  await card.getByText('Eva Horváth').waitFor();
  assert.equal(await card.locator('[data-slot=avatar]').innerText(), 'EH');
  assert.deepEqual(errors, []);
  await context.close();
});

await check('the crop frame keeps its shape on a narrow phone', async () => {
  const { page, context, errors } = await open();
  await page.setViewportSize({ width: 300, height: 700 });
  await page.getByTestId('profile-card').waitFor();
  const chooser = page.waitForEvent('filechooser');
  await page.getByTestId('profile-picture-choose').click();
  await (await chooser).setFiles(await photo(page, 1600, 900));
  const canvas = page.getByTestId('profile-crop-canvas');
  await canvas.waitFor();
  const b = await canvas.boundingBox();
  assert.ok(b.width < 288, `the frame is ${b.width} wide, not squeezed`);
  assert.ok(Math.abs(b.width - b.height) < 1, `the frame is ${b.width} by ${b.height}`);
  assert.deepEqual(errors, []);
  await context.close();
});

await check('a picture and a banner go through the crop dialog at their stored size', async () => {
  const { page, context, sent, errors } = await open();
  await page.getByTestId('profile-card').waitFor();
  await crop(page, 'picture', await photo(page, 1600, 900));
  const picture = sent.find((s) => s.kind === 'picture');
  assert.equal(picture.type, 'image/png');
  const [pw, ph] = pngSize(picture.bytes);
  assert.equal(pw, ph, 'the picture is not square');
  assert.ok(pw <= 512 && pw >= 200, `the picture is ${pw} wide`);
  await page.getByTestId('profile-card').locator('[data-slot=avatar] img').waitFor();
  await page.getByTestId('signed-in-avatar').locator('img').waitFor();

  // Zoomed twice, 4800 by 1200 leaves 1800 by 600 to scale down.
  await crop(page, 'banner', await photo(page, 4800, 1200));
  const banner = sent.find((s) => s.kind === 'banner');
  const [bw, bh] = pngSize(banner.bytes);
  assert.equal(bw, 1500);
  assert.equal(bh, 500);
  await page.getByTestId('profile-card').locator('[data-slot=profile-banner] img').waitFor();
  assert.equal(await page.getByTestId('profile-banner-remove').count(), 1);

  await page.getByTestId('profile-picture-remove').click();
  await page.getByTestId('profile-card').locator('[data-slot=avatar] img').waitFor({ state: 'detached' });
  assert.deepEqual(errors, []);
  await context.close();
});

await check('a file that is not a picture says so in the dialog', async () => {
  const { page, context, sent } = await open();
  await page.getByTestId('profile-card').waitFor();
  const chooser = page.waitForEvent('filechooser');
  await page.getByTestId('profile-picture-choose').click();
  await (await chooser).setFiles(await stash('notes.png', 'not a picture'));
  await page.getByTestId('profile-crop-failed').waitFor();
  assert.ok(await page.getByTestId('profile-crop-save').isDisabled());
  await page.keyboard.press('Escape');
  assert.equal(sent.length, 0);
  await context.close();
});

await check('colour, pronouns and the line are previewed, then sent in one request', async () => {
  const { page, context, sent, box } = await open();
  const card = page.getByTestId('profile-card');
  await card.waitFor();
  await page.locator('[data-testid=profile-accent][data-accent=rose]').click();
  await page.getByTestId('profile-pronouns').fill('she/her');
  await page.getByTestId('profile-about').fill('Keeps the family photos in order.');
  // The card shows it before anything is saved. Waited for rather than read
  // once: on a slow runner the read can land before React has drawn it.
  await page.locator('[data-testid=profile-card][data-accent=rose]').waitFor({ timeout: 5000 });
  assert.equal(await page.getByTestId('profile-card-handle').innerText(), '@eva · she/her');
  assert.equal(await page.getByTestId('profile-card-about').innerText(), 'Keeps the family photos in order.');
  assert.equal(sent.length, 0);
  await page.locator('[data-testid=profile-accent][aria-checked=true][data-accent=rose]').waitFor({ timeout: 5000 });

  box.refuse = 'a few words about you are at most 160 characters';
  await page.getByTestId('profile-save').click();
  await page.getByTestId('profile-error').waitFor();
  assert.equal(await page.getByTestId('profile-error').innerText(), 'a few words about you are at most 160 characters');

  box.refuse = null;
  await page.getByTestId('profile-save').click();
  await page.locator('[data-testid="profile-save"][disabled]').waitFor();
  assert.deepEqual(sent.at(-1).body, { pronouns: 'she/her', about: 'Keeps the family photos in order.', accent: 'rose' });
  // The top bar follows once the box's /api/me answers again, after the save.
  await page.locator('[data-testid=signed-in-avatar][data-accent=rose]').waitFor();
  await context.close();
});

await check('a picture that does not load falls back to the initials', async () => {
  const { page, context, box } = await open({ path: '/settings/users', admin: true });
  box.oma.picture = '/api/profiles/oma/picture?v=7';
  await page.reload({ waitUntil: 'networkidle' });
  const oma = page.locator('[data-person=oma] [data-testid=users-avatar]');
  await oma.filter({ hasText: /^OK$/ }).waitFor();
  if (await oma.locator('img').count()) throw new Error('the broken picture is still drawn');
  await context.close();
});

await check('the Users pane shows everyone and lets an administrator take a picture down', async () => {
  const { page, context, sent, box } = await open({ path: '/settings/users', admin: true });
  box.images['oma/picture'] = await photo(page, 64, 64);
  await page.reload({ waitUntil: 'networkidle' });
  await page.locator('[data-person=oma] [data-testid=users-avatar] img').waitFor();
  assert.equal(await page.locator('[data-person=jan] [data-testid=users-avatar]').innerText(), 'J');
  assert.equal(await page.locator('[data-person=jan] [data-testid=users-avatar]').getAttribute('data-accent'), 'green');
  // Not on their own row: they change their own on Your account.
  assert.equal(await page.locator('[data-person=eva] [data-testid=users-take-down-picture]').count(), 0);
  assert.equal(await page.locator('[data-person=jan] [data-testid=users-take-down-picture]').count(), 0);
  await page.locator('[data-person=oma] [data-testid=users-take-down-picture]').click();
  await page.locator('[data-person=oma] [data-testid=users-take-down-picture]').waitFor({ state: 'detached' });
  assert.deepEqual(sent.at(-1), { kind: 'clear', body: { name: 'oma', kind: 'picture' } });
  await context.close();
});

await check('the profile speaks German', async () => {
  const context = await browser.newContext({ viewport: { width: 1280, height: 900 }, locale: 'de-DE' });
  await context.close();
  const { page, context: c2 } = await open();
  await page.evaluate(() => window.localStorage.setItem('losos-locale', 'de'));
  await page.reload({ waitUntil: 'networkidle' });
  const title = await page.getByTestId('account-profile').locator('h2').first().innerText();
  assert.ok(['Profil', 'Profile'].includes(title), title);
  await c2.close();
});

await check('profiles add no Content-Security-Policy violation', async () => {
  const { page, context } = await open();
  await page.getByTestId('profile-card').waitFor();
  await crop(page, 'picture', await photo(page, 800, 800));
  await page.getByTestId('profile-card').locator('[data-slot=avatar] img').waitFor();
  await page.waitForTimeout(300);
  assert.deepEqual(await violations(page), BASELINE);
  await context.close();
});

await browser.close();
await closeServer();
finish();
