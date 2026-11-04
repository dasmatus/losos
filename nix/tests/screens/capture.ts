// Screenshots of a real box, taken by a browser on another VM.
//
// Any VM test can use it: give a node `nix/tests/screens/default.nix`'s
// `browser` module, put a plan on it and run
//
//   losos-capture --base https://192.168.1.1 --plan /tmp/plan.json \
//       --out /tmp/shots [--user NAME --password WORD] [--proxy socks5://HOST:PORT]
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
// as against `colorScheme`, the browser's, which Auto follows. A variant can
// also set `dpr` (device pixels per CSS pixel), `timezoneId`, and
// `hideCaret`, which makes the text caret transparent. `"script": false`, on
// the plan or a variant, is a browser with JavaScript off: the shots are
// what such a browser is served (the pages under /basic/), and the policy's
// refusals are read from the console, where the browser reports them.
//
// A shot with `"film": {"title", "action", "targets": {key: locator}}` is
// also listed in film.json beside the pictures: its file, path, title, the
// action that leads to the next shot, and the CSS px box of each target on
// this picture, for a film cut from the stills.
//
// A shot with `"strip": [ms, ...]` also gets its motion frame by frame: the
// same steps in a fresh browser with motion allowed (stills take theirs under
// reduced motion, so nothing is caught halfway), with every animation frozen
// at its start through the DevTools protocol, then moved to each time in the
// list and photographed, `<shot>-<ms>ms.png`. A recording would lose the
// frames a slow emulated machine never paints; a frozen animation waits.
// `"clip": {"fps": 60, "ms": 300}` takes the same frames at an even rate
// instead, `<shot>-clip-NNN.png`, for a video at that rate.
//
// Every run also writes report.json beside the pictures: per picture, the
// page's width against the window's (a page wider than a phone scrolls
// sideways) and the Content-Security-Policy violations the page raised.
//
// Steps, in order: `click` / `waitFor` / `gone` / `absent` ({role, name},
// {text} or {selector}; `gone` waits for it to leave, `absent` fails if it
// is there now), `fill` ({label, value}), `select` ({label, value}, an
// option of a native select), `upload` ({selector, file}),
// `press` (a key), `type` (text typed at the focus), `hold` (the mouse pressed on a control and left
// down), `waitText` (exact text), `sleep` (milliseconds), `goto` (a path),
// `scroll` ("end": the page scrolled to its foot; a locator: that brought
// into view), `stay` (the page's own reloads answered 204, so it holds
// still for its picture)
// and, for a strip, `freeze` (what comes before it plays at full speed,
// only what comes after is taken frame by frame). A shot fails the run when a step does or the page throws:
// a picture of a half-loaded page is worse than none.

const join = (...parts) => parts.join('/');

// Playwright's driver from nixpkgs, which the wrapper names.
const { chromium } = await import(Bun.env.PLAYWRIGHT_CORE ?? 'playwright-core');

/* `--name value` or `--name=value`, for each of `names`; anything else is refused. */
function flags(names) {
  const out = {};
  const argv = Bun.argv.slice(2);
  for (let i = 0; i < argv.length; i++) {
    const m = /^--([^=]+)(?:=(.*))?$/.exec(argv[i]);
    if (!m || !names.includes(m[1])) throw new Error(`unknown argument ${argv[i]}`);
    out[m[1]] = m[2] ?? argv[++i];
  }
  return out;
}

// --proxy: a browser outside the test network (the driver's own) reaches
// the box through a proxy on one of the VMs.
const args = flags(['base', 'plan', 'out', 'user', 'password', 'proxy']);
for (const need of ['base', 'plan', 'out']) {
  if (!args[need]) {
    // Thrown: an uncaught error is what makes Bun exit non-zero.
    throw new Error(`losos-capture: --${need} is required`);
  }
}

const plan = await Bun.file(args.plan).json();
const base = args.base.replace(/\/+$/, '');
Bun.spawnSync(['mkdir', '-p', args.out]);

