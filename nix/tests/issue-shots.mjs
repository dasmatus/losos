// Pictures of LosOS Git's issue tracker on a real box, from a real browser.
//
// Chromium (Playwright) signs in through LosOS Login as the owner and works
// through the issues the way a person would, one full-window PNG per state,
// with a manifest that says where each picture was taken and where the
// things on it are. nix/tests/git-issues.nix runs it in its laptop VM
// against the box's own nginx; the same pictures serve the pull request and
// the launch film.
//
//   node issue-shots.mjs BASE NAME PASSWORD OUT PHASE [--commit SHA] [--test NAME] [--ca FILE]
//
// PHASE is one of
//
//   tracker  my issues, the backup-tool project as a list, an issue a commit
//            closed, the board and a card dragged to In Progress, the
//            status menu from the keyboard, the palette (Ctrl+K) finding an
//            issue a pull request closed, a new issue typed word by word,
//            the inbox, the cycles, the Garden board LosOS Cloud had, and
//            the repository with its issues in the sidebar;
//   before   the same repository and /git/issues on pages without the
//            tracker.
//
// The window is 1280 x 900 at twice the pixels, dark, en-US, reduced motion,
// Europe/Bratislava. Each picture waits for the page to settle: no request
// in flight, fonts and pictures loaded, no skeleton and no toast on screen.

import { createHash, X509Certificate } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const VIEW = { width: 1280, height: 900 };
const { chromium } = await import(process.env.PLAYWRIGHT_CORE ?? "playwright-core");

function spkiPin(pemFile) {
  const cert = new X509Certificate(readFileSync(pemFile));
  const der = cert.publicKey.export({ type: "spki", format: "der" });
  return createHash("sha256").update(der).digest("base64");
}

class Shots {
  constructor(page, base, out, meta) {
    this.page = page;
    this.host = new URL(base).hostname;
    this.out = out;
    this.meta = meta;
    mkdirSync(out, { recursive: true });
    const manifest = join(out, "manifest.json");
    this.steps = existsSync(manifest) ? JSON.parse(readFileSync(manifest, "utf8")).steps : [];
  }

  async settle() {
    const p = this.page;
    await p.waitForLoadState("networkidle");
    await p.evaluate(() => document.fonts.ready.then(() => true));
    await p.waitForFunction(
      () => {
        const seen = (el) => {
          const r = el.getBoundingClientRect();
          return r.bottom > 0 && r.top < innerHeight && r.width > 0;
        };
        const imgs = [...document.images].filter(seen);
        if (imgs.some((i) => !i.complete || i.naturalWidth === 0)) return false;
        if ([...document.querySelectorAll("[data-skeleton], [data-slot=spinner]")].some(seen)) return false;
        if (document.querySelector("[data-sonner-toast]")) return false;
        return true;
      },
      undefined,
      { timeout: 120_000 },
    );
    await p.evaluate(() => {
      document.documentElement.style.caretColor = "transparent";
    });
    await p.waitForTimeout(300);
  }

  async take(slug, title, action = { kind: "none" }, targets = {}) {
    await this.settle();
    const file = `${String(this.steps.length + 1).padStart(2, "0")}-${slug}.png`;
    await this.page.screenshot({ path: join(this.out, file), timeout: 180_000 });
    const found = {};
    for (const [key, loc] of Object.entries(targets)) {
      const b = await loc.boundingBox().catch(() => null);
      if (b) found[key] = { x: Math.round(b.x), y: Math.round(b.y), w: Math.round(b.width), h: Math.round(b.height) };
    }
    const url = new URL(this.page.url());
    this.steps.push({ file, url: `${this.host}${url.pathname}${url.search}`, title, action, targets: found });
    console.log(file, title);
  }

  save() {
    const manifest = { feature: "issues", viewport: VIEW, dpr: 2, ...this.meta, steps: this.steps };
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
    [name, password, `${base}/git/`],
  );
  if (status !== 200) throw new Error(`sign-in refused: ${status}`);
}

