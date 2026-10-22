// The right-click menu on a real box, from a real browser.
//
// Chromium (Playwright) signs in through LosOS Login as the owner and opens
// the menu where a person would: empty space on the Overview, an app tile,
// a file, a folder and the space around them in LosOS Cloud, a row in the
// trash, from the keyboard on a focused row, and by a long press on a
// phone. One PNG per state and colour scheme, and a manifest.
// nix/tests/context-menu.nix runs it in its laptop VM:
//
//   node context-menu-shots.mjs BASE NAME PASSWORD OUT [--before] [--ca FILE]
//
// --before takes the same states on the box's build without the menu, for
// the pull request's "before" column, and checks nothing. Otherwise every
// state is checked as well: the menu is an ARIA menu with the expected
// items, a text field keeps the browser's menu, F2 and Delete do what the
// menu says, and the page logs no CSP refusal beyond its own baseline.
//
// The window is 1280 x 900 (390 x 844 for the phone) at twice the pixels,
// en-US, reduced motion.

import assert from "node:assert/strict";
import { createHash, X509Certificate } from "node:crypto";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const { chromium } = await import(process.env.PLAYWRIGHT_CORE ?? "playwright-core");

const DESKTOP = { width: 1280, height: 900 };
const PHONE = { width: 390, height: 844 };

function spkiPin(pemFile) {
  const cert = new X509Certificate(readFileSync(pemFile));
  const der = cert.publicKey.export({ type: "spki", format: "der" });
  return createHash("sha256").update(der).digest("base64");
}

const argv = process.argv.slice(2);
const flag = (name) => argv.includes(name);
const option = (name, fallback) => (argv.includes(name) ? argv[argv.indexOf(name) + 1] : fallback);
const [base, name, password, out] = argv.filter((a, i) => !a.startsWith("--") && !argv[i - 1]?.startsWith("--ca"));
const before = flag("--before");
const ca = option("--ca", "/etc/losos-box.pem");
mkdirSync(out, { recursive: true });

const browser = await chromium.launch({ args: [`--ignore-certificate-errors-spki-list=${spkiPin(ca)}`] });
const steps = [];

async function session(scheme, viewport, touch = false) {
  const context = await browser.newContext({
    viewport,
    deviceScaleFactor: 2,
    colorScheme: scheme,
    locale: "en-US",
    reducedMotion: "reduce",
    hasTouch: touch,
    isMobile: touch,
  });
  await context.addInitScript(() => {
    window.__violations = [];
    document.addEventListener("securitypolicyviolation", (e) => window.__violations.push(`${e.violatedDirective} ${e.blockedURI}`));
    window.addEventListener("contextmenu", (e) => {
      window.__lastMenu = { prevented: e.defaultPrevented };
    });
  });
  const page = await context.newPage();
  const errors = [];
  page.on("pageerror", (e) => errors.push(String(e)));
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
    [name, password, `${base}/`],
  );
  if (status !== 200) throw new Error(`sign-in refused: ${status}`);
  return { context, page, errors };
}

async function settle(page) {
  await page.waitForLoadState("networkidle");
  await page.evaluate(() => document.fonts.ready.then(() => true));
  await page.waitForFunction(() => {
    const seen = (el) => {
      const r = el.getBoundingClientRect();
      return r.bottom > 0 && r.top < innerHeight && r.width > 0;
    };
    if ([...document.images].filter(seen).some((i) => !i.complete || i.naturalWidth === 0)) return false;
    if ([...document.querySelectorAll("[data-slot=spinner]")].some(seen)) return false;
    return document.querySelector("[data-sonner-toast]") === null;
  }, undefined, { timeout: 120_000 });
  await page.evaluate(() => {
    document.documentElement.style.caretColor = "transparent";
  });
  await page.waitForTimeout(400);
}

async function take(page, slug, title) {
  await settle(page);
  const file = `${slug}.png`;
  await page.screenshot({ path: join(out, file) });
  steps.push({ file, url: new URL(page.url()).pathname, title });
  console.log(file, title);
}

const menu = (page) => page.locator('[data-slot="context-menu"]').first();
const texts = async (page) =>
  (await menu(page).locator('[role^="menuitem"]').allInnerTexts()).map((s) => s.replace(/\s+/g, " ").trim());

