// The owner's side of nix/tests/binance-onramp.nix, in Chromium.
//
// Signs in to mattbox.local through LosOS Login with a wallet made on this
// box, opens Settings, Market and finds the euro row of the USDC wallet.
//
//   --mode before  the row as it was: closed, then opened
//   --mode order   the row closed; opened with Binance's four documents not
//                  yet opened; each document opened and the terms accepted
//                  for 25 euros; the order made, with Binance's link
//   --mode done    after Binance's notification: the USDC sent, with the
//                  transaction
//
// Each state is saved light and dark under --out as pr/<mode>/<state>-<scheme>.png.
// `order` keeps the browser's storage in /tmp/onramp-state.json, so `done`
// is the same browser coming back to the page.
//
// Playwright is nixpkgs' playwright-driver, named by $PLAYWRIGHT_CORE.

const exists = (file) => Bun.file(file).exists();
const join = (...parts) => parts.join("/");
const dirname = (file) => file.slice(0, file.lastIndexOf("/"));

const { chromium } = await import(join(Bun.env.PLAYWRIGHT_CORE, "index.mjs"));

const BASE = "https://mattbox.local";
const PHRASE = "test test test test test test test test test test test junk";
const ADDRESS = "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266";
const WALLET_PASSWORD = "right horse battery";
const VIEWPORT = { width: 1280, height: 900 };
const STATE = "/tmp/onramp-state.json";
const DOCUMENTS = ["Terms of Use", "Product Terms", "Privacy Notice", "Risk Warning"];

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

const args = flags(["mode", "out", "user", "password"]);

// The wallet made on this box, sealed into this browser the way
// wallet/hot.ts seals it (nix/tests/box-wallet-name.mjs does the same).
async function seal({ phrase, address, password }) {
  const enc = new TextEncoder();
  const salt = crypto.getRandomValues(new Uint8Array(16));
  const iv = crypto.getRandomValues(new Uint8Array(12));
  const base = await crypto.subtle.importKey("raw", enc.encode(password), "PBKDF2", false, ["deriveKey"]);
  const key = await crypto.subtle.deriveKey(
    { name: "PBKDF2", salt, iterations: 210000, hash: "SHA-256" },
    base,
    { name: "AES-GCM", length: 256 },
    false,
    ["encrypt"],
  );
  const ct = new Uint8Array(await crypto.subtle.encrypt({ name: "AES-GCM", iv }, key, enc.encode(phrase)));
  const b64 = (b) => btoa(String.fromCharCode(...b));
  const db = await new Promise((resolve, reject) => {
    const req = indexedDB.open("losos-wallet", 1);
    req.onupgradeneeded = () => req.result.createObjectStore("key");
    req.onsuccess = () => resolve(req.result);
    req.onerror = () => reject(req.error);
  });
  await new Promise((resolve) => {
    const tx = db.transaction("key", "readwrite");
    tx.objectStore("key").put({ v: 1, salt: b64(salt), iv: b64(iv), ct: b64(ct), evm: address, made: true }, "phrase");
    tx.oncomplete = resolve;
  });
  db.close();
}

async function settle(page) {
  await page.waitForLoadState("networkidle");
  await page.evaluate(() => document.fonts.ready.then(() => true));
  await page.evaluate(() => document.documentElement.style.setProperty("caret-color", "transparent"));
  await page.waitForTimeout(400);
}

// The row in the middle of the window, light and dark.
async function shoot(page, row, state) {
  await row.scrollIntoViewIfNeeded();
  await row.evaluate((el) => el.scrollIntoView({ block: "center" }));
  for (const colorScheme of ["light", "dark"]) {
    await page.emulateMedia({ colorScheme });
    await settle(page);
    const full = join(args.out, `pr/${args.mode}/${state}-${colorScheme}.png`);
    Bun.spawnSync(["mkdir", "-p", dirname(full)]);
    await page.screenshot({ path: full, timeout: 300_000 });
  }
  await page.emulateMedia({ colorScheme: "light" });
}

async function open(browser) {
  const ctx = await browser.newContext({
    viewport: VIEWPORT,
    deviceScaleFactor: 2,
    locale: "en-US",
    timezoneId: "Europe/Bratislava",
    reducedMotion: "reduce",
    ignoreHTTPSErrors: true,
    ...(args.mode === "done" && await exists(STATE) ? { storageState: STATE } : {}),
  });
  ctx.setDefaultTimeout(120_000);
  // Binance's pages, which the VM cannot reach: a stand-in tab for each.
  await ctx.route(/^https:\/\/([a-z]+\.)?binance\.com\//, (route) =>
    route.fulfill({ status: 200, contentType: "text/html", body: "<title>Binance</title><p>Binance</p>" }),
  );
  const reply = await ctx.request.post(`${BASE}/auth/api/firstfactor`, {
    data: { username: args.user, password: args.password, keepMeLoggedIn: false },
  });
  if (!reply.ok()) throw new Error(`sign-in answered ${reply.status()}: ${await reply.text()}`);
  const page = await ctx.newPage();
  const errors = [];
  page.on("pageerror", (e) => errors.push(String(e)));
  await page.goto(`${BASE}/`);
  await page.waitForLoadState("networkidle");
  await page.evaluate(seal, { phrase: PHRASE, address: ADDRESS, password: WALLET_PASSWORD });
  await page.goto(`${BASE}/settings/market`);
  const row = page.getByTestId("onramp-topup");
  await row.waitFor();
  await settle(page);
  return { ctx, page, row, errors };
}

const browser = await chromium.launch();
const { ctx, page, row, errors } = await open(browser);

if (args.mode === "before" || args.mode === "order") {
  await shoot(page, row, "wallet");
  await row.getByRole("button", { name: "Buy", exact: true }).click();
  await settle(page);
  await shoot(page, row, "documents");
}

if (args.mode === "order") {
  const accept = row.getByRole("checkbox");
  if (!(await accept.isDisabled())) throw new Error("the terms can be accepted before Binance's documents were opened");
  for (const name of DOCUMENTS) {
    const [tab] = await Promise.all([page.waitForEvent("popup"), row.getByRole("link", { name }).click()]);
    await tab.close();
  }
  await accept.check();
  await row.getByLabel("Amount in euros").fill("25");
  await settle(page);
  await shoot(page, row, "accepted");
  await row.getByRole("button", { name: "Create the order" }).click();
  const order = page.getByTestId("onramp-order");
  await order.getByText("Your order for 25 euros is ready. Open Binance to sign in and pay.").waitFor();
  const link = await order.getByRole("link", { name: "Open Binance" }).getAttribute("href");
  if (!link?.startsWith("https://app.binance.com/")) throw new Error(`not Binance's checkout: ${link}`);
  await shoot(page, row, "order");
  await ctx.storageState({ path: STATE });
}

if (args.mode === "done") {
  const order = page.getByTestId("onramp-order");
  await order.getByText("Binance sent 24.61 USDC to this wallet on Base.").waitFor({ timeout: 180_000 });
  const tx = await order.getByRole("link", { name: "View the transaction" }).getAttribute("href");
  if (tx !== `https://basescan.org/tx/0x${"ab".repeat(32)}`) throw new Error(`wrong transaction link: ${tx}`);
  await shoot(page, row, "done");
}

if (errors.length > 0) throw new Error(`the page threw: ${errors.join("; ")}`);
await ctx.close();
await browser.close();
