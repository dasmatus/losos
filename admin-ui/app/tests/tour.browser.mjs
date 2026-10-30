/* The first-run tour (src/tour/): a step per thing the box does, in a
 * dialog, each a looping picture drawn as SVG from a frame number, over one
 * sentence. Six steps on every box, and one more for each feature the box
 * reports (src/tour/features.ts): Office, lending memory to AI models, a
 * name to be paid at, chat, Build with AI and profiles, twelve in all.
 *
 * What is checked: it opens over the app when this browser has it pending
 * (the wizard's Finish sets that; wizard.browser.mjs follows that path), the
 * buttons, the dots and the arrow keys move between steps, Skip, Escape and
 * Done close it and it stays closed on a reload, the About pane opens it
 * again, the picture moves and holds still under prefers-reduced-motion, and
 * none of it trips the appliance's Content-Security-Policy. The scenes are
 * SVG presentation attributes and Tailwind classes only, so a style
 * attribute slipping in would show up here as a violation. (Sonner's refused
 * copy of its stylesheet is logged at every load, tour or not; what counts is
 * that the tour adds nothing to it.)
 *
 * Same arrangement as the other files: the real dist/ bundle, the API
 * stubbed with page.route().
 */

import assert from './assert.mjs';
import { launch, runner, serve } from './harness.mjs';

const TOKEN = 'a'.repeat(64);
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

/* What a box with every feature answers, one answer per feature. */
const ME = { name: 'matus', displayName: 'Matus', groups: ['losos-admins'], admin: true, spareKey: false, sections: [], standalone: true };
const option = (name, current) => ({ name, group: name.split('.')[0], editor: { kind: 'bool' }, nixType: 'boolean', description: '', default: false, defaultText: null, current, danger: false, fixed: null, readOnly: false, set: null });
const options = (...rows) => ({ available: true, version: 1, options: rows, stray: [], excluded: {} });
const DOMAINS = { available: true, eligible: true, reason: null, target: null, addresses: [], max_domains: 3, domains: [], names: ['mattbox.boxes.losos.dasmat.us'], wallet: null };
const FULL = {
  me: { ...ME, shared: ['chat'], picture: null },
  options: options(option('office.enable', true), option('inference.lend.enable', false), option('builder.enable', true)),
  domains: DOMAINS,
};
const STOCK = ['Your apps, on your box', 'Add more apps', 'See where the space goes', 'Lend it while you sleep', 'It updates itself at night', 'Find this tour again'];
const ALL = [
  'Your apps, on your box',
  'Write together',
  'Add more apps',
  'See where the space goes',
  'Lend it while you sleep',
  'Lend memory to AI models',
  'Chat and calls',
  "Get paid at this box's name",
  'Describe a widget, get a widget',
  'Put a face to your name',
  'It updates itself at night',
  'Find this tour again',
];

/* A claimed box, signed in. `tour` is what this browser remembers about the
 * tour: "pending" after the wizard, "seen" once closed, null for nothing.
 * `box` is what the box answers on /api/me, /api/options and /api/domains;
 * nothing there is a box that predates every feature. `optionsDelay` holds
 * the options answer back. */
