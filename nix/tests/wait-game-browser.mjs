/* The browser half of nix/tests/wait-game.nix: play the game a long wait
 * offers, on a booted box, and photograph it.
 *
 *   bun wait-game-browser.mjs '{"origin": "http://127.0.0.1:18081",
 *     "mode": "rebuild", "token": "…", "out": "/…/shots",
 *     "sync": "/…/sync"}'
 *
 * The box's wait is real: a rebuild unit lososd started. Only the test script can
 * end them, so when this side has seen what it needs it writes a file
 * named for the step into `sync` and waits for the page to see the wait
 * end. It prints one JSON line last, what it found, for the script to
 * assert on. Playwright is nixpkgs' playwright-core, named by
 * LOSOS_PLAYWRIGHT. */

const exists = (file) => Bun.file(file).exists();

const { chromium } = await import(Bun.env.LOSOS_PLAYWRIGHT);
const plan = JSON.parse(Bun.argv[2]);
Bun.spawnSync(['mkdir', '-p', plan.out]);
// Thrown: an uncaught error is what makes Bun exit non-zero.
const fail = (message) => {
  throw new Error(message);
};
const ask = (step) => Bun.write(`${plan.sync}/${step}`, '');
const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

const browser = await chromium.launch({ args: ['--no-sandbox'] });
const found = { games: {}, violations: [] };

async function newPage({ width = 1280, height = 860, colorScheme = 'light', hasTouch = false, video = null } = {}) {
  const context = await browser.newContext({
    viewport: { width, height },
    deviceScaleFactor: video === null ? 2 : 1,
    locale: 'en-US',
    colorScheme,
    hasTouch,
    reducedMotion: 'no-preference',
    timezoneId: 'Europe/Bratislava',
    ...(video === null ? {} : { recordVideo: { dir: video, size: { width, height } } }),
  });
  const page = await context.newPage();
  page.setDefaultTimeout(90000);
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  await page.addInitScript((token) => {
    if (token !== null) window.sessionStorage.setItem('losos-token', token);
    window.__violations = [];
    document.addEventListener('securitypolicyviolation', (e) =>
      window.__violations.push(`${e.violatedDirective} ${e.blockedURI} ${e.sourceFile}`),
    );
  }, plan.token ?? null);
  return { context, page, errors };
}

const offer = (page) => page.getByRole('button', { name: 'Play while you wait' });
const game = (page) => page.getByTestId('game');
const canvas = (page) => game(page).locator('canvas');
const attr = (page, name) => game(page).getAttribute(name);
const score = async (page) => Number(/(\d+)/.exec(await page.getByTestId('game-score').innerText())?.[1] ?? NaN);

async function switchTo(page, id) {
  for (let i = 0; i < 40; i++) {
    if ((await attr(page, 'data-game')) === id) return;
    await page.getByRole('button', { name: 'Another game' }).click();
  }
  fail(`never offered ${id}`);
}

/* A short round of each game, by the inputs a person would use. */
const PLAY = {
  async upstream(page, seconds = 4) {
    await canvas(page).press('Space');
    const end = Date.now() + seconds * 1000;
    while (Date.now() < end && (await attr(page, 'data-phase')) === 'playing') {
      await page.keyboard.down('Space');
      await sleep(180);
      await page.keyboard.up('Space');
      await sleep(420);
    }
  },
  async eel(page) {
    for (const key of ['ArrowDown', 'ArrowRight', 'ArrowUp', 'ArrowRight', 'ArrowDown']) {
      await canvas(page).press(key);
      await sleep(500);
    }
  },
  async merge(page) {
    const keys = ['ArrowLeft', 'ArrowUp', 'ArrowRight', 'ArrowUp'];
    for (let i = 0; i < 24; i++) await canvas(page).press(keys[i % 4]);
  },
  async bricks(page) {
    const box = await canvas(page).boundingBox();
    await page.mouse.click(box.x + box.width / 2, box.y + box.height * 0.9);
    for (let i = 0; i < 12; i++) {
      await page.mouse.move(box.x + box.width * (0.3 + 0.4 * (i % 2)), box.y + box.height * 0.9, { steps: 6 });
      await sleep(150);
    }
  },
  async pairs(page) {
    const box = await canvas(page).boundingBox();
    const cell = box.width / 4;
    for (const [a, b] of [[0, 1], [2, 3], [5, 10]]) {
      for (const i of [a, b]) await page.mouse.click(box.x + cell * ((i % 4) + 0.5), box.y + cell * (Math.floor(i / 4) + 0.5));
      await sleep(250);
    }
  },
};

/* What a page load logs anyway (Sonner's stylesheet, refused by
 * style-src 'self') is not the game's: count from just before the press. */
