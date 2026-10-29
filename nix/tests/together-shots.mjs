// Pictures of the LosOS apps handing each other a file, on a real box, from
// a real browser.
//
// Chromium (Playwright) signs in through LosOS Login as the owner and walks
// one file through the apps: Eva's mail with a plan attached, saved into
// LosOS Cloud and opened in Notes from the message, sent on by mail from
// Cloud's right-click menu, and read by the box's AI from Cloud and from
// Mail. One full-window PNG per state, with a manifest of where each was
// taken and where the things on it are, as nix/tests/notes-shots.mjs does.
// nix/tests/apps-together.nix runs it in its laptop VM against the box's own
// nginx.
//
//   node together-shots.mjs VERB BASE NAME PASSWORD OUT [--test NAME] [--ca FILE]
//                           [--commit SHA] [--typing] [--view WxH] [--dpr N]
//                           [--video DIR]
//
// VERB is `after` (the walk), `before` (the same screens on the admin pages
// from before the apps were tied together) or `closed` (the box once it
// stops sharing: no Mail, so Cloud's menu offers no "Send by mail").
// `--typing` adds a picture per word where the walk types, `--view` and
// `--dpr` change the window from 1280x900 at 2x, and `--video` records the
// walk as WebM into DIR as well, all for a film.

const join = (...parts) => parts.join("/");

let VIEW = { width: 1280, height: 900 };
let DPR = 2;
const FILE = "Lake trip.md";
const SUBJECT = "Plan for the lake";
const EVA = "eva@mattbox.home.arpa";

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

  // The admin page's policy refuses waitForFunction's evaluation, so every
  // wait here is a locator.
  async settle(p) {
    await p.waitForLoadState("networkidle");
    for (let i = 0; i < 240; i++) {
      const busy = await p.locator("[data-slot=spinner]:visible, [data-slot=skeleton]:visible").count();
      const toasts = await p.locator("[data-sonner-toast], [data-slot=toast]").count();
      if (busy === 0 && toasts === 0) break;
      await p.waitForTimeout(500);
    }
    await p.waitForTimeout(500);
  }

  async take(p, slug, title, action = { kind: "none" }, targets = {}) {
    await this.settle(p);
    const file = `${String(this.steps.length + 1).padStart(2, "0")}-${slug}.png`;
    // A modal's backdrop at 2x on an emulated machine is slow to paint.
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
    const manifest = { feature: "apps-together", viewport: VIEW, dpr: DPR, ...this.meta, steps: this.steps };
    await Bun.write(join(this.out, "manifest.json"), JSON.stringify(manifest, null, 2) + "\n");
  }
}

async function signIn(context, base, name, password) {
  const r = await context.request.post(`${base}/auth/api/firstfactor`, {
    data: { username: name, password, keepMeLoggedIn: false, targetURL: `${base}/` },
  });
  if (r.status() !== 200) throw new Error(`sign-in refused: ${r.status()} ${await r.text()}`);
}

const menu = (p) => p.locator('[data-slot="context-menu"]');
const item = (p, id) => menu(p).locator(`[data-item="${id}"]`);

async function openMessage(p, base) {
  await p.goto(`${base}/mail`);
  const row = p.getByRole("button", { name: new RegExp(SUBJECT) }).first();
  await row.waitFor();
  await row.click();
  await p.getByText("Saturday at nine", { exact: false }).first().waitFor();
}

async function fileMenu(p, base) {
  await p.goto(`${base}/cloud/files/Mail`);
  const row = p.locator(`[data-entry="${FILE}"]`);
  await row.waitFor();
  await row.click({ button: "right" });
  await menu(p).waitFor();
  return row;
}

async function answered(p) {
  const dialog = p.locator('[data-slot="ask-ai"]');
  await dialog.locator('[data-slot="ai-answer"]').waitFor();
  await dialog.locator('[data-action="save-note"]').waitFor();
  return dialog;
}

