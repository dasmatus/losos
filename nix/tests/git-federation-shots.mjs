// Pictures of LosOS Git between two boxes, from a real browser: someone of
// another box signing in with their chat address, and a repository's room
// in LosOS Chat on both boxes. nix/tests/git-federation.nix runs it in its
// laptop VM against the two boxes' own nginx, in phases, with the test's
// own requests in between; the same pictures serve the pull request and the
// launch film.
//
//   node git-federation-shots.mjs OUT PHASE [--old] [--commit SHA] [--test NAME]
//
// The boxes are https://mattbox.local (Matus, the owner, with the
// repository notshared/site) and https://kitchenbox.local (Eva). The
// browser trusts each box's certificate by its public key
// (/etc/losos-mattbox.pem, /etc/losos-kitchenbox.pem). PHASE is one of
//
//   room     Matus on mattbox: the repository's settings, the room switch
//            turned on, the repository with its Chat button;
//   eva      Eva: mattbox's repository as a visitor, "Use your chat
//            address", her box asking her, back on mattbox as
//            @eva:kitchenbox.local, her pull request, and the repository's
//            room in her own box's chat;
//   chat     Matus: the repository's room in mattbox's chat;
//   before   with --old, the pages before this change: the visitor's
//            repository page, the owner's settings and his repository.
//
// The window is 1280 x 900 at twice the pixels, dark, en-US, reduced motion,
// Europe/Bratislava.

const exists = (file) => Bun.file(file).exists();
const join = (...parts) => parts.join("/");

const VIEW = { width: 1280, height: 900 };
const MATT = "https://mattbox.local";
const KITCHEN = "https://kitchenbox.local";
const REPO = "/git/notshared/site";
const MATUS = ["matus", "Correct-Horse-Battery-9"];
const EVA = ["eva", "Eva-Long-Password-3"];

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
  constructor(out, meta, steps) {
    this.out = out;
    this.meta = meta;
    this.steps = steps;
  }

  /** A run that goes on from the manifest an earlier phase left in `out`. */
  static async open(out, meta) {
    const manifest = Bun.file(join(out, "manifest.json"));
    const steps = (await manifest.exists()) ? (await manifest.json()).steps : [];
    return new Shots(out, meta, steps);
  }

  async settle(p) {
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
        if ([...document.querySelectorAll("[data-skeleton], [data-slot=spinner], .animate-spin")].some(seen)) return false;
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

  async take(p, slug, title, action = { kind: "none" }, targets = {}) {
    await this.settle(p);
    const file = `${String(this.steps.length + 1).padStart(2, "0")}-${slug}.png`;
    await p.screenshot({ path: join(this.out, file), timeout: 180_000 });
    const found = {};
    for (const [key, loc] of Object.entries(targets)) {
      const b = await loc.boundingBox().catch(() => null);
      if (b) found[key] = { x: Math.round(b.x), y: Math.round(b.y), w: Math.round(b.width), h: Math.round(b.height) };
    }
    const url = new URL(p.url());
    this.steps.push({ file, url: `${url.host}${url.pathname}${url.search}`, title, action, targets: found });
    console.log(file, title);
  }

  async save() {
    const manifest = { feature: "git-federation", viewport: VIEW, dpr: 2, ...this.meta, steps: this.steps };
    await Bun.write(join(this.out, "manifest.json"), JSON.stringify(manifest, null, 2) + "\n");
  }
}

async function signIn(page, base, [name, password]) {
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
  if (status !== 200) throw new Error(`sign-in refused at ${base}: ${status}`);
}

