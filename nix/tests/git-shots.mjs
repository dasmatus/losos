// Pictures of LosOS Git's pages on a real box, from a real browser.
//
// Chromium (Playwright) signs in through LosOS Login as the owner and walks
// the pages, one full-window PNG per state, with a manifest that says where
// each picture was taken and where the things on it are. nix/tests/git-pages.nix
// runs it in its laptop VM against the box's own nginx, in phases, with the
// test's own requests in between; the same pictures serve the pull request
// and the launch film. Run it again against any box:
//
//   node git-shots.mjs BASE NAME PASSWORD OUT PHASE [--commit SHA] [--test NAME] [--ca FILE]
//
// BASE is the box (https://mattbox.local), OUT a folder for NN-slug.png and
// manifest.json (a later phase adds to the pictures an earlier one left).
// PHASE is one of
//
//   browse   the repositories, the code, a file, the pull requests, the pull
//            request and its files; a comment on a line kept for a review,
//            and the review sent with Approve;
//   finish   the reply on that line resolved, the worktrees, the merge box,
//            a squash merge that deletes the branch, the worktrees after;
//   before   the same addresses on a box without the pages.
//
// The repository is notshared/backup-tool with pull request #1 from the
// branch fix-backup, as the test makes them. The browser trusts the box's
// certificate by its public key (--ca, default /etc/losos-box.pem).
//
// Playwright comes from PLAYWRIGHT_CORE (a path to playwright-core, as
// nixpkgs' playwright-driver is) or the `playwright-core` package.
//
// The window is 1280 x 900 at twice the pixels, dark, en-US, reduced motion,
// Europe/Bratislava. Each picture waits for the page to settle: no request
// in flight, fonts and pictures loaded, no skeleton and no toast on screen.

import { createHash, X509Certificate } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const VIEW = { width: 1280, height: 900 };
const REPO = "/git/notshared/backup-tool";

const { chromium } = await import(process.env.PLAYWRIGHT_CORE ?? "playwright-core");

/** The certificate's public key hash, which Chromium pins to. */
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
    // Every picture on screen has arrived, nothing is still loading, no
    // toast is still sliding.
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
    const manifest = { feature: "git", viewport: VIEW, dpr: 2, ...this.meta, steps: this.steps };
    writeFileSync(join(this.out, "manifest.json"), JSON.stringify(manifest, null, 2) + "\n");
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
    [name, password, `${base}/git/`],
  );
  if (status !== 200) throw new Error(`sign-in refused: ${status}`);
}

async function browse(s, base) {
  const p = s.page;
  await p.goto(`${base}/git/`);
  await p.waitForSelector("[data-repos]");
  const repo = p.getByRole("link", { name: "backup-tool", exact: true });
  await s.take("repos", "the owner's repositories", { kind: "click", target: "repo" }, {
    repo,
    create: p.getByRole("button", { name: "New repository" }),
  });
  await repo.click();
  // A README's own headings sit one level under the page's title.
  await p.locator("[data-readme] h2", { hasText: "Backup tool" }).waitFor();
  const script = p.getByRole("link", { name: "backup.sh", exact: true });
  await s.take("code", "the code of main, with its README", { kind: "click", target: "script" }, {
    script,
    readme: p.locator("[data-readme]"),
    branch: p.getByRole("combobox").first(),
  });
  await script.click();
  await p.getByText("rsync", { exact: false }).first().waitFor();
  const pulls = p.getByRole("link", { name: "Pull requests", exact: true }).first();
  await s.take("file", "one file, with line numbers", { kind: "click", target: "pulls" }, { pulls });
  await pulls.click();
  const pull = p.getByRole("link", { name: "Retry a failed copy", exact: true });
  await pull.waitFor();
  await s.take("pulls", "the open pull requests", { kind: "click", target: "pull" }, { pull });
  await pull.click();
  await p.locator("[data-pull-title]").waitFor();
  await p.locator("[data-checks]").getByText("All checks passed").waitFor();
  await p.locator("[data-worktree-badge]").waitFor();
  const files = p.getByRole("link", { name: "Files changed", exact: true });
  await s.take("pull", "a pull request from a named worktree", { kind: "click", target: "files" }, {
    files,
    checks: p.locator("[data-checks]"),
    reviewers: p.locator("[data-reviewers]"),
    worktree: p.locator("[data-worktree-badge]"),
    merge: p.locator("[data-merge-box]"),
  });
  await files.click();
  await p.locator('[data-file-diff="backup.sh"]').waitFor();
  await s.take("files", "what the pull request changes", { kind: "hover", target: "line" }, {
    line: p.locator('[data-file-diff="backup.sh"] tr[data-row="add"]').first(),
  });
  const line = p.locator('[data-file-diff="backup.sh"] tr[data-row="add"]').first();
  await line.hover();
  await line.locator("[data-comment-line]").click();
  const composer = p.locator("[data-composer] textarea");
  await composer.fill("Three tries, then give up? Say so in the log line too.");
  await s.take("line-comment", "a comment on one line", { kind: "click", target: "add" }, {
    composer: p.locator("[data-composer]"),
    add: p.getByRole("button", { name: "Add to review" }),
  });
  await p.getByRole("button", { name: "Add to review" }).click();
  await p.locator("[data-pending]").waitFor();
  const panel = p.locator("[data-review-panel]");
  await panel.getByRole("radio", { name: "Approve" }).click();
  await panel.getByRole("textbox").fill("Works on my box. One note on the log line.");
  await panel.scrollIntoViewIfNeeded();
  await s.take("review", "the review, ready to send", { kind: "click", target: "send" }, {
    panel,
    send: p.locator("[data-submit-review]"),
  });
  await p.locator("[data-submit-review]").click();
  await p.locator("[data-thread]").first().waitFor();
  await p.locator("[data-pending]").waitFor({ state: "detached" });
  await p.locator("[data-thread]").first().scrollIntoViewIfNeeded();
  await s.take("reviewed", "the review sent; the note is a thread on its line", { kind: "none" }, {
    thread: p.locator("[data-thread]").first(),
  });
}

