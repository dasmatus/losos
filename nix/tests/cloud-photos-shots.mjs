// Photos in LosOS Cloud in pictures, for the pull request. Run on the box by
// nix/tests/cloud-photos.nix, after the pass has read every photo:
//
//   node cloud-photos-shots.mjs after|before OUT NAME PASSWORD ADDRESS
//
// Signs in through LosOS Login at mattbox.local (resolved to ADDRESS) and
// takes the Photos page and the viewer; `after` also takes a person, the
// photos by camera and by place, and the viewer's details. One window for
// all: 1280 x 900 at twice the pixels, dark, en-US, reduced motion.
//
// Playwright comes from PLAYWRIGHT_CORE (a path to playwright-core, as
// nixpkgs' playwright-driver is) or the `playwright-core` package.

import { mkdirSync } from "node:fs";
import { join } from "node:path";

const { chromium } = await import(process.env.PLAYWRIGHT_CORE ?? "playwright-core");

const [which, out, login, password, lan] = process.argv.slice(2);
if (!["after", "before"].includes(which) || !lan) {
  console.error("usage: cloud-photos-shots.mjs after|before OUT NAME PASSWORD ADDRESS");
  process.exit(2);
}
mkdirSync(out, { recursive: true });

const browser = await chromium.launch({ args: [`--host-resolver-rules=MAP mattbox.local ${lan}`] });
const context = await browser.newContext({
  viewport: { width: 1280, height: 900 },
  deviceScaleFactor: 2,
  colorScheme: "dark",
  locale: "en-US",
  timezoneId: "Europe/Bratislava",
  reducedMotion: "reduce",
  ignoreHTTPSErrors: true,
});
const page = await context.newPage();
page.setDefaultTimeout(300_000);
page.setDefaultNavigationTimeout(300_000);
const errors = [];
page.on("pageerror", (e) => errors.push(String(e)));
// The admin page's policy (style-src 'self') refuses anything a library
// injects; a refusal is a bug, not a look.
page.on("console", (m) => {
  if (m.type() === "error" && /Content Security Policy/i.test(m.text())) errors.push(m.text());
});

const base = "https://mattbox.local";

async function settle() {
  await page.waitForLoadState("networkidle");
  await page.evaluate(() => document.fonts.ready.then(() => true));
  // Every picture on screen has arrived.
  await page.waitForFunction(() => [...document.images].every((i) => i.complete), null, { timeout: 300_000 });
  await page.waitForTimeout(800);
}

async function take(file) {
  await settle();
  await page.screenshot({ path: join(out, file), timeout: 300_000 });
  console.log(file);
}

async function photos(query = "") {
  await page.goto(`${base}/cloud/photos${query}`);
  await page.locator("[data-photos] img").first().waitFor();
}

async function viewer(name) {
  await page.getByRole("button", { name, exact: true }).first().click();
  await page.locator("#cloud-viewer-title").waitFor();
}

async function walk() {
  await page.goto(`${base}/auth/`);
  const status = await page.evaluate(
    async ([username, password, targetURL]) => {
      const r = await fetch("/auth/api/firstfactor", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ username, password, keepMeLoggedIn: false, targetURL }),
      });
      return r.status;
    },
    [login, password, `${base}/cloud/`],
  );
  if (status !== 200) throw new Error(`sign-in refused: ${status}`);

  await photos();
  await take("photos.png");
  if (which === "after") {
    await page.locator("[data-photos-people] [data-person]").first().click();
    await page.locator("[data-person-name]").waitFor();
    await take("person.png");
    await photos("?by=camera");
    await take("by-camera.png");
    await photos("?by=place");
    await take("by-place.png");
    await photos();
  }
  await viewer("iss074e0365172.jpg");
  if (which === "after") {
    await page.locator("[data-photo-info]").click();
    await page.locator("[data-photo-details]").waitFor();
    await page.locator("[data-face-box]").first().waitFor();
    await page.locator("[data-photo-tags] summary").click();
  }
  await take("viewer.png");
}

try {
  await walk();
} catch (e) {
  await page.screenshot({ path: join(out, "failed.png") }).catch(() => {});
  throw e;
} finally {
  await browser.close();
}
if (errors.length > 0) {
  console.error(`the page threw or was refused:\n${errors.join("\n")}`);
  process.exit(1);
}