async function openApp({ path = '/', tour = 'pending', reducedMotion = 'no-preference', locale = 'en-US', settings = SETTINGS, box = {}, optionsDelay = 0, clock = false } = {}) {
  const context = await browser.newContext({ viewport: { width: 1280, height: 900 }, locale, reducedMotion });
  const page = await context.newPage();
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));

  await page.route('**/api/**', (route) => json(route, 200, {}));
  await page.route('**/api/setup/claim', (route) => json(route, 200, { claimed: true }));
  await page.route('**/api/state', (route) => json(route, 200, { mode: 'local', sharing: false }));
  await page.route('**/api/settings', (route) => json(route, 200, settings));
  if (box.me !== undefined) await page.route('**/api/me', (route) => json(route, 200, box.me));
  if (box.domains !== undefined) await page.route('**/api/domains', (route) => json(route, 200, box.domains));
  if (box.options !== undefined) {
    await page.route('**/api/options', async (route) => {
      if (optionsDelay > 0) await new Promise((resolve) => setTimeout(resolve, optionsDelay));
      await json(route, 200, box.options).catch(() => {});
    });
  }
  await page.route('**/api/status', (route) => json(route, 200, { state: 'idle', progress: 0, message: '' }));
  await page.addInitScript(
    ({ token, tour }) => {
      window.sessionStorage.setItem('losos-token', token);
      // Only on the first load: a reload must see what the page itself wrote.
      if (tour !== null && window.sessionStorage.getItem('tour-seeded') === null) {
        window.localStorage.setItem('losos-tour', tour);
        window.sessionStorage.setItem('tour-seeded', '1');
      }
      window.__violations = [];
      document.addEventListener('securitypolicyviolation', (e) =>
        window.__violations.push(`${e.violatedDirective} ${e.blockedURI}`),
      );
    },
    { token: TOKEN, tour },
  );
  // A fake clock drives requestAnimationFrame too, so a test can step the scene's frames.
  if (clock) await page.clock.install();
  // A late answer would hold networkidle until it arrives, which is the moment under test.
  await page.goto(origin + path, { waitUntil: optionsDelay > 0 ? 'load' : 'networkidle' });
  return { page, context, errors };
}

const tour = (page) => page.getByTestId('tour');
const stored = (page) => page.evaluate(() => window.localStorage.getItem('losos-tour'));
const violations = (page) => page.evaluate(() => window.__violations);
/* The picture's markup: what the frame clock changes from one frame to the next. */
/* What a load logs with the tour closed: the baseline the tour must not add to. */
async function violationsAtLoad(path) {
  const { page, context } = await openApp({ path, tour: 'seen' });
  await page.waitForTimeout(300);
  const seen = await violations(page);
  await context.close();
  return seen;
}
const picture = (page) => tour(page).locator('svg').first().evaluate((svg) => svg.innerHTML);

console.log('admin-ui tour checks');

/* Walk every step with Next, checking each title and the count. */
async function walk(page, titles) {
  for (const [i, title] of titles.entries()) {
    await tour(page).getByRole('heading', { name: title, exact: true }).waitFor({ timeout: 3000 });
    assert.ok(await tour(page).getByText(`${i + 1} of ${titles.length}`).isVisible(), `no "${i + 1} of ${titles.length}" on ${title}`);
    // Let the scene play a little, so every scene's drawing runs under the policy.
    await page.waitForTimeout(400);
    if (i < titles.length - 1) await tour(page).getByRole('button', { name: 'Next' }).click();
  }
}

await check('a pending tour opens over the app at step 1, and a box with no extra features has the six steps', async () => {
  const baseline = await violationsAtLoad('/');
  const { page, context, errors } = await openApp();
  await tour(page).waitFor({ timeout: 5000 });
  await walk(page, STOCK);
  // The last step has Done and no Skip.
  assert.ok(await tour(page).getByRole('button', { name: 'Done' }).isVisible(), 'no Done on the last step');
  assert.equal(await tour(page).getByRole('button', { name: 'Skip the tour' }).count(), 0, 'Skip on the last step');
  await tour(page).getByRole('button', { name: 'Done' }).click();
  await tour(page).waitFor({ state: 'detached', timeout: 3000 });
  assert.equal(await stored(page), 'seen');
  assert.deepEqual(await violations(page), baseline, 'the tour added CSP violations');
  assert.deepEqual(errors, []);
  await context.close();
});

