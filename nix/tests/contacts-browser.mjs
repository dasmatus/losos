// The browser half of nix/tests/contacts.nix: LosOS Contacts in Chromium,
// and the pictures of it.
//
//   node contacts-browser.mjs before <out>  the admin pages before Contacts
//   node contacts-browser.mjs off    <out>  the book with sharing off
//   node contacts-browser.mjs on     <out>  the book and the apps that use it
//   node contacts-browser.mjs film   <out>  the launch film's states, in <out>/film/
//
// As in nix/tests/chat-browser.mjs the box is reached through a SOCKS proxy
// on the LAN, so pages load at https://mattbox.local/. Pictures go to
// <out>/pr/. It exits non-zero at the first step that does not happen,
// after a picture of every open page.

const { chromium } = await import(Bun.env.CONTACTS_PLAYWRIGHT ?? "playwright");

const [phase, out] = Bun.argv.slice(2);
const HOST = Bun.env.CONTACTS_HOST ?? "mattbox.local";
const ORIGIN = `https://${HOST}`;
const PROXY = Bun.env.CONTACTS_PROXY ?? "socks5://127.0.0.1:1080";
const OLD_CARD = Bun.env.CONTACTS_OLD_CARD;
// The card under the name the page sees: Playwright picks a file by path.
const PHONE_CARD = `${Bun.env.TMPDIR ?? "/tmp"}/contacts-${crypto.randomUUID()}/phone.vcf`;
await Bun.write(PHONE_CARD, Bun.file(OLD_CARD));
Bun.spawnSync(["mkdir", "-p", `${out}/pr`]);

const browser = await chromium.launch({ chromiumSandbox: false, proxy: { server: PROXY } });
const problems = [];
const pages = [];

async function person(name, password, film = false) {
  const context = await browser.newContext({
    viewport: film ? { width: 1280, height: 900 } : { width: 1280, height: 860 },
    deviceScaleFactor: 2,
    colorScheme: "dark",
    locale: "en-US",
    timezoneId: "Europe/Bratislava",
    reducedMotion: "reduce",
    ignoreHTTPSErrors: true,
    acceptDownloads: true,
  });
  const page = await context.newPage();
  page.setDefaultTimeout(120_000);
  page.on("pageerror", (error) => problems.push(`${name}: ${error.message}`));
  page.on("console", (msg) => {
    if (msg.type() === "error" && msg.text().includes("Content Security Policy") && !msg.text().includes("inline style")) {
      problems.push(`${name}: ${msg.text()}`);
    }
  });
  const answer = await context.request.post(`${ORIGIN}/auth/api/firstfactor`, { data: { username: name, password } });
  if (!answer.ok()) throw new Error(`${name} could not sign in: ${answer.status()} ${await answer.text()}`);
  pages.push(page);
  return page;
}

async function settle(page) {
  await page.evaluate(() => document.fonts.ready);
  await page.evaluate(() => {
    document.documentElement.style.caretColor = "transparent";
  });
  await page.waitForFunction(() => !document.querySelector(".animate-spin"), null, { timeout: 30_000 }).catch(() => {});
  await page.waitForTimeout(1000);
}

async function shot(page, name) {
  await settle(page);
  await page.screenshot({ path: `${out}/pr/${name}.png` });
  console.log(`shot ${name}`);
}

const contactsLink = (page) => page.getByRole("link", { name: "Contacts", exact: true });

/** The overview once its app tiles are in, so Before and After show the
 *  same state rather than one of them still looking for the apps. */
async function overview(page) {
  await page.goto(`${ORIGIN}/`);
  await page
    .getByText("Looking for the apps on this box.")
    .waitFor({ state: "detached", timeout: 120_000 })
    .catch(() => {});
}

/** Mail's To field with "ev" typed into it. */
async function mailTo(page, name) {
  await page.goto(`${ORIGIN}/mail`);
  await page.getByRole("button", { name: "Write" }).first().click();
  const to = page.locator("#mail-to");
  await to.waitFor();
  await to.click();
  await to.pressSequentially("ev", { delay: 80 });
  await page.waitForTimeout(1500);
  await shot(page, name);
  await page.keyboard.press("Escape");
}

