// Pictures of LosOS Cloud on a real box, from a real browser.
//
// Chromium (Playwright) signs in through LosOS Login as the owner and walks
// the web app, one full-window PNG per state, with a manifest that says where
// each picture was taken and where the things on it are. nix/tests/cloud.nix
// runs it in its laptop VM against the box's own nginx; the same pictures
// serve the pull request and the launch film. Run it again against any box:
//
//   node cloud-shots.mjs BASE NAME PASSWORD OUT [--before] [--commit SHA]
//                        [--test NAME] [--connect LOGIN_URL] [--ca FILE]
//
// BASE is the box (https://mattbox.local), OUT a folder for NN-slug.png and
// manifest.json. The browser trusts the box's certificate by its public key
// (--ca, default /etc/losos-box.pem). --before takes the same addresses on a
// box without LosOS Cloud, for the "before" column. --connect also takes the
// page a desktop client opens to sign in, before and after Allow.
//
// Playwright comes from PLAYWRIGHT_CORE (a path to playwright-core, as
// nixpkgs' playwright-driver is) or the `playwright-core` package.
//
// The window is 1280 x 900 at twice the pixels, dark, en-US, reduced motion,
// Europe/Bratislava. Each picture waits for the page to settle: no request
// in flight, fonts and pictures loaded, no spinner and no toast on screen.

const join = (...parts) => parts.join("/");

const VIEW = { width: 1280, height: 900 };

const { chromium } = await import(Bun.env.PLAYWRIGHT_CORE ?? "playwright-core");

/** The certificate's public key hash, which Chromium pins to: SHA-256 over
 * the DER SubjectPublicKeyInfo, read out of the PEM by walking its ASN.1. */
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

class Shots {
  constructor(page, base, out, meta) {
    this.page = page;
    this.host = new URL(base).hostname;
    this.out = out;
    this.meta = meta;
    this.steps = [];
    Bun.spawnSync(["mkdir", "-p", out]);
  }

  async settle() {
    const p = this.page;
    await p.waitForLoadState("networkidle");
    await p.evaluate(() => document.fonts.ready.then(() => true));
    // Every picture on screen has arrived, no spinner turns, no toast is
    // still sliding.
    await p.waitForFunction(
      () => {
        const seen = (el) => {
          const r = el.getBoundingClientRect();
          return r.bottom > 0 && r.top < innerHeight && r.width > 0;
        };
        const imgs = [...document.images].filter(seen);
        if (imgs.some((i) => !i.complete || i.naturalWidth === 0)) return false;
        if ([...document.querySelectorAll("[data-slot=spinner]")].some(seen)) return false;
        if (document.querySelector("[data-sonner-toast]")) return false;
        return true;
      },
      undefined,
      { timeout: 600_000 },
    );
    // The caret would blink into one picture and not the next. A CSSOM
    // write, which the box's style-src 'self' allows.
    await p.evaluate(() => {
      document.documentElement.style.caretColor = "transparent";
    });
    await p.waitForTimeout(300);
  }

  async take(slug, title, action = { kind: "none" }, targets = {}) {
    await this.settle();
    const file = `${String(this.steps.length + 1).padStart(2, "0")}-${slug}.png`;
    await this.page.screenshot({ path: join(this.out, file), timeout: 600_000 });
    const found = {};
    for (const [key, loc] of Object.entries(targets)) {
      const b = await loc.boundingBox().catch(() => null);
      if (b) found[key] = { x: Math.round(b.x), y: Math.round(b.y), w: Math.round(b.width), h: Math.round(b.height) };
    }
    const url = new URL(this.page.url());
    this.steps.push({ file, url: `${this.host}${url.pathname}${url.search}`, title, action, targets: found });
    console.log(file, title);
  }

  async save() {
    const manifest = { feature: "cloud", viewport: VIEW, dpr: 2, ...this.meta, steps: this.steps };
    await Bun.write(join(this.out, "manifest.json"), JSON.stringify(manifest, null, 2) + "\n");
  }
}

