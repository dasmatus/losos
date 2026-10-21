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
// Steps, in order: `click` / `waitFor` ({role, name}, {text} or
// {selector}), `fill` ({label, value}), `upload` ({selector, file}),
// `press` (a key), `waitText` (exact text), `sleep` (milliseconds) and
// `goto` (a path). A shot fails the run when a step does or the page throws:
// a picture of a half-loaded page is worse than none.

import { mkdirSync, readFileSync } from 'node:fs';
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
  else if (s.fill) await page.getByLabel(s.fill.label, { exact: true }).fill(s.fill.value);
  else if (s.upload) await page.locator(s.upload.selector).setInputFiles(s.upload.file);
  else if (s.press) await page.keyboard.press(s.press);
  else if (s.waitText) await page.getByText(s.waitText, { exact: true }).first().waitFor({ timeout: 60000 });
  else if (s.sleep) await page.waitForTimeout(s.sleep);
  else if (s.goto) await page.goto(base + s.goto, { waitUntil: 'networkidle', timeout: 120000 });
  else throw new Error(`unknown step ${JSON.stringify(s)}`);
}

// The full Chromium in its new headless mode: nixpkgs' bundle has no
// separate headless shell.
const browser = await chromium.launch({ channel: 'chromium', args: ['--disable-gpu'] });
try {
  const context = await browser.newContext({
    viewport: { width: plan.width ?? 1280, height: plan.height ?? 800 },
    ignoreHTTPSErrors: true,
    locale: plan.locale ?? 'en-US',
    colorScheme: plan.colorScheme ?? 'light',
    reducedMotion: 'reduce',
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
    if (errors.length > 0) throw new Error(`${shot.name}: the page threw: ${errors.join('; ')}`);
    const path = join(args.out, `${shot.name}.png`);
    await page.screenshot({ path, fullPage: shot.fullPage ?? false });
    console.log(`captured ${path}`);
    await page.close();
  }
} finally {
  await browser.close();
}
