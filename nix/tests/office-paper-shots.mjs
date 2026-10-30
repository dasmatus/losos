// Pictures of LosOS Office's own documents and presentations (.paper and
// .slides, MDX edited as it looks) on a real box, from a real browser.
//
// Chromium (Playwright) signs in through LosOS Login as the owner, starts a
// document and a presentation from LosOS Cloud's files bar, writes into
// them on the page with the bar above it and its Insert menu (the box saves
// what is typed), sets the page up, adds a picture from the computer,
// plays the presentation full screen and opens a document full of script.
// One full-window PNG per state, with a manifest, as
// nix/tests/notes-shots.mjs does. nix/tests/office-paper.nix runs it in its
// laptop VM against the box's own nginx.
//
//   bun office-paper-shots.mjs BASE NAME PASSWORD OUT [--before]
//                               [--commit SHA] [--test NAME] [--ca FILE]
//
// --before takes the same places with the admin UI as it was before: the
// files bar, a .paper file's menu, and the files the way that UI opened
// them.

const join = (...parts) => parts.join("/");

const VIEW = { width: 1280, height: 900 };
const DOC = "Untitled document.paper";
const DECK = "Untitled presentation.slides";
const HOSTILE = "Hostile.paper";
// The laptop's copy of the picture (nix/tests/office-paper.nix).
const PICTURE = "/tmp/tomato.png";

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
    const manifest = { feature: "office-paper", viewport: VIEW, dpr: 2, ...this.meta, steps: this.steps };
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
  await p.locator("[data-testid=paper-toolbar] [data-action=strong]:not([disabled])").waitFor();
}

const pm = (p) => p.locator("[data-testid=paper-editor] .ProseMirror");
const act = (p, key) => p.locator(`[data-testid=paper-toolbar] [data-action="${key}"]`);

async function insert(p, id) {
  await p.getByTestId("paper-insert").click();
  await p.locator(`[data-insert="${id}"]`).click();
  await p.locator("[data-insert]").first().waitFor({ state: "hidden" });
  await p.waitForTimeout(300);
}

/** Waits until the keys go to the page, not the bar. */
async function typing(p) {
  await p.waitForFunction(() => document.activeElement?.closest(".ProseMirror") != null, undefined, { timeout: 30_000 });
}

/** Puts the cursor right after `words` on the page. */
async function caretAfter(p, words) {
  await p.evaluate((w) => {
    const root = document.querySelector("[data-testid=paper-editor] .ProseMirror");
    const walk = document.createTreeWalker(root, NodeFilter.SHOW_TEXT);
    for (let n = walk.nextNode(); n !== null; n = walk.nextNode()) {
      const at = n.data.indexOf(w);
      if (at < 0) continue;
      root.focus();
      const r = document.createRange();
      r.setStart(n, at + w.length);
      r.collapse(true);
      getSelection().removeAllRanges();
      getSelection().addRange(r);
      return;
    }
    throw new Error(`no "${w}" on the page`);
  }, words);
  // The editor takes the new cursor on selectionchange; a key pressed
  // before that is drawn against the old one.
  await p.waitForTimeout(500);
}

/** Selects the `n` characters before the cursor. */
async function selectBack(p, n) {
  for (let i = 0; i < n; i++) await p.keyboard.press("Shift+ArrowLeft");
  await p.waitForTimeout(300);
}