/** Open the menu at a point, wait for it (unless this is the build without one). */
async function rightClick(page, locator, where) {
  const b = await locator.boundingBox();
  const x = where?.x ?? b.x + Math.min(40, b.width / 2);
  const y = where?.y ?? b.y + b.height / 2;
  await page.mouse.click(x, y, { button: "right" });
  if (!before) await menu(page).waitFor();
  else await page.waitForTimeout(500);
}

async function close(page) {
  await page.keyboard.press("Escape");
  await page.keyboard.press("Escape");
  if (!before) await page.locator('[data-slot="context-menu"]').first().waitFor({ state: "detached" });
}

async function desktop(scheme) {
  const { context, page, errors } = await session(scheme, DESKTOP);
  await page.goto(`${base}/`);
  const main = page.getByRole("main");
  await main.locator("h1, h2").first().waitFor();
  await settle(page);
  const atLoad = await page.evaluate(() => window.__violations.length);

  // Empty space: the page's own items, with the theme submenu open.
  const area = await main.boundingBox();
  await rightClick(page, main, { x: area.x + area.width - 40, y: area.y + 12 });
  if (!before) {
    assert.equal(await menu(page).getAttribute("role"), "menu");
    const got = await texts(page);
    assert.deepEqual(got.map((s) => s.split(" ")[0]), ["Search", "Back", "Forward", "Reload", "Appearance", "Handbook"]);
    await menu(page).getByRole("menuitem", { name: "Appearance" }).hover();
    await page.locator('[data-slot="context-menu"][aria-label="Appearance"]').waitFor();
  }
  await take(page, `${scheme}-overview-page`, "empty space on the Overview, the theme submenu open");
  await close(page);

  // An app tile: a new tab and the sidebar pin.
  const tile = main.getByRole("listitem").getByRole("link").first();
  if ((await tile.count()) > 0) {
    await rightClick(page, tile);
    if (!before) assert.deepEqual(await texts(page), ["Open in new tab", "Pin to sidebar"]);
    await take(page, `${scheme}-overview-tile`, "an app tile");
    await close(page);
  }

  // LosOS Cloud.
  await page.goto(`${base}/cloud/files/Documents`);
  await page.waitForSelector('[data-entry="budget.ods"]');
  await settle(page);
  const budget = page.locator('[data-entry="budget.ods"]');
  await rightClick(page, budget, { x: (await budget.boundingBox()).x + 420, y: (await budget.boundingBox()).y + 20 });
  if (!before) {
    assert.equal(await menu(page).getAttribute("aria-label"), "budget.ods");
    const got = await texts(page);
    for (const want of ["Open", "Download", "Rename… F2", "Move…", "Copy…", "Versions…", "Share…", "Copy link", "Delete Del"]) {
      assert.ok(got.includes(want), `${want} missing from ${JSON.stringify(got)}`);
    }
    // A right click picks the row it lands on.
    await page.locator("[data-selection]").waitFor();
  }
  await take(page, `${scheme}-cloud-file`, "a file in LosOS Cloud");
  await close(page);

  if (!before && scheme === "light") {
    // F2 renames and Delete deletes what is picked, as the menu says. In a
    // folder no picture shows, so every picture has the same files.
    await page.goto(`${base}/cloud/files/Music`);
    await page.waitForSelector('[data-entry="playlist.m3u"]');
    await page.getByRole("checkbox", { name: "Select playlist.m3u" }).check();
    await page.keyboard.press("F2");
    const dialog = page.locator("dialog[open]");
    await dialog.waitFor();
    await dialog.locator("input").fill("old playlist.m3u");
    await dialog.locator("input").press("Enter");
    await page.waitForSelector('[data-entry="old playlist.m3u"]');
    await page.getByRole("checkbox", { name: "Select old playlist.m3u" }).check();
    await page.keyboard.press("Delete");
    await page.waitForSelector('[data-entry="old playlist.m3u"]', { state: "detached" });
  }

  await page.goto(`${base}/cloud/files`);
  await page.waitForSelector('[data-entry="Documents"]');
  await settle(page);
  const folder = page.locator('[data-entry="Documents"]');
  await rightClick(page, folder, { x: (await folder.boundingBox()).x + 420, y: (await folder.boundingBox()).y + 20 });
  if (!before) {
    const got = await texts(page);
    assert.ok(got.includes("Open in new tab") && !got.includes("Download"), JSON.stringify(got));
  }
  await take(page, `${scheme}-cloud-folder`, "a folder in LosOS Cloud");
  await close(page);

  // The space under the list: what a folder can take.
  const last = page.locator("[data-entry]").last();
  const lb = await last.boundingBox();
  assert.ok(lb.y + lb.height + 60 < DESKTOP.height, "no space under the list");
  await rightClick(page, last, { x: lb.x + lb.width / 2, y: lb.y + lb.height + 60 });
  if (!before) assert.deepEqual((await texts(page)).slice(0, 3), ["New folder…", "Upload…", "Upload folder…"]);
  await take(page, `${scheme}-cloud-space`, "the space around the files");
  await close(page);

  // From the keyboard: Shift+F10 on a focused row's name.
  await page.getByRole("link", { name: "Photos", exact: true }).last().focus();
  await page.keyboard.press("Shift+F10");
  if (!before) {
    await menu(page).waitFor();
    await page.waitForFunction(() => document.activeElement?.getAttribute("role") === "menuitem");
  }
  await take(page, `${scheme}-cloud-keyboard`, "Shift+F10 on a focused row");
  await close(page);

  if (!before) {
    // The search box keeps the browser's menu.
    await page.getByRole("searchbox").click({ button: "right" });
    await page.waitForTimeout(300);
    assert.equal(await page.locator('[data-slot="context-menu"]').count(), 0);
    assert.equal(await page.evaluate(() => window.__lastMenu.prevented), false);
    await page.keyboard.press("Escape");
  }

  await page.goto(`${base}/cloud/trash`);
  await page.waitForSelector("[data-trash]");
  const row = page.locator("[data-trash] [data-entry]").first();
  if ((await row.count()) > 0) {
    await rightClick(page, row, { x: (await row.boundingBox()).x + 300, y: (await row.boundingBox()).y + 20 });
    if (!before) assert.deepEqual(await texts(page), ["Restore", "Delete for good…"]);
    await take(page, `${scheme}-cloud-trash`, "a file in the trash");
    await close(page);
  }

  if (!before) {
    const violations = (await page.evaluate(() => window.__violations)).slice(atLoad);
    assert.deepEqual(violations.filter((v) => !v.startsWith("style-src-elem")), [], "the menu added CSP refusals");
    assert.deepEqual(errors, [], "uncaught errors");
  }
  await context.close();
}

