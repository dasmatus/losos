// The bar over a selection on a real box, from a real browser: picked words
// in a LosOS Office document and in a note, and picked cards in LosOS
// Diagrams (admin-ui/app/src/components/quick-actions.tsx).
//
// Chromium (Playwright) signs in through LosOS Login as the owner, picks
// words and formats, links and mentions with the bar (the box's shared
// editor saves what it does), reads a note and copies a link to its
// section, then picks two cards in a diagram from LosOS Cloud, colours
// them, joins them with an arrow and saves the file. One full-window PNG per
// state with a manifest, as nix/tests/office-markdown-shots.mjs does, and a
// clip of the bar arriving and gliding, filmed at a tenth of real speed and
// played back at real speed. nix/tests/quick-actions.nix runs it in its
// laptop VM against the box's own nginx.
//
//   bun quick-actions-shots.mjs BASE NAME PASSWORD OUT [--before]
//                                [--commit SHA] [--test NAME] [--ca FILE]
//
// --before takes the same states with the admin UI as it was before, where
// a selection brings up nothing.

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
  constructor(base, out, meta) {
    this.host = new URL(base).hostname === "192.168.1.1" ? "mattbox.local" : new URL(base).hostname;
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
        if ([...document.images].filter(seen).some((i) => !i.complete || i.naturalWidth === 0)) return false;
        if ([...document.querySelectorAll("[data-slot=spinner]")].some(seen)) return false;
        if (document.querySelector("[data-sonner-toast], [data-slot=skeleton]")) return false;
        return true;
      },
      undefined,
      { timeout: 120_000 },
    );
    await p.evaluate(() => {
      document.documentElement.style.caretColor = "transparent";
    });
    await p.waitForTimeout(500);
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

  clip(file, title) {
    this.steps.push({ file, url: `${this.host}/cloud/office`, title, action: { kind: "none" }, targets: {} });
    console.log(file, title);
  }

  async save() {
    const manifest = { feature: "quick-actions", viewport: VIEW, dpr: 2, ...this.meta, steps: this.steps };
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

/** A file's number in LosOS Cloud, from its folder's listing. */
async function fileId(p, folder, name) {
  const id = await p.evaluate(
    async ([folder, name]) => {
      const r = await fetch(`/cloud/api/files?path=${encodeURIComponent(folder)}`);
      const body = await r.json();
      return body.entries.find((e) => e.name === name)?.id ?? null;
    },
    [folder, name],
  );
  if (id === null) throw new Error(`${folder}/${name} is not in LosOS Cloud`);
  return id;
}

const quick = (p) => p.locator("[data-slot=quick-actions]");
const action = (p, id) => quick(p).locator(`[data-action="${id}"]`);

/** Picks `word` on the line with `line` on it, from the keyboard. */
async function pick(p, line, word) {
  const l = p.locator(".cm-line", { hasText: line }).first();
  await l.click();
  await p.keyboard.press("End");
  const text = (await l.textContent()) ?? "";
  const end = text.lastIndexOf(word) + word.length;
  for (let i = text.length; i > end; i--) await p.keyboard.press("ArrowLeft");
  for (let i = 0; i < word.length; i++) await p.keyboard.press("Shift+ArrowLeft");
}

async function waitOffice(p) {
  await p.locator("[data-markdown-office]").waitFor();
  await p.locator('[data-testid=office-saved][data-connection="live"]').waitFor();
  await p.locator("[data-testid=md-toolbar] [data-action=bold]:not([disabled])").waitFor();
  await p.locator(".cm-line", { hasText: "allotment" }).first().waitFor();
}

async function office(s, p, base, old) {
  const id = await fileId(p, "Notes", "Saturday.md");
  await p.goto(`${base}/cloud/office/${id}`);
  await waitOffice(p);
  await pick(p, "a few tools", "everyone");
  if (old) {
    await s.take(p, "office-picked", "words picked in a document");
    return;
  }
  await quick(p).waitFor();
  await s.take(p, "office-picked", "words picked in a document, and the bar above them", { kind: "click", target: "bold" }, {
    bar: quick(p),
    bold: action(p, "bold"),
  });
  await action(p, "bold").click();
  await p.locator(".cm-line", { hasText: "**everyone**" }).first().waitFor();
  await action(p, "bold").hover();
  await p.getByRole("tooltip").waitFor().catch(() => {});
  await s.take(p, "office-bold", "Bold from the bar, shown pressed", { kind: "click", target: "link" }, {
    bar: quick(p),
    link: action(p, "link"),
  });

  await pick(p, "a few tools", "allotment");
  await action(p, "link").click();
  const field = quick(p).getByRole("textbox", { name: "Link" });
  await field.waitFor();
  for (const word of ["losos.dasmat.us", "/handbook/"]) await field.pressSequentially(word, { delay: 20 });
  await s.take(p, "office-link", "Link takes the bar's place with a field", { kind: "type", text: "losos.dasmat.us/handbook/" }, {
    field,
  });
  await p.keyboard.press("Enter");
  await p.locator(".cm-line", { hasText: "[allotment](https://losos.dasmat.us/handbook/)" }).first().waitFor();

  await pick(p, "calendar", "calendar");
  await action(p, "wikilink").click();
  const find = quick(p).getByRole("combobox", { name: "Link to a note" });
  await find.waitFor();
  await find.fill("Plant");
  await quick(p).getByRole("option", { name: /Planting calendar/ }).waitFor();
  await s.take(p, "office-note", "Link to a note offers the notes beside the document", { kind: "click", target: "option" }, {
    option: quick(p).getByRole("option", { name: /Planting calendar/ }),
  });
  await p.keyboard.press("Enter");
  await p.locator(".cm-line", { hasText: "[[Planting calendar|calendar]]" }).first().waitFor();
  await p.locator('[data-testid=office-saved][data-connection="live"]').waitFor();
  // The shared editor writes the file a moment after the typing stops.
  await p.waitForTimeout(3000);
}

/** The bar arriving over a selection and gliding to the next one, filmed
 *  at a tenth of real speed in its own context with motion on. */
/** Centre of `word` on the line with `line` on it, for a double click. */
async function wordAt(p, line, word) {
  const l = p.locator(".cm-line", { hasText: line }).first();
  await l.waitFor();
  return l.evaluate((el, word) => {
    const walk = document.createTreeWalker(el, NodeFilter.SHOW_TEXT);
    for (let n = walk.nextNode(); n !== null; n = walk.nextNode()) {
      const at = n.data.lastIndexOf(word);
      if (at < 0) continue;
      const r = document.createRange();
      r.setStart(n, at);
      r.setEnd(n, at + word.length);
      const box = r.getBoundingClientRect();
      return { x: box.left + box.width / 2, y: box.top + box.height / 2 };
    }
    throw new Error(`${word} is not on the line`);
  }, word);
}

/**
 * The clip, frame by frame: CDP holds every new animation at its start, then
 * each frame sets the ones that began since the last action to its time and
 * is screenshotted, so an emulated laptop draws 60 frames a second however
 * slowly it runs. A still stretch is one screenshot copied.
 */
class Reel {
  constructor(p, dir) {
    this.p = p;
    this.dir = dir;
    this.n = 0;
  }

  path() {
    return join(this.dir, `${String(this.n).padStart(4, "0")}.png`);
  }

  async frame() {
    await this.p.evaluate(() => new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r))));
    await this.p.screenshot({ path: this.path() });
    this.n++;
  }

  async hold(frames) {
    await this.frame();
    const last = join(this.dir, `${String(this.n - 1).padStart(4, "0")}.png`);
    for (let i = 1; i < frames; i++) {
      await Bun.write(this.path(), Bun.file(last));
      this.n++;
    }
  }

  /** Plays what the last action started, at 60 frames a second. */
  async play() {
    const end = await this.p.evaluate(() => {
      const live = document
        .getAnimations()
        .filter((a) => a.playState !== "finished" && Number.isFinite(a.effect?.getComputedTiming().endTime ?? Infinity));
      for (const a of live) a.pause();
      window.__reel = live;
      return Math.max(0, ...live.map((a) => Number(a.effect.getComputedTiming().endTime)));
    });
    for (let t = 0; t < end; t += 1000 / 60) {
      await this.p.evaluate((t) => {
        for (const a of window.__reel) a.currentTime = t;
      }, t);
      await this.frame();
    }
    await this.p.evaluate(() => {
      for (const a of window.__reel) a.finish();
    });
  }
}

