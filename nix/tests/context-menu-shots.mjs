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
//
// Without --before it also takes the handbook's pictures of the menu into
// OUT/handbook: light, in English, Slovak and German, named the way the
// handbook names them (`context-menu-file.png`, `-sk`, `-de`).

const join = (...parts) => parts.join("/");

// The three checks this script makes, strict like node:assert/strict.
const assert = {
  ok(value, message) {
    if (!value) throw new Error(message ?? `expected a truthy value, got ${Bun.inspect(value)}`);
  },
  equal(actual, expected, message) {
    if (!Object.is(actual, expected)) throw new Error(message ?? `${Bun.inspect(actual)} !== ${Bun.inspect(expected)}`);
  },
  deepEqual(actual, expected, message) {
    if (!Bun.deepEquals(actual, expected, true)) throw new Error(message ?? `${Bun.inspect(actual)} does not equal ${Bun.inspect(expected)}`);
  },
};

const { chromium } = await import(Bun.env.PLAYWRIGHT_CORE ?? "playwright-core");

const DESKTOP = { width: 1280, height: 900 };
const PHONE = { width: 390, height: 844 };

async function spkiPin(pemFile) {
  const pem = await Bun.file(pemFile).text();
  const der = Uint8Array.fromBase64(/-----BEGIN CERTIFICATE-----([^-]+)-----END CERTIFICATE-----/.exec(pem)[1].replace(/\s/g, ""));
  // One DER element at `at`: where its contents start and where it ends.
  const element = (at) => {
    let len = der[at + 1];
    let start = at + 2;
    if (len & 0x80) {
      const n = len & 0x7f;
      len = 0;
      for (let i = 0; i < n; i++) len = len * 256 + der[start + i];
      start += n;
    }
    return { start, end: start + len };
  };
  // Certificate > tbsCertificate > [0] version, serial, signature, issuer,
  // validity, subject, then the subjectPublicKeyInfo Chromium hashes.
  let at = element(element(0).start).start;
  if (der[at] === 0xa0) at = element(at).end;
  for (let i = 0; i < 5; i++) at = element(at).end;
  const spki = der.subarray(at, element(at).end);
  return new Uint8Array(await crypto.subtle.digest("SHA-256", spki)).toBase64();
}

const argv = Bun.argv.slice(2);
const flag = (name) => argv.includes(name);
const option = (name, fallback) => (argv.includes(name) ? argv[argv.indexOf(name) + 1] : fallback);
const [base, name, password, out] = argv.filter((a, i) => !a.startsWith("--") && !argv[i - 1]?.startsWith("--ca"));
const before = flag("--before");
const ca = option("--ca", "/etc/losos-box.pem");
Bun.spawnSync(["mkdir", "-p", out]);

const browser = await chromium.launch({ args: [`--ignore-certificate-errors-spki-list=${await spkiPin(ca)}`] });
const steps = [];

async function session(scheme, viewport, touch = false, locale = "en-US") {
  const context = await browser.newContext({
    viewport,
    deviceScaleFactor: 2,
    colorScheme: scheme,
    locale,
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

// The handbook's pictures: the page's own menu with Appearance open, and a
// file's menu in LosOS Cloud, in each language the handbook has.
async function handbook(locale, suffix) {
  const { context, page, errors } = await session("light", DESKTOP, false, locale);
  const dir = join(out, "handbook");
  Bun.spawnSync(["mkdir", "-p", dir]);
  const shoot = async (slug) => {
    await settle(page);
    await page.screenshot({ path: join(dir, `${slug}${suffix}.png`) });
    console.log(`handbook/${slug}${suffix}.png`);
  };

  await page.goto(`${base}/`);
  const main = page.getByRole("main");
  await main.locator("h1, h2").first().waitFor();
  await settle(page);
  const area = await main.boundingBox();
  await rightClick(page, main, { x: area.x + area.width - 300, y: area.y + 12 });
  await menu(page).locator('[role="menuitem"][aria-haspopup]').first().hover();
  await page.locator('[data-slot="context-menu"]').nth(1).waitFor();
  await shoot("context-menu-page");
  await close(page);

  await page.goto(`${base}/cloud/files/Documents`);
  await page.waitForSelector('[data-entry="budget.ods"]');
  await settle(page);
  const budget = await page.locator('[data-entry="budget.ods"]').boundingBox();
  await rightClick(page, page.locator('[data-entry="budget.ods"]'), { x: budget.x + 420, y: budget.y + 20 });
  await page.locator("[data-selection]").waitFor();
  await shoot("context-menu-file");
  await close(page);

  assert.deepEqual(errors, [], "uncaught errors");
  await context.close();
}

await desktop("light");
await desktop("dark");
await phone();
if (!before) {
  await handbook("en-US", "");
  await handbook("sk-SK", "-sk");
  await handbook("de-DE", "-de");
}
await Bun.write(join(out, "manifest.json"), JSON.stringify({ feature: "context-menu", before, steps }, null, 2) + "\n");
await browser.close();