async function after(s, p, base) {
  // Mail: Eva's message, its file saved into Cloud and opened from there.
  await openMessage(p, base);
  const attachment = p.locator(`[data-attachment="${FILE}"]`);
  const save = attachment.locator('[data-action="save-cloud"]');
  await save.waitFor();
  await s.take(p, "mail-message", "Eva's message: the plan can go straight into LosOS Cloud", { kind: "click", target: "save" }, {
    save,
    summarise: p.locator('[data-action="ask-ai"]'),
    attachment,
  });

  await save.click();
  const open = attachment.locator('[data-action="open"]');
  await open.waitFor();
  await s.take(p, "mail-saved", "saved in the Mail folder of LosOS Cloud, and it opens from here", { kind: "click", target: "open" }, { open });

  await open.click();
  await p.waitForURL(/\/cloud\/notes/);
  await p.locator(".note-body h1").first().waitFor();
  await s.take(p, "opened-in-notes", "the plan opened in Notes, the app that edits it", { kind: "none" }, {
    note: p.locator(".note-body").first(),
  });

  // Cloud: the file's menu hands it to the other apps.
  await fileMenu(p, base);
  const ask = item(p, "ask-ai");
  await s.take(p, "cloud-menu", "a right click on the file: send it by mail, or ask the box's AI", { kind: "click", target: "ask" }, {
    mail: item(p, "send-mail"),
    ask,
  });

  await ask.click();
  const dialog = p.locator('[data-slot="ask-ai"]');
  const summarise = dialog.locator('[data-action="summarise"]');
  await summarise.waitFor();
  await s.take(p, "ask-ai", "asking the box's AI about the file", { kind: "click", target: "summarise" }, {
    summarise,
    question: dialog.locator("textarea"),
  });
  await summarise.click();
  await answered(p);
  await s.take(p, "ai-answer", "the answer, written on the box from the file", { kind: "none" }, {
    answer: dialog.locator('[data-slot="ai-answer"]'),
    note: dialog.locator('[data-action="save-note"]'),
  });
  await p.keyboard.press("Escape");

  await fileMenu(p, base);
  await item(p, "send-mail").click();
  await p.waitForURL(/\/mail/);
  const chip = p.locator(`[data-attached="${FILE}"]`);
  await chip.waitFor();
  // Read from Cloud: the spinner gives way to the file's size.
  await chip.locator("[data-slot=spinner]").waitFor({ state: "detached" });
  await p.locator("#mail-to").fill(EVA);
  await p.locator("#mail-subject").fill(`Re: ${SUBJECT}`);
  const body = p.getByRole("dialog").locator("textarea");
  const reply = "Added the second canoe. See the plan attached.";
  if (s.typing) {
    const words = reply.split(" ");
    for (let i = 1; i <= words.length; i++) {
      await body.fill(words.slice(0, i).join(" "));
      await s.take(p, `reply-${i}`, "typing the reply", { kind: "type", target: "body" }, { body });
    }
  } else {
    await body.fill(reply);
  }
  const send = p.getByRole("dialog").getByRole("button", { name: "Send", exact: true });
  await s.take(p, "compose-from-cloud", "a new message with the file from LosOS Cloud already attached", { kind: "click", target: "send" }, {
    chip,
    send,
  });
  await send.click();
  await p.getByRole("dialog").waitFor({ state: "detached" });

  // Mail: the box's AI on the thread itself.
  await openMessage(p, base);
  await p.locator('[data-action="ask-ai"]').click();
  await answered(p);
  await s.take(p, "mail-summary", "the message summarised by the box's AI, without it leaving the box", { kind: "none" }, {
    answer: p.locator('[data-slot="ai-answer"]'),
  });
}

async function before(s, p, base) {
  await openMessage(p, base);
  const attachment = p.locator("li").filter({ hasText: FILE }).first();
  await attachment.waitFor();
  await s.take(p, "mail-message", "Eva's message: the plan can only be downloaded", { kind: "none" }, { attachment });
  await p.goto(`${base}/cloud/files/Mail`);
  const row = p.locator(`[data-entry="${FILE}"]`);
  await row.waitFor();
  await row.click({ button: "right" });
  await menu(p).waitFor();
  await s.take(p, "cloud-menu", "a right click on the file: nothing leads to the other apps", { kind: "none" }, { menu: menu(p) });
  await p.keyboard.press("Escape");
  await p.goto(`${base}/mail`);
  await p.getByRole("button", { name: "Write", exact: true }).click();
  await p.locator("#mail-to").waitFor();
  await s.take(p, "compose", "a new message: no way to attach a file", { kind: "none" }, {});
}

