// Chromium on the laptop, pointed at mattbox's own nginx: the Market pane's
// AI models section, as an owner sees it.
//
// One command per call, so the test script decides the order and the box's
// state between calls. The browser profile lives on disk, so the wallet the
// page keeps in IndexedDB survives from one call to the next; the spare key
// goes into sessionStorage before every page load.
//
//   shots import-wallet PHRASE PASSWORD
//   shots shot LOOK OUT ANCHOR
//   shots start LIMIT TOPUP PASSWORD OUTDIR
//   shots chat PROMPT MAXTOKENS OUTDIR
//   shots stop OUTDIR
//   shots settled OUTDIR
//
// LOOK is `pr` (light, scale 1) or `film` (dark, scale 2). ANCHOR is the
// `data-testid` of the section scrolled to the top of the window. Commands
// that take OUTDIR write numbered frames there (none for `-`) and print one
// JSON line per frame: its file, the action that follows it and the boxes of
// what it shows, in CSS pixels.

const join = (...parts) => parts.join("/");
const dirname = (file) => file.slice(0, file.lastIndexOf("/"));

const { chromium } = await import(join(Bun.env.PLAYWRIGHT_CORE, "index.mjs"));

const BASE = "http://mattbox";
const PANE = `${BASE}/settings/market`;
const PROFILE = "/var/lib/shots/profile";
const TOKEN = (await Bun.file("/var/lib/shots/token").text()).trim();

const LOOKS = {
  pr: { colorScheme: "light", deviceScaleFactor: 1 },
  film: { colorScheme: "dark", deviceScaleFactor: 2 },
};

// The spare key before the page's own scripts run, the theme the look asks
// for, and no text caret. A constructed sheet, because the page's CSP
// refuses a <style> element.
function init({ token, theme }) {
  try {
    sessionStorage.setItem("losos-token", token);
    localStorage.setItem("losos-theme", theme);
    localStorage.setItem("losos-lang", "en");
    localStorage.setItem("losos-tour", "seen");
  } catch {}
  const hide = () => {
    try {
      const sheet = new CSSStyleSheet();
      sheet.replaceSync("*, *::before, *::after { caret-color: transparent !important; }");
      document.adoptedStyleSheets = [...document.adoptedStyleSheets, sheet];
    } catch {}
  };
  if (document.readyState === "loading") document.addEventListener("DOMContentLoaded", hide);
  else hide();
}

async function problem(page) {
  const errors = page.locator('[data-sonner-toast][data-type="error"]');
  return (await errors.count()) > 0 ? await errors.first().innerText() : null;
}

// Network idle, fonts loaded, no spinner and no toast on screen.
async function settle(page, timeoutMs = 180_000) {
  await page.waitForLoadState("networkidle");
  await page.evaluate(() => document.fonts.ready.then(() => true));
  const deadline = Date.now() + timeoutMs;
  for (;;) {
    const said = await problem(page);
    if (said) throw new Error(`the page says: ${said}`);
    const busy = await page.evaluate(() =>
      [...document.querySelectorAll('[data-slot="spinner"], [data-sonner-toast]')]
        .filter((el) => el.getClientRects().length > 0)
        .map((el) => (el.closest("[data-testid]")?.getAttribute("data-testid") ?? "") + ": " + el.outerHTML.slice(0, 300)),
    );
    if (busy.length === 0) break;
    if (Date.now() > deadline) {
      await page.screenshot({ path: "/tmp/shots/unsettled.png" }).catch(() => {});
      const text = await page.evaluate(() => document.body.innerText.slice(0, 3000));
      throw new Error(`the page did not settle: ${busy.join(" | ")}\n--- the page reads:\n${text}`);
    }
    await page.waitForTimeout(500);
  }
  await page.waitForLoadState("networkidle");
  await page.waitForTimeout(800);
}

// The page of the command that is running, for the report when it fails.
let current = null;

