// Pictures of Markdown documents and presentations in LosOS Office on a
// real box, from a real browser.
//
// Chromium (Playwright) signs in through LosOS Login as the owner, starts a
// document and a presentation from LosOS Cloud's files bar, writes into
// them with the formatting bar and the keyboard (the box saves what is
// typed), puts the presentation's picture on the right of its slide and
// plays it full screen. One full-window PNG per state, with a manifest, as
// nix/tests/notes-shots.mjs does. nix/tests/office-markdown.nix runs it in
// its laptop VM against the box's own nginx.
//
//   node office-markdown-shots.mjs BASE NAME PASSWORD OUT [--before]
//                                  [--commit SHA] [--test NAME] [--ca FILE]
//
// --before takes the same places with the admin UI as it was before: the
// files bar, a Markdown file's menu, and the two files the way that UI
// opened them.

import { createHash, X509Certificate } from "node:crypto";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const VIEW = { width: 1280, height: 900 };
const DOC = "Untitled document.md";
const DECK = "Untitled presentation.md";

const { chromium } = await import(process.env.PLAYWRIGHT_CORE ?? "playwright-core");

function spkiPin(pemFile) {
  const cert = new X509Certificate(readFileSync(pemFile));
  const der = cert.publicKey.export({ type: "spki", format: "der" });
  return createHash("sha256").update(der).digest("base64");
}