function locate(page, what) {
  if (what.role) return page.getByRole(what.role, { name: what.name, exact: true }).first();
  if (what.text) return page.getByText(what.text, { exact: true }).first();
  if (what.selector) return page.locator(what.selector).first();
  throw new Error(`cannot locate ${JSON.stringify(what)}`);
}

async function step(page, s, { force = false } = {}) {
  if (s.click) await locate(page, s.click).click({ force });
  else if (s.waitFor) await locate(page, s.waitFor).waitFor({ timeout: s.timeout ?? 60000 });
  else if (s.gone) await locate(page, s.gone).waitFor({ state: 'detached', timeout: s.timeout ?? 60000 });
  else if (s.absent) {
    if ((await locate(page, s.absent).count()) > 0) throw new Error(`present: ${JSON.stringify(s.absent)}`);
  }
  else if (s.fill) await page.getByLabel(s.fill.label, { exact: true }).fill(s.fill.value);
  else if (s.select) await page.getByLabel(s.select.label, { exact: true }).selectOption(s.select.value);
  else if (s.upload) await page.locator(s.upload.selector).setInputFiles(s.upload.file);
  else if (s.press) await page.keyboard.press(s.press);
  else if (s.type !== undefined) await page.keyboard.type(s.type);
  else if (s.waitText) await page.getByText(s.waitText, { exact: true }).first().waitFor({ timeout: 60000 });
  else if (s.sleep) await page.waitForTimeout(s.sleep);
  else if (s.goto) await page.goto(base + s.goto, { waitUntil: 'networkidle', timeout: 120000 });
  else if (s.scroll === 'end') await page.evaluate(() => window.scrollTo(0, document.documentElement.scrollHeight));
  else if (s.scroll) await locate(page, s.scroll).scrollIntoViewIfNeeded();
  else if (s.hold) {
    // The mouse pressed on it and left down, for what a held control shows.
    await locate(page, s.hold).hover({ force });
    await page.mouse.down();
  }
  else if (s.stay) {
    // Keep the page as it is: a page that reloads itself (a refresh the
    // box sends while a rebuild runs) is answered 204, which leaves the
    // document in place, so a slow screenshot is not cut short.
    await page.route('**/*', (r) =>
      r.request().isNavigationRequest() && r.request().frame() === page.mainFrame()
        ? r.fulfill({ status: 204 })
        : r.continue(),
    );
  }
  else if (s.freeze) {
    // Only marks where a strip freezes the motion; nothing to do here.
  }
  else throw new Error(`unknown step ${JSON.stringify(s)}`);
}