async function after(s, p, base) {
  await p.goto(`${base}/cloud/files`);
  const newDoc = p.getByRole("button", { name: "New document" });
  await newDoc.waitFor();

  // A document, written on the page.
  await newDoc.click();
  await p.waitForURL(/\/cloud\/office\/\d+$/);
  await waitPaper(p, DOC);
  await caretAfter(p, "Untitled document");
  await selectBack(p, "Untitled document".length);
  await p.keyboard.type("Garden day");
  await p.keyboard.press("Enter");
  await p.keyboard.type("Saturday at the allotment, with everyone");
  await selectBack(p, "everyone".length);
  await act(p, "strong").click();
  await p.keyboard.press("End");
  await insert(p, "footnote");
  await typing(p);
  await p.keyboard.type("Bring a hat if it is sunny.");
  await caretAfter(p, "everyone");
  await p.keyboard.press("End");
  await p.keyboard.type(".");

  // Each component goes in after the block the cursor is in, so they are
  // put in from the end of that paragraph, last first, and the footnote's
  // text stays at the end of the document on its own.

  // A callout, its kind picked in the fields under the bar.
  await insert(p, "callout");
  await typing(p);
  await p.keyboard.type("Gloves, water and the seed list.");
  await p.getByTestId("paper-fields").locator("select[data-field=type], [data-field=type] select").selectOption("tip");

  // A picture from the computer, uploaded beside the document.
  await caretAfter(p, "everyone");
  await p.keyboard.press("End");
  const [chooser] = await Promise.all([p.waitForEvent("filechooser"), insert(p, "picture")]);
  await chooser.setFiles(PICTURE);
  await pm(p).locator("figure img").waitFor();
  await p.getByTestId("paper-fields").locator("[data-field=caption]").fill("This year's first tomatoes");

  // A table, filled with Tab, its spare column deleted with the table tools.
  await caretAfter(p, "everyone");
  await p.keyboard.press("End");
  await insert(p, "table");
  await typing(p);
  for (const cell of ["Bed", "Crop", "", "1", "Tomatoes", "", "2", "Beans"]) {
    if (cell !== "") await p.keyboard.type(cell);
    await p.keyboard.press("Tab");
  }
  await caretAfter(p, "Crop");
  await p.keyboard.press("Tab");
  await p.getByTestId("paper-table-tools").getByRole("button", { name: "Delete column" }).click();

  // A footer with the page number, and the page set up for Letter.
  await insert(p, "footer");
  await typing(p);
  await p.keyboard.type("Page ");
  await act(p, "pageSetup").click();
  const setup = p.getByTestId("paper-page-setup");
  await setup.locator("select[data-field=size]").selectOption("Letter");
  await setup.locator("[data-action=apply]").click();
  await p.locator("[data-testid=paper-page][data-size=Letter]").waitFor();

  await caretAfter(p, "everyone");
  await p.locator('[data-testid=office-saved][data-connection="live"]').waitFor();
  await p.getByTestId("paper-desk").evaluate((d) => d.scrollTo(0, 0));
  await s.take(p, "document", "a new document, written on the page with the bar above it", { kind: "none" }, {
    bar: p.getByTestId("paper-toolbar"),
    page: p.getByTestId("paper-page"),
    name: p.getByTestId("paper-name"),
  });

  // Everything the Insert menu puts on a page.
  await p.getByTestId("paper-insert").click();
  await p.locator("[data-insert=table]").waitFor();
  await s.take(p, "insert", "the Insert menu: tables, pictures, charts, equations, callouts, page parts", { kind: "click", target: "insert" }, {
    insert: p.getByTestId("paper-insert"),
  });
  await p.keyboard.press("Escape");

  // A component's fields, under the bar.
  await caretAfter(p, "the seed list");
  await p.getByTestId("paper-fields").waitFor();
  // The callout sits under the picture: bring it up beside its fields.
  await pm(p).locator("aside.callout").evaluate((el) => el.scrollIntoView({ block: "center" }));
  await p.waitForTimeout(300);
  await s.take(p, "fields", "the callout's fields under the bar", { kind: "none" }, {
    fields: p.getByTestId("paper-fields"),
    callout: pm(p).locator("aside.callout"),
  });

  // A presentation: slides in the strip, each a 16:9 card on the desk.
  await p.goto(`${base}/cloud/files`);
  await p.getByRole("button", { name: "New presentation" }).click();
  await p.waitForURL(/\/cloud\/office\/\d+$/);
  await waitPaper(p, DECK);
  await caretAfter(p, "What it is about");
  await p.locator("[data-testid=paper-toolbar] [data-action=slide]").click();
  await p.keyboard.type("This year");
  await p.keyboard.press("Enter");
  await p.locator("[data-testid=paper-toolbar] [data-action=numbers]").click();
  for (const line of ["Rotate the beds", "Water in the morning", "Feed with compost"]) {
    await p.keyboard.type(line);
    await p.keyboard.press("Enter");
  }
  await p.keyboard.press("Enter");
  await insert(p, "notes");
  await typing(p);
  await p.keyboard.type("Thank everyone who came last year.");
  await p.waitForFunction(() => document.querySelectorAll("[data-testid=paper-filmstrip] button").length === 3);
  await p.locator('[data-testid=office-saved][data-connection="live"]').waitFor();
  await s.take(p, "presentation", "a new presentation: slides in the strip, each a card on the desk", { kind: "click", target: "present" }, {
    strip: p.getByTestId("paper-filmstrip"),
    present: p.getByTestId("paper-present"),
  });

  await p.getByTestId("paper-present").click();
  const show = p.getByTestId("paper-presenter");
  await show.waitFor();
  await p.keyboard.press("Home");
  await show.getByRole("heading", { name: "Untitled presentation" }).waitFor();
  await s.take(p, "presenting", "playing it full screen, from the first slide", { kind: "key", key: "ArrowRight" }, {
    slide: show,
  });
  await p.keyboard.press("Escape");
  await show.waitFor({ state: "detached" });

  // A document full of script: drawn, kept as written, and nothing runs.
  await p.goto(`${base}/cloud/files`);
  const row = p.locator(`[data-entry="${HOSTILE}"]`);
  await row.waitFor();
  const href = await row.locator("a[href]").first().getAttribute("href");
  await p.goto(new URL(href, `${base}/cloud/`).toString());
  await waitPaper(p, HOSTILE);
  await pm(p).locator(".paper-raw").first().waitFor();
  for (const el of await pm(p).locator("a, img, .paper-raw").all()) await el.click({ force: true }).catch(() => {});
  await p.waitForTimeout(500);
  const ran = await p.evaluate(() => window.__ran);
  if (ran !== undefined) throw new Error(`script in the document ran: ${ran}`);
  if ((await pm(p).locator("script, [onerror], [onclick]").count()) > 0) throw new Error("the page holds live script");
  await s.take(p, "script", "a document with script in it: kept as written, nothing runs", { kind: "none" }, {
    page: p.getByTestId("paper-page"),
  });

  await p.goto(`${base}/cloud/files`);
  await files(s, p, false);
}

