/* The browser half of nix/tests/setup-desktop.nix: set up a booted box the
 * way its owner does, through the wizard, and photograph the last step.
 *
 *   node setup-desktop-browser.mjs '{"origin": "https://mattbox.local",
 *     "map": "127.0.0.1:18443", "password": "…", "out": "/…/shots"}'
 *
 * One browser walks the three steps from a box nobody owns: the claim in
 * step 2, then step 3 signing in to the box's real LosOS Login with that
 * password and drawing the desktop behind the card. The arrival and the
 * growth on "Let's go" are taken frame by frame: every animation frozen
 * through the DevTools protocol, then each moved to the times below and
 * photographed, so a slow emulated machine misses none. Then the same last
 * step in more windows and languages, each a fresh browser whose tab holds
 * the spare key and is on step 3, as a tab reloaded there is: it asks for
 * the password once more and goes on the same way.
 *
 * `map` sends the box's name to the forwarded port, so the page is at the
 * name LosOS Login keeps a cookie for. Prints one JSON line last, what the
 * test script asserts on. Playwright is nixpkgs' playwright-core, named by
 * LOSOS_PLAYWRIGHT. */

const join = (...parts) => parts.join('/');

const { chromium } = await import(Bun.env.LOSOS_PLAYWRIGHT);
const plan = JSON.parse(Bun.argv[2]);
Bun.spawnSync(["mkdir", "-p", plan.out]);

/* Milliseconds into the motion: the desktop arriving (900 ms) and the card
 * after it, then the desktop growing (700 ms). */
const ARRIVE = [0, 150, 300, 450, 600, 900];
const GROW = [0, 120, 240, 360, 500, 700];

const WORDS = {
  'en-US': { go: "Let's go", signIn: 'Sign in', password: 'Password', skip: 'Skip the tour' },
  'sk-SK': { go: 'Poďme na to', signIn: 'Prihlásiť sa', password: 'Heslo', skip: 'Preskočiť prehliadku' },
  'de-DE': { go: "Los geht's", signIn: 'Anmelden', password: 'Passwort', skip: 'Tour überspringen' },
};

const browser = await chromium.launch({
  args: ['--no-sandbox', `--host-resolver-rules=MAP mattbox.local ${plan.map}`],
});

async function open({ locale = 'en-US', colorScheme = 'light', width = 1280, height = 800, motion = false, seed = null }) {
  const context = await browser.newContext({
    viewport: { width, height },
    ignoreHTTPSErrors: true,
    locale,
    colorScheme,
    reducedMotion: motion ? 'no-preference' : 'reduce',
    timezoneId: 'Europe/Bratislava',
  });
  if (seed !== null) {
    // A tab on step 3 that holds the spare key and has forgotten the password.
    await context.addInitScript((s) => {
      if (sessionStorage.getItem('seeded') !== null) return;
      sessionStorage.setItem('seeded', '1');
      sessionStorage.setItem('losos-token', s.token);
      sessionStorage.setItem(
        'losos-setup-progress',
        JSON.stringify({ step: 'finish', account: s.account, login: 'matus', keyShown: true, signedIn: null }),
      );
    }, seed);
  }
  await context.addInitScript(() => {
    window.__violations = [];
    document.addEventListener('securitypolicyviolation', (e) =>
      window.__violations.push(`${e.violatedDirective} ${e.blockedURI || 'inline'}`),
    );
  });
  const page = await context.newPage();
  page.setDefaultTimeout(120000);
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  return { context, page, errors };
}

const shot = (page, name) => page.screenshot({ path: join(plan.out, `${name}.png`), timeout: 300000 });

/* Freeze the page's clock of animations; what starts after this waits at 0. */
async function freeze(context, page) {
  const cdp = await context.newCDPSession(page);
  await cdp.send('Animation.enable');
  await cdp.send('Animation.setPlaybackRate', { playbackRate: 0 });
  await page.evaluate(() => {
    window.__before = new Set(document.getAnimations());
  });
  return cdp;
}

/* Each animation started since `freeze` moved to each time, and photographed. */
async function frames(page, name, times) {
  for (const ms of times) {
    const count = await page.evaluate((time) => {
      const fresh = document.getAnimations().filter((a) => !window.__before.has(a) && a.effect?.getComputedTiming().iterations !== Infinity);
      for (const a of fresh) a.currentTime = time;
      return fresh.length;
    }, ms);
    if (ms === 0) console.error(`${name}: ${count} animations`);
    await shot(page, `${name}-${String(ms).padStart(3, '0')}ms`);
  }
}

/* Let the frozen animations end, and the clock run again. */
async function thaw(cdp, page) {
  await page.evaluate(() => {
    for (const a of document.getAnimations()) {
      if (!window.__before.has(a) && a.effect?.getComputedTiming().iterations !== Infinity) a.finish();
    }
  });
  await cdp.send('Animation.setPlaybackRate', { playbackRate: 1 });
}

const stage = (page) => page.getByTestId('desktop').getAttribute('data-stage');
const whoAmI = (page) =>
  page.evaluate(async () => {
    const r = await fetch('/api/me', { cache: 'no-store' });
    return r.ok ? (await r.json()).name : r.status;
  });

const result = { variants: [] };

