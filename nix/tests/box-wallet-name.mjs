// The owner's side of nix/tests/box-wallet-name.nix, in Chromium.
//
// Signs in to mattbox.local through LosOS Login, opens Settings, Network and,
// in `after` mode, binds the wallet made on this box to the box's name: Bind
// wallet, the wallet's password typed one key at a time, Sign. In `found`
// mode, run once public DNS validates the name, it shows the pane saying
// wallets find it. Every state is saved as a PNG under --out:
//
//   pr/<mode>/network-{light,dark}.png      the Network pane, scrolled to the end
//   pr/after/network-bound-{light,dark}.png the same once the wallet is bound
//   pr/found/network-found-{light,dark}.png the same once wallets find the name
//   film/name-pay/NN-<slug>.png             each state the owner sees, in dark,
//   film/name-pay/manifest.json             with what happens next and where
//
// The wallet stands in for one the owner made on the Market pane earlier: the
// standard test phrase, sealed into this browser the way wallet/hot.ts seals
// it. Its first account is 0xf39f…2266.
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
const NAME = "mattbox.boxes.losos.dasmat.us";

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

// Every scrollable box to its end, the way a person scrolls to the last
// section. The page's own scroller is not always the document.
function scrollToEnd() {
  for (const el of [document.scrollingElement, ...document.querySelectorAll("*")]) {
    if (!el) continue;
    if (el !== document.scrollingElement && !/(auto|scroll)/.test(getComputedStyle(el).overflowY)) continue;
    if (el.scrollHeight > el.clientHeight) el.scrollTop = el.scrollHeight;
  }
}

async function settle(page) {
  await page.waitForLoadState("networkidle");
  await page.evaluate(() => document.fonts.ready.then(() => true));
  // The film draws its own pointer and caret. A CSSOM write, which the admin
  // page's style-src allows; an injected <style> it would refuse.
  await page.evaluate(() => document.documentElement.style.setProperty("caret-color", "transparent"));
  await page.locator("[data-toast]").first().waitFor({ state: "detached", timeout: 60_000 });
  await page.waitForTimeout(400);
}

async function save(page, path) {
  const full = join(args.out, path);
  Bun.spawnSync(["mkdir", "-p", dirname(full)]);
  await page.screenshot({ path: full });
}

const MANIFEST = () => join(args.out, "film/name-pay/manifest.json");
// A later mode adds to the steps an earlier one filmed.
const steps = await exists(MANIFEST()) ? (await Bun.file(MANIFEST()).json()).steps : [];
async function writeManifest() {
  await Bun.write(
    MANIFEST(),
    `${JSON.stringify({ feature: "name-pay", viewport: VIEWPORT, dpr: 2, commit: "unknown", test: "losos-box-wallet-name", steps }, null, 2)}\n`,
  );
}

async function film(page, slug, title, action, targets = {}) {
  const file = `${String(steps.length + 1).padStart(2, "0")}-${slug}.png`;
  await save(page, join("film/name-pay", file));
  const boxes = {};
  for (const [key, locator] of Object.entries(targets)) {
    const box = await locator.boundingBox();
    if (box) boxes[key] = { x: Math.round(box.x), y: Math.round(box.y), w: Math.round(box.width), h: Math.round(box.height) };
  }
  steps.push({ file, url: page.url().split("://")[1], title, action, targets: boxes });
}

async function context(browser, colorScheme) {
  const ctx = await browser.newContext({
    viewport: VIEWPORT,
    deviceScaleFactor: 2,
    colorScheme,
    locale: "en-US",
    timezoneId: "Europe/Bratislava",
    reducedMotion: "reduce",
    ignoreHTTPSErrors: true,
  });
  ctx.setDefaultTimeout(120_000);
  const reply = await ctx.request.post(`${BASE}/auth/api/firstfactor`, {
    data: { username: args.user, password: args.password, keepMeLoggedIn: false },
  });
  if (!reply.ok()) throw new Error(`sign-in answered ${reply.status()}: ${await reply.text()}`);
  return ctx;
}