async function finish(s, base) {
  const p = s.page;
  await p.goto(`${base}${REPO}/pulls/1/files`);
  const thread = p.locator("[data-thread]").first();
  await thread.getByText("Logs it now", { exact: false }).waitFor();
  await thread.scrollIntoViewIfNeeded();
  const resolve = thread.getByRole("button", { name: "Resolve" });
  await s.take("reply", "the author answered on the line", { kind: "click", target: "resolve" }, { thread, resolve });
  await resolve.click();
  await p.locator("[data-thread]", { hasText: "Resolved by" }).first().waitFor();
  await s.take("resolved", "the thread resolved and folded", { kind: "none" }, {
    thread: p.locator("[data-thread]").first(),
  });
  const worktrees = p.getByRole("link", { name: "Worktrees", exact: true }).first();
  await worktrees.click();
  await p.locator('[data-worktree="fix-backup"]').waitFor();
  await s.take("worktrees", "the worktrees that push here", { kind: "none" }, {
    fix: p.locator('[data-worktree="fix-backup"]'),
    notes: p.locator('[data-worktree="release-notes"]'),
    how: p.locator("[data-worktree-howto]"),
  });
  await p.goto(`${base}${REPO}/pulls/1`);
  const box = p.locator('[data-merge-box="open"]');
  await box.getByText("Ready to merge").waitFor();
  await box.locator("button", { hasText: "Squash" }).click();
  // On by default: make sure of it rather than click it, which would turn it off.
  const drop = box.getByRole("switch", { name: "Delete fix-backup afterwards" });
  if (!(await drop.isChecked())) await drop.click();
  await box.scrollIntoViewIfNeeded();
  await s.take("merge", "approved, checks green: squash and delete the branch", { kind: "click", target: "merge" }, {
    box,
    merge: p.locator("[data-merge]"),
  });
  await p.locator("[data-merge]").click();
  await p.locator('[data-merge-box="merged"]').waitFor();
  await p.locator('[data-merge-box="merged"]').scrollIntoViewIfNeeded();
  await s.take("merged", "merged into main", { kind: "none" }, { box: p.locator('[data-merge-box="merged"]') });
  await p.goto(`${base}${REPO}/worktrees`);
  await p.locator('[data-worktree="fix-backup"]').getByText("Branch deleted").waitFor();
  await s.take("worktrees-after", "the worktree's branch, merged and gone", { kind: "none" }, {
    fix: p.locator('[data-worktree="fix-backup"]'),
  });
}

async function before(s, base) {
  for (const [path, slug, title] of [
    ["/git/", "repos", "the repositories address, before the pages"],
    [REPO, "code", "the repository's address, before the pages"],
    [`${REPO}/pulls/1`, "pull", "the pull request's address, before the pages"],
    [`${REPO}/pulls/1/files`, "files", "its files' address, before the pages"],
    [`${REPO}/worktrees`, "worktrees", "the worktrees' address, before the pages"],
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
    if (["--commit", "--test", "--ca"].includes(a)) opts[a.slice(2)] = argv[++i];
    else pos.push(a);
  }
  if (pos.length < 5 || !["browse", "finish", "before"].includes(pos[4])) {
    console.error("usage: git-shots.mjs BASE NAME PASSWORD OUT browse|finish|before [--commit SHA] [--test NAME] [--ca FILE]");
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
// An emulated laptop at 2x renders slowly: Playwright's 30 s default is
// too short for a locator or a navigation under TCG.
page.setDefaultTimeout(180_000);
page.setDefaultNavigationTimeout(180_000);
await signIn(page, base, name, password);
// What the admin page itself logs on a plain load (sonner's stylesheet,
// which the box's style-src 'self' refuses and the bundle carries instead,
// as tests/*.browser.mjs baseline it): LosOS Git's pages may log nothing
// beyond that.
const baseline = new Set();
const listen = (sink) => {
  // A failed request is named by its address; Chromium's own console line
  // for it ("Failed to load resource") says only the status.
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
  if (phase === "browse") await browse(s, base);
  else if (phase === "finish") await finish(s, base);
  else await before(s, base);
} catch (e) {
  // The window as it was when a step gave up, for the test's output.
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
