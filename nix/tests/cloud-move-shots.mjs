// The move from Nextcloud to LosOS Cloud in pictures: the same box and the
// same account, before and after the nightly switch.
// nix/tests/cloud-from-nextcloud.nix runs it on the box itself, twice:
//
//   node cloud-move-shots.mjs nextcloud OUT LOGIN PASSWORD
//   node cloud-move-shots.mjs cloud OUT NAME PASSWORD ADDRESS
//
// The first signs in with Nextcloud's own form at nextcloud.test and takes
// its Files and Deleted files pages; the second signs in through LosOS Login
// at mattbox.local (resolved to ADDRESS) and takes the same two places in
// LosOS Cloud. One window for
// both: 1280 x 900 at twice the pixels, dark, en-US, reduced motion.
//
// Playwright comes from PLAYWRIGHT_CORE (a path to playwright-core, as
// nixpkgs' playwright-driver is) or the `playwright-core` package.

const join = (...parts) => parts.join("/");

const { chromium } = await import(Bun.env.PLAYWRIGHT_CORE ?? "playwright-core");

const [which, out, login, password, lan] = Bun.argv.slice(2);
if (!["nextcloud", "cloud"].includes(which) || !password || (which === "cloud" && !lan)) {
  // Thrown: an uncaught error is what makes Bun exit non-zero.
  throw new Error("usage: cloud-move-shots.mjs nextcloud|cloud OUT LOGIN PASSWORD [ADDRESS]");
}
Bun.spawnSync(["mkdir", "-p", out]);

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
// Nextcloud's first page load under emulation takes minutes, not seconds.
page.setDefaultTimeout(600_000);
page.setDefaultNavigationTimeout(600_000);
// LosOS Cloud's page may not throw; Nextcloud's is shown as it was.
const errors = [];
if (which === "cloud") page.on("pageerror", (e) => errors.push(String(e)));

async function take(file) {
  await page.waitForLoadState("networkidle");
  await page.evaluate(() => document.fonts.ready.then(() => true));
  await page.waitForTimeout(800);
  await page.screenshot({ path: join(out, file), timeout: 600_000 });
  console.log(file);
}

async function walk() {
  if (which === "nextcloud") {
    const base = "http://nextcloud.test";
    await page.goto(`${base}/index.php/login`);
    await page.locator("input[name=user]").fill(login);
    await page.locator("input[name=password]").fill(password);
    await page.keyboard.press("Enter");
    await page.waitForURL((u) => !u.pathname.includes("/login"));
    await page.goto(`${base}/index.php/apps/files/`);
    await page.locator('[data-cy-files-list-row-name="Documents"]').waitFor();
    await take("files.png");
    await page.getByRole("link", { name: "Deleted files" }).click();
    // A trashed file's row is named after the file plus when it was deleted.
    await page.locator('[data-cy-files-list-row-name^="old notes"]').first().waitFor();
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
    await page.getByRole("link", { name: "Documents", exact: true }).waitFor();
    await take("files.png");
    await page.goto(`${base}/cloud/trash`);
    await page.locator("[data-trash]").getByText("old notes", { exact: false }).first().waitFor();
    await take("trash.png");
  }
}

try {
  await walk();
} catch (e) {
  // What the page showed when it gave up, beside the pictures it took.
  await page.screenshot({ path: join(out, "failed.png") }).catch(() => {});
  throw e;
} finally {
  await browser.close();
}
if (errors.length > 0) {
  // Thrown: an uncaught error is what makes Bun exit non-zero.
  throw new Error(`the page threw:\n${errors.join("\n")}`);
}