async function strip(variant, width, shot, name, storageState) {
  const context = await browser.newContext({
    viewport: { width, height: variant.height ?? plan.height ?? 800 },
    ignoreHTTPSErrors: true,
    locale: variant.locale ?? plan.locale ?? 'en-US',
    colorScheme: variant.colorScheme ?? plan.colorScheme ?? 'light',
    reducedMotion: 'no-preference',
    javaScriptEnabled: (variant.script ?? plan.script) !== false,
    deviceScaleFactor: variant.dpr ?? 1,
    ...(variant.timezoneId ? { timezoneId: variant.timezoneId } : {}),
    // The session and the page's storage of the context that took the
    // stills, so the strip opens on the same signed-in page.
    storageState,
  });
  const page = await context.newPage();
  try {
    await page.goto(base + shot.path, { waitUntil: 'networkidle', timeout: 120000 });
    if (shot.waitFor) await locate(page, shot.waitFor).waitFor({ timeout: 120000 });
    // Steps before a `freeze` step play at full speed (opening the dialog
    // whose closing is the subject, say); the rest are frozen.
    const all = shot.steps ?? [];
    const cut = all.findIndex((s) => s.freeze);
    const frozen = cut < 0 ? all : all.slice(cut + 1);
    for (const s of cut < 0 ? [] : all.slice(0, cut)) await step(page, s);
    await page.evaluate(() => document.fonts.ready);
    await page.waitForLoadState('networkidle');
    // The page's own entrance has to be over before anything is frozen, or
    // it lands in the strip; a slow emulated box can still be fading the
    // content in seconds after load. Looping animations (a skeleton's
    // pulse) never end and are left out.
    await page.waitForFunction(
      () =>
        document
          .getAnimations()
          .every((a) => a.playState !== 'running' || a.effect?.getComputedTiming().iterations === Infinity),
      null,
      { timeout: 120000 },
    );
    await page.waitForTimeout(500);
    // A forced click skips Playwright's checks, so wait for the first
    // control to be there and enabled: a switch clicked while its setting
    // is still loading does nothing. (Later ones may appear only after it.)
    const first = frozen[0]?.click ?? frozen[0]?.hold;
    if (first) {
      const target = locate(page, first);
      await target.waitFor({ timeout: 60000 });
      await page.waitForFunction(
        (el) => !el.matches(':disabled, [aria-disabled=true], [data-disabled]'),
        await target.elementHandle(),
        { timeout: 60000 },
      );
    }
    const cdp = await context.newCDPSession(page);
    let started = [];
    cdp.on('Animation.animationStarted', ({ animation }) => started.push(animation.id));
    await cdp.send('Animation.enable');
    await cdp.send('Animation.setPlaybackRate', { playbackRate: 0 });
    // Only what the steps start: the page's own entrance has played. The
    // page keeps its own list too, for animations the protocol never
    // reports as started (a dialog's way out, on a slow emulated machine).
    started = [];
    await page.evaluate(() => {
      window.__lososBeforeStrip = new Set(document.getAnimations());
    });
    for (const s of frozen) {
      // The stills already waited for each control to be ready, and a frozen
      // press would keep Playwright waiting for it to settle.
      await step(page, s, { force: true });
    }
    await page.waitForTimeout(1500);
    // One at a time: an animation a later one replaced (a hover's, say) is
    // gone, and the protocol refuses the whole list for one missing id.
    let ids = [...started];
    const times = shot.clip
      ? Array.from({ length: Math.floor((shot.clip.ms * shot.clip.fps) / 1000) + 1 }, (_, i) => (i * 1000) / shot.clip.fps)
      : shot.strip;
    for (const [frame, ms] of times.entries()) {
      const live = [];
      for (const id of ids) {
        try {
          await cdp.send('Animation.seekAnimations', { animations: [id], currentTime: ms });
          live.push(id);
        } catch {
          // Finished or cancelled since.
        }
      }
      ids = live;
      const fromPage = await page.evaluate((time) => {
        const fresh = document.getAnimations().filter((a) => !window.__lososBeforeStrip.has(a));
        for (const a of fresh) a.currentTime = time;
        return fresh.length;
      }, ms);
      if (frame === 0) console.log(`strip ${name}: the page lists ${fromPage} new animations`);
      const file = shot.clip ? `clip-${String(frame).padStart(3, '0')}` : `${String(ms).padStart(3, '0')}ms`;
      const path = join(args.out, `${name}-${file}.png`);
      await page.screenshot({ path, timeout: 180000 });
    }
    console.log(`strip ${name}: ${ids.length} animations over ${times.length} frames`);
    // No motion is a finding too (the before picture of a change that adds
    // some), so the frames are kept and it is only said.
    if (ids.length === 0) console.log(`strip ${name}: the steps started no animation`);
  } catch (e) {
    console.error(`strip ${name} at ${page.url()}: ${e.message}`);
    await page.screenshot({ path: join(args.out, `${name}-strip-failed.png`), timeout: 180000 }).catch(() => {});
    throw e;
  } finally {
    await context.close();
  }
}