/**
 * LosOS Login, as its page does it, from inside its page: the request and
 * the cookie are the browser's, which trusts the box by the pin above.
 */
async function signIn(page, base, name, password) {
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
    [name, password, `${base}/cloud/`],
  );
  if (status !== 200) throw new Error(`sign-in refused: ${status}`);
}

async function after(s, base) {
  const p = s.page;
  const link = (name) => p.getByRole("link", { name, exact: true });
  await p.goto(`${base}/cloud/files`);
  await p.waitForSelector("[data-files]");
  await s.take("files", "the owner's files", { kind: "click", target: "documents" }, {
    documents: link("Documents"),
    upload: p.getByRole("button", { name: "Upload", exact: true }),
    places: p.getByRole("navigation", { name: "Places" }),
  });
  await link("Documents").click();
  await p.waitForSelector('[data-entry="budget.ods"]');
  const budget = p.getByRole("checkbox", { name: "Select budget.ods" });
  await s.take("documents", "the Documents folder", { kind: "click", target: "budget" }, {
    budget,
    breadcrumb: p.getByRole("navigation", { name: "Folder path" }),
  });
  await budget.check();
  await p.waitForSelector("[data-selection]");
  await s.take("selected", "one file picked, and what can be done with it", { kind: "click", target: "photos" }, {
    bar: p.locator("[data-selection]"),
    versions: p.getByRole("button", { name: "Versions" }),
    photos: link("Photos"),
  });
  await link("Photos").click();
  await p.waitForSelector("[data-photos] img");
  const first = p.locator("[data-photos] button").first();
  await s.take("photos", "every picture, by month", { kind: "click", target: "photo" }, { photo: first });
  await first.click();
  await p.waitForSelector("dialog[open] img");
  await s.take("photo", "one picture, large", { kind: "click", target: "next" }, {
    next: p.getByRole("button", { name: "Next photo" }),
  });
  await p.keyboard.press("Escape");
  await link("Recent").click();
  await p.waitForSelector("[data-files]");
  const field = p.getByRole("searchbox");
  await s.take("recent", "what changed lately", { kind: "click", target: "search" }, { search: field });
  await field.click();
  let typed = "";
  for (const ch of "budget") {
    typed += ch;
    await field.press(ch);
    await s.take(`search-${typed}`, `typing “${typed}”`, { kind: "type", target: "search" }, { search: field });
  }
  await field.press("Enter");
  await p.waitForSelector('[data-entry="budget.ods"]');
  await s.take("search-results", "what the search found", { kind: "click", target: "devices" }, {
    devices: link("Devices"),
  });
  await link("Devices").click();
  await p.waitForSelector("[data-devices]");
  await s.take("devices", "the computers and phones that sync", { kind: "none" }, { list: p.locator("[data-devices]") });
  await link("Trash").click();
  await p.waitForSelector("[data-trash]");
  await s.take("trash", "deleted files, ready to come back", { kind: "none" }, { list: p.locator("[data-trash]") });
  // The admin pages around it: the Overview's tile and the Apps pane's
  // LosOS Cloud fold, which holds its one box-wide setting.
  await p.goto(`${base}/`);
  await p.getByRole("link", { name: "Files" }).first().waitFor();
  await s.take("overview", "the admin home with LosOS Cloud's tiles");
  await p.goto(`${base}/apps`);
  const fold = p.getByTestId("accordion-cloud");
  await fold.getByRole("button").first().click();
  await p.getByTestId("site-cloud").waitFor();
  await fold.scrollIntoViewIfNeeded();
  await s.take("apps-cloud", "LosOS Cloud's fold on the Apps pane", { kind: "none" }, { fold });
}

/** The page the desktop client opens to sign in, before and after Allow. */
async function connect(s, loginUrl) {
  const p = s.page;
  await p.goto(loginUrl);
  await p.waitForSelector("[data-connect]");
  const allow = p.getByRole("button", { name: "Allow" });
  await s.take("connect", "the desktop client asks to sync", { kind: "click", target: "allow" }, { allow });
  await allow.click();
  await p.getByText("Connected").first().waitFor();
  await s.take("connected", "the desktop client got its password");
}