await check('a box with every feature has twelve steps, in the sidebar\'s order', async () => {
  const baseline = await violationsAtLoad('/');
  const { page, context, errors } = await openApp({ box: FULL });
  await tour(page).waitFor({ timeout: 5000 });
  await tour(page).getByText('1 of 12').waitFor({ timeout: 5000 });
  await walk(page, ALL);
  await tour(page).getByRole('button', { name: 'Done' }).click();
  await tour(page).waitFor({ state: 'detached', timeout: 3000 });
  assert.deepEqual(await violations(page), baseline, 'the tour added CSP violations');
  assert.deepEqual(errors, []);
  await context.close();
});

await check('Next holds the focus, so Enter walks the tour', async () => {
  const { page, context } = await openApp();
  await tour(page).waitFor({ timeout: 5000 });
  await page.waitForTimeout(200);
  assert.equal(await page.evaluate(() => document.activeElement?.textContent), 'Next');
  await page.keyboard.press('Enter');
  await tour(page).getByRole('heading', { name: 'Add more apps' }).waitFor({ timeout: 3000 });
  await context.close();
});

/* The miniature's sidebar labels: the text inside the picture. */
const pictureText = (page) => tour(page).locator('svg').first().evaluate((svg) => svg.textContent);
const titlesOf = async (page) => {
  const total = Number((await tour(page).getByText(/^1 of \d+$/).innerText()).split(' of ')[1]);
  const seen = [];
  for (let i = 1; i <= total; i++) {
    await tour(page).getByRole('button', { name: `Go to step ${i}`, exact: true }).click();
    seen.push(await tour(page).getByRole('heading').innerText());
  }
  return seen;
};

await check('each step is there only when the box reports its feature', async () => {
  const cases = [
    ['Write together', { options: options(option('office.enable', true)) }],
    ['Lend memory to AI models', { options: options(option('inference.lend.enable', false)) }],
    ["Get paid at this box's name", { domains: DOMAINS }],
    ['Chat and calls', { me: { ...ME, shared: ['chat'] } }],
    ['Describe a widget, get a widget', { options: options(option('builder.enable', true)) }],
    ['Put a face to your name', { me: { ...ME, picture: '/api/profiles/matus/picture?v=1' } }],
  ];
  for (const [title, box] of cases) {
    const { page, context, errors } = await openApp({ box });
    await tour(page).waitFor({ timeout: 5000 });
    await tour(page).getByText('1 of 7').waitFor({ timeout: 5000 });
    const titles = await titlesOf(page);
    assert.deepEqual(titles, ALL.filter((t) => STOCK.includes(t) || t === title), `${title}: ${titles.join(' | ')}`);
    assert.equal(/Chat/.test(await pictureText(page)), title === 'Chat and calls', `${title}: Chat in the picture's sidebar`);
    assert.deepEqual(errors, []);
    await context.close();
  }
});

await check('a feature the box has but has not turned on, or this person cannot open, is not a step', async () => {
  const cases = [
    ['Office off', { options: options(option('office.enable', false)) }],
    ['Build with AI off', { options: options(option('builder.enable', false)) }],
    ['no options document', { options: { available: false } }],
    ['no names on the edge', { domains: { ...DOMAINS, names: [] } }],
    ['an edge without wallets', { domains: { ...DOMAINS, names: undefined } }],
    ['no official edge', { domains: { available: false, reason: 'noOfficialEdge' } }],
    ['no shared apps', { me: { ...ME, shared: [] } }],
    ['the spare key has no Your account', { me: { ...ME, name: null, displayName: null, spareKey: true, picture: null } }],
    ['a box whose sign-in runs elsewhere has no Your account', { me: { ...ME, standalone: false, picture: null } }],
    ['Market is not this person\'s', { me: { ...ME, admin: false, sections: ['storage'] }, options: options(option('inference.lend.enable', true)) }],
  ];
  for (const [name, box] of cases) {
    const { page, context } = await openApp({ box });
    await tour(page).waitFor({ timeout: 5000 });
    await page.waitForTimeout(400);
    assert.ok(await tour(page).getByText(`1 of ${STOCK.length}`).isVisible(), `${name}: not 1 of ${STOCK.length}`);
    await context.close();
  }
});

