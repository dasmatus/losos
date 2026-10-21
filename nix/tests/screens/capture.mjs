// Screenshots of a real box, taken by a browser on another VM.
//
// Any VM test can use it: give a node `nix/tests/screens/default.nix`'s
// `browser` module, put a plan on it and run
//
//   losos-capture --base https://192.168.1.1 --plan /tmp/plan.json \
//       --out /tmp/shots [--user NAME --password WORD]
//
// then `copy_from_machine("/tmp/shots", "")` puts the PNGs in the test's output.
// With a user, the browser signs in to LosOS Login first, the way the
// sign-in page does, so every shot is the admin UI as that person sees it.
//
// A plan is JSON:
//
//   {"width": 1280, "height": 800,
//    "shots": [{"name": "look", "path": "/settings/look",
//               "waitFor": {"role": "radiogroup", "name": "Background"},
//               "steps": [{"click": {"role": "radio", "name": "Tide"}},
//                         {"waitText": "Background set."}],
//               "fullPage": false}]}
//
// `variants` takes each shot more than once, in a fresh browser each time,
// named `<shot>-<suffix>`; without it there is one pass at the plan's own
// width, height and colour scheme:
//
//   "variants": [{"suffix": "light-desktop", "colorScheme": "light"},
//                {"suffix": "dark-phone", "colorScheme": "dark",
//                 "width": 390, "height": 844},
//                {"suffix": "picked-dark", "theme": "dark"}]
//
// `theme` is the choice made on the page's own theme toggle (Light or Dark),
// as against `colorScheme`, the browser's, which Auto follows.
//
// A shot with `"clip": true` is also filmed: the same steps in a fresh
// browser with motion allowed (stills take theirs under reduced motion, so
// nothing is caught halfway), recorded to `<shot>.webm`. Animations run at
// `clipRate` of their speed (default 0.1) through the DevTools protocol, so a
// slow emulated machine still records every frame of a 150 ms transition;
// play the file back at 1 / clipRate to see real time.
//
// Every run also writes report.json beside the pictures: per picture, the
// page's width against the window's (a page wider than a phone scrolls
// sideways) and the Content-Security-Policy violations the page raised.
//
// Steps, in order: `click` / `waitFor` / `gone` / `absent` ({role, name},
// {text} or {selector}; `gone` waits for it to leave, `absent` fails if it
// is there now), `fill` ({label, value}), `upload` ({selector, file}),
// `press` (a key), `waitText` (exact text), `sleep` (milliseconds) and
// `goto` (a path). A shot fails the run when a step does or the page throws:
// a picture of a half-loaded page is worse than none.

import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { join } from 'node:path';
import { parseArgs } from 'node:util';

// Through require, which honours NODE_PATH; the wrapper points it at
// Playwright's driver from nixpkgs.
const { chromium } = createRequire(import.meta.url)('playwright-core');

const { values: args } = parseArgs({
  options: {
    base: { type: 'string' },
    plan: { type: 'string' },
    out: { type: 'string' },
    user: { type: 'string' },
    password: { type: 'string' },
  },
});
for (const need of ['base', 'plan', 'out']) {
  if (!args[need]) {
    console.error(`losos-capture: --${need} is required`);
    process.exit(2);
  }
}

const plan = JSON.parse(readFileSync(args.plan, 'utf8'));
const base = args.base.replace(/\/+$/, '');
mkdirSync(args.out, { recursive: true });

function locate(page, what) {
  if (what.role) return page.getByRole(what.role, { name: what.name, exact: true }).first();
  if (what.text) return page.getByText(what.text, { exact: true }).first();
  if (what.selector) return page.locator(what.selector).first();
  throw new Error(`cannot locate ${JSON.stringify(what)}`);
}

async function step(page, s) {
  if (s.click) await locate(page, s.click).click();
  else if (s.waitFor) await locate(page, s.waitFor).waitFor({ timeout: s.timeout ?? 60000 });
  else if (s.gone) await locate(page, s.gone).waitFor({ state: 'detached', timeout: s.timeout ?? 60000 });
  else if (s.absent) {
    if ((await locate(page, s.absent).count()) > 0) throw new Error(`present: ${JSON.stringify(s.absent)}`);
  }
  else if (s.fill) await page.getByLabel(s.fill.label, { exact: true }).fill(s.fill.value);
  else if (s.upload) await page.locator(s.upload.selector).setInputFiles(s.upload.file);
  else if (s.press) await page.keyboard.press(s.press);
  else if (s.waitText) await page.getByText(s.waitText, { exact: true }).first().waitFor({ timeout: 60000 });
  else if (s.sleep) await page.waitForTimeout(s.sleep);
  else if (s.goto) await page.goto(base + s.goto, { waitUntil: 'networkidle', timeout: 120000 });
  else throw new Error(`unknown step ${JSON.stringify(s)}`);
}