async function browser(look) {
  const opts = LOOKS[look];
  Bun.spawnSync(["mkdir", "-p", PROFILE]);
  const ctx = await chromium.launchPersistentContext(PROFILE, {
    // Full Chromium in its headless mode: the headless shell ignores the
    // secure-origin switch below.
    channel: "chromium",
    headless: true,
    viewport: { width: 1280, height: 900 },
    deviceScaleFactor: opts.deviceScaleFactor,
    colorScheme: opts.colorScheme,
    reducedMotion: "reduce",
    locale: "en-US",
    timezoneId: "Europe/Bratislava",
    // The wallet seals its phrase with WebCrypto, which Chromium gives only
    // to a secure context. An owner opens the box over its own https; here
    // plain http is treated as secure instead, which spares the emulated box
    // a TLS handshake per connection.
    args: ["--disable-gpu", "--no-sandbox", `--unsafely-treat-insecure-origin-as-secure=${BASE}`],
  });
  await ctx.addInitScript(init, { token: TOKEN, theme: opts.colorScheme });
  ctx.setDefaultTimeout(120_000);
  const page = ctx.pages()[0] ?? (await ctx.newPage());
  page.on("pageerror", (e) => console.error(`page error: ${e.message}`));
  page.on("console", (m) => {
    if (m.type() === "error") console.error(`console: ${m.text()}`);
  });
  current = page;
  return { ctx, page };
}

async function toTop(page, anchor) {
  const section = page.locator(`[data-testid="${anchor}"]`);
  await section.waitFor();
  // Below the sticky header, with the section's title above it in view.
  await section.evaluate((el) => {
    const header = document.querySelector("header.sticky");
    const offset = (header ? header.getBoundingClientRect().bottom : 0) + 52;
    window.scrollTo({ top: window.scrollY + el.getBoundingClientRect().top - offset });
  });
  await page.waitForTimeout(300);
}

// What the pane says when one of its reads failed. lososd relays the market
// calls outside its state lock, so a slow edge call never makes another read
// wait into a 408: a failed read fails the test.
const FAILED_READ = /could not be read|did not answer/;

async function openPane(page, anchor = "ai-models") {
  const models = page.locator('[data-testid="ai-models"]');
  const failed = page.getByText(FAILED_READ);
  await page.goto(PANE);
  await models.or(failed).first().waitFor({ timeout: 300_000 });
  await settle(page);
  if ((await failed.count()) > 0) {
    throw new Error(`the Market pane failed a read: ${await failed.first().innerText()}`);
  }
  await toTop(page, anchor);
}

async function targets(names) {
  const out = {};
  for (const [name, locator] of Object.entries(names)) {
    const first = locator.first();
    if ((await locator.count()) === 0 || !(await first.isVisible())) continue;
    const b = await first.boundingBox();
    if (b) out[name] = { x: Math.round(b.x), y: Math.round(b.y), w: Math.round(b.width), h: Math.round(b.height) };
  }
  return out;
}

class Frames {
  constructor(page, outdir) {
    this.page = page;
    this.outdir = outdir;
    if (outdir === "-") return;
    this.counter = Bun.file(join(outdir, ".next"));
  }

  async take(slug, action, target, names) {
    if (this.outdir === "-") return;
    // The number goes on from an earlier call's pictures in the same folder.
    this.n ??= (await this.counter.exists()) ? Number(await this.counter.text()) : 1;
    const file = `${String(this.n).padStart(2, "0")}-${slug}.png`;
    await this.page.screenshot({ path: join(this.outdir, file) });
    const step = { file, action: target ? { kind: action, target } : { kind: action }, targets: await targets(names) };
    console.log(JSON.stringify(step));
    this.n += 1;
    await Bun.write(this.counter, String(this.n));
  }
}

const modelRow = (page) => page.locator('[data-testid="ai-catalogue"] [data-model]').first();

async function importWallet(phrase, password) {
  const { ctx, page } = await browser("pr");
  await openPane(page);
  const field = page.getByPlaceholder("12 or 24 words");
  await field.waitFor({ timeout: 600_000 });
  await field.scrollIntoViewIfNeeded();
  await field.fill(phrase);
  await page.getByPlaceholder("A password for this browser").last().fill(password);
  await page.getByRole("button", { name: "Import", exact: true }).click();
  // The phrase is stretched in the page's own JavaScript (one seed per
  // chain family) and sealed with PBKDF2, which is slow under emulation.
  // The field empties once the wallet is saved.
  const deadline = Date.now() + 900_000;
  while ((await field.inputValue()) !== "") {
    const said = await problem(page);
    if (said) throw new Error(`the page says: ${said}`);
    if (Date.now() > deadline) throw new Error("the wallet was not saved in 15 minutes");
    await page.waitForTimeout(2_000);
  }
  await openPane(page);
  await page.getByText("The wallet on this box").first().waitFor({ timeout: 300_000 });
  await ctx.close();
}

