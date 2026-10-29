// Two people on two boxes in one Chromium: Matus on mattbox shares a note
// with Eva on kitchenbox, she accepts it, and they write in it together.
//
// nix/tests/mesh-collab.nix runs it in its laptop VM, one phase at a time,
// and changes the network between the boxes in between; the pictures serve
// the pull request and the launch film. Run it again against any two boxes:
//
//   node mesh-collab-shots.mjs PHASE CONFIG OUT
//
// PHASE is share, apart, merged, readonly or before. CONFIG is a JSON file:
//
//   { "file": "Summer trip.md",
//     "owner": { "base": "https://mattbox.local", "name": "matus", "password": "…" },
//     "guest": { "base": "https://kitchenbox.local", "name": "eva", "password": "…" },
//     "certs": ["/etc/losos-mattbox.pem", "/etc/losos-kitchenbox.pem"],
//     "commit": "abc1234", "test": "losos-mesh-collab" }
//
// The owner's box holds the file; the guest's address is name@<guest host>.
// OUT collects NN-<box>-<slug>.png and one manifest.json; each phase adds
// to what the phases before it left there. The browser trusts each box's
// certificate by its public key.
//
// Playwright comes from PLAYWRIGHT_CORE (a path to playwright-core, as
// nixpkgs' playwright-driver is) or the `playwright-core` package.
//
// Each window is 1280 x 900 at twice the pixels, dark, en-US, reduced
// motion, Europe/Bratislava. Each picture waits for its page to settle:
// no request in flight, fonts loaded, no spinner and no toast on screen.
// A console error, a page error or a refusal by the page's security policy
// that a plain Files page does not also log fails the run, except in the
// before phase.

const exists = (file) => Bun.file(file).exists();
const join = (...parts) => parts.join("/");

const VIEW = { width: 1280, height: 900 };
const WAIT = 180_000;

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

/** The pictures of both windows, in the order a person sees them. */
class Shots {
  constructor(out, meta, steps) {
    this.out = out;
    this.steps = steps;
    this.meta = meta;
  }

  /** A run that goes on from the manifest an earlier phase left in `out`. */
  static async open(out, meta) {
    const manifest = Bun.file(join(out, "manifest.json"));
    const steps = (await manifest.exists()) ? (await manifest.json()).steps : [];
    return new Shots(out, meta, steps);
  }

  async take(w, slug, title, action = { kind: "none" }, targets = {}) {
    await settle(w.page);
    const file = `${String(this.steps.length + 1).padStart(2, "0")}-${w.box}-${slug}.png`;
    await w.page.screenshot({ path: join(this.out, file), timeout: WAIT });
    const found = {};
    for (const [key, loc] of Object.entries(targets)) {
      const b = await loc.boundingBox().catch(() => null);
      if (b) found[key] = { x: Math.round(b.x), y: Math.round(b.y), w: Math.round(b.width), h: Math.round(b.height) };
    }
    const url = new URL(w.page.url());
    this.steps.push({ file, url: `${url.host}${url.pathname}${url.search}`, title, action, targets: found });
    console.log(file, title);
    await this.save();
  }

  async save() {
    const manifest = { feature: "mesh-collab", viewport: VIEW, dpr: 2, ...this.meta, steps: this.steps };
    await Bun.write(join(this.out, "manifest.json"), JSON.stringify(manifest, null, 2) + "\n");
  }
}

async function settle(p) {
  await p.waitForLoadState("networkidle").catch(() => {});
  await p.evaluate(() => document.fonts.ready.then(() => true));
  await p.waitForFunction(
    () => {
      const seen = (el) => {
        const r = el.getBoundingClientRect();
        return r.bottom > 0 && r.top < innerHeight && r.width > 0;
      };
      if ([...document.images].filter(seen).some((i) => !i.complete || i.naturalWidth === 0)) return false;
      if ([...document.querySelectorAll("[data-slot=spinner]")].some(seen)) return false;
      if (document.querySelector("[data-sonner-toast]")) return false;
      return true;
    },
    undefined,
    { timeout: WAIT },
  );
  // The film draws its own pointer and caret. A CSSOM write, which the
  // box's style-src 'self' allows; the editor's own cursor is in its
  // shadow root and goes the same way.
  await p.evaluate(() => {
    document.documentElement.style.caretColor = "transparent";
    const host = document.querySelector("[data-testid=collab-editor]");
    for (const c of host?.shadowRoot?.querySelectorAll(".cm-cursor") ?? []) c.style.display = "none";
  });
  await p.waitForTimeout(400);
}