/** Chat's "Message someone", open. */
async function chatPicker(page, name) {
  await page.goto(`${ORIGIN}/chat/`);
  await page.getByRole("heading", { name: "Conversations" }).waitFor();
  await page.getByRole("button", { name: "Message someone" }).first().click();
  await page.getByRole("textbox", { name: "Name or @name:server" }).waitFor();
  await page.waitForTimeout(2000);
  await shot(page, name);
  await page.keyboard.press("Escape");
}

async function before() {
  const owner = await person("matus", Bun.env.CONTACTS_MATUS_PASSWORD);
  await overview(owner);
  await owner.getByRole("link", { name: "Chat", exact: true }).waitFor();
  await shot(owner, "sidebar-before");
  await mailTo(owner, "mail-to-before");
  await chatPicker(owner, "chat-picker-before");
}

async function off() {
  // Without the shared side Contacts stays, LosOS Cloud keeps the book;
  // only the ways to reach someone through chat and mail are gone.
  const owner = await person("matus", Bun.env.CONTACTS_MATUS_PASSWORD);
  await overview(owner);
  await contactsLink(owner).waitFor();
  if ((await owner.getByRole("link", { name: "Chat", exact: true }).count()) !== 0) {
    throw new Error("Chat is in the sidebar with sharing off");
  }
  await shot(owner, "sidebar-sharing-off");
  await contactsLink(owner).click();
  const list = owner.getByRole("region", { name: "Your contacts" });
  await list.getByRole("button", { name: /Eva Nováková/ }).click();
  const page = owner.getByRole("article");
  await page.getByRole("heading", { name: "Eva Nováková" }).waitFor();
  await page.getByText("eva@mattbox.home.arpa").first().waitFor();
  for (const action of ["Mail", "Chat", "Call", "Video"]) {
    if ((await page.getByRole("button", { name: action, exact: true }).count()) !== 0) {
      throw new Error(`a contact has a ${action} button with sharing off`);
    }
  }
  await shot(owner, "contacts-sharing-off");
}