await check('an answer that comes late adds its step without moving the owner', async () => {
  const { page, context } = await openApp({ box: { options: options(option('office.enable', true)) }, optionsDelay: 2500 });
  await tour(page).waitFor({ timeout: 5000 });
  await tour(page).getByRole('button', { name: 'Go to step 2', exact: true }).click();
  await tour(page).getByRole('heading', { name: 'Add more apps' }).waitFor({ timeout: 3000 });
  await tour(page).getByText('2 of 6').waitFor({ timeout: 1000 });
  // Office answers: its step goes in before this one, and this one stays on screen.
  await tour(page).getByText('3 of 7').waitFor({ timeout: 5000 });
  assert.ok(await tour(page).getByRole('heading', { name: 'Add more apps' }).isVisible());
  await tour(page).getByRole('button', { name: 'Back' }).click();
  await tour(page).getByRole('heading', { name: 'Write together' }).waitFor({ timeout: 3000 });
  await context.close();
});

await check('the tour is wider than the other dialogs, and still fits a phone', async () => {
  for (const [width, expected] of [[1280, 736], [390, 358]]) {
    const { page, context } = await openApp();
    await page.setViewportSize({ width, height: 900 });
    await tour(page).waitFor({ timeout: 5000 });
    const box = await page.locator('dialog[open]').boundingBox();
    assert.ok(Math.abs(box.width - expected) <= 2, `at ${width} px the tour is ${box.width} px wide, not ${expected}`);
    await context.close();
  }
});

await check('Back, the step dots and the arrow keys move between steps', async () => {
  const { page, context } = await openApp();
  await tour(page).waitFor({ timeout: 5000 });
  await tour(page).getByRole('button', { name: 'Go to step 4', exact: true }).click();
  await tour(page).getByRole('heading', { name: 'Lend it while you sleep' }).waitFor();
  assert.equal(await tour(page).getByRole('button', { name: 'Go to step 4', exact: true }).getAttribute('aria-current'), 'step');
  await tour(page).getByRole('button', { name: 'Back' }).click();
  await tour(page).getByRole('heading', { name: 'See where the space goes' }).waitFor();
  await page.keyboard.press('ArrowRight');
  await tour(page).getByRole('heading', { name: 'Lend it while you sleep' }).waitFor();
  await page.keyboard.press('ArrowLeft');
  await page.keyboard.press('ArrowLeft');
  await tour(page).getByRole('heading', { name: 'Add more apps' }).waitFor();
  // Step 1 has no Back.
  await tour(page).getByRole('button', { name: 'Go to step 1', exact: true }).click();
  assert.equal(await tour(page).getByRole('button', { name: 'Back' }).count(), 0, 'Back on step 1');
  await context.close();
});

await check('Skip closes the tour and a reload does not bring it back', async () => {
  const { page, context } = await openApp();
  await tour(page).waitFor({ timeout: 5000 });
  await tour(page).getByRole('button', { name: 'Skip the tour' }).click();
  await tour(page).waitFor({ state: 'detached', timeout: 3000 });
  assert.equal(await stored(page), 'seen');
  await page.reload({ waitUntil: 'networkidle' });
  await page.waitForTimeout(500);
  assert.equal(await tour(page).count(), 0, 'the tour came back after a reload');
  await context.close();
});

await check('Escape closes the tour the same way', async () => {
  const { page, context } = await openApp();
  await tour(page).waitFor({ timeout: 5000 });
  await page.keyboard.press('Escape');
  await tour(page).waitFor({ state: 'detached', timeout: 3000 });
  assert.equal(await stored(page), 'seen');
  await context.close();
});