// ── The owner's own run through setup ─────────────────────────────────────
{
  const { context, page, errors } = await open({ motion: true });
  await page.goto(`${plan.origin}/`, { waitUntil: 'networkidle' });
  await page.getByRole('button', { name: 'Continue', exact: true }).click();

  // Step 2: the box takes a password once it is ready for one.
  const name = page.locator('input[name="username"]');
  await name.waitFor();
  await page.waitForFunction(() => !document.querySelector('input[name="username"]')?.disabled, null, { timeout: 900000 });
  await name.fill('matus');
  await page.locator('input[name="new-password"]').fill(plan.password);
  await page.locator('input[name="confirm-password"]').fill(plan.password);
  await page.getByRole('button', { name: 'Set the password', exact: true }).click();
  await page.getByText('notshared').first().waitFor({ timeout: 300000 });
  result.token = await page.evaluate(() => sessionStorage.getItem('losos-token'));
  result.account = 'notshared';

  // Step 3: signed in by the page itself, the desktop arriving behind it.
  await page.getByRole('button', { name: 'Continue', exact: true }).click();
  await page.getByText('Signing you in as matus').waitFor({ timeout: 60000 });
  let cdp = await freeze(context, page);
  await page.getByTestId('desktop-ready').waitFor({ timeout: 600000 });
  await page.getByTestId('signed-in-avatar').waitFor({ timeout: 120000 });
  await page.evaluate(() => document.fonts.ready);
  await frames(page, 'arrive', ARRIVE);
  await thaw(cdp, page);
  await page.waitForLoadState('networkidle');
  await page.waitForTimeout(1500);
  result.preview = {
    stage: await stage(page),
    inert: await page.getByTestId('desktop').evaluate((el) => el.inert),
    docked: await page.getByTestId('wizard').getAttribute('data-docked'),
    token: await page.evaluate(() => sessionStorage.getItem('losos-token')),
    me: await whoAmI(page),
    text: await page.getByTestId('desktop-ready').innerText(),
  };
  await shot(page, 'preview-light-desktop');

  // "Let's go": the desktop grows to the window, then setup is over.
  cdp = await freeze(context, page);
  await page.getByRole('button', { name: "Let's go", exact: true }).click({ force: true });
  await page.waitForTimeout(500);
  result.growing = await stage(page);
  await frames(page, 'grow', GROW);
  await thaw(cdp, page);
  const tour = page.getByTestId('tour');
  await tour.waitFor({ timeout: 60000 });
  // The scene loops; its still (what reduced motion shows) is the frame
  // chosen to say what it is about, so the picture is that, not a frame
  // the emulated machine happened to land on.
  await page.emulateMedia({ reducedMotion: 'reduce' });
  await page.waitForTimeout(1500);
  await shot(page, 'after-tour-light-desktop');
  await tour.getByRole('button', { name: WORDS['en-US'].skip }).click();
  await tour.waitFor({ state: 'detached' });
  await page.waitForTimeout(1000);
  await shot(page, 'after-light-desktop');
  result.after = {
    stage: await stage(page),
    wizard: await page.getByTestId('wizard').count(),
    me: await whoAmI(page),
  };
  // A reload stays signed in, by the session, and is not setup any more.
  await page.reload({ waitUntil: 'networkidle' });
  await page.getByTestId('signed-in-avatar').waitFor({ timeout: 120000 });
  result.reload = { wizard: await page.getByTestId('wizard').count(), me: await whoAmI(page) };
  result.errors = errors;
  result.violations = await page.evaluate(() => window.__violations);
  await context.close();
}

// ── The same last step in other windows and languages ─────────────────────
for (const v of [
  { suffix: 'dark-desktop', colorScheme: 'dark' },
  { suffix: 'light-phone', width: 390, height: 844 },
  { suffix: 'dark-phone', colorScheme: 'dark', width: 390, height: 844 },
  { suffix: 'sk-light-desktop', locale: 'sk-SK' },
  { suffix: 'de-light-desktop', locale: 'de-DE' },
]) {
  const words = WORDS[v.locale ?? 'en-US'];
  const { context, page, errors } = await open({ ...v, seed: { token: result.token, account: result.account } });
  await page.goto(`${plan.origin}/`, { waitUntil: 'networkidle' });
  const field = page.getByLabel(words.password, { exact: true });
  await field.waitFor();
  await field.fill(plan.password);
  await page.getByRole('button', { name: words.signIn, exact: true }).click();
  await page.getByTestId('desktop-ready').waitFor({ timeout: 120000 });
  await page.getByTestId('signed-in-avatar').waitFor({ timeout: 120000 });
  await page.waitForLoadState('networkidle');
  await page.waitForTimeout(1500);
  await shot(page, `preview-${v.suffix}`);
  const seen = { suffix: v.suffix, stage: await stage(page), me: await whoAmI(page) };
  await page.getByRole('button', { name: words.go, exact: true }).click();
  await page.getByTestId('tour').waitFor({ timeout: 60000 });
  await page.getByTestId('tour').getByRole('button', { name: words.skip }).click();
  await page.getByTestId('tour').waitFor({ state: 'detached' });
  await page.waitForTimeout(1000);
  await shot(page, `after-${v.suffix}`);
  seen.after = await stage(page);
  seen.pageWidth = await page.evaluate(() => document.documentElement.scrollWidth);
  seen.width = v.width ?? 1280;
  seen.errors = errors;
  seen.violations = await page.evaluate(() => window.__violations);
  result.variants.push(seen);
  await context.close();
}

await browser.close();
console.log(JSON.stringify(result));
