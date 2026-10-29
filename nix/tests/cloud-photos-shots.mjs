// Photos in LosOS Cloud in pictures, for the pull request. Run on the box by
// nix/tests/cloud-photos.nix, after the pass has read every photo:
//
//   node cloud-photos-shots.mjs after|before|film OUT NAME PASSWORD ADDRESS
//
// Signs in through LosOS Login at mattbox.local (resolved to ADDRESS) and
// takes the Photos page and the viewer; `after` also takes a person, the
// photos by camera and by place, and the viewer's details. `film` takes the
// launch film's frames, numbered, with a manifest.json of what each shows
// and where to point. One window for all: 1280 x 900 at twice the pixels,
// dark, en-US, reduced motion, no caret.
//
// Playwright comes from PLAYWRIGHT_CORE (a path to playwright-core, as
// nixpkgs' playwright-driver is) or the `playwright-core` package.

const join = (...parts) => parts.join("/");

const { chromium } = await import(Bun.env.PLAYWRIGHT_CORE ?? "playwright-core");

const [which, out, login, password, lan] = Bun.argv.slice(2);
if (!["after", "before", "film"].includes(which) || !lan) {
  // Thrown: an uncaught error is what makes Bun exit non-zero.
  throw new Error("usage: cloud-photos-shots.mjs after|before|film OUT NAME PASSWORD ADDRESS");
}
Bun.spawnSync(["mkdir", "-p", out]);

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
// The admin page's policy (style-src 'self') refuses anything a library
// injects; a refusal is a bug, not a look. What a plain load of LosOS
// Cloud's Files page already logs (sonner's stylesheet, which the bundle
// carries instead) is the baseline, as tests/*.browser.mjs keep it.
const baseline = new Set();
let watching = false;
page.on("pageerror", (e) => errors.push(String(e)));
page.on("console", (m) => {
  if (m.type() !== "error" || !/Content Security Policy/i.test(m.text())) return;
  if (!watching) baseline.add(m.text());
  else if (!baseline.has(m.text())) errors.push(m.text());
});

const base = "https://mattbox.local";

async function settle() {
  // A CSSOM write, which style-src 'self' allows; a <style> it refuses.
  await page.evaluate(() => {
    document.documentElement.style.caretColor = "transparent";
  });
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

const steps = [];
async function box(locator) {
  // A target that is not on screen is a broken frame, not a wait.
  if ((await locator.count()) === 0) throw new Error(`no ${locator} on screen`);
  const b = await locator.first().boundingBox();
  return b && { x: Math.round(b.x), y: Math.round(b.y), w: Math.round(b.width), h: Math.round(b.height) };
}
async function frame(file, url, title, action, targets) {
  await take(file);
  const boxes = {};
  for (const [name, locator] of Object.entries(targets)) {
    const b = await box(locator);
    if (b) boxes[name] = b;
  }
  steps.push({ file, url: `mattbox.local${url}`, title, action, targets: boxes });
}

async function film() {
  const select = page.locator("select[data-photos-by]");
  await photos();
  await frame("01-photos.png", "/cloud/photos", "every photo by the month it was taken", { kind: "click", target: "groupBy" }, {
    groupBy: select,
    first: page.locator("[data-photos] button"),
  });
  await photos("?by=camera");
  await frame("02-by-camera.png", "/cloud/photos?by=camera", "the photos by camera", { kind: "click", target: "groupBy" }, {
    groupBy: select,
  });
  await photos("?by=place");
  const oldTown = page.getByRole("button", { name: "old-town.jpg", exact: true });
  await frame("03-by-place.png", "/cloud/photos?by=place", "the photos by where they were taken", { kind: "click", target: "photo" }, {
    groupBy: select,
    photo: oldTown,
  });
  await viewer("old-town.jpg");
  await frame("04-viewer.png", "/cloud/photos?by=place", "one photo, large", { kind: "click", target: "details" }, {
    details: page.locator("[data-photo-info]"),
  });
  await page.locator("[data-photo-info]").click();
  await page.locator("[data-photo-details]").waitFor();
  await frame("05-details.png", "/cloud/photos?by=place", "what the camera wrote: when, which camera, where", { kind: "click", target: "tags" }, {
    panel: page.locator("[data-photo-details]"),
    tags: page.locator("[data-photo-tags] summary"),
  });
  await page.locator("[data-photo-tags] summary").click();
  await frame("06-tags.png", "/cloud/photos?by=place", "every tag the camera saved", { kind: "none" }, {
    panel: page.locator("[data-photo-details]"),
  });
  await Bun.write(
    join(out, "manifest.json"),
    JSON.stringify(
      {
        feature: "photos",
        viewport: { width: 1280, height: 900 },
        dpr: 2,
        commit: Bun.env.LOSOS_COMMIT ?? "",
        test: "losos-cloud-photos",
        steps,
      },
      null,
      2,
    ),
  );
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
  await page.goto(`${base}/cloud/`);
  await page.waitForLoadState("networkidle");
  await page.waitForTimeout(1000);
  watching = true;
  if (which === "film") return film();

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
  // Thrown: an uncaught error is what makes Bun exit non-zero.
  throw new Error(`the page threw or was refused:\n${errors.join("\n")}`);
}
