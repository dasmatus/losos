/* The browser half of nix/tests/tour.nix: walk the first-run tour on a
 * booted box and photograph every step.
 *
 *   node tour-browser.mjs '{"origin": "http://127.0.0.1:18080", "token": "…",
 *     "out": "/…/shots/local", "locales": ["en-US"], "themes": ["light"],
 *     "expect": 7, "shots": true, "about": false}'
 *
 * The page is signed in the way the wizard leaves it: the spare key in
 * sessionStorage and the tour owed in localStorage. Each run checks the
 * step count, that every step draws without a page error or a Content-
 * Security-Policy violation the page does not log anyway, and whether Chat
 * is in the picture's sidebar. It prints one JSON line last: the English
 * titles in order and that finding, for the test script to assert on.
 * Playwright is nixpkgs' playwright-core, named by LOSOS_PLAYWRIGHT. */

const { chromium } = await import(Bun.env.LOSOS_PLAYWRIGHT);
const plan = { shots: true, about: false, ...JSON.parse(Bun.argv[2]) };
const fail = (message) => {
  // Thrown: an uncaught error is what makes Bun exit non-zero.
  throw new Error(message);
};

/* The words that differ per language: what to wait for and what to click. */
const WORDS = {
  'en-US': { count: (n, total) => `${n} of ${total}`, next: 'Next', done: 'Done', take: 'Take the tour', step: (n) => `Go to step ${n}` },
  'sk-SK': { count: (n, total) => `${n} z ${total}`, next: 'Ďalej', done: 'Hotovo', take: 'Spustiť prehliadku', step: (n) => `Prejsť na krok ${n}` },
  'de-DE': { count: (n, total) => `${n} von ${total}`, next: 'Weiter', done: 'Fertig', take: 'Tour starten', step: (n) => `Zu Schritt ${n}` },
};
const CHAT = { 'en-US': 'Chat', 'sk-SK': 'Chat', 'de-DE': 'Chat' };

const browser = await chromium.launch({ args: ['--no-sandbox'] });
let titles = [];
let chatInSidebar = false;

for (const locale of plan.locales) {
  for (const theme of plan.themes) {
    const words = WORDS[locale];
    const context = await browser.newContext({
      viewport: { width: 1280, height: 900 },
      deviceScaleFactor: 2,
      locale,
      colorScheme: theme,
      reducedMotion: 'reduce',
      timezoneId: 'Europe/Bratislava',
    });
    const page = await context.newPage();
    page.setDefaultTimeout(60000);
    const errors = [];
    page.on('pageerror', (e) => errors.push(String(e)));
    await page.addInitScript(
      ({ token, about }) => {
        window.sessionStorage.setItem('losos-token', token);
        if (window.sessionStorage.getItem('tour-seeded') === null) {
          window.localStorage.setItem('losos-tour', about ? 'seen' : 'pending');
          window.sessionStorage.setItem('tour-seeded', '1');
        }
        window.__violations = [];
        document.addEventListener('securitypolicyviolation', (e) =>
          window.__violations.push(`${e.violatedDirective} ${e.blockedURI}`),
        );
      },
      { token: plan.token, about: plan.about },
    );

    const tour = page.getByTestId('tour');
    if (plan.about) {
      await page.goto(`${plan.origin}/settings/about`, { waitUntil: 'load' });
      await page.waitForTimeout(500);
      if ((await tour.count()) !== 0) fail('the tour opened for an owner who had seen it');
      await page.getByRole('button', { name: words.take }).click();
    } else {
      await page.goto(`${plan.origin}/`, { waitUntil: 'load' });
    }
    await tour.waitFor();
    // The step count comes from the box's answers; wait for the settled one,
    // and give a late answer the time to change it if it would.
    await tour.getByText(words.count(1, plan.expect), { exact: true }).waitFor({ timeout: 20000 });
    await page.waitForTimeout(3000);
    await tour.getByText(words.count(1, plan.expect), { exact: true }).waitFor({ timeout: 1000 });
    const baseline = (await page.evaluate(() => window.__violations)).length;

    const seen = [];
    for (let i = 1; i <= plan.expect; i++) {
      if (i > 1) await tour.getByRole('button', { name: words.next, exact: true }).click();
      await tour.getByText(words.count(i, plan.expect), { exact: true }).waitFor();
      await page.waitForTimeout(700);
      const title = await tour.getByRole('heading').innerText();
      seen.push(title);
      if (i === 1) {
        const sidebar = await tour.locator('svg').first().evaluate((svg) => svg.textContent);
        if (locale === 'en-US') chatInSidebar = sidebar.includes(CHAT[locale]);
      }
      if (plan.shots) {
        Bun.spawnSync(["mkdir", "-p", plan.out]);
        await page.locator('dialog[open]').screenshot({ path: `${plan.out}/${locale}-${theme}-${String(i).padStart(2, '0')}.png` });
        if (i === 1 && locale === 'en-US') await page.screenshot({ path: `${plan.out}/${locale}-${theme}-page.png` });
      }
    }
    if ((await tour.getByRole('button', { name: words.next, exact: true }).count()) !== 0) fail(`more than ${plan.expect} steps`);
    await tour.getByRole('button', { name: words.done, exact: true }).click();
    await tour.waitFor({ state: 'detached' });
    if ((await page.evaluate(() => window.localStorage.getItem('losos-tour'))) !== 'seen') fail('Done did not mark the tour seen');

    const added = (await page.evaluate(() => window.__violations)).slice(baseline);
    if (added.length > 0) fail(`the tour added CSP violations in ${locale} ${theme}: ${added.join(', ')}`);
    if (errors.length > 0) fail(`page errors in ${locale} ${theme}: ${errors.join(', ')}`);
    console.error(`${locale} ${theme}: ${seen.length} steps: ${seen.join(' | ')}`);
    if (locale === 'en-US') titles = seen;
    await context.close();
  }
}

await browser.close();
console.log(JSON.stringify({ titles, chatInSidebar }));
