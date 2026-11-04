// Pictures of LosOS Office's conversions on a real box, from a real
// browser: a document's and a presentation's Download menu, a Word file's
// menu in LosOS Cloud, and the document made from that Word file.
//
// Chromium (Playwright) signs in through LosOS Login as the owner, opens
// "Garden day.paper", downloads it as Word from the menu above the page
// (the file goes to OUT/downloads/ for nix/tests/office-convert.nix to
// read), does the same for "Talk.slides" as PowerPoint, then opens "Old
// report.docx" from its menu, which makes a document of it beside it. One
// full-window PNG per state, with a manifest, as
// nix/tests/office-paper-shots.mjs does.
//
//   bun office-convert-shots.mjs BASE NAME PASSWORD OUT [--before]
//                                 [--commit SHA] [--test NAME] [--ca FILE]
//
// --before takes the same places with the admin UI as it was before.

const join = (...parts) => parts.join("/");

const VIEW = { width: 1280, height: 900 };
const DOC = "Garden day.paper";
const DECK = "Talk.slides";
const WORD = "Old report.docx";

const { chromium } = await import(Bun.env.PLAYWRIGHT_CORE ?? "playwright-core");

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
  constructor(base, out, meta) {
    this.host = new URL(base).hostname;
    this.out = out;
    this.meta = meta;
    this.steps = [];
    Bun.spawnSync(["mkdir", "-p", out]);
  }

  async settle(p) {
    await p.waitForLoadState("networkidle").catch(() => {});
    await p.evaluate(() => document.fonts.ready.then(() => true));
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
      { timeout: 120_000 },
    ).catch(async (err) => {
      // Say what was still not ready.
      const left = await p.evaluate(() => ({
        images: [...document.images].filter((i) => !i.complete || i.naturalWidth === 0).map((i) => i.src),
        spinners: document.querySelectorAll("[data-slot=spinner]").length,
        toasts: document.querySelectorAll("[data-sonner-toast]").length,
      }));
      console.error("not settled:", JSON.stringify(left));
      throw err;
    });
    await p.evaluate(() => {
      document.documentElement.style.caretColor = "transparent";
    });
    await p.waitForTimeout(400);
  }

  async take(p, slug, title, action = { kind: "none" }, targets = {}) {
    await this.settle(p);
    const file = `${String(this.steps.length + 1).padStart(2, "0")}-${slug}.png`;
    await p.screenshot({ path: join(this.out, file), timeout: 300_000 });
    const found = {};
    for (const [key, loc] of Object.entries(targets)) {
      const b = await loc.boundingBox().catch(() => null);
      if (b) found[key] = { x: Math.round(b.x), y: Math.round(b.y), w: Math.round(b.width), h: Math.round(b.height) };
    }
    const url = new URL(p.url());
    this.steps.push({ file, url: `${this.host}${decodeURI(url.pathname)}${url.search}`, title, action, targets: found });
    console.log(file, title);
  }

  async save() {
    const manifest = { feature: "office-convert", viewport: VIEW, dpr: 2, ...this.meta, steps: this.steps };
    await Bun.write(join(this.out, "manifest.json"), JSON.stringify(manifest, null, 2) + "\n");
  }
}

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

/** Waits for the editor, live with the box and drawn. */
async function waitPaper(p, name) {
  await p.locator("[data-paper-office]").waitFor();
  await p.getByRole("heading", { level: 1, name }).waitFor();
  await p.locator('[data-testid=office-saved][data-connection="live"]').waitFor();
  await p.locator("[data-testid=paper-editor][data-ready]").waitFor();
}

/** Opens a file of the root folder as its row's link does. */
async function open(p, base, name) {
  await p.goto(`${base}/cloud/files`);
  const row = p.locator(`[data-entry="${name}"]`);
  await row.waitFor();
  const link = row.locator("a[href]").first();
  const href = (await link.count()) > 0 ? await link.getAttribute("href") : null;
  if (href !== null) await p.goto(new URL(href, `${base}/cloud/`).toString());
  return href;
}

/** Opens the Download menu and saves FORMAT into OUT/downloads. */
async function download(s, p, out, slug, title, format, file) {
  await p.getByTestId("paper-download").click();
  const item = p.locator(`[data-export="${format}"]`);
  await item.waitFor();
  await s.take(p, slug, title, { kind: "click", target: "item" }, {
    menu: p.locator("[data-export]").first().locator(".."),
    item,
  });
  const [got] = await Promise.all([p.waitForEvent("download", { timeout: 300_000 }), item.click()]);
  if (got.suggestedFilename() !== file) throw new Error(`saved as ${got.suggestedFilename()}, not ${file}`);
  await got.saveAs(join(out, "downloads", file));
  await p.getByTestId("paper-download").and(p.locator(":not([aria-busy=true])")).waitFor();
}