async function closed(s, p, base) {
  await fileMenu(p, base);
  if ((await item(p, "send-mail").count()) !== 0) throw new Error("Send by mail is offered on a box without Mail");
  await item(p, "ask-ai").waitFor();
  await s.take(p, "cloud-menu-closed", "the box no longer shares: no Mail to send to, the AI is still there", { kind: "none" }, {
    menu: menu(p),
  });
}

function parse(argv) {
  const pos = [];
  const opts = {};
  for (let i = 0; i < argv.length; i++) {
    if (argv[i] === "--test") opts.test = argv[++i];
    else if (argv[i] === "--ca") opts.ca = argv[++i];
    else if (argv[i] === "--commit") opts.commit = argv[++i];
    else if (argv[i] === "--typing") opts.typing = true;
    else if (argv[i] === "--view") opts.view = argv[++i];
    else if (argv[i] === "--dpr") opts.dpr = Number(argv[++i]);
    else if (argv[i] === "--video") opts.video = argv[++i];
    else pos.push(argv[i]);
  }
  return { verb: pos[0], base: pos[1], name: pos[2], password: pos[3], out: pos[4], opts };
}

const { verb, base, name, password, out, opts } = parse(Bun.argv.slice(2));
const walks = { after, before, closed };
if (opts.view) {
  const [width, height] = opts.view.split("x").map(Number);
  VIEW = { width, height };
}
if (opts.dpr) DPR = opts.dpr;
if (!(verb in walks)) {
  // Thrown: an uncaught error is what makes Bun exit non-zero.
  throw new Error(`unknown verb ${verb}`);
}
const browser = await chromium.launch({
  args: [`--ignore-certificate-errors-spki-list=${await spkiPin(opts.ca ?? "/etc/losos-box.pem")}`],
});
const context = await browser.newContext({
  viewport: VIEW,
  deviceScaleFactor: DPR,
  // The video at the pictures' own pixels.
  ...(opts.video ? { recordVideo: { dir: opts.video, size: { width: VIEW.width * DPR, height: VIEW.height * DPR } } } : {}),
  colorScheme: "dark",
  locale: "en-US",
  timezoneId: "Europe/Bratislava",
  reducedMotion: "reduce",
  // The sign-in below goes through Node's TLS, which the pin above does not
  // reach; the box's certificate is the self-signed one either way.
  ignoreHTTPSErrors: true,
  // The film's take hides the caret with a style of its own, which the
  // admin page's style-src would refuse.
  ...(opts.typing ? { bypassCSP: true } : {}),
});
if (opts.typing) {
  // The film draws its own caret and pointer.
  await context.addInitScript(() => {
    document.addEventListener("DOMContentLoaded", () => {
      const style = document.createElement("style");
      style.textContent = "* { caret-color: transparent !important; }";
      document.head.append(style);
    });
  });
}
const page = await context.newPage();
page.setDefaultTimeout(180_000);
const problems = [];
page.on("pageerror", (e) => problems.push(String(e)));
await signIn(context, base, name, password);

const s = new Shots(base, out, { commit: opts.commit ?? "", test: opts.test ?? "", verb });
s.typing = opts.typing === true;
try {
  await walks[verb](s, page, base);
} catch (err) {
  await page.screenshot({ path: join(out, `failed-${verb}.png`) }).catch(() => {});
  throw err;
} finally {
  await s.save();
  // The video is written when its page closes.
  await context.close();
}
await browser.close();
if (problems.length > 0) {
  // Thrown: an uncaught error is what makes Bun exit non-zero.
  throw new Error(`the pages threw:\n${problems.join("\n")}`);
}