const row = (p, key) => p.locator(`[data-issue-key="${key}"]`).first();

/** Typed one word at a time, a picture after each, as the film shows typing. */
async function typeWords(s, text, slug, title, targets) {
  const words = text.split(" ");
  for (let i = 0; i < words.length; i++) {
    await s.page.keyboard.type((i > 0 ? " " : "") + words[i]);
    await s.take(`${slug}-${i + 1}`, `${title}: "${words.slice(0, i + 1).join(" ")}"`, { kind: "type" }, targets());
  }
}

async function tracker(s, base) {
  const p = s.page;
  await p.goto(`${base}/git/issues`);
  await p.locator("[data-tracker-scope=mine]").waitFor();
  await row(p, "BT-5").waitFor();
  const project = p.locator("[data-slot=sidebar-menu-button]", { hasText: "backup-tool" }).first();
  await s.take("my-issues", "the issues given to me, across projects", { kind: "click", target: "project" }, {
    project,
    inbox: p.locator("[data-slot=sidebar-menu-button]", { hasText: "Inbox" }).first(),
  });

  await project.click();
  await p.locator("[data-tracker-scope=project]").waitFor();
  await row(p, "BT-7").waitFor();
  await s.take("project", "the backup-tool project, grouped by status", { kind: "click", target: "issue" }, {
    issue: row(p, "BT-1"),
    create: p.getByRole("button", { name: /New issue/ }),
  });

  // Done is folded, so the issue opened here is one still to do.
  await row(p, "BT-7").click();
  await p.locator("[data-timeline]").waitFor();
  await s.take("issue", "an issue beside the list, with its properties and history", { kind: "press", target: "status" }, {
    status: p.locator("[data-prop=state]"),
    timeline: p.locator("[data-timeline]"),
  });

  // The status menu from the keyboard, on the issue just closed: the list
  // keeps it as the one in focus.
  await p.locator("body").press("Escape");
  await p.keyboard.press("s");
  const menu = p.locator('[data-menu="state"]');
  await menu.waitFor();
  const started = menu.getByRole("menuitem", { name: "In Progress" });
  await s.take("status-menu", "S opens the status menu", { kind: "click", target: "started" }, { started, menu });
  await started.click();
  await menu.waitFor({ state: "hidden" });

  await p.locator("[data-layout-board]").click();
  await p.locator("[data-board]").waitFor();
  const card = p.locator('[data-card][data-issue-key="BT-8"]');
  const doing = p.locator('[data-column="In Progress"]');
  await card.waitFor();
  await s.take("board", "the same issues as a board", { kind: "drag", target: "card" }, { card, doing });
  await card.dragTo(doing);
  await doing.locator('[data-issue-key="BT-8"]').waitFor();
  await s.take("board-moved", "a card dragged to In Progress", { kind: "none" }, {
    card: doing.locator('[data-issue-key="BT-8"]'),
  });

  // Ctrl+K finds any issue by its words.
  await p.keyboard.press("Control+k");
  const palette = p.locator("[data-palette]");
  await palette.waitFor();
  await typeWords(s, "free space", "palette", "the palette", () => ({ palette }));
  const found = p.getByRole("option", { name: /Check free space/ });
  await found.waitFor();
  await s.take("palette-found", "the issue a pull request closed", { kind: "press", target: "found" }, { found });
  await p.keyboard.press("Enter");
  await p.locator("[data-links]").waitFor();
  await s.take("closed-by-pull", "closed by Eva's merged pull request", { kind: "none" }, {
    links: p.locator("[data-links]"),
    status: p.locator("[data-prop=state]"),
  });

  // C makes an issue.
  await p.locator("body").press("Escape");
  await p.waitForTimeout(300);
  await p.keyboard.press("c");
  const dialog = p.getByRole("dialog");
  await dialog.waitFor();
  await typeWords(s, "Rotate the backup key every year", "create", "a new issue", () => ({ dialog }));
  await p.keyboard.press("Control+Enter");
  await dialog.waitFor({ state: "hidden" });
  await p.locator("[data-card]", { hasText: "Rotate the backup key every year" }).waitFor();
  await s.take("created", "the new issue on the board", { kind: "click", target: "inbox" }, {
    inbox: p.locator("[data-slot=sidebar-menu-button]", { hasText: "Inbox" }).first(),
  });

  await p.locator("[data-slot=sidebar-menu-button]", { hasText: "Inbox" }).first().click();
  await p.locator("[data-inbox-item]").first().waitFor();
  await s.take("inbox", "what happened to the issues I follow", { kind: "click", target: "item" }, {
    item: p.locator("[data-inbox-item]").first(),
  });
  await p.locator("[data-inbox-item]").first().click();
  await p.locator("[data-timeline]").waitFor();
  await s.take("inbox-open", "a mention, read beside the list");

  await p.goto(`${base}/git/issues/p/BT/cycles`);
  await p.locator("[data-cycle]").first().waitFor();
  await s.take("cycles", "the cycles and how far each is", { kind: "none" }, { cycle: p.locator("[data-cycle]").first() });

  await p.goto(`${base}/git/issues/p/GAR?layout=board`);
  await p.locator("[data-board]").waitFor();
  await p.locator("[data-card]", { hasText: "Plant tomatoes" }).waitFor();
  await s.take("garden", "the Garden board from LosOS Cloud, now a project", { kind: "none" }, {
    card: p.locator("[data-card]", { hasText: "Plant tomatoes" }),
  });

  await p.goto(`${base}/git/notshared/backup-tool`);
  await p.locator("[data-readme]").waitFor();
  await s.take("repository", "the repository, with its issues in the sidebar", { kind: "none" }, {
    issues: p.locator("[data-slot=sidebar-menu-button]", { hasText: "Issues" }).last(),
  });
}