class Shots {
  constructor(base, out, meta) {
    this.host = new URL(base).hostname;
    this.out = out;
    this.meta = meta;
    this.steps = [];
    mkdirSync(out, { recursive: true });
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

  save() {
    const manifest = { feature: "office-markdown", viewport: VIEW, dpr: 2, ...this.meta, steps: this.steps };
    writeFileSync(join(this.out, "manifest.json"), JSON.stringify(manifest, null, 2) + "\n");
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

/** Waits for the Markdown editor, live with the box. */
async function waitOffice(p, name) {
  await p.locator("[data-markdown-office]").waitFor();
  await p.getByRole("heading", { level: 1, name }).waitFor();
  await p.locator('[data-testid=office-saved][data-connection="live"]').waitFor();
  await p.locator("[data-testid=md-toolbar] [data-action=bold]:not([disabled])").waitFor();
}

const tool = (p, name) => p.getByTestId("md-toolbar").getByRole("button", { name, exact: true });

/** Types at the end of the text, as a person would. */
async function write(p, text) {
  await p.locator(".cm-content").focus();
  await p.keyboard.press("Control+End");
  await p.keyboard.insertText(text);
}

/** Selects `word` on the line that ends the text. */
async function selectLast(p, word) {
  await p.keyboard.press("Control+End");
  for (let i = 0; i < word.length; i++) await p.keyboard.press("Shift+ArrowLeft");
}

async function after(s, p, base) {
  await p.goto(`${base}/cloud/files`);
  const newDoc = p.getByRole("button", { name: "New document" });
  await newDoc.waitFor();

  // A document: the bar writes the Markdown, the right side reads it.
  await newDoc.click();
  await p.waitForURL(/\/cloud\/office\/\d+$/);
  await waitOffice(p, DOC);
  await write(p, "Saturday at the allotment, with ");
  await write(p, "everyone");
  await selectLast(p, "everyone");
  await tool(p, "Bold").click();
  await write(p, " and a few ");
  await write(p, "tools");
  await selectLast(p, "tools");
  await tool(p, "Highlight").click();
  await write(p, ". See [[Planting calendar]] #garden\n\n");
  await write(p, "Bring gloves");
  await tool(p, "Task list").click();
  await write(p, "\n- [x] Order the compost\n\n");
  await p.getByTestId("md-toolbar").getByLabel("Callout").selectOption("tip");
  await write(p, "\n\n| Bed | Crop |\n| --- | --- |\n| 1 | Tomatoes |\n| 2 | Beans |\n\n$$\nA = w \\times l\n$$\n");
  const read = p.getByTestId("md-read");
  await read.locator("table").waitFor();
  await read.locator(".callout").waitFor();
  await p.locator('[data-testid=office-saved][data-connection="live"]').waitFor();
  await s.take(p, "document", "a new document, written with the bar and read beside it", { kind: "none" }, {
    bar: p.getByTestId("md-toolbar"),
    read,
    name: p.getByTestId("md-name"),
  });

  // The bar's callouts, the menu GitHub's alerts and Obsidian's share.
  await p.getByTestId("md-toolbar").getByRole("button", { name: "Bold", exact: true }).hover();
  await p.getByRole("tooltip").waitFor().catch(() => {});
  await s.take(p, "bar", "the formatting bar, every Markdown action a button", { kind: "hover", target: "bold" }, {
    bold: tool(p, "Bold"),
  });

  // A presentation with a picture: the text centred, the picture on the right.
  await p.goto(`${base}/cloud/files`);
  await p.getByRole("button", { name: "New presentation" }).click();
  await p.waitForURL(/\/cloud\/office\/\d+$/);
  await waitOffice(p, DECK);
  await p.locator("[data-markdown-office][data-deck]").waitFor();
  await write(p, "\n![A tomato plant](tomato.png)\n\n---\n\n## This year\n\n1. Rotate the beds\n2. Water in the morning\n3. Feed with compost\n");
  await p.locator(".cm-line", { hasText: "What it is about" }).first().click();
  await p.keyboard.press("End");
  const stage = p.getByTestId("md-stage");
  await stage.locator(".md-slide-pictures img").waitFor();
  await p.locator('[data-testid=office-saved][data-connection="live"]').waitFor();
  await s.take(p, "presentation", "a new presentation: slides in the strip, the slide beside its text", { kind: "click", target: "present" }, {
    strip: p.getByTestId("md-strip"),
    stage,
    present: p.getByTestId("md-present"),
  });

  await p.getByTestId("md-present").click();
  const show = p.getByTestId("md-presenter");
  await show.waitFor();
  await show.locator(".md-slide-pictures img").waitFor();
  await s.take(p, "presenting", "playing it full screen: text centred, the picture on the right", { kind: "key", key: "ArrowRight" }, {
    slide: show.locator(".md-slide"),
  });
  await p.keyboard.press("Escape");
  await show.waitFor({ state: "detached" });

  await p.goto(`${base}/cloud/files`);
  await files(s, p, false);
}

/** The files bar with both files in it, and the presentation's menu. */
async function files(s, p, old) {
  const row = p.locator(`[data-entry="${DECK}"]`);
  await row.waitFor();
  await s.take(p, "files", old ? "the files bar" : "the files bar, with New document and New presentation", { kind: "click", target: "newDoc" }, {
    newDoc: p.getByRole("button", { name: "New document" }),
    newDeck: p.getByRole("button", { name: "New presentation" }),
  });
  await row.click({ button: "right" });
  await p.locator("[data-row-menu]").waitFor();
  await s.take(p, "menu", "a right click on a Markdown file", { kind: "none" }, {
    office: p.locator('[data-row-menu] [data-action="office-markdown"]'),
  });
  await p.keyboard.press("Escape");
}

async function before(s, p, base) {
  await p.goto(`${base}/cloud/files`);
  await files(s, p, true);
  // The two files the way this UI opened them: their first opener.
  for (const [name, slug] of [
    [DOC, "document"],
    [DECK, "presentation"],
  ]) {
    await p.goto(`${base}/cloud/files`);
    const row = p.locator(`[data-entry="${name}"]`);
    await row.waitFor();
    // The name is a link to the file's first opener, in a tab of its own.
    const href = await row.locator("a[href]").first().getAttribute("href");
    await p.goto(new URL(href, `${base}/cloud/`).toString());
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
    console.error("usage: office-markdown-shots.mjs BASE NAME PASSWORD OUT [--before] [--commit SHA] [--test NAME] [--ca FILE]");
    process.exit(2);
  }
  return { base: pos[0], name: pos[1], password: pos[2], out: pos[3], opts };
}

const { base, name, password, out, opts } = parse(process.argv.slice(2));
const browser = await chromium.launch({
  args: [`--ignore-certificate-errors-spki-list=${spkiPin(opts.ca ?? "/etc/losos-box.pem")}`],
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
  if (collecting) {
    if (!baseline.has(m.text())) problems.push(m.text());
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
  s.save();
}
await browser.close();
if (problems.length > 0) {
  console.error(`the page reported errors:\n${problems.join("\n")}`);
  process.exit(1);
}