async function before(s, base) {
  for (const [path, slug, title] of [
    ["/cloud/files", "files", "the files address, before LosOS Cloud"],
    ["/cloud/photos", "photos", "the photos address, before LosOS Cloud"],
    ["/cloud/devices", "devices", "the devices address, before LosOS Cloud"],
  ]) {
    await s.page.goto(`${base}${path}`);
    await s.take(slug, title);
  }
}

function parse(argv) {
  const pos = [];
  const opts = {};
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (a === "--before") opts.before = true;
    else if (["--commit", "--test", "--connect", "--ca"].includes(a)) opts[a.slice(2)] = argv[++i];
    else pos.push(a);
  }
  if (pos.length < 4) {
    // Thrown: an uncaught error is what makes Bun exit non-zero.
    throw new Error("usage: cloud-shots.mjs BASE NAME PASSWORD OUT [--before] [--commit SHA] [--test NAME] [--connect URL] [--ca FILE]");
  }
  return { base: pos[0], name: pos[1], password: pos[2], out: pos[3], opts };
}

const { base, name, password, out, opts } = parse(Bun.argv.slice(2));
const browser = await chromium.launch({
  args: [`--ignore-certificate-errors-spki-list=${await spkiPin(opts.ca ?? "/etc/losos-box.pem")}`],
});
const context = await browser.newContext({
  viewport: VIEW,
  deviceScaleFactor: 2,
  colorScheme: "dark",
  locale: "en-US",
  timezoneId: "Europe/Bratislava",
  reducedMotion: "reduce",
});
const page = await context.newPage();
// Emulated without KVM, a 2x screenshot of a full page can take minutes;
// Playwright's 30 s default fails the run on a slow machine, not a bug.
page.setDefaultTimeout(600_000);
page.setDefaultNavigationTimeout(600_000);
await signIn(page, base, name, password);
// What the admin page itself logs on a plain load (sonner's stylesheet,
// which the box's style-src 'self' refuses and the bundle carries instead,
// as tests/*.browser.mjs baseline it): LosOS Cloud's pages may log nothing
// beyond that.
const baseline = new Set();
// Answers a box without LosOS Office gives on purpose: LosOS Cloud asks
// which types Office opens and takes a 404 as "none" (src/cloud/office/opener.ts).
const expected = new Set(["404 GET /cloud/api/office/extensions"]);
const listen = (sink) => {
  // Chromium logs a failed request without its address; the response
  // names it, so a 404 says which route the box did not serve.
  const onConsole = (m) => {
    if (m.type() === "error" && !m.text().startsWith("Failed to load resource")) sink(m.text());
  };
  const onError = (e) => sink(String(e));
  const onResponse = (r) => {
    const line = `${r.status()} ${r.request().method()} ${new URL(r.url()).pathname}`;
    if (r.status() >= 400 && !expected.has(line)) sink(line);
  };
  page.on("console", onConsole);
  page.on("pageerror", onError);
  page.on("response", onResponse);
  return () => {
    page.off("console", onConsole);
    page.off("pageerror", onError);
    page.off("response", onResponse);
  };
};
const stop = listen((text) => baseline.add(text));
await page.goto(`${base}/`);
await page.waitForLoadState("networkidle");
stop();
const problems = [];
listen((text) => {
  if (!baseline.has(text)) problems.push(text);
});
const s = new Shots(page, base, out, { commit: opts.commit ?? "", test: opts.test ?? "" });
if (opts.before) {
  await before(s, base);
} else {
  await after(s, base);
  if (opts.connect) await connect(s, opts.connect);
}
await s.save();
await browser.close();
if (problems.length > 0 && !opts.before) {
  // Thrown: an uncaught error is what makes Bun exit non-zero.
  throw new Error(`the page reported errors:\n${problems.join("\n")}`);
}