const baseline = (page) => page.evaluate(() => (window.__base = window.__violations.length));

async function collect(page, errors, where) {
  const violations = await page.evaluate(() => window.__violations.slice(window.__base ?? 0));
  if (violations.length > 0) console.error(`${where}: refused since the press: ${violations.join('; ')}`);
  // The toaster mounts with the first notification and logs its own; only
  // what the games' chunk did, or anything with no source, is the game's.
  found.violations.push(...violations.filter((v) => !/\/assets\/(?!arcade-)[^ ]+\.js$/.test(v)).map((v) => `${where}: ${v}`));
  if (errors.length > 0) fail(`${where}: the page threw: ${errors.join('; ')}`);
}

{
  /* A claimed box: Apply starts a real rebuild, which runs until the
   * script lets it finish. */
  {
    const { context, page, errors } = await newPage();
    await page.goto(`${plan.origin}/settings/network`, { waitUntil: 'load' });
    await page.getByLabel('This box is called').fill('fishbox');
    await page.getByRole('button', { name: 'Apply', exact: true }).click();
    await offer(page).waitFor();
    await ask('applied');
    await page.screenshot({ path: `${plan.out}/settings-offer.png` });
    await baseline(page);
    await offer(page).click();
    await game(page).waitFor();
    for (const id of ['upstream', 'eel', 'merge', 'bricks', 'pairs']) {
      await switchTo(page, id);
      await PLAY[id](page);
      found.games[id] = { score: await score(page), phase: await attr(page, 'data-phase') };
      await page.screenshot({ path: `${plan.out}/settings-${id}.png` });
    }
    await collect(page, errors, 'settings');
    await context.close();
  }
  {
    const { context, page, errors } = await newPage();
    await page.goto(`${plan.origin}/`, { waitUntil: 'load' });
    await offer(page).waitFor();
    await page.screenshot({ path: `${plan.out}/home-offer.png` });
    await baseline(page);
    await offer(page).click();
    await switchTo(page, 'merge');
    await PLAY.merge(page);
    await page.screenshot({ path: `${plan.out}/home-merge.png` });
    await collect(page, errors, 'home');
    await context.close();
  }
  {
    const { context, page, errors } = await newPage({ colorScheme: 'dark' });
    await page.goto(`${plan.origin}/settings/network`, { waitUntil: 'load' });
    await baseline(page);
    await offer(page).click();
    for (const id of ['upstream', 'merge', 'bricks']) {
      await switchTo(page, id);
      await PLAY[id](page);
      await page.screenshot({ path: `${plan.out}/dark-${id}.png` });
    }
    await collect(page, errors, 'dark');
    await context.close();
  }
  {
    const { context, page, errors } = await newPage({ width: 390, height: 844, hasTouch: true });
    await page.goto(`${plan.origin}/settings/network`, { waitUntil: 'load' });
    await baseline(page);
    await offer(page).tap();
    await switchTo(page, 'pairs');
    const box = await canvas(page).boundingBox();
    await page.touchscreen.tap(box.x + box.width / 8, box.y + box.height / 8);
    await page.touchscreen.tap(box.x + (box.width * 3) / 8, box.y + box.height / 8);
    found.phoneTries = await score(page);
    found.phoneSideways = await page.evaluate(() => document.documentElement.scrollWidth > window.innerWidth);
    await page.screenshot({ path: `${plan.out}/phone-pairs.png` });
    await collect(page, errors, 'phone');
    await context.close();
  }
  {
    /* The clip: a round of Upstream while the box rebuilds, and the end of
     * the rebuild stopping it. */
    const video = `${plan.out}/video`;
    const { context, page, errors } = await newPage({ width: 1120, height: 760, video });
    await page.goto(`${plan.origin}/settings/network`, { waitUntil: 'load' });
    await offer(page).waitFor();
    await sleep(1200);
    await baseline(page);
    await offer(page).click();
    await switchTo(page, 'upstream');
    await sleep(800);
    await PLAY.upstream(page, 7);
    await ask('release');
    await page.getByTestId('game-ready').waitFor({ timeout: 180000 });
    found.rebuildReady = true;
    await sleep(2500);
    await page.screenshot({ path: `${plan.out}/settings-ready.png` });
    await page.getByRole('button', { name: 'Close game' }).click();
    await sleep(1200);
    await collect(page, errors, 'clip');
    await context.close();
    const [file] = await Array.fromAsync(new Bun.Glob('*.webm').scan({ cwd: video }));
    if (file !== undefined) await Bun.$`mv ${video}/${file} ${plan.out}/clip.webm`;
    found.clip = await exists(`${plan.out}/clip.webm`);
  }
}

await browser.close();
console.log(JSON.stringify(found));