/** One person's window: their own browser profile, signed in to their box. */
async function window(browser, who, box, problems, strict) {
  const context = await browser.newContext({
    viewport: VIEW,
    deviceScaleFactor: 2,
    colorScheme: "dark",
    locale: "en-US",
    timezoneId: "Europe/Bratislava",
    reducedMotion: "reduce",
  });
  // An emulated laptop loading the whole bundle takes longer than
  // Playwright's 30 s default for a page load.
  context.setDefaultNavigationTimeout(WAIT);
  // A refusal by the page's policy shows up here even when nothing logs it.
  await context.addInitScript(() => {
    document.addEventListener("securitypolicyviolation", (e) => {
      // The element a refused style attribute was on says what wrote it.
      const on = e.target instanceof Element ? ` on ${e.target.outerHTML.slice(0, 200)}` : "";
      console.error(`policy refused ${e.violatedDirective}: ${e.blockedURI || "inline"} (${e.sourceFile}:${e.lineNumber})${on}`);
    });
  });
  const page = await context.newPage();
  const r = await page.request.post(`${who.base}/auth/api/firstfactor`, {
    data: { username: who.name, password: who.password, keepMeLoggedIn: false, targetURL: `${who.base}/cloud/` },
  });
  if (!r.ok()) throw new Error(`${box}: sign-in refused: ${r.status()} ${await r.text()}`);
  if (strict) {
    // What a plain Files page logs is the baseline (the admin bundle's
    // toast stylesheet, which style-src 'self' refuses, as
    // tests/*.browser.mjs baseline it); sharing and editing may add nothing.
    const baseline = new Set();
    let sink = (text) => baseline.add(text);
    page.on("console", (m) => {
      if (m.type() === "error") sink(m.text());
    });
    page.on("pageerror", (e) => sink(String(e)));
    await page.goto(`${who.base}/cloud/files`);
    await page.waitForLoadState("networkidle");
    sink = (text) => {
      if (!baseline.has(text)) problems.push(`${box}: ${text}`);
    };
  }
  return { page, box, base: who.base, who };
}

const editorOf = (p) => p.locator("[data-testid=collab-editor] .cm-content");

async function text(p) {
  return editorOf(p).evaluate((el) => el.innerText.replace(/ /g, " "));
}

async function waitText(p, pred, what) {
  const until = Date.now() + WAIT;
  for (;;) {
    const t = await text(p).catch(() => "");
    if (pred(t)) return t;
    if (Date.now() > until) throw new Error(`never happened: ${what}; the editor says:\n${t}`);
    await p.waitForTimeout(250);
  }
}

async function openEditor(w) {
  await w.page.waitForSelector("[data-testid=collab-editor][data-ready]", { timeout: WAIT });
  await w.page.waitForSelector("[data-connection=live]", { timeout: WAIT });
}

/** One row of Shared with you, by the file's name. */
function incoming(page, file) {
  return page.locator("[data-incoming-share]").filter({ hasText: file });
}

/** The guest's copy, from Shared with you. */
async function openShared(w, file) {
  await w.page.goto(`${w.base}/cloud/shared`);
  const open = incoming(w.page, file).locator("[data-open]");
  await open.waitFor({ timeout: WAIT });
  await open.click();
  await openEditor(w);
}

