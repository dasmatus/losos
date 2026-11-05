// The owner's browser on a box: sign in through LosOS Login, upload one text
// file in LosOS Cloud's Files page, and picture the list with the file in it.
// nix/demo/live-edge/proof.nix runs it in a VM against the box's own nginx;
// run it again at a real box the same way:
//
//   bun proof.mjs BASE NAME PASSWORD FILE OUT.png
//
// Playwright comes from PLAYWRIGHT_CORE (a path to playwright-core).
const { chromium } = await import(Bun.env.PLAYWRIGHT_CORE ?? "playwright-core");

const [base, name, password, file, out] = Bun.argv.slice(2);
if (!out) throw new Error("usage: proof.mjs BASE NAME PASSWORD FILE OUT.png");

const browser = await chromium.launch();
const context = await browser.newContext({
  viewport: { width: 1280, height: 800 },
  deviceScaleFactor: 1,
  colorScheme: "dark",
  locale: "en-US",
  reducedMotion: "reduce",
  // The box's certificate is its own; a real owner trusts it at first run.
  ignoreHTTPSErrors: true,
});
const page = await context.newPage();
// Emulated without KVM, every step is slow.
page.setDefaultTimeout(600_000);
page.setDefaultNavigationTimeout(600_000);

await page.goto(`${base}/auth/`);
const status = await page.evaluate(
  async ([username, pass, targetURL]) => {
    const r = await fetch("/auth/api/firstfactor", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ username, password: pass, keepMeLoggedIn: false, targetURL }),
    });
    return r.status;
  },
  [name, password, `${base}/cloud/`],
);
if (status !== 200) throw new Error(`sign-in refused: ${status}`);

const leaf = file.split("/").pop();
try {
  await page.goto(`${base}/cloud/files`);
  // A new box has no files, and the list (`[data-files]`) is drawn only
  // once there is one: the Upload control is what shows the page is up.
  await page.locator("input[data-upload]").waitFor({ state: "attached", timeout: 300_000 });
  await page.locator("input[data-upload]").setInputFiles(file);
  await page.waitForSelector(`[data-entry="${leaf}"]`, { timeout: 300_000 });
  // The upload's progress card leaves the screen before the picture.
  await page.waitForTimeout(3000);
  await page.screenshot({ path: out });
  console.log(JSON.stringify({ uploaded: leaf, url: page.url() }));
} catch (err) {
  // What the page looked like when it stopped, for whoever reads the log.
  console.error(`proof failed at ${page.url()}: ${err.message}`);
  console.error((await page.locator("body").innerText().catch(() => "")).slice(0, 1500));
  await page.screenshot({ path: out }).catch(() => {});
  await browser.close();
  // Thrown: an uncaught error is what makes Bun exit non-zero.
  throw err;
}
await browser.close();