/** The files bar with the files in it, and the document's menu. */
async function files(s, p, old) {
  const row = p.locator(`[data-entry="${DOC}"]`);
  await row.waitFor();
  await s.take(p, "files", old ? "the files bar" : "the files bar: New document makes a .paper, New presentation a .slides", { kind: "click", target: "newDoc" }, {
    newDoc: p.getByRole("button", { name: "New document" }),
    newDeck: p.getByRole("button", { name: "New presentation" }),
  });
  await row.click({ button: "right" });
  const menu = p.locator('[data-slot="context-menu"], [data-row-menu]').first();
  await menu.waitFor();
  await s.take(p, "menu", "a right click on a .paper document", { kind: "none" }, {
    open: menu.getByRole("menuitem", { name: "Open", exact: true }),
  });
  await p.keyboard.press("Escape");
}

async function before(s, p, base) {
  await p.goto(`${base}/cloud/files`);
  await files(s, p, true);
  // The files the way this UI opened them: their first opener, if any.
  for (const [name, slug] of [
    [DOC, "document"],
    [DECK, "presentation"],
    [HOSTILE, "script"],
  ]) {
    await p.goto(`${base}/cloud/files`);
    const row = p.locator(`[data-entry="${name}"]`);
    await row.waitFor();
    const link = row.locator("a[href]").first();
    const href = (await link.count()) > 0 ? await link.getAttribute("href") : null;
    if (href !== null) await p.goto(new URL(href, `${base}/cloud/`).toString());
    await p.waitForTimeout(1500);
    await s.take(p, slug, `${name} as it opened before`);
  }
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
    throw new Error("usage: office-paper-shots.mjs BASE NAME PASSWORD OUT [--before] [--commit SHA] [--test NAME] [--ca FILE]");
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
page.setDefaultTimeout(120_000);
await signIn(page, base, name, password);

// What the page logs on a plain load: Office may log nothing beyond it.
const baseline = new Set();
const problems = [];
let collecting = false;
page.on("console", (m) => {
  if (m.type() !== "error") return;
  // A box without Collabora answers Office's list of types with 404.
  if (/status of 404/.test(m.text()) && /\/cloud\/api\/office\//.test(m.location().url)) return;
  // Hostile.paper names a picture "x" that is not there.
  if (/status of 404/.test(m.text()) && /\/x$/.test(m.location().url)) return;
  if (collecting) {
    if (!baseline.has(m.text())) problems.push(`${m.text()} ${m.location().url}`);
  } else baseline.add(m.text());
});
page.on("pageerror", (e) => problems.push(String(e)));
await page.goto(`${base}/cloud/files`);
await page.waitForLoadState("networkidle");
collecting = !opts.before;

const s = new Shots(base, out, { commit: opts.commit ?? "", test: opts.test ?? "" });
try {
  if (opts.before) await before(s, page, base);
  else await after(s, page, base);
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