async function openNetwork(page) {
  await page.goto(`${BASE}/settings/network`);
  await page.getByTestId("custom-domains").waitFor();
  // The edge's answer draws the last section; scroll once it is there.
  if (args.mode !== "before") await page.getByTestId("domain-wallet").waitFor();
  await settle(page);
  // Twice: a late layout change (a font, the edge's answer) can move the end.
  for (let i = 0; i < 2; i++) {
    await page.evaluate(scrollToEnd);
    await settle(page);
  }
}

async function prShots(browser, name) {
  for (const scheme of ["light", "dark"]) {
    const ctx = await context(browser, scheme);
    const page = await ctx.newPage();
    await openNetwork(page);
    await save(page, `pr/${args.mode}/${name}-${scheme}.png`);
    await ctx.close();
  }
}

async function bind(browser) {
  const ctx = await context(browser, "dark");
  const page = await ctx.newPage();
  // Seal on another page, so the Network pane below is a fresh load.
  await page.goto(`${BASE}/`);
  await page.waitForLoadState("networkidle");
  await page.evaluate(seal, { phrase: PHRASE, address: ADDRESS, password: WALLET_PASSWORD });
  await openNetwork(page);

  const section = page.getByTestId("domain-wallet");
  const button = section.getByTestId("wallet-bind");
  const before = await section.innerText();
  if (!before.includes(NAME)) throw new Error(`the section does not name ${NAME}: ${before}`);
  await film(page, "paid-by-name", "Settings, Network: Paid by name, no wallet bound yet",
    { kind: "click", target: "bind" }, { section, bind: button });

  await button.click();
  const field = section.getByLabel("Password of the wallet on this box");
  await field.waitFor();
  await settle(page);
  const sign = section.getByRole("button", { name: "Sign", exact: true });
  await film(page, "wallet-password", "The wallet made on this box asks for its password",
    { kind: "click", target: "password" }, { password: field, sign });

  await field.click();
  for (let i = 1; i <= WALLET_PASSWORD.length; i++) {
    await field.pressSequentially(WALLET_PASSWORD[i - 1]);
    await settle(page);
    const last = i === WALLET_PASSWORD.length;
    await film(page, `wallet-password-${String(i).padStart(2, "0")}`, "Typing the wallet's password",
      last ? { kind: "click", target: "sign" } : { kind: "type", target: "password" }, { password: field, sign });
  }

  await sign.click();
  const name = section.getByTestId("wallet-name").first();
  await name.waitFor({ timeout: 300_000 });
  // Until the edge's first look at public DNS the row says it is checking.
  await page.waitForFunction(
    () => document.querySelector('[data-testid="wallet-name"]')?.getAttribute("data-published") !== "checking",
    null,
    { timeout: 120_000 },
  );
  await settle(page);
  await page.evaluate(scrollToEnd);
  await settle(page);
  const after = await section.innerText();
  if (!after.includes(NAME) || !after.includes(ADDRESS.toLowerCase())) throw new Error(`not bound: ${after}`);
  await film(page, "bound", `Bound to ${NAME}; wallets do not find the name until public DNS vouches for the zone`,
    { kind: "none" }, { section, name });
  await writeManifest();
  await ctx.close();
}

async function found(browser) {
  const ctx = await context(browser, "dark");
  const page = await ctx.newPage();
  await openNetwork(page);
  const section = page.getByTestId("domain-wallet");
  const name = section.locator('[data-testid="wallet-name"][data-published="found"]');
  await name.waitFor({ timeout: 120_000 });
  await settle(page);
  await film(page, "wallets-find-it", `${NAME} resolves: wallets find it and pay this box`, { kind: "none" },
    { section, name });
  await writeManifest();
  await ctx.close();
}

const browser = await chromium.launch();
if (args.mode === "found") {
  await found(browser);
  await prShots(browser, "network-found");
} else {
  await prShots(browser, "network");
  if (args.mode === "after") {
    await bind(browser);
    await prShots(browser, "network-bound");
  }
}
await browser.close();