async function shot(look, out, anchor) {
  const { ctx, page } = await browser(look);
  await openPane(page, anchor);
  Bun.spawnSync(["mkdir", "-p", dirname(out)]);
  await page.screenshot({ path: out });
  await ctx.close();
}

async function start(limit, topUp, password, outdir) {
  const { ctx, page } = await browser("film");
  await openPane(page);
  const frames = new Frames(page, outdir);
  const row = modelRow(page);
  const go = () => row.getByRole("button", { name: "Start", exact: true });
  await frames.take("models", "click", "start", { model: row, start: go() });
  await go().click();
  const limitField = page.getByLabel("Most it may spend in an hour");
  await limitField.waitFor();
  await limitField.fill("");
  const topField = page.getByLabel("Add to your balance");
  await topField.fill(topUp);
  await page.getByLabel("Wallet password", { exact: true }).fill(password);
  await page.waitForTimeout(400);
  await frames.take("start-form", "type", "limit", { model: row, limit: limitField, topUp: topField });
  for (let i = 0; i < limit.length; i++) {
    await limitField.pressSequentially(limit[i]);
    await page.waitForTimeout(150);
    const last = i === limit.length - 1;
    await frames.take(`limit-${i + 1}`, last ? "click" : "type", last ? "start" : "limit", {
      limit: limitField,
      topUp: topField,
      start: go(),
    });
  }
  await go().click();
  await page
    .locator('[data-testid="ai-meters"]')
    .getByText("· Open")
    .waitFor({ timeout: 600_000 });
  await settle(page);
  await toTop(page, "ai-meters");
  await frames.take("running", "none", null, {
    meter: page.locator('[data-testid="ai-meters"]'),
    balance: page.locator('[data-testid="ai-balance"]'),
  });
  await ctx.close();
}

async function chat(prompt, maxTokens, outdir) {
  const { ctx, page } = await browser("film");
  await openPane(page, "ai-chat");
  const frames = new Frames(page, outdir);
  const box = page.locator('[data-testid="ai-chat"]');
  const field = box.getByPlaceholder("Ask something");
  await box.getByLabel("Longest answer, in tokens", { exact: true }).fill(maxTokens);
  const words = prompt.split(" ");
  for (let i = 0; i < words.length; i++) {
    await field.pressSequentially((i === 0 ? "" : " ") + words[i]);
    await page.waitForTimeout(150);
    const last = i === words.length - 1;
    await frames.take(`ask-${i + 1}`, last ? "click" : "type", last ? "send" : "prompt", {
      prompt: field,
      send: box.getByRole("button", { name: "Send", exact: true }),
    });
  }
  await box.getByRole("button", { name: "Send", exact: true }).click();
  await box.getByText("This answer cost").last().waitFor({ timeout: 900_000 });
  await settle(page);
  await toTop(page, "ai-chat");
  await frames.take("answer", "none", null, { answer: box.locator("p.whitespace-pre-wrap").last() });
  await ctx.close();
}

async function stop(outdir) {
  const { ctx, page } = await browser("film");
  await openPane(page, "ai-meters");
  const frames = new Frames(page, outdir);
  const button = page.locator('[data-testid="ai-meters"]').getByRole("button", { name: "Stop", exact: true });
  await frames.take("stop", "click", "stop", { stop: button });
  await button.click();
  await settle(page);
  await ctx.close();
}

async function settled(outdir) {
  const { ctx, page } = await browser("film");
  await openPane(page, "ai-models");
  const frames = new Frames(page, outdir);
  await frames.take("settled", "none", null, {
    meter: page.locator('[data-testid="ai-meters"]').getByText("Stopped"),
    fee: page.getByText("the edge keeps"),
  });
  await ctx.close();
}

const [cmd, ...args] = Bun.argv.slice(2);
const commands = { "import-wallet": importWallet, shot, start, chat, stop, settled };
if (!(cmd in commands)) {
  // Thrown: an uncaught error is what makes Bun exit non-zero.
  throw new Error(`unknown command ${cmd}`);
}
try {
  await commands[cmd](...args);
} catch (e) {
  if (current) {
    Bun.spawnSync(["mkdir", "-p", "/tmp/shots"]);
    await current.screenshot({ path: "/tmp/shots/failed.png", fullPage: true }).catch(() => {});
    const text = await current.evaluate(() => location.href + "\n" + document.body.innerText.slice(0, 4000)).catch(() => "");
    console.error(`--- the page at the failure reads:\n${text}`);
  }
  throw e;
}