async function on() {
  const owner = await person("matus", Bun.env.CONTACTS_MATUS_PASSWORD);
  await overview(owner);
  await contactsLink(owner).waitFor();
  await shot(owner, "sidebar-after");

  // The book, and a person's page.
  await contactsLink(owner).click();
  await owner.waitForURL(`${ORIGIN}/contacts`);
  const list = owner.getByRole("region", { name: "Your contacts" });
  await list.getByRole("button", { name: /Jana Kováčová/ }).click();
  const page = owner.getByRole("article");
  await page.getByRole("heading", { name: "Jana Kováčová" }).waitFor();
  await page.getByText("Bread on Fridays, rye only.").waitFor();
  await shot(owner, "contacts-person");

  await list.getByRole("button", { name: /Eva Nováková/ }).click();
  await page.getByRole("heading", { name: "Eva Nováková" }).waitFor();
  for (const action of ["Mail", "Chat", "Call", "Video"]) {
    await page.getByRole("button", { name: action, exact: true }).waitFor();
  }
  await shot(owner, "contacts-eva");

  // Right-click on a row: the ways to reach them, edit, export, delete.
  await list.getByRole("button", { name: /Tomáš Bielik/ }).click({ button: "right" });
  const menu = owner.locator('[data-slot="context-menu"]').first();
  await menu.waitFor();
  const items = (await menu.locator('[role^="menuitem"]').allInnerTexts()).map((s) => s.trim().split("\n")[0]);
  for (const want of ["Chat", "Call", "Video", "Edit", "Export as .vcf", "Delete"]) {
    if (!items.includes(want)) throw new Error(`the row's menu has no ${want}: ${JSON.stringify(items)}`);
  }
  await shot(owner, "contacts-menu");
  await owner.keyboard.press("Escape");
  await menu.waitFor({ state: "detached" });

  // Everyone on the box, contact or not.
  await owner.getByRole("button", { name: /On this box/ }).click();
  const boxList = owner.getByRole("region", { name: "On this box" });
  await boxList.getByRole("button", { name: /^Eva/ }).click();
  await page.getByText("Has an account on this box").waitFor();
  await shot(owner, "contacts-box");
  await owner.getByRole("button", { name: /Your contacts/ }).click();

  // A new contact by hand; the chat field offers people as it is typed.
  await owner.getByRole("button", { name: "New contact" }).click();
  const editor = owner.getByRole("dialog");
  await editor.locator("#contact-name").fill("Lucia Malá");
  await editor.locator("#contact-org").fill("Knižnica Bystrica");
  await editor.locator("#contact-email").fill("lucia@kniznica.example");
  const matrix = editor.locator("#contact-matrix");
  await matrix.click();
  await matrix.pressSequentially("ev", { delay: 80 });
  await owner.getByRole("option", { name: /Eva Nováková/ }).waitFor();
  await shot(owner, "contacts-editor-picker");
  // Escape closes the suggestions, not the editor.
  await owner.keyboard.press("Escape");
  await owner.getByRole("option", { name: /Eva Nováková/ }).waitFor({ state: "hidden" });
  if (!(await editor.isVisible())) throw new Error("Escape in the picker closed the editor");
  await matrix.fill("@lucia:kitchenbox.local");
  await editor.locator("#contact-name").click();
  await editor.getByRole("button", { name: "Save" }).click();
  await editor.waitFor({ state: "detached" });
  await page.getByRole("heading", { name: "Lucia Malá" }).waitFor();
  await shot(owner, "contacts-saved");

  // An import: the old phone's 2.1 card.
  await owner.getByRole("button", { name: "Import", exact: true }).click();
  const importer = owner.getByRole("dialog");
  await importer.locator('input[type="file"]').setInputFiles(PHONE_CARD);
  await importer.getByText("1 added, 0 updated").waitFor();
  await shot(owner, "contacts-import");
  await importer.getByRole("button", { name: "Done" }).click();
  await list.getByRole("button", { name: /Ondrej Rak/ }).waitFor();

  await owner.getByRole("button", { name: "Export", exact: true }).click();
  await owner.getByRole("dialog").getByRole("button", { name: /vCard 3\.0/ }).waitFor();
  await shot(owner, "contacts-export");
  await owner.keyboard.press("Escape");

  // Mail: the To field offers contacts, and a contact's Mail opens a message to them.
  await mailTo(owner, "mail-to-after");
  await owner.goto(`${ORIGIN}/contacts`);
  await list.getByRole("button", { name: /Eva Nováková/ }).click();
  await page.getByRole("button", { name: "Mail", exact: true }).click();
  await owner.waitForURL(`${ORIGIN}/mail**`);
  await owner.locator("#mail-to").waitFor();
  const to = await owner.locator("#mail-to").inputValue();
  if (!to.includes("eva@mattbox.home.arpa")) throw new Error(`Mail opened to "${to}"`);
  await shot(owner, "mail-from-contact");

  // Chat: "Message someone" offers contacts, and a contact's Chat opens the conversation.
  await chatPicker(owner, "chat-picker-after");
  await owner.goto(`${ORIGIN}/contacts?person=eva`);
  await page.getByText("Has an account on this box").waitFor();
  await owner.goto(`${ORIGIN}/contacts`);
  await list.getByRole("button", { name: /Eva Nováková/ }).click();
  await page.getByRole("button", { name: "Chat", exact: true }).click();
  await owner.waitForURL(`${ORIGIN}/chat/**`);
  await owner.getByRole("textbox", { name: "Message Eva" }).waitFor();
  if (owner.url().includes("with=")) throw new Error(`the link stayed in the address bar: ${owner.url()}`);
  await shot(owner, "chat-from-contact");
}

// ── The launch film ────────────────────────────────────────────────────────
// One 1280x900 picture at 2x per state and per typed letter, and the whole
// walk as one 60 fps clip at the same size. Chromium's screencast hands over a frame
// whenever the page draws one; ffmpeg lays them out on a 60 fps timeline by
// their own timestamps. The manifest says where each picture leads, what it
// may point at, and where it sits in the clip.

