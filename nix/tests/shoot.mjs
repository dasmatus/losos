// Takes screenshots of a booted box for nix/tests/screens.nix.
//
// Runs on the client VM with Playwright's Chromium (playwright-core from
// nixpkgs, passed as PLAYWRIGHT_CORE). Reads a plan (JSON) and writes one PNG
// per shot plus manifest.json into the output folder:
//
//   {
//     "viewport": {"width": 1280, "height": 900}, "dpr": 2,
//     "colorScheme": "dark", "locale": "en-US", "timezone": "Europe/Bratislava",
//     "shots": [
//       {"file": "01-apps.png", "path": "/settings/apps",
//        "waitFor": "text=Apps", "title": "the Apps pane",
//        "steps": [{"kind": "click", "target": "[data-testid=accordion-git] button"}],
//        "targets": {"tile": "text=LosOS Git"}}
//     ]
//   }
//
// `steps` run before the shot (click, type, press, hover, scroll, wait);
// `targets` are measured on the finished page and land in the manifest as
// CSS px boxes, for a film that points at them.

const { chromium } = await import(Bun.env.PLAYWRIGHT_CORE);

const [planFile, outDir, origin, user, password] = Bun.argv.slice(2);
const plan = (await Bun.file(planFile).json());
const HIDE = "*, *::before, *::after { caret-color: transparent !important; }";

async function settle(page) {
  await page.waitForLoadState("networkidle");
  await page.evaluate(() => document.fonts.ready.then(() => true));
  // Spinners and skeletons announce themselves; wait them out.
  await page.waitForFunction(
    () => !document.querySelector("[role=status][aria-busy=true], .animate-spin, [data-skeleton]"),
    null,
    { timeout: 60_000 },
  );
  await page.waitForTimeout(500);
}

async function runStep(page, step) {
  const at = () => page.locator(step.target).first();
  switch (step.kind) {
    case "click":
      await at().click();
      break;
    case "type":
      await at().fill(step.text);
      break;
    case "press":
      await page.keyboard.press(step.key);
      break;
    case "hover":
      await at().hover();
      break;
    case "scroll":
      await at().scrollIntoViewIfNeeded();
      break;
    case "wait":
      await at().waitFor({ timeout: 60_000 });
      break;
    default:
      throw new Error(`unknown step kind ${step.kind}`);
  }
  await settle(page);
}

const viewport = plan.viewport ?? { width: 1280, height: 900 };
const dpr = plan.dpr ?? 2;
const manifest = { viewport, dpr, steps: [] };
const browser = await chromium.launch();
const context = await browser.newContext({
  viewport,
  deviceScaleFactor: dpr,
  colorScheme: plan.colorScheme ?? "dark",
  locale: plan.locale ?? "en-US",
  timezoneId: plan.timezone ?? "Europe/Bratislava",
  reducedMotion: "reduce",
  ignoreHTTPSErrors: true,
});
if (user) {
  // LosOS Login's own first-factor call: the session cookie lands in this
  // context, as it would after the sign-in page.
  const r = await context.request.post(`${origin}/auth/api/firstfactor`, {
    data: { username: user, password },
  });
  if (!r.ok()) throw new Error(`sign-in refused: ${r.status()} ${await r.text()}`);
}
const page = await context.newPage();
const failed = [];
page.on("response", (r) => {
  if (r.status() >= 500) failed.push(`${r.status()} ${r.url()}`);
});
const missed = [];
for (const shot of plan.shots) {
  try {
    await take(shot);
  } catch (e) {
    // One shot that cannot be taken does not cost the others.
    console.log(`missed ${shot.file}: ${e.message.split("\n")[0]}`);
    missed.push(shot.file);
  }
}
await browser.close();
await Bun.write(`${outDir}/manifest.json`, JSON.stringify(manifest, null, 2));
if (missed.length) {
  // Thrown: an uncaught error is what makes Bun exit non-zero.
  throw new Error(`missed: ${missed.join(", ")}`);
}

async function take(shot) {
  // Under TCG the first load of a pane can take longer than Playwright's 30 s.
  await page.goto(`${origin}${shot.path}`, { timeout: 180_000 });
  // A constructed sheet, since the admin page's CSP refuses <style>.
  await page.evaluate((css) => {
    const sheet = new CSSStyleSheet();
    sheet.replaceSync(css);
    document.adoptedStyleSheets = [...document.adoptedStyleSheets, sheet];
  }, HIDE);
  await settle(page);
  if (shot.waitFor) {
    await page.locator(shot.waitFor).first().waitFor({ timeout: 60_000 });
    await settle(page);
  }
  for (const step of shot.steps ?? []) await runStep(page, step);
  if (failed.length) {
    console.log(`warning: ${shot.file}: the box answered ${failed.join(", ")}`);
    failed.length = 0;
  }
  const targets = {};
  for (const [name, sel] of Object.entries(shot.targets ?? {})) {
    const box = await page.locator(sel).first().boundingBox();
    if (box) targets[name] = Object.fromEntries(Object.entries(box).map(([k, v]) => [k, Math.round(v)]));
  }
  await page.screenshot({ path: `${outDir}/${shot.file}` });
  manifest.steps.push({
    file: shot.file,
    url: shot.url ?? shot.path,
    title: shot.title ?? "",
    action: shot.action ?? { kind: "none" },
    targets,
  });
  console.log(`shot ${shot.file}`);
}