async function before(s, base) {
  const p = s.page;
  await p.goto(`${base}/git/notshared/backup-tool`);
  await p.locator("[data-readme]").waitFor();
  await s.take("repository", "the repository");
  await p.goto(`${base}/git/issues`);
  await p.waitForLoadState("networkidle");
  await s.take("issues", "/git/issues");
}

function parse(argv) {
  const pos = [];
  const opts = {};
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (["--commit", "--test", "--ca"].includes(a)) opts[a.slice(2)] = argv[++i];
    else pos.push(a);
  }
  if (pos.length < 5 || !["tracker", "before"].includes(pos[4])) {
    console.error("usage: issue-shots.mjs BASE NAME PASSWORD OUT tracker|before [--commit SHA] [--test NAME] [--ca FILE]");
    process.exit(2);
  }
  return { base: pos[0], name: pos[1], password: pos[2], out: pos[3], phase: pos[4], opts };
}

const { base, name, password, out, phase, opts } = parse(process.argv.slice(2));
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
page.setDefaultTimeout(180_000);
page.setDefaultNavigationTimeout(180_000);
await signIn(page, base, name, password);
// What the admin page logs on a plain load is the baseline; the issue
// pages may log nothing beyond it.
const baseline = new Set();
const listen = (sink) => {
  const onConsole = (m) => {
    if (m.type() === "error" && !m.text().startsWith("Failed to load resource")) sink(m.text());
  };
  const onError = (e) => sink(String(e));
  const onResponse = (r) => {
    if (r.status() >= 400) sink(`${r.status()} ${r.request().method()} ${new URL(r.url()).pathname}`);
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
try {
  if (phase === "tracker") await tracker(s, base);
  else await before(s, base);
} catch (e) {
  await page.screenshot({ path: join(out, `failed-${phase}.png`) }).catch(() => {});
  throw e;
} finally {
  s.save();
}
await browser.close();
if (problems.length > 0 && phase !== "before") {
  console.error(`the page reported errors:\n${problems.join("\n")}`);
  process.exit(1);
}