async function room(s, p) {
  await p.goto(`${MATT}${REPO}/settings`);
  const card = p.locator("[data-chat-room-setting]");
  await card.waitFor();
  await card.scrollIntoViewIfNeeded();
  const toggle = card.getByRole("switch");
  await s.take(p, "room-off", "a repository's settings: its room in LosOS Chat, off", { kind: "click", target: "toggle" }, { card, toggle });
  await toggle.click({ force: true });
  await p.locator('[data-chat-room-setting="on"]').waitFor();
  await p.locator("[data-chat-room]").waitFor();
  await s.take(p, "room-on", "the room made; the repository links it", { kind: "none" }, {
    card,
    chat: p.locator("[data-chat-room]"),
  });
  await p.goto(`${MATT}${REPO}`);
  await p.locator("[data-readme]").waitFor();
  await s.take(p, "repo-chat", "the repository with its Chat button", { kind: "click", target: "chat" }, {
    chat: p.locator("[data-chat-room]"),
  });
}

async function eva(s, p) {
  await p.goto(`${MATT}${REPO}`);
  await p.locator("[data-readme]").waitFor();
  const ask = p.locator("[data-chat-sign-in]");
  await s.take(p, "visitor", "mattbox's public repository, for someone who has no account there", { kind: "click", target: "ask" }, {
    ask,
    signIn: p.locator("[data-sign-in]"),
  });
  await ask.click();
  await p.locator("[data-chat-address]").fill("@eva:kitchenbox.local");
  const go = p.locator("[data-chat-continue]");
  await s.take(p, "address", "Eva gives her chat address", { kind: "click", target: "go" }, {
    dialog: p.getByRole("dialog"),
    go,
  });
  await go.click();
  await p.waitForURL(`${KITCHEN}/chat/vouch**`);
  const yes = p.locator("[data-vouch-yes]");
  await yes.waitFor();
  await s.take(p, "vouch", "her own box asks before it tells mattbox who she is", { kind: "click", target: "yes" }, { yes });
  await yes.click();
  await p.waitForURL(`${MATT}${REPO}**`);
  await p.locator("[data-contributor-note]").waitFor();
  await p.locator("[data-testid='signed-in-name']", { hasText: "Eva" }).waitFor();
  await s.take(p, "contributor", "back on mattbox as @eva:kitchenbox.local, with her own branches", { kind: "none" }, {
    me: p.locator("[data-testid='signed-in-name']"),
    note: p.locator("[data-contributor-note]"),
  });
  await p.goto(`${MATT}${REPO}/pulls/1`);
  await p.locator("[data-pull-title]").waitFor();
  await s.take(p, "pull", "her pull request, from her branch, with Matus's answer", { kind: "click", target: "chat" }, {
    title: p.locator("[data-pull-title]"),
    chat: p.locator("[data-chat-room]"),
  });
  await p.locator("[data-chat-room]").click();
  await p.waitForURL(`${KITCHEN}/chat/**`);
  const log = p.getByRole("log");
  await log.getByText("opened pull request #1", { exact: false }).first().waitFor();
  await s.take(p, "eva-room", "the repository's room, in Eva's chat on kitchenbox", { kind: "none" }, { log });
}

async function chat(s, p) {
  await p.goto(`${MATT}${REPO}`);
  await p.locator("[data-chat-room]").click();
  await p.waitForURL(`${MATT}/chat/**`);
  const log = p.getByRole("log");
  await log.getByText("opened pull request #1", { exact: false }).first().waitFor();
  await log.getByText("@eva:kitchenbox.local", { exact: false }).first().waitFor();
  await s.take(p, "matus-room", "the same room in Matus's chat on mattbox", { kind: "none" }, { log });
}

async function before(s, visitor, owner) {
  await visitor.goto(`${MATT}${REPO}`);
  await visitor.locator("[data-readme]").waitFor();
  await s.take(visitor, "visitor", "mattbox's public repository, for someone who has no account there", { kind: "none" }, {
    signIn: visitor.locator("[data-sign-in]"),
  });
  await owner.goto(`${MATT}${REPO}/settings`);
  await owner.locator("[data-repo-settings]").waitFor();
  await s.take(owner, "settings", "a repository's settings", { kind: "none" });
  await owner.goto(`${MATT}${REPO}`);
  await owner.locator("[data-readme]").waitFor();
  await s.take(owner, "repo", "the repository", { kind: "none" });
}