async function phone() {
  const { context, page } = await session("light", PHONE, true);
  await page.goto(`${base}/cloud/files/Documents`);
  await page.waitForSelector('[data-entry="budget.ods"]');
  await settle(page);
  const row = page.locator('[data-entry="budget.ods"]');
  // On a phone the places sit above the list and push the row down.
  await row.scrollIntoViewIfNeeded();
  await page.waitForTimeout(400);
  const b = await row.boundingBox();
  assert.ok(b.y > 0 && b.y + b.height < PHONE.height, `row off the screen: ${JSON.stringify(b)}`);
  const cdp = await context.newCDPSession(page);
  // The row is wider than a phone; press on the part that is on screen.
  const point = [{ x: Math.min(b.x + b.width / 2, PHONE.width / 2), y: b.y + b.height / 2 }];
  await cdp.send("Input.dispatchTouchEvent", { type: "touchStart", touchPoints: point });
  await page.waitForTimeout(1500);
  await cdp.send("Input.dispatchTouchEvent", { type: "touchEnd", touchPoints: [] });
  if (!before) {
    await menu(page).waitFor();
    const m = await menu(page).boundingBox();
    assert.ok(m.x >= 0 && m.x + m.width <= PHONE.width, `menu off the screen: ${JSON.stringify(m)}`);
  } else await page.waitForTimeout(500);
  await take(page, "light-phone-long-press", "a long press on a phone");
  await context.close();
}

await desktop("light");
await desktop("dark");
await phone();
writeFileSync(join(out, "manifest.json"), JSON.stringify({ feature: "context-menu", before, steps }, null, 2) + "\n");
await browser.close();
