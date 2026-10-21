// The move from Nextcloud to LosOS Cloud in pictures: the same box and the
// same account, before and after the nightly switch.
// nix/tests/cloud-from-nextcloud.nix runs it on the box itself, twice:
//
//   node cloud-move-shots.mjs nextcloud OUT LOGIN PASSWORD
//   node cloud-move-shots.mjs cloud OUT NAME PASSWORD LAN_ADDRESS
//
// The first signs in with Nextcloud's own form at nextcloud.test and takes
// its Files and Deleted files pages; the second signs in through LosOS Login
// at mattbox.local (the box's LAN address, which the admin routes' LAN guard
// lets in) and takes the same two places in LosOS Cloud. One window for
// both: 1280 x 900 at twice the pixels, dark, en-US, reduced motion.
//
// Playwright comes from PLAYWRIGHT_CORE (a path to playwright-core, as
// nixpkgs' playwright-driver is) or the `playwright-core` package.

import { mkdirSync } from "node:fs";
import { join } from "node:path";

const { chromium } = await import(process.env.PLAYWRIGHT_CORE ?? "playwright-core");

const [which, out, login, password, lan] = process.argv.slice(2);
if (!["nextcloud", "cloud"].includes(which) || !password || (which === "cloud" && !lan)) {
  console.error("usage: cloud-move-shots.mjs nextcloud|cloud OUT LOGIN PASSWORD [LAN_ADDRESS]");
  process.exit(2);
}
mkdirSync(out, { recursive: true });

const rules = ["MAP nextcloud.test 127.0.0.1", lan && `MAP mattbox.local ${lan}`].filter(Boolean);
const browser = await chromium.launch({ args: [`--host-resolver-rules=${rules.join(", ")}`] });
const context = await browser.newContext({
  viewport: { width: 1280, height: 900 },
  deviceScaleFactor: 2,
  colorScheme: "dark",
  locale: "en-US",
  timezoneId: "Europe/Bratislava",
  reducedMotion: "reduce",
  // The box's own certificate; the protocol checks in the test pin it.
  ignoreHTTPSErrors: true,
});
const page = await context.newPage();
// LosOS Cloud's page may not throw; Nextcloud's is shown as it was.
const errors = [];
if (which === "cloud") page.on("pageerror", (e) => errors.push(String(e)));

async function take(file) {
  await page.waitForLoadState("networkidle");
  await page.evaluate(() => document.fonts.ready.then(() => true));
  await page.waitForTimeout(800);
  await page.screenshot({ path: join(out, file) });
  console.log(file);
}

if (which === "nextcloud") {
  const base = "http://nextcloud.test";
  await page.goto(`${base}/index.php/login`);
  await page.locator("input[name=user]").fill(login);
  await page.locator("input[name=password]").fill(password);
  await page.keyboard.press("Enter");
  await page.waitForURL((u) => !u.pathname.includes("/login"), { timeout: 120_000 });
  await page.goto(`${base}/index.php/apps/files/`);
  await page.locator('[data-cy-files-list-row-name="Documents"]').waitFor({ timeout: 120_000 });
  await take("files.png");
  await page.goto(`${base}/index.php/apps/files/trashbin`);
  await page.getByText("old notes", { exact: false }).first().waitFor({ timeout: 120_000 });
  await take("trash.png");
} else {
  const base = "https://mattbox.local";
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
  await page.goto(`${base}/cloud/files`);
  await page.getByRole("link", { name: "Documents", exact: true }).waitFor({ timeout: 120_000 });
  await take("files.png");
  await page.goto(`${base}/cloud/trash`);
  await page.locator("[data-trash]").getByText("old notes", { exact: false }).first().waitFor({ timeout: 120_000 });
  await take("trash.png");
}

await browser.close();
if (errors.length > 0) {
  console.error(`the page threw:\n${errors.join("\n")}`);
  process.exit(1);
}