async function film(browser, base, name, password, s, old) {
  const dir = join(s.out, "clip");
  await Bun.$`rm -rf ${dir}`;
  Bun.spawnSync(["mkdir", "-p", dir]);
  const context = await browser.newContext({
    viewport: VIEW,
    deviceScaleFactor: 2,
    colorScheme: "dark",
    locale: "en-US",
    timezoneId: "Europe/Bratislava",
  });
  const p = await context.newPage();
  p.setDefaultTimeout(120_000);
  await signIn(p, base, name, password);
  const id = await fileId(p, "Notes", "Saturday.md");
  await p.goto(`${base}/cloud/office/${id}`);
  await waitOffice(p);
  await p.evaluate(() => {
    document.documentElement.style.caretColor = "transparent";
  });
  const cdp = await context.newCDPSession(p);
  await cdp.send("Animation.enable");
  await cdp.send("Animation.setPlaybackRate", { playbackRate: 0 });
  const reel = new Reel(p, dir);
  await reel.hold(40);
  for (const [line, word] of [
    ["a few tools", "everyone"],
    ["calendar", "calendar"],
  ]) {
    const at = await wordAt(p, line, word);
    await p.mouse.dblclick(at.x, at.y);
    if (!old) await quick(p).waitFor();
    await reel.play();
    await reel.hold(70);
  }
  await context.close();
  const file = `${String(s.steps.length + 1).padStart(2, "0")}-${old ? "office-select" : "office-motion"}.mp4`;
  const ffmpeg = Bun.spawnSync([
    Bun.env.FFMPEG ?? "ffmpeg",
    "-y", "-loglevel", "error", "-framerate", "60", "-i", join(dir, "%04d.png"),
    "-c:v", "libx264", "-pix_fmt", "yuv420p", "-an", join(s.out, file),
  ]);
  if (ffmpeg.exitCode !== 0) throw new Error(`ffmpeg: ${new TextDecoder().decode(ffmpeg.stderr)}`);
  await Bun.$`rm -rf ${dir}`;
  s.clip(file, old ? "picking words in a document" : "the bar arrives over picked words and glides to the next ones");
}

