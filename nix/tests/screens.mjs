/* Screenshots of a booted box, taken from a client VM in a NixOS test.
 *
 * A test puts this file and its own script on a client VM with Chromium and
 * playwright-core (the version admin-ui/app's lock file pins) and runs the
 * script against the box's nginx. Each shot is a full-viewport PNG plus a
 * step in `manifest.json`: what is on screen, the address bar's text, what
 * happens next and where on the page it happens, so the PNGs can go into a
 * pull request or be cut into a film.
 *
 *     import { Shots } from './screens.mjs';
 *     const shots = new Shots('/tmp/shots/dark', { feature: 'profiles',
 *       test: 'losos-profile-screens', base: 'https://mattbox.local', scheme: 'dark' });
 *     const page = await shots.open();
 *     await shots.signIn(page, 'matus', 'Correct-Horse-Battery-9');
 *     await page.goto(shots.url('/settings/account'));
 *     await shots.take(page, 'account', 'Your account', {
 *       action: { kind: 'click', target: 'choose' },
 *       targets: { choose: page.getByTestId('profile-picture-choose') },
 *     });
 *     await shots.finish();
 *
 * Settings are the ones the PNGs are compared at: 1280 x 900 CSS pixels at
 * twice the density, en-US (or `locale`), Europe/Bratislava, reduced
 * motion, the given colour scheme. The caret is hidden and nothing is taken while a spinner,
 * skeleton or toast is on screen. Waits poll locators: page.waitForFunction
 * compiles its predicate in the page, and the admin page's CSP refuses it. */

import { chromium } from 'playwright-core';

const join = (...parts) => parts.join('/');

export const VIEWPORT = { width: 1280, height: 900 };
export const DPR = 2;
// Hides the caret, and the scrollbars a headless window draws.
const QUIET = `
* { caret-color: transparent !important; }
::-webkit-scrollbar { display: none; }
`;
const BUSY = '[data-slot=spinner], [data-slot=skeleton], [data-sonner-toast], [role=status][data-state=open]';
const TOASTS = '[data-sonner-toast], li[role=status]';

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

export class Shots {
  constructor(out, { feature, test, base, scheme = 'dark', commit = '', host = null, locale = 'en-US' }) {
    this.out = out;
    this.feature = feature;
    this.test = test;
    this.base = base.replace(/\/+$/, '');
    this.scheme = scheme;
    this.locale = locale;
    this.commit = commit || Bun.env.LOSOS_COMMIT || '';
    this.host = host ?? this.base.split('://').pop();
    this.steps = [];
    Bun.spawnSync(["mkdir", "-p", out]);
  }

  url(path) {
    return this.base + path;
  }

  async open() {
    this.browser = await chromium.launch({
      executablePath: Bun.env.LOSOS_CHROMIUM,
      args: ['--disable-gpu', '--font-render-hinting=none'],
    });
    this.context = await this.browser.newContext({
      viewport: VIEWPORT,
      deviceScaleFactor: DPR,
      colorScheme: this.scheme,
      locale: this.locale,
      timezoneId: 'Europe/Bratislava',
      reducedMotion: 'reduce',
      ignoreHTTPSErrors: true,
    });
    // Under TCG a page takes its time.
    this.context.setDefaultTimeout(120_000);
    return this.context.newPage();
  }

  /* Sign in at LosOS Login the way its page does; the cookie stays in the
   * browser for every page after. */
  async signIn(page, name, password) {
    const answer = await page.request.post(this.url('/auth/api/firstfactor'), {
      data: { username: name, password },
    });
    if (!answer.ok()) throw new Error(`LosOS Login refused ${name}: ${answer.status()} ${await answer.text()}`);
  }

  /* Wait until the page has nothing left to load or animate. */
  async settle(page, extra = 300) {
    await page.waitForLoadState('networkidle');
    await page.evaluate(() => document.fonts.ready.then(() => true));
    // A constructed sheet rather than a <style> element, which the CSP refuses.
    await page.evaluate((css) => {
      if (window.__quiet) return;
      window.__quiet = new CSSStyleSheet();
      window.__quiet.replaceSync(css);
      document.adoptedStyleSheets = [...document.adoptedStyleSheets, window.__quiet];
    }, QUIET);
    const deadline = Date.now() + 60_000;
    for (;;) {
      // Visible ones only: a closed <dialog> keeps its spinner in the DOM.
      const busy = await page.locator(BUSY).filter({ visible: true }).count();
      const loading = await page.evaluate(() => [...document.images].some((i) => !i.complete));
      if (busy === 0 && !loading) break;
      if (Date.now() > deadline) {
        const still = await page.evaluate(
          (busy) => [
            ...[...document.querySelectorAll(busy)]
              .filter((e) => e.checkVisibility())
              .map((e) => e.parentElement.outerHTML.slice(0, 400)),
            ...[...document.images].filter((i) => !i.complete).map((i) => `loading ${i.src}`),
          ],
          BUSY,
        );
        throw new Error(`the page never settled at ${page.url()}:\n${still.join('\n')}`);
      }
      await sleep(250);
    }
    await sleep(extra);
  }

  /* A toast fades out by itself; wait for it rather than shoot it. */
  async toastsGone(page) {
    const toasts = page.locator(TOASTS);
    const deadline = Date.now() + 60_000;
    while ((await toasts.filter({ visible: true }).count()) > 0) {
      if (Date.now() > deadline) throw new Error('a toast never went away');
      await sleep(250);
    }
  }

  static async box(locator) {
    const b = await locator.boundingBox();
    if (b === null) throw new Error(`${locator} is not on screen`);
    return { x: Math.round(b.x), y: Math.round(b.y), w: Math.round(b.width), h: Math.round(b.height) };
  }

  /* One full-viewport PNG, numbered in order, and its manifest step. */
  async take(page, slug, title, { action = null, targets = {}, settle = true } = {}) {
    if (settle) await this.settle(page);
    const file = `${String(this.steps.length + 1).padStart(2, '0')}-${slug}.png`;
    await page.screenshot({ path: join(this.out, file) });
    const { pathname, search } = new URL(page.url());
    const boxes = {};
    for (const [key, locator] of Object.entries(targets)) boxes[key] = await Shots.box(locator);
    this.steps.push({ file, url: this.host + pathname + search, title, action: action ?? { kind: 'none' }, targets: boxes });
    return file;
  }

  async finish() {
    const manifest = {
      feature: this.feature,
      viewport: VIEWPORT,
      dpr: DPR,
      scheme: this.scheme,
      commit: this.commit,
      test: this.test,
      steps: this.steps,
    };
    await Bun.write(join(this.out, 'manifest.json'), JSON.stringify(manifest, null, 2));
    await this.context?.close();
    await this.browser?.close();
  }
}