/** The Word file's menu in the files. */
async function wordMenu(s, p, base) {
  await p.goto(`${base}/cloud/files`);
  const row = p.locator(`[data-entry="${WORD}"]`);
  await row.waitFor();
  await row.click({ button: "right" });
  const menu = p.locator('[data-slot="context-menu"], [data-row-menu]').first();
  await menu.waitFor();
  await s.take(p, "word-menu", "a right click on a Word file", { kind: "none" }, {
    open: menu.getByRole("menuitem", { name: "Open", exact: true }),
  });
  await p.keyboard.press("Escape");
}

async function after(s, p, base, out) {
  Bun.spawnSync(["mkdir", "-p", join(out, "downloads")]);
  await open(p, base, DOC);
  await waitPaper(p, DOC);
  await download(s, p, out, "document-download", "Download above a document: Word, OpenDocument, PDF or a web page", "docx", "Garden day.docx");
  for (const [format, file] of [
    ["pdf", "Garden day.pdf"],
    ["html", "Garden day.html"],
  ]) {
    await p.getByTestId("paper-download").click();
    const item = p.locator(`[data-export="${format}"]`);
    const [got] = await Promise.all([p.waitForEvent("download", { timeout: 300_000 }), item.click()]);
    await got.saveAs(join(out, "downloads", file));
  }

  await open(p, base, DECK);
  await waitPaper(p, DECK);
  await download(s, p, out, "presentation-download", "Download above a presentation: PowerPoint or a web page", "pptx", "Talk.pptx");

  await wordMenu(s, p, base);
  const href = await open(p, base, WORD);
  if (href === null || !href.endsWith("?as=paper")) throw new Error(`the Word file opens at ${href}`);
  await p.waitForURL(/\/cloud\/office\/\d+$/, { timeout: 300_000 });
  await waitPaper(p, "Old report.paper");
  await p.locator("[data-testid=paper-editor] .ProseMirror table").waitFor();
  await s.take(p, "imported", "the Word file, opened as a document saved beside it", { kind: "none" }, {
    page: p.getByTestId("paper-page"),
  });
}

async function before(s, p, base) {
  await open(p, base, DOC);
  await p.waitForTimeout(3000);
  await s.take(p, "document-download", `${DOC} as it opened before`);
  await open(p, base, DECK);
  await p.waitForTimeout(3000);
  await s.take(p, "presentation-download", `${DECK} as it opened before`);
  await wordMenu(s, p, base);
  // Before, a Word file with no Collabora on the box was only a download.
  await p.goto(`${base}/cloud/files`);
  const row = p.locator(`[data-entry="${WORD}"]`);
  await row.waitFor();
  const saved = p.waitForEvent("download");
  await row.locator("a[href]").first().click();
  await saved;
  await p.waitForTimeout(1000);
  await s.take(p, "imported", `${WORD} was only downloaded before`);
}

function parse(argv) {
  const pos = [];
  const opts = {};
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (a === "--before") opts.before = true;
    else if (["--commit", "--test", "--ca"].includes(a)) opts[a.slice(2)] = argv[++i];
    else pos.push(a);
  }
  if (pos.length < 4) {
    // Thrown: an uncaught error is what makes Bun exit non-zero.
    throw new Error("usage: office-convert-shots.mjs BASE NAME PASSWORD OUT [--before] [--commit SHA] [--test NAME] [--ca FILE]");
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
  acceptDownloads: true,
});
const page = await context.newPage();
page.setDefaultTimeout(120_000);
await signIn(page, base, name, password);

const problems = [];
page.on("pageerror", (e) => problems.push(String(e)));

const s = new Shots(base, out, { commit: opts.commit ?? "", test: opts.test ?? "" });
try {
  if (opts.before) await before(s, page, base);
  else await after(s, page, base, out);
} catch (err) {
  await page.screenshot({ path: join(out, "failed.png") }).catch(() => {});
  throw err;
} finally {
  await s.save();
}
await browser.close();
if (problems.length > 0) {
  // Thrown: an uncaught error is what makes Bun exit non-zero.
  throw new Error(`the page reported errors:\n${problems.join("\n")}`);
}