await check('a box whose owner has seen the tour does not open it, and About opens it again', async () => {
  const { page, context, errors } = await openApp({ path: '/settings/about', tour: 'seen' });
  await page.waitForTimeout(500);
  assert.equal(await tour(page).count(), 0, 'the tour opened for an owner who had seen it');
  const atLoad = (await violations(page)).length;
  await page.getByRole('button', { name: 'Take the tour' }).click();
  await tour(page).getByRole('heading', { name: 'Your apps, on your box' }).waitFor({ timeout: 5000 });
  await tour(page).getByRole('button', { name: 'Skip the tour' }).click();
  await tour(page).waitFor({ state: 'detached', timeout: 3000 });
  const after = await violations(page);
  assert.equal(after.length, atLoad, `the tour added CSP violations: ${after.slice(atLoad).join(', ')}`);
  assert.deepEqual(errors, []);
  await context.close();
});

await check('the picture moves, and holds still under prefers-reduced-motion', async () => {
  const moving = await openApp();
  await tour(moving.page).waitFor({ timeout: 5000 });
  const a = await picture(moving.page);
  await moving.page.waitForTimeout(600);
  assert.notEqual(await picture(moving.page), a, 'the scene did not move');
  await moving.context.close();

  const still = await openApp({ reducedMotion: 'reduce' });
  await tour(still.page).waitFor({ timeout: 5000 });
  const b = await picture(still.page);
  await still.page.waitForTimeout(600);
  assert.equal(await picture(still.page), b, 'the scene moved under reduced motion');
  await still.context.close();
});

/* A window that grows out of the page and folds back into it (the Overview's
 * Files, About's tour) must never be drawn as an empty box: whenever it has
 * grown well past its starting size, what is inside it shows. The scene's clock
 * is stepped a tenth of a second at a time through one whole loop. */
async function neverEmpty(page, part, word, side, least) {
  let open = 0;
  for (let t = 0; t < 6000; t += 100) {
    await page.clock.runFor(100);
    const seen = await tour(page).locator('svg').first().evaluate((svg, [part, side]) => {
      const win = svg.querySelector(`g[data-part="${part}"]`);
      if (win === null) return null;
      const box = win.querySelector(':scope > rect:not([x="-166"])');
      const inside = win.querySelector(':scope > g');
      return { size: Number(box.getAttribute(side)), opacity: Number(inside.getAttribute('opacity') ?? 1), text: inside.textContent };
    }, [part, side]);
    if (seen === null || seen.size < least) continue;
    open += 1;
    assert.ok(seen.opacity >= 0.5 && seen.text.includes(word), `${part} at ${t} ms is ${Math.round(seen.size)} in ${side} and shows nothing: ${JSON.stringify(seen)}`);
  }
  assert.ok(open >= 10, `${part} was open in only ${open} samples`);
}

await check('the Overview step never shows the Files window as an empty box, growing or folding back', async () => {
  const { page, context } = await openApp({ clock: true });
  await tour(page).waitFor({ timeout: 5000 });
  await neverEmpty(page, 'files-window', 'Files', 'width', 200);
  await context.close();
});

await check('the last step never shows the tour it opens as an empty box', async () => {
  const { page, context } = await openApp({ clock: true });
  await tour(page).waitFor({ timeout: 5000 });
  for (let i = 1; i < STOCK.length; i++) await tour(page).getByRole('button', { name: 'Next' }).click();
  await tour(page).getByRole('heading', { name: STOCK[STOCK.length - 1], exact: true }).waitFor({ timeout: 3000 });
  await neverEmpty(page, 'tour-window', 'Your apps, on your box', 'height', 100);
  await context.close();
});

await check('the tour follows the language on screen', async () => {
  const { page, context } = await openApp({ locale: 'sk-SK' });
  await tour(page).waitFor({ timeout: 5000 });
  const text = await tour(page).innerText();
  assert.ok(/1 z 6/.test(text), `the count is not in Slovak:\n${text}`);
  assert.ok(!/Your apps, on your box/.test(text), `the title stayed in English:\n${text}`);
  await context.close();
});

await browser.close();
await closeServer();
finish();
