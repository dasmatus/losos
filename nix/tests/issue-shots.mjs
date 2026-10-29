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
//            the inbox, the cycles, the Garden board LosOS Cloud had, the
//            repository with its issues in the sidebar, then the rest of
//            the box: an issue pinned from its top bar and another from
//            its right-click menu, both on the admin page's sidebar, and
//            the admin page's search listing and finding issues;
//   before   the same repository, /git/issues and the admin page's search
//            on pages without the tracker.
//
// The window is 1280 x 900 at twice the pixels, dark, en-US, reduced motion,
// Europe/Bratislava. Each picture waits for the page to settle: no request
// in flight, fonts and pictures loaded, no skeleton and no toast on screen.

const exists = (file) => Bun.file(file).exists();
const join = (...parts) => parts.join("/");

const VIEW = { width: 1280, height: 900 };
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
  constructor(page, base, out, meta, steps) {
    this.page = page;
    this.host = new URL(base).hostname;
    this.out = out;
    this.meta = meta;
    this.steps = steps;
  }

  /** A run that goes on from the manifest an earlier phase left in `out`. */
  static async open(page, base, out, meta) {
    const manifest = Bun.file(join(out, "manifest.json"));
    const steps = (await manifest.exists()) ? (await manifest.json()).steps : [];
    return new Shots(page, base, out, meta, steps);
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

  async save() {
    const manifest = { feature: "issues", viewport: VIEW, dpr: 2, ...this.meta, steps: this.steps };
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
  await p.locator("[data-links]").scrollIntoViewIfNeeded();
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
  // The palette's issue opened in its project's list, so the new issue
  // opens there too.
  await p.locator("[data-issue-key]", { hasText: "Rotate the backup key every year" }).first().waitFor();
  await s.take("created", "the new issue, open beside the list", { kind: "click", target: "inbox" }, {
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

  // The rest of the box: pins on the admin page's sidebar and its search.
  await p.goto(`${base}/git/issues/p/BT?open=BT-5`);
  const pin = p.locator('[data-pin="issue:BT-5"]');
  await pin.waitFor();
  await s.take("pin-button", "the pin at the top of an issue", { kind: "click", target: "pin" }, { pin });
  await pin.click();
  await p.locator('[data-pin="issue:BT-5"][aria-pressed="true"]').waitFor();
  await row(p, "BT-1").click({ button: "right" });
  const rowMenu = p.locator('[data-slot="context-menu"]');
  const pinItem = rowMenu.getByRole("menuitem", { name: "Pin to sidebar" });
  await pinItem.waitFor();
  await s.take("row-menu", "an issue's right-click menu", { kind: "click", target: "pin" }, { menu: rowMenu, pin: pinItem });
  await pinItem.click();
  await rowMenu.waitFor({ state: "detached" });

  await p.goto(`${base}/`);
  const pinned = p.getByRole("navigation", { name: "Sections", exact: true }).getByRole("group", { name: "Pinned" });
  await pinned.getByRole("link", { name: /Backup runs twice/ }).waitFor();
  const tile = p.getByRole("main").getByRole("link", { name: /^Issues/ }).first();
  await tile.waitFor();
  await s.take("sidebar-pins", "two issues on the admin page's sidebar, and the Issues tile", { kind: "press", target: "search" }, {
    pinned,
    tile,
  });

  await p.keyboard.press("Control+Shift+P");
  const field = p.getByRole("combobox", { name: /^Search pages/ });
  await field.waitFor();
  const issues = p.locator("dialog[open]").getByRole("group", { name: "Issues in LosOS Git", exact: true });
  await issues.getByRole("option").first().waitFor();
  await s.take("search-idle", "the search with nothing typed: the open issues given to me", { kind: "type", target: "field" }, {
    field,
    issues,
  });
  await typeWords(s, "free space", "search", "the admin page's search", () => ({ field, issues }));
  const hit = issues.getByRole("option", { name: /Check free space/ });
  await hit.waitFor();
  await s.take("search-found", "an issue found from the admin page", { kind: "click", target: "hit" }, { hit });
  await hit.click();
  await p.waitForURL(/\/git\/issues\/BT-6/);
  await p.locator("[data-reader]").waitFor();
  await s.take("search-opened", "the issue, opened from the admin page");
}

async function before(s, base) {
  const p = s.page;
  await p.goto(`${base}/git/notshared/backup-tool`);
  await p.locator("[data-readme]").waitFor();
  await s.take("repository", "the repository");
  await p.goto(`${base}/git/issues`);
  await p.waitForLoadState("networkidle");
  await s.take("issues", "/git/issues");
  await p.goto(`${base}/`);
  await p.getByRole("navigation", { name: "Sections", exact: true }).waitFor();
  await s.take("overview", "the admin page");
  await p.keyboard.press("Control+Shift+P");
  const field = p.getByRole("combobox", { name: /^Search pages/ });
  await field.fill("free space");
  await p.waitForTimeout(2000);
  await s.take("search", "the admin page's search for free space");
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
    // Thrown: an uncaught error is what makes Bun exit non-zero.
    throw new Error("usage: issue-shots.mjs BASE NAME PASSWORD OUT tracker|before [--commit SHA] [--test NAME] [--ca FILE]");
  }
  return { base: pos[0], name: pos[1], password: pos[2], out: pos[3], phase: pos[4], opts };
}

const { base, name, password, out, phase, opts } = parse(Bun.argv.slice(2));
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
    // The box is offline, so the palette's app catalogue search (Artifact
    // Hub) cannot answer; that 500 says nothing about the issue pages.
    if (r.status() >= 400 && new URL(r.url()).pathname !== "/api/apps/search")
      sink(`${r.status()} ${r.request().method()} ${new URL(r.url()).pathname}`);
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
const s = await Shots.open(page, base, out, { commit: opts.commit ?? "", test: opts.test ?? "" });
try {
  if (phase === "tracker") await tracker(s, base);
  else await before(s, base);
} catch (e) {
  await page.screenshot({ path: join(out, `failed-${phase}.png`) }).catch(() => {});
  throw e;
} finally {
  await s.save();
}
await browser.close();
if (problems.length > 0 && phase !== "before") {
  // Thrown: an uncaught error is what makes Bun exit non-zero.
  throw new Error(`the page reported errors:\n${problems.join("\n")}`);
}