// The full Chromium in its new headless mode: nixpkgs' bundle has no
// separate headless shell.
const browser = await chromium.launch({
  channel: 'chromium',
  args: ['--disable-gpu'],
  ...(args.proxy ? { proxy: { server: args.proxy } } : {}),
});
const variants = plan.variants ?? [{ suffix: null }];
const report = [];
const film = [];
try {
  for (const variant of variants) {
    const width = variant.width ?? plan.width ?? 1280;
    const script = (variant.script ?? plan.script) !== false;
    const context = await browser.newContext({
      viewport: { width, height: variant.height ?? plan.height ?? 800 },
      ignoreHTTPSErrors: true,
      locale: variant.locale ?? plan.locale ?? 'en-US',
      colorScheme: variant.colorScheme ?? plan.colorScheme ?? 'light',
      reducedMotion: 'reduce',
      javaScriptEnabled: script,
      deviceScaleFactor: variant.dpr ?? 1,
      ...(variant.timezoneId ? { timezoneId: variant.timezoneId } : {}),
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
      // What the browser has kept for the page (a shot's `storage`, key to
      // string), as an owner's own browser would have it: the Overview's
      // board, for one, lives only there.
      if (shot.storage) {
        await page.addInitScript((items) => {
          try {
            for (const [k, v] of Object.entries(items)) localStorage.setItem(k, v);
          } catch {
            // about:blank has no storage.
          }
        }, shot.storage);
      }
      const errors = [];
      page.on('pageerror', (e) => errors.push(String(e)));
      // Without a script the page cannot count its own refusals.
      const refused = [];
      if (!script) {
        page.on('console', (m) => {
          if (/Content Security Policy/.test(m.text())) refused.push(m.text());
        });
      }
      await page.goto(base + shot.path, { waitUntil: 'networkidle', timeout: 120000 });
      if (shot.waitFor) await locate(page, shot.waitFor).waitFor({ timeout: 120000 });
      for (const s of shot.steps ?? []) await step(page, s);
      // A CSSOM write, which the page's style-src 'self' allows; caret-color
      // inherits, so the root's reaches every field.
      if (variant.hideCaret) await page.evaluate(() => document.documentElement.style.setProperty('caret-color', 'transparent'));
      // Fonts and pictures, then a moment for the layout to settle.
      await page.evaluate(() => document.fonts.ready);
      // A `live` page keeps a connection busy for as long as it is open (an
      // editor in a frame), so the network never goes quiet; its `settle`
      // stands in.
      if (!shot.live) await page.waitForLoadState('networkidle');
      await page.waitForTimeout(shot.settle ?? 500);
      if (errors.length > 0) throw new Error(`${name}: the page threw: ${errors.join('; ')}`);
      const path = join(args.out, `${name}.png`);
      // Under TCG a busy page can take more than Playwright's 30 s to paint.
      await page.screenshot({ path, fullPage: shot.fullPage ?? false, timeout: 180000 });
      const seen = await page.evaluate(() => ({
        pageWidth: document.documentElement.scrollWidth,
        csp: window.__lososCsp ?? [],
      }));
      if (!script) seen.csp = refused;
      report.push({ name, path: shot.path, width, ...seen });
      if (shot.film) {
        const targets = {};
        for (const [key, what] of Object.entries(shot.film.targets ?? {})) {
          const box = await locate(page, what).boundingBox();
          if (box === null) throw new Error(`${name}: film target ${key} is not on the page`);
          targets[key] = Object.fromEntries(Object.entries(box).map(([k, v]) => [k, Math.round(v)]));
        }
        film.push({ file: `${name}.png`, path: page.url().slice(base.length) || '/', title: shot.film.title, action: shot.film.action ?? { kind: 'none' }, targets });
      }
      console.log(`captured ${path}${seen.pageWidth > width ? ` (page is ${seen.pageWidth}px wide)` : ''}`);
      await page.close();
    }
    const storageState = await context.storageState();
    await context.close();
    for (const shot of plan.shots.filter((s) => s.strip || s.clip)) {
      const name = variant.suffix ? `${shot.name}-${variant.suffix}` : shot.name;
      await strip(variant, width, shot, name, storageState);
    }
  }
} finally {
  await Bun.write(join(args.out, 'report.json'), `${JSON.stringify(report, null, 2)}\n`);
  if (film.length > 0) await Bun.write(join(args.out, 'film.json'), `${JSON.stringify(film, null, 2)}\n`);
  await browser.close();
}