function parse(argv) {
  const pos = [];
  const opts = {};
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (["--commit", "--test"].includes(a)) opts[a.slice(2)] = argv[++i];
    else if (a === "--old") opts.old = true;
    else pos.push(a);
  }
  if (pos.length < 2 || !["room", "eva", "chat", "before"].includes(pos[1])) {
    // Thrown: an uncaught error is what makes Bun exit non-zero.
    throw new Error("usage: git-federation-shots.mjs OUT room|eva|chat|before [--old] [--commit SHA] [--test NAME]");
  }
  return { out: pos[0], phase: pos[1], opts };
}

const { out, phase, opts } = parse(Bun.argv.slice(2));
const pins = ["/etc/losos-mattbox.pem", "/etc/losos-kitchenbox.pem"].map(spkiPin).join(",");
const browser = await chromium.launch({ args: [`--ignore-certificate-errors-spki-list=${pins}`] });
// The boxes share but never join a real mesh here, so the admin pages would
// carry the "not joined a mesh yet" notice; it is closed as a person would.
const window = async () => {
  const ctx = await browser.newContext({
    viewport: VIEW,
    deviceScaleFactor: 2,
    colorScheme: "dark",
    locale: "en-US",
    timezoneId: "Europe/Bratislava",
    reducedMotion: "reduce",
  });
  await ctx.addInitScript(() => {
    try {
      localStorage.setItem("losos-mesh-notice", "dismissed");
    } catch {
      /* no storage: the notice stays */
    }
  });
  return ctx;
};
const page = async (ctx) => {
  const p = await ctx.newPage();
  p.setDefaultTimeout(180_000);
  p.setDefaultNavigationTimeout(180_000);
  return p;
};

const problems = [];
const watch = (p) => {
  p.on("pageerror", (e) => problems.push(`${e}\n${e.stack ?? ""}`));
  p.on("console", (m) => {
    if (m.type() === "error") console.error(`console: ${m.text()}`);
  });
  p.on("response", (r) => {
    const path = new URL(r.url()).pathname;
    // LosOS Git without LosOS Reviewer answers the AI review's address 404;
    // a visitor's first /user is the 401 that makes them one.
    if (r.status() === 404 && path.endsWith("/ai-review")) return;
    if (r.status() === 401 && path === "/git/api/v1/user") return;
    if (r.status() >= 500) problems.push(`${r.status()} ${r.request().method()} ${r.url()}`);
  });
};

const s = await Shots.open(out, { commit: opts.commit ?? "", test: opts.test ?? "" });
let last = null;
try {
  if (phase === "before") {
    const visitor = await page(await window());
    const owner = await page(await window());
    last = owner;
    await signIn(owner, MATT, MATUS);
    await before(s, visitor, owner);
  } else {
    const p = await page(await window());
    last = p;
    watch(p);
    if (phase === "eva") {
      await signIn(p, KITCHEN, EVA);
      await eva(s, p);
    } else {
      await signIn(p, MATT, MATUS);
      if (phase === "room") await room(s, p);
      else await chat(s, p);
    }
  }
} catch (e) {
  if (last !== null) {
    await last.screenshot({ path: join(out, `failed-${phase}.png`) }).catch(() => {});
    const html = await last.content().catch(() => "");
    await Bun.write(join(out, `failed-${phase}.html`), html);
    console.error(`failed at ${last.url()}`);
  }
  if (problems.length > 0) console.error(`the pages reported errors:\n${problems.join("\n")}`);
  throw e;
} finally {
  await s.save();
}
await browser.close();
if (problems.length > 0 && opts.old !== true) {
  // Thrown: an uncaught error is what makes Bun exit non-zero.
  throw new Error(`the pages reported errors:\n${problems.join("\n")}`);
}