async function notes(s, p, base, old) {
  await p.goto(`${base}/cloud/notes?vault=Notes&note=Planting%20calendar.md&view=edit`);
  await p.locator("[data-note-editor] .cm-line", { hasText: "Sow the beans" }).first().waitFor();
  await p.locator('[data-note-editor] [data-connection="live"]').waitFor();
  await pick(p, "Sow the beans", "beans");
  if (!old) await quick(p).waitFor();
  await s.take(p, "notes-edit", old ? "words picked in a note" : "the same bar in a note being written", { kind: "none" }, old ? {} : { bar: quick(p) });

  await p.goto(`${base}/cloud/notes?vault=Notes&note=Planting%20calendar.md`);
  const para = p.locator(".note-body p", { hasText: "Water in the morning" }).first();
  await para.waitFor();
  await para.selectText();
  if (old) {
    await s.take(p, "notes-read", "words picked in a note being read");
    return;
  }
  await quick(p).waitFor();
  await s.take(p, "notes-read", "reading: copy the words or a link to their section", { kind: "click", target: "copyLink" }, {
    bar: quick(p),
    copyLink: action(p, "copy-link"),
  });
}

async function diagrams(s, p, base, old) {
  await p.goto(`${base}/diagrams/?canvas=svg&file=Diagrams%2Fgarden.canvas`);
  await p.getByTestId("lab-splash").waitFor({ state: "detached", timeout: 180_000 });
  const card = (label) => p.locator(`[data-testid=diagram] [data-node][aria-label="${label}"]`);
  await card("Seeds").waitFor();
  await card("Seeds").click();
  await card("Seedlings").click({ modifiers: ["Shift"] });
  if (old) {
    await s.take(p, "diagrams-picked", "two cards picked in a diagram");
    return;
  }
  await quick(p).waitFor();
  await s.take(p, "diagrams-picked", "two cards picked, and what can be done to them", { kind: "click", target: "color" }, {
    bar: quick(p),
    color: action(p, "color"),
    connect: action(p, "connect"),
  });
  await action(p, "color").click();
  const green = p.locator('[data-quick-popup] [data-option="4"]');
  await green.waitFor();
  await s.take(p, "diagrams-colour", "a colour for both", { kind: "click", target: "green" }, { green });
  await green.click();
  await p.waitForFunction(() => document.querySelectorAll("[data-testid=diagram] .dg-node.colored").length >= 2);
  await action(p, "connect").click();
  await p.waitForFunction(() => document.querySelectorAll("[data-testid=diagram] [data-edge]").length >= 2);
  await card("Seeds").click();
  await card("Seedlings").click({ modifiers: ["Shift"] });
  await quick(p).waitFor();
  await s.take(p, "diagrams-connected", "coloured and joined by an arrow from the bar", { kind: "none" }, { bar: quick(p) });
  await p.locator("[data-testid=diagram]").focus();
  await p.keyboard.press("Control+s");
  await p.waitForTimeout(3000);
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
    throw new Error("usage: quick-actions-shots.mjs BASE NAME PASSWORD OUT [--before] [--commit SHA] [--test NAME] [--ca FILE]");
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
await context.grantPermissions(["clipboard-read", "clipboard-write"], { origin: base });
const page = await context.newPage();
page.setDefaultTimeout(120_000);
await signIn(page, base, name, password);

const problems = [];
page.on("pageerror", (e) => problems.push(String(e)));

const s = new Shots(base, out, { commit: opts.commit ?? "", test: opts.test ?? "" });
try {
  await office(s, page, base, opts.before === true);
  await notes(s, page, base, opts.before === true);
  await diagrams(s, page, base, opts.before === true);
  await film(browser, base, name, password, s, opts.before === true);
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
