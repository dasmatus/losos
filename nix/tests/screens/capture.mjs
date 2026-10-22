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
// Steps, in order: `click` / `waitFor` / `gone` / `absent` ({role, name},
// {text} or {selector}; `gone` waits for it to leave, `absent` fails if it
// is there now), `fill` ({label, value}), `select` ({label, value}, an
// option of a native select), `upload` ({selector, file}),
// `press` (a key), `waitText` (exact text), `sleep` (milliseconds) and
// `goto` (a path). A shot fails the run when a step does or the page throws:
// a picture of a half-loaded page is worse than none.
//
// A plan may also set `dpr` (device scale factor), `timezone` and
// `colorScheme`, and a shot `targets` ({name: what to locate}): their boxes,
// in CSS px, are written beside the picture as `<name>.json` with the page's
// path, for a film that points at them. With `manifest` ({feature, commit,
// test, host}) the run also writes `manifest.json` in the launch film's
// shape: each shot's `title` and `action` with its targets and address.
// With `clips` ({frames, fps}) as well, a shot whose action is a click on
// one of its targets also gets a clip of that click: the page's animations
// are held, the click is made and the page settles, then the animations are
// stepped one frame at a time, one picture each, under full motion, into
// `<name>.frames/`, listed in the manifest as `<name>.mp4` after the
// picture. The test turns the frames into that video.

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
  else if (s.select) await page.getByLabel(s.select.label, { exact: true }).selectOption(s.select.value);
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
    deviceScaleFactor: plan.dpr ?? 1,
    timezoneId: plan.timezone,
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
  const steps = [];
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
    if (shot.targets || plan.manifest) {
      const targets = {};
      for (const [key, what] of Object.entries(shot.targets ?? {})) {
        const box = await locate(page, what).boundingBox();
        if (box === null) throw new Error(`${shot.name}: target ${key} is not on screen`);
        targets[key] = { x: Math.round(box.x), y: Math.round(box.y), w: Math.round(box.width), h: Math.round(box.height) };
      }
      const where = new URL(page.url());
      const at = where.pathname + where.hash;
      writeFileSync(join(args.out, `${shot.name}.json`), JSON.stringify({ path: at, targets }));
      if (plan.manifest) {
        steps.push({
          file: `${shot.name}.png`,
          url: plan.manifest.host + at,
          title: shot.title ?? shot.name,
          action: shot.action ?? { kind: 'none' },
          targets,
        });
      }
    }
    // Under TCG a busy page can take more than Playwright's 30 s to paint.
    await page.screenshot({ path, fullPage: shot.fullPage ?? false, timeout: 180000 });
    console.log(`captured ${path}`);
    const click = shot.action?.kind === 'click' ? shot.targets?.[shot.action.target] : undefined;
    if (plan.clips && click) {
      const dir = join(args.out, `${shot.name}.frames`);
      mkdirSync(dir, { recursive: true });
      await page.emulateMedia({ reducedMotion: 'no-preference' });
      const cdp = await context.newCDPSession(page);
      await cdp.send('Animation.enable');
      await cdp.send('Animation.setPlaybackRate', { playbackRate: 0 });
      await locate(page, click).click();
      await page.waitForLoadState('networkidle');
      await page.evaluate(() => document.fonts.ready);
      await page.waitForTimeout(500);
      const fps = plan.clips.fps ?? 60;
      for (let i = 0; i < (plan.clips.frames ?? 30); i++) {
        // Every animation the click started (and any that were running)
        // at the same moment, i frames in.
        await page.evaluate((t) => {
          for (const a of document.getAnimations()) a.currentTime = t;
        }, (i * 1000) / fps);
        await page.screenshot({ path: join(dir, `${String(i).padStart(3, '0')}.png`), timeout: 180000 });
      }
      console.log(`captured ${dir}`);
      if (plan.manifest) {
        const after = new URL(page.url());
        steps.push({
          file: `${shot.name}.mp4`,
          url: plan.manifest.host + after.pathname + after.hash,
          title: `${shot.title ?? shot.name}: the click, at ${fps} frames a second`,
          action: { kind: 'none' },
          targets: {},
        });
      }
    }
    await page.close();
  }
  if (plan.manifest) {
    const { host: _, ...about } = plan.manifest;
    const manifest = {
      ...about,
      viewport: { width: plan.width ?? 1280, height: plan.height ?? 800 },
      dpr: plan.dpr ?? 1,
      steps,
    };
    writeFileSync(join(args.out, 'manifest.json'), JSON.stringify(manifest, null, 2) + '\n');
  }
} finally {
  await browser.close();
}