/** The owner's file, from Files. */
async function openOwn(w, file) {
  await w.page.goto(`${w.base}/cloud/files`);
  const row = w.page.locator(`[data-entry="${file}"]`);
  await row.waitFor({ timeout: WAIT });
  // Files open their app in a new tab; the pictures keep one window each.
  const href = await row.getByRole("link", { name: file, exact: true }).getAttribute("href");
  await w.page.goto(new URL(href, w.base).href);
  // A Markdown file opens in Notes to read; the shared editor is its Edit view.
  const edit = w.page.locator("[data-view=edit]");
  await edit.waitFor({ timeout: WAIT });
  await edit.click();
  await openEditor(w);
}

async function endOfText(w) {
  await editorOf(w.page).click();
  await w.page.keyboard.press("Control+End");
}

/**
 * Starts a fresh line at the end of the text: a new one unless the last is
 * empty already. Enter on a list item carries the "- " over, so whatever the
 * editor put at the start of the line is selected and the typing replaces it.
 */
async function newLine(w) {
  await endOfText(w);
  const last = await editorOf(w.page).locator(".cm-line").last().innerText();
  if (last.trim() === "") return;
  await w.page.keyboard.press("Enter");
  await w.page.keyboard.press("Shift+Home");
}

const phases = {
  async share(s, m, e, cfg) {
    const file = cfg.file;
    const guest = `${e.who.name}@${new URL(e.base).hostname}`;
    const p = m.page;
    await p.goto(`${m.base}/cloud/files`);
    const pick = p.getByRole("checkbox", { name: `Select ${file}` });
    await pick.waitFor({ timeout: WAIT });
    await s.take(m, "files", "Matus's files on mattbox", { kind: "click", target: "file" }, { file: pick });
    await pick.check();
    await p.waitForSelector("[data-selection]");
    const share = p.locator("[data-selection]").getByRole("button", { name: "Share", exact: true });
    await s.take(m, "selected", "one note picked, with Share among what can be done", { kind: "click", target: "share" }, {
      share,
      bar: p.locator("[data-selection]"),
    });
    await share.click();
    const form = p.locator("[data-mesh-share-form]");
    await form.waitFor({ timeout: WAIT });
    const field = form.getByRole("textbox");
    await s.take(m, "share-dialog", "sharing with someone on another box", { kind: "type", target: "address" }, {
      address: field,
      mine: p.getByText("Your address for people on other boxes"),
    });
    await field.click();
    let typed = "";
    for (const part of [e.who.name, "@", new URL(e.base).hostname]) {
      typed += part;
      await field.pressSequentially(part);
      await s.take(m, `share-address-${typed.length}`, `typing ${typed}`, { kind: "type", target: "address" }, { address: field });
    }
    const submit = form.getByRole("button", { name: "Share", exact: true });
    await s.take(m, "share-ready", "Eva's address, can edit", { kind: "click", target: "submit" }, { submit });
    await submit.click();
    const row = p.locator(`[data-share="${guest}"]`);
    await row.waitFor({ timeout: WAIT });
    await p.locator(`[data-share="${guest}"] [data-state=pending], [data-share="${guest}"] [data-state=accepted]`).waitFor({ timeout: WAIT });
    await s.take(m, "share-sent", "the offer is on its way to kitchenbox", { kind: "none" }, { row });

    const q = e.page;
    await q.goto(`${e.base}/cloud/shared`);
    const offer = incoming(q, file);
    await offer.waitFor({ timeout: WAIT });
    const accept = offer.locator("[data-accept]");
    await accept.waitFor({ timeout: WAIT });
    await s.take(e, "incoming", "on kitchenbox, Eva sees what Matus shared", { kind: "click", target: "accept" }, { accept, offer });
    await accept.click();
    const open = offer.locator("[data-open]");
    await open.waitFor({ timeout: WAIT });
    await s.take(e, "accepted", "accepted, ready to open", { kind: "click", target: "open" }, { open });

    await p.locator(`[data-share="${guest}"] [data-state=accepted]`).waitFor({ timeout: WAIT });
    await s.take(m, "share-accepted", "Matus sees that Eva accepted", { kind: "click", target: "done" }, {
      row,
      done: p.getByRole("button", { name: "Done", exact: true }),
    });
    await p.getByRole("button", { name: "Done", exact: true }).click();

    await openOwn(m, file);
    await s.take(m, "editor", "Matus opens the note", { kind: "none" }, { faces: p.locator("[data-faces]") });
    await open.click();
    await openEditor(e);
    const both = (page) => page.waitForFunction(() => document.querySelectorAll("[data-faces] [data-face]").length >= 2, undefined, { timeout: WAIT });
    await both(q);
    await both(p);
    await s.take(e, "editor", "Eva opens her copy on kitchenbox; Matus is in it too", { kind: "none" }, { faces: q.locator("[data-faces]") });
    await s.take(m, "together", "Matus sees Eva arrive", { kind: "none" }, { faces: p.locator("[data-faces]") });

    // Eva writes a line, one picture per word in both windows.
    await newLine(e);
    const words = cfg.evaSays.split(" ");
    let line = "";
    for (const [i, word] of words.entries()) {
      const piece = i === 0 ? word : ` ${word}`;
      line += piece;
      await q.keyboard.type(piece);
      const want = line;
      await waitText(p, (t) => t.includes(want), `Matus sees "${want}"`);
      await s.take(e, `eva-types-${i + 1}`, `Eva types: ${line}`, { kind: "type", target: "editor" }, { editor: editorOf(q) });
      await s.take(m, `eva-types-${i + 1}`, `Matus watches: ${line}`, { kind: "type", target: "editor" }, { editor: editorOf(p) });
    }
    // Matus adds a line under hers; his caret shows in her window.
    await newLine(m);
    await p.keyboard.type(cfg.matusSays);
    await waitText(q, (t) => t.includes(cfg.matusSays), "Eva sees Matus's line");
    await s.take(m, "matus-types", "Matus adds a line", { kind: "none" }, { editor: editorOf(p) });
    await s.take(e, "matus-types", "Eva sees his line and his name at his cursor", { kind: "none" }, {
      caret: q.locator("[data-testid=collab-editor] .cm-who-caret").first(),
    });
    await q.locator("[data-edited]").getByText("Matus").waitFor({ timeout: WAIT }).catch(() => {});
    await s.take(e, "edited-by", "who wrote last, under the note", { kind: "none" }, { edited: q.locator("[data-edited]") });
  },

  async apart(s, m, e, cfg) {
    // The test has cut kitchenbox off from mattbox.
    await openShared(e, cfg.file);
    await e.page.locator("[data-apart]").waitFor({ timeout: WAIT });
    await s.take(e, "apart", "kitchenbox cannot reach mattbox; Eva keeps writing in her copy", { kind: "type", target: "editor" }, {
      alert: e.page.locator("[data-apart]"),
    });
    await newLine(e);
    await e.page.keyboard.type(cfg.evaApart);
    await s.take(e, "apart-typed", "a line written while apart", { kind: "none" }, { editor: editorOf(e.page) });
    await openOwn(m, cfg.file);
    await newLine(m);
    await m.page.keyboard.type(cfg.matusApart);
    await s.take(m, "apart-typed", "Matus writes on mattbox meanwhile", { kind: "none" }, { editor: editorOf(m.page) });
    await e.page.goto(`${e.base}/cloud/shared`);
    await incoming(e.page, cfg.file).getByText("Changes waiting for").waitFor({ timeout: WAIT });
    await s.take(e, "unsynced", "Shared with you says the copy has changes mattbox has not seen", { kind: "none" }, {
      offer: incoming(e.page, cfg.file),
    });
  },

  async merged(s, m, e, cfg) {
    // The test has joined the boxes again.
    await openShared(e, cfg.file);
    await openOwn(m, cfg.file);
    const both = (t) => t.includes(cfg.evaApart) && t.includes(cfg.matusApart);
    await waitText(e.page, both, "Eva's copy has both lines");
    await waitText(m.page, both, "Matus's note has both lines");
    await s.take(e, "merged", "back in reach: both lines in Eva's copy", { kind: "none" }, { editor: editorOf(e.page) });
    await s.take(m, "merged", "and in Matus's note", { kind: "none" }, { editor: editorOf(m.page) });
  },

  async readonly(s, m, e, cfg) {
    const guest = `${e.who.name}@${new URL(e.base).hostname}`;
    const p = m.page;
    await p.goto(`${m.base}/cloud/files`);
    const pick = p.getByRole("checkbox", { name: `Select ${cfg.file}` });
    await pick.waitFor({ timeout: WAIT });
    await pick.check();
    await p.locator("[data-selection]").getByRole("button", { name: "Share", exact: true }).click();
    const perm = p.getByRole("combobox", { name: `What ${guest} can do` });
    await perm.waitFor({ timeout: WAIT });
    await perm.selectOption("read");
    await p.waitForTimeout(1000);
    await s.take(m, "view-only", "Matus lets Eva view only", { kind: "none" }, { perm });
    await p.getByRole("button", { name: "Done", exact: true }).click();
    await openShared(e, cfg.file);
    await e.page.getByText("View only", { exact: true }).first().waitFor({ timeout: WAIT });
    const before = await text(e.page);
    await editorOf(e.page).click();
    await e.page.keyboard.type("Sneaky");
    await e.page.waitForTimeout(1500);
    if ((await text(e.page)) !== before) throw new Error("a view-only copy took typing");
    await s.take(e, "view-only", "Eva can read the note and not change it", { kind: "none" }, { editor: editorOf(e.page) });
  },

  async before(s, m, e, cfg) {
    // The same addresses on the boxes without editing together.
    const p = m.page;
    await p.goto(`${m.base}/cloud/files`);
    const pick = p.getByRole("checkbox", { name: `Select ${cfg.file}` });
    await pick.waitFor({ timeout: WAIT });
    await pick.check();
    await p.waitForSelector("[data-selection]");
    await s.take(m, "selected", "one note picked, before", { kind: "none" }, { bar: p.locator("[data-selection]") });
    await p.goto(`${m.base}/cloud/edit/${cfg.fileId}`);
    await s.take(m, "editor", "the editor's address, before", { kind: "none" });
    await e.page.goto(`${e.base}/cloud/shared`);
    await s.take(e, "shared", "Shared with you, before", { kind: "none" });
  },
};