async function film(variant, width, shot, name, storageState) {
  const height = variant.height ?? plan.height ?? 800;
  const dir = join(args.out, `.video-${name}`);
  const context = await browser.newContext({
    viewport: { width, height },
    ignoreHTTPSErrors: true,
    locale: variant.locale ?? plan.locale ?? 'en-US',
    colorScheme: variant.colorScheme ?? plan.colorScheme ?? 'light',
    reducedMotion: 'no-preference',
    recordVideo: { dir, size: { width, height } },
    // The session and the page's storage of the context that took the
    // stills, so the clip opens on the same signed-in page.
    storageState,
  });
  const page = await context.newPage();
  try {
    await page.goto(base + shot.path, { waitUntil: 'networkidle', timeout: 120000 });
    if (shot.waitFor) await locate(page, shot.waitFor).waitFor({ timeout: 120000 });
    await page.evaluate(() => document.fonts.ready);
    await page.waitForTimeout(1000);
    const cdp = await context.newCDPSession(page);
    await cdp.send('Animation.enable');
    await cdp.send('Animation.setPlaybackRate', { playbackRate: plan.clipRate ?? 0.1 });
    for (const s of shot.steps ?? []) {
      await step(page, s);
      await page.waitForTimeout(shot.clipPause ?? 2500);
    }
  } catch (e) {
    // What the camera saw instead, for the log and the pictures.
    console.error(`filming ${name} at ${page.url()}: ${e.message}`);
    await page.screenshot({ path: join(args.out, `${name}-film-failed.png`), timeout: 180000 }).catch(() => {});
    await context.close();
    throw e;
  }
  const video = page.video();
  await page.close();
  await context.close();
  if (video) {
    const path = join(args.out, `${name}.webm`);
    await video.saveAs(path);
    console.log(`filmed ${path}`);
  }
}

// The full Chromium in its new headless mode: nixpkgs' bundle has no
// separate headless shell.
const browser = await chromium.launch({ channel: 'chromium', args: ['--disable-gpu'] });
const variants = plan.variants ?? [{ suffix: null }];
const report = [];
try {
  for (const variant of variants) {
    const width = variant.width ?? plan.width ?? 1280;
    const context = await browser.newContext({
      viewport: { width, height: variant.height ?? plan.height ?? 800 },
      ignoreHTTPSErrors: true,
      locale: variant.locale ?? plan.locale ?? 'en-US',
      colorScheme: variant.colorScheme ?? plan.colorScheme ?? 'light',
      reducedMotion: 'reduce',
    });
    // A theme picked on the page (lib/theme.ts), as against the browser's
    // own colour scheme, which is what Auto follows.
    if (variant.theme) {
      await context.addInitScript((theme) => {
        try {
          localStorage.setItem('losos-theme', theme);
        } catch {
          // about:blank has no storage.
        }
      }, variant.theme);
    }
    // The policy's refusals, counted in the page, since a refused style
    // shows nowhere else.
    await context.addInitScript(() => {
      window.__lososCsp = [];
      document.addEventListener('securitypolicyviolation', (e) => {
        window.__lososCsp.push(`${e.violatedDirective} ${e.blockedURI || 'inline'}`);
      });
    });
    if (args.user) {
      // LosOS Login's own API, as its sign-in page calls it: the session
      // cookie lands in this context.
      const r = await context.request.post(`${base}/auth/api/firstfactor`, {
        data: { username: args.user, password: args.password },
      });
      if (!r.ok()) throw new Error(`signing in as ${args.user}: ${r.status()} ${await r.text()}`);
    }
    for (const shot of plan.shots) {
      const name = variant.suffix ? `${shot.name}-${variant.suffix}` : shot.name;
      const page = await context.newPage();
      const errors = [];
      page.on('pageerror', (e) => errors.push(String(e)));
      await page.goto(base + shot.path, { waitUntil: 'networkidle', timeout: 120000 });
      if (shot.waitFor) await locate(page, shot.waitFor).waitFor({ timeout: 120000 });
      for (const s of shot.steps ?? []) await step(page, s);
      // Fonts and pictures, then a moment for the layout to settle.
      await page.evaluate(() => document.fonts.ready);
      await page.waitForLoadState('networkidle');
      await page.waitForTimeout(shot.settle ?? 500);
      if (errors.length > 0) throw new Error(`${name}: the page threw: ${errors.join('; ')}`);
      const path = join(args.out, `${name}.png`);
      // Under TCG a busy page can take more than Playwright's 30 s to paint.
      await page.screenshot({ path, fullPage: shot.fullPage ?? false, timeout: 180000 });
      const seen = await page.evaluate(() => ({
        pageWidth: document.documentElement.scrollWidth,
        csp: window.__lososCsp ?? [],
      }));
      report.push({ name, path: shot.path, width, ...seen });
      console.log(`captured ${path}${seen.pageWidth > width ? ` (page is ${seen.pageWidth}px wide)` : ''}`);
      await page.close();
    }
    const storageState = await context.storageState();
    await context.close();
    for (const shot of plan.shots.filter((s) => s.clip)) {
      const name = variant.suffix ? `${shot.name}-${variant.suffix}` : shot.name;
      await film(variant, width, shot, name, storageState);
    }
  }
} finally {
  writeFileSync(join(args.out, 'report.json'), `${JSON.stringify(report, null, 2)}\n`);
  await browser.close();
}