const steps = [];
const frames = [];
let clipStart = null;

async function box(locator) {
  const b = await locator.boundingBox();
  if (b === null) throw new Error("a film target is not on the screen");
  return { x: Math.round(b.x), y: Math.round(b.y), w: Math.round(b.width), h: Math.round(b.height) };
}

async function frame(page, title, action = { kind: "none" }, targets = {}) {
  await settle(page);
  const file = `${String(steps.length + 1).padStart(2, "0")}-${title.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "")}.png`;
  await page.screenshot({ path: `${out}/film/${file}` });
  const at = new URL(page.url());
  const shown = {};
  for (const [name, locator] of Object.entries(targets)) shown[name] = await box(locator);
  const clipAt = clipStart === null ? null : Math.round((Date.now() / 1000 - clipStart) * 1000) / 1000;
  steps.push({ file, url: `${HOST}${at.pathname}${at.search}`, title, action, targets: shown, clipAt });
  console.log(`film ${file}`);
}

/** Types `text` a letter at a time, a picture after each. */
async function typed(page, field, text, title) {
  await field.click();
  for (let i = 1; i <= text.length; i++) {
    await field.pressSequentially(text[i - 1], { delay: 40 });
    await page.waitForTimeout(i === text.length ? 1500 : 400);
    await frame(page, `${title} ${text.slice(0, i)}`, { kind: "type", target: "field", text: text.slice(0, i) }, { field });
  }
}

async function record(page) {
  Bun.spawnSync(["mkdir", "-p", `${out}/film/frames`]);
  const cdp = await page.context().newCDPSession(page);
  const writes = [];
  cdp.on("Page.screencastFrame", ({ data, metadata, sessionId }) => {
    const name = `frames/${String(frames.length).padStart(6, "0")}.jpg`;
    // Written as they come; the clip waits for every one before ffmpeg.
    writes.push(Bun.write(`${out}/film/${name}`, Uint8Array.fromBase64(data)));
    frames.push({ name, t: metadata.timestamp });
    if (clipStart === null) clipStart = metadata.timestamp;
    cdp.send("Page.screencastFrameAck", { sessionId }).catch(() => {});
  });
  await cdp.send("Page.startScreencast", { format: "jpeg", quality: 95, maxWidth: 2560, maxHeight: 1800, everyNthFrame: 1 });
  return async () => {
    await page.waitForTimeout(1000);
    const clipName = `${String(steps.length + 1).padStart(2, "0")}-walkthrough.mp4`;
    await cdp.send("Page.stopScreencast");
    await Promise.all(writes);
    // Each frame stays on screen until the next one; ffmpeg's fps filter
    // then samples that at 60 per second.
    const lines = ["ffconcat version 1.0"];
    for (const [i, f] of frames.entries()) {
      const next = frames[i + 1]?.t ?? f.t + 1;
      lines.push(`file '${f.name}'`, `duration ${Math.max(next - f.t, 0.001).toFixed(6)}`);
    }
    lines.push(`file '${frames.at(-1).name}'`);
    await Bun.write(`${out}/film/frames.ffconcat`, lines.join("\n") + "\n");
    const ffmpeg = Bun.spawnSync(
      [Bun.env.CONTACTS_FFMPEG ?? "ffmpeg",
       "-y", "-loglevel", "error", "-f", "concat", "-safe", "0", "-i", "frames.ffconcat",
       "-vf", "fps=60,scale=2560:1800:flags=lanczos,format=yuv420p", "-r", "60",
       "-c:v", "libx264", "-preset", "slow", "-crf", "16", "-movflags", "+faststart", clipName],
      { cwd: `${out}/film` },
    );
    if (ffmpeg.exitCode !== 0) throw new Error(`ffmpeg: ${new TextDecoder().decode(ffmpeg.stderr)}`);
    console.log(`film ${clipName} from ${frames.length} frames`);
    steps.push({ file: clipName, url: `${HOST}/`, title: "The walk through Contacts, Mail and Chat", action: { kind: "none" }, targets: {}, fps: 60, frames: frames.length });
  };
}