const [phase, configFile, out] = Bun.argv.slice(2);
if (!(phase in phases) || configFile === undefined || out === undefined) {
  // Thrown: an uncaught error is what makes Bun exit non-zero.
  throw new Error(`usage: mesh-collab-shots.mjs ${Object.keys(phases).join("|")} CONFIG OUT`);
}
const cfg = {
  evaSays: "- Pack the tent and two sleeping bags",
  matusSays: "- I'll bring the camping stove",
  evaApart: "- Buy a map of the Wachau",
  matusApart: "- Leave at seven",
  ...(await Bun.file(configFile).json()),
};
const strict = phase !== "before";
const browser = await chromium.launch({
  args: [`--ignore-certificate-errors-spki-list=${cfg.certs.map(spkiPin).join(",")}`],
});
const problems = [];
const s = await Shots.open(out, { commit: cfg.commit ?? "", test: cfg.test ?? "" });
const hostOf = (who) => new URL(who.base).hostname.split(".")[0];
const m = await window(browser, cfg.owner, hostOf(cfg.owner), problems, strict);
const e = await window(browser, cfg.guest, hostOf(cfg.guest), problems, strict);
try {
  await phases[phase](s, m, e, cfg);
} catch (err) {
  // What each window showed when it went wrong.
  for (const w of [m, e]) {
    console.error(`${w.box} was at ${w.page.url()}`);
    await w.page.screenshot({ path: join(out, `failed-${phase}-${w.box}.png`) }).catch(() => {});
  }
  throw err;
} finally {
  await browser.close();
}
if (problems.length > 0) {
  // Thrown: an uncaught error is what makes Bun exit non-zero.
  throw new Error(`the pages reported errors:\n${problems.join("\n")}`);
}