async function film() {
  Bun.spawnSync(["mkdir", "-p", `${out}/film`]);
  const owner = await person("matus", Bun.env.CONTACTS_MATUS_PASSWORD, true);
  await overview(owner);
  await settle(owner);
  const stop = await record(owner);
  await owner.waitForTimeout(500);
  await frame(owner, "Overview with Contacts", { kind: "click", target: "contacts" }, { contacts: contactsLink(owner) });

  await contactsLink(owner).click();
  await owner.waitForURL(`${ORIGIN}/contacts`);
  const list = owner.getByRole("region", { name: "Your contacts" });
  const eva = list.getByRole("button", { name: /Eva Nováková/ });
  await eva.waitFor();
  await frame(owner, "The book", { kind: "click", target: "eva" }, { eva });

  await eva.click();
  const card = owner.getByRole("article");
  await card.getByRole("heading", { name: "Eva Nováková" }).waitFor();
  const button = (name) => card.getByRole("button", { name, exact: true });
  await frame(owner, "Eva's card", { kind: "click", target: "mail" }, {
    mail: button("Mail"),
    chat: button("Chat"),
    call: button("Call"),
    video: button("Video"),
  });

  await button("Mail").click();
  await owner.waitForURL(`${ORIGIN}/mail**`);
  await owner.locator("#mail-to").waitFor();
  await frame(owner, "A message to Eva", { kind: "none" }, { to: owner.locator("#mail-to") });
  await owner.keyboard.press("Escape");

  // Mail's To field suggests the book as a name is typed.
  await owner.goto(`${ORIGIN}/mail`);
  await owner.getByRole("button", { name: "Write" }).first().click();
  const to = owner.locator("#mail-to");
  await to.waitFor();
  await typed(owner, to, "Eva", "Mail To");
  await frame(owner, "Mail suggests Eva", { kind: "click", target: "eva" }, {
    eva: owner.getByRole("option", { name: /Eva Nováková/ }).first(),
  });
  await owner.keyboard.press("Escape");

  // Chat's "Message someone" does the same.
  await owner.goto(`${ORIGIN}/chat/`);
  await owner.getByRole("heading", { name: "Conversations" }).waitFor();
  await owner.getByRole("button", { name: "Message someone" }).first().click();
  const who = owner.getByRole("textbox", { name: "Name or @name:server" });
  await who.waitFor();
  await typed(owner, who, "Eva", "Chat");
  await frame(owner, "Chat suggests Eva", { kind: "none" });
  await owner.keyboard.press("Escape");

  // Importing a .vcf file from an old phone.
  await owner.goto(`${ORIGIN}/contacts`);
  await owner.getByRole("button", { name: "Import", exact: true }).click();
  const importer = owner.getByRole("dialog");
  await importer.locator('input[type="file"]').setInputFiles(PHONE_CARD);
  await importer.getByText(/updated/).waitFor();
  await frame(owner, "Import from a phone", { kind: "none" });
  await importer.getByRole("button", { name: "Done" }).click();
  await stop();

  await Bun.write(
    `${out}/film/manifest.json`,
    JSON.stringify(
      {
        feature: "contacts",
        viewport: { width: 1280, height: 900 },
        dpr: 2,
        commit: Bun.env.CONTACTS_COMMIT ?? "",
        test: "losos-contacts",
        steps,
      },
      null,
      2,
    ) + "\n",
  );
}

// Thrown at the very end, after the pictures of the failure are saved:
// an uncaught error is what makes Bun exit non-zero.
let failed = false;
try {
  await { before, off, on, film }[phase]();
  if (problems.length > 0) throw new Error(`the page reported: ${problems.join("; ")}`);
} catch (error) {
  for (const [i, page] of pages.entries()) {
    await page.screenshot({ path: `${out}/pr/failed-${phase}-${i}.png` }).catch(() => {});
  }
  console.error(error);
  failed = true;
} finally {
  await browser.close();
}
if (failed) throw new Error(`the ${phase} phase failed, see above`);
