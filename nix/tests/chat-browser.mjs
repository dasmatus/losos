// The browser half of nix/tests/chat.nix: two people on the chat page in
// Chromium, and the pictures of it.
//
//   node chat-browser.mjs off <out>   the admin pages with sharing off
//   node chat-browser.mjs on  <out>   everything else, once sharing is on
//
// The test driver runs it with CHAT_PLAYWRIGHT naming the playwright
// package, PLAYWRIGHT_BROWSERS_PATH at nixpkgs' browsers, and the box
// reached through a SOCKS proxy on the LAN, so pages load at
// https://mattbox.local/ as on any computer there. Pictures go to
// <out>/film/ (the launch film's states, numbered, with manifest.json) and
// <out>/pr/ (the pull request's). It exits non-zero at the first step that
// does not happen, after a picture of every open page.

const exists = (file) => Bun.file(file).exists();

const { chromium } = await import(Bun.env.CHAT_PLAYWRIGHT ?? "playwright");

const [phase, out] = Bun.argv.slice(2);
const HOST = Bun.env.CHAT_HOST ?? "mattbox.local";
const ORIGIN = `https://${HOST}`;
const PROXY = Bun.env.CHAT_PROXY ?? "socks5://127.0.0.1:1080";
const PICTURE = Bun.env.CHAT_PICTURE;
// A plain red square and a plain green one: the stand-in nudity model the
// test builds flags the first and passes the second.
const RED = Bun.env.CHAT_RED;
const GREEN = Bun.env.CHAT_GREEN;
const PEOPLE = {
  matus: Bun.env.CHAT_MATUS_PASSWORD,
  eva: Bun.env.CHAT_EVA_PASSWORD,
};
Bun.spawnSync(["mkdir", "-p", `${out}/film`]);
Bun.spawnSync(["mkdir", "-p", `${out}/pr`]);
const stepsFile = `${out}/film/steps.json`;
const steps = await exists(stepsFile) ? (await Bun.file(stepsFile).json()) : [];

const browser = await chromium.launch({
  chromiumSandbox: false,
  proxy: { server: PROXY },
  args: ["--use-fake-ui-for-media-stream", "--use-fake-device-for-media-stream"],
});
const problems = [];
const network = [];
const start = performance.now();

/** A browser of its own for one person, signed in through LosOS Login. */
async function person(name) {
  const context = await browser.newContext({
    viewport: { width: 1280, height: 900 },
    deviceScaleFactor: 2,
    colorScheme: "dark",
    locale: "en-US",
    timezoneId: "Europe/Bratislava",
    reducedMotion: "reduce",
    ignoreHTTPSErrors: true,
    acceptDownloads: true,
  });
  const page = await context.newPage();
  // The box runs under emulation in the test, so first loads are slow.
  page.setDefaultTimeout(120_000);
  page.on("pageerror", (error) => problems.push(`${name}: ${error.message}`));
  // What the network did, in <out>/network-<phase>.log, and its tail when
  // a step does not happen.
  const began = new Map();
  const at = () => ((performance.now() - start) / 1000).toFixed(1).padStart(7);
  page.on("request", (r) => began.set(r, performance.now()));
  page.on("requestfinished", async (r) => {
    const took = ((performance.now() - (began.get(r) ?? 0)) / 1000).toFixed(1);
    const status = (await r.response().catch(() => null))?.status() ?? "-";
    network.push(`${at()} ${name} ${status} ${took}s ${r.method()} ${r.url().replace(ORIGIN, "")}`);
  });
  page.on("requestfailed", (r) => network.push(`${at()} ${name} failed ${r.method()} ${r.url().replace(ORIGIN, "")} ${r.failure()?.errorText}`));
  // Any refusal of the page's policy, but sonner's two known inline styles.
  page.on("console", (msg) => {
    if (msg.type() === "error" && msg.text().includes("Content Security Policy") && !msg.text().includes("inline style")) {
      problems.push(`${name}: ${msg.text()}`);
    }
  });
  const answer = await context.request.post(`${ORIGIN}/auth/api/firstfactor`, {
    data: { username: name, password: PEOPLE[name] },
  });
  if (!answer.ok()) throw new Error(`${name} could not sign in: ${answer.status()} ${await answer.text()}`);
  return page;
}

async function settle(page) {
  await page.evaluate(() => document.fonts.ready);
  await page.evaluate(() => {
    document.documentElement.style.caretColor = "transparent";
  });
  await page.waitForFunction(() => !document.querySelector(".animate-spin"), null, { timeout: 30_000 });
  await page.waitForTimeout(1000);
}

async function boxOf(locator) {
  const b = await locator.boundingBox();
  return b === null ? null : { x: Math.round(b.x), y: Math.round(b.y), w: Math.round(b.width), h: Math.round(b.height) };
}

/** One state of the launch film: a PNG and its line in manifest.json. */
async function film(page, slug, url, title, action = "none", targets = {}) {
  await settle(page);
  const file = `${String(steps.length + 1).padStart(2, "0")}-${slug}.png`;
  await page.screenshot({ path: `${out}/film/${file}` });
  const boxes = {};
  for (const [key, locator] of Object.entries(targets)) boxes[key] = await boxOf(locator);
  const target = Object.keys(targets)[0];
  steps.push({ file, url, title, action: action === "none" || target === undefined ? { kind: action } : { kind: action, target }, targets: boxes });
}

async function shot(page, name) {
  await settle(page);
  await page.screenshot({ path: `${out}/pr/${name}.png` });
}

/** The handbook's picture of the conversation (handbook/docs/img/chat*.png),
 *  light, in each of its languages, on a device of Matus's unlocked with the
 *  recovery key. The labels are src/i18n/chat.ts's. */
const UNLOCK = {
  en: ["Unlock this device to read encrypted messages", "Recovery key", "Unlock"],
  sk: ["Odomknite toto zariadenie na čítanie šifrovaných správ", "Kľúč na obnovenie", "Odomknúť"],
  de: ["Entsperre dieses Gerät, um verschlüsselte Nachrichten zu lesen", "Wiederherstellungsschlüssel", "Entsperren"],
};
async function handbook(key) {
  for (const [lang, locale] of [["en", "en-US"], ["sk", "sk-SK"], ["de", "de-DE"]]) {
    const [unlock, field, button] = UNLOCK[lang];
    const context = await browser.newContext({
      viewport: { width: 1280, height: 900 },
      deviceScaleFactor: 2,
      colorScheme: "light",
      locale,
      timezoneId: "Europe/Bratislava",
      reducedMotion: "reduce",
      ignoreHTTPSErrors: true,
    });
    const page = await context.newPage();
    page.setDefaultTimeout(120_000);
    const answer = await context.request.post(`${ORIGIN}/auth/api/firstfactor`, {
      data: { username: "matus", password: PEOPLE.matus },
    });
    if (!answer.ok()) throw new Error(`matus could not sign in: ${answer.status()}`);
    await page.goto(`${ORIGIN}/chat/`);
    await page.getByRole("button", { name: /Eva/ }).first().click();
    await page.getByRole("button", { name: unlock }).click();
    await page.getByRole("textbox", { name: field }).fill(key);
    await page.getByRole("button", { name: button, exact: true }).click();
    await page.getByRole("log").getByText("Hi Eva, chat is up on mattbox.").waitFor();
    await page.getByRole("log").getByText("Nice. Can we plan the holiday here?").waitFor();
    Bun.spawnSync(["mkdir", "-p", `${out}/handbook/${lang}`]);
    await settle(page);
    await page.screenshot({ path: `${out}/handbook/${lang}/chat.png`, timeout: 300_000 });
    await context.close();
  }
}

const chatLink = (page) => page.getByRole("link", { name: "Chat", exact: true });

async function off() {
  const owner = await person("matus");
  await owner.goto(`${ORIGIN}/`);
  await owner.getByRole("link", { name: "Overview", exact: true }).waitFor();
  await owner.waitForTimeout(3000);
  if ((await chatLink(owner).count()) !== 0) throw new Error("Chat is in the sidebar with sharing off");
  await film(owner, "sharing-off", `${HOST}/`, "the admin pages with sharing off: no Chat");
  await shot(owner, "sidebar-before");
}

async function on() {
  const owner = await person("matus");
  await owner.goto(`${ORIGIN}/`);
  await chatLink(owner).waitFor();
  await film(owner, "sharing-on", `${HOST}/`, "the admin pages with sharing on: Chat in the sidebar", "click", { chat: chatLink(owner) });
  await shot(owner, "sidebar-after");
  await chatLink(owner).click();
  await owner.waitForURL(`${ORIGIN}/chat/**`);
  await owner.getByRole("heading", { name: "Conversations" }).waitFor();

  // Secure messages: a recovery key on the first device.
  await owner.getByRole("button", { name: "Set up secure messages" }).click();
  await owner.getByRole("button", { name: "Make my recovery key" }).click();
  const key = ((await owner.getByTestId("recovery-key").textContent()) ?? "").trim();
  if (key.split(/\s+/).length < 10) throw new Error(`not a recovery key: ${key}`);
  await shot(owner, "recovery-key");
  const download = owner.waitForEvent("download");
  await owner.getByRole("button", { name: "Download" }).click();
  await download;
  await owner.getByRole("button", { name: "I saved it" }).click();
  await owner.getByRole("button", { name: "Set up secure messages" }).waitFor({ state: "detached" });

  // Matus writes to Eva; the room is end to end encrypted.
  const eva = await person("eva");
  await eva.goto(`${ORIGIN}/chat/`);
  await eva.getByRole("heading", { name: "Conversations" }).waitFor();
  await owner.getByRole("button", { name: "Message someone" }).first().click();
  await owner.getByRole("textbox", { name: "Name or @name:server" }).fill("eva");
  await owner.getByRole("dialog").getByRole("button", { name: /Eva/ }).first().click();
  const composer = owner.getByRole("textbox", { name: "Message Eva" });
  await composer.waitFor();
  for (const text of ["Hi Eva, chat is up on the box.", "It's end to end encrypted, and nobody needs a chat password."]) {
    await composer.fill(text);
    await owner.keyboard.press("Enter");
    await owner.getByRole("log").getByText(text).waitFor();
  }
  await eva.getByRole("button", { name: /Matus/ }).first().click();
  await eva.getByRole("button", { name: "Join", exact: true }).click();
  await eva.getByRole("log").getByText("Hi Eva, chat is up on the box.").waitFor();
  await eva.getByRole("textbox", { name: "Message Matus" }).fill("Nice. Can we plan the holiday here?");
  await eva.keyboard.press("Enter");
  await owner.getByRole("log").getByText("Nice. Can we plan the holiday here?").waitFor();
  // Both see the conversation in the order it was written, Eva too,
  // though she joined after the first two messages.
  for (const page of [owner, eva]) {
    const log = page.getByRole("log");
    await log.getByText("Nice. Can we plan the holiday here?").waitFor();
    const order = await log.getByText(/^(Hi Eva, chat is up|It's end to end|Nice\. Can we plan)/).allTextContents();
    const want = ["Hi Eva, chat is up on the box.", "It's end to end encrypted, and nobody needs a chat password.", "Nice. Can we plan the holiday here?"];
    if (JSON.stringify(order) !== JSON.stringify(want)) throw new Error(`out of order: ${JSON.stringify(order)}`);
  }
  await film(owner, "room-with-eva", `${HOST}/chat/`, "a conversation with Eva", "type", { composer });
  await shot(owner, "conversation");
  await shot(eva, "conversation-eva");

  // Matus types a reply, one picture per word, and sends it.
  const words = "Yes, I'll make a room for it tonight.".split(" ");
  let typed = "";
  await composer.click();
  for (const [i, word] of words.entries()) {
    typed = i === 0 ? word : `${typed} ${word}`;
    await composer.fill(typed);
    await film(owner, `typing-${i + 1}`, `${HOST}/chat/`, `typing: ${typed}`, "type", { composer });
  }
  await owner.keyboard.press("Enter");
  await owner.getByRole("log").getByText(typed).waitFor();
  await eva.getByRole("log").getByText(typed).waitFor();
  await film(owner, "sent", `${HOST}/chat/`, "the reply sent");

  // A reaction, a thread, a picture and an edit.
  // Eva reacts from the message's right-click menu, which offers what
  // the hover toolbar does.
  await eva.getByRole("log").getByText("Hi Eva, chat is up on the box.", { exact: true }).click({ button: "right" });
  const menu = eva.locator('[data-slot="context-menu"]').first();
  await menu.waitFor();
  if ((await menu.getAttribute("aria-label")) !== "Message from Matus") throw new Error(`menu named ${await menu.getAttribute("aria-label")}`);
  const items = (await menu.locator('[role^="menuitem"]').allInnerTexts()).map((s) => s.trim().split("\n")[0]);
  if (JSON.stringify(items.slice(0, 4)) !== JSON.stringify(["Reply", "Reply in a thread", "React", "Copy"]) || items.includes("Edit")) {
    throw new Error(`message menu: ${JSON.stringify(items)}`);
  }
  await menu.getByRole("menuitem", { name: "React" }).hover();
  const reactions = eva.locator('[data-slot="context-menu"][aria-label="React"]');
  await reactions.waitFor();
  await film(eva, "message-menu", `${HOST}/chat/`, "a message's right-click menu: reply, thread, react, copy", "click", { react: menu.getByRole("menuitem", { name: "React" }) });
  await shot(eva, "message-menu");
  await reactions.getByRole("menuitem", { name: "👍" }).click();
  // The reaction under the message reads "👍 1"; the quick reactions'
  // pill under every message has a bare "👍".
  await owner.getByRole("log").getByRole("button", { name: /^👍\s*1$/ }).waitFor();
  // Hovering a message shows the quick reactions in a pill under it; one
  // press adds a reaction and a second takes it back.
  const evaRow = owner.getByRole("log").locator("[data-event-id]", { hasText: "Nice. Can we plan the holiday here?" });
  await evaRow.getByText("Nice. Can we plan the holiday here?").hover();
  const pill = evaRow.locator('[data-slot="quick-reactions"]');
  await owner.waitForFunction((el) => el !== null && getComputedStyle(el).opacity === "1", await pill.elementHandle());
  const heart = pill.getByRole("button", { name: "❤️", exact: true });
  await film(owner, "quick-reactions", `${HOST}/chat/`, "the quick reactions under a message", "click", { heart });
  await shot(owner, "quick-reactions");
  await heart.click();
  await eva.getByRole("log").getByRole("button", { name: /^❤️\s*1$/ }).waitFor();
  if ((await heart.getAttribute("aria-pressed")) !== "true") throw new Error("the pill does not light a reaction of yours");
  await heart.click();
  await eva.getByRole("log").getByRole("button", { name: /^❤️\s*1$/ }).waitFor({ state: "detached" });
  await heart.click();
  await eva.getByRole("log").getByRole("button", { name: /^❤️\s*1$/ }).waitFor();
  await evaRow.getByText("Nice. Can we plan the holiday here?").hover();
  await owner.getByRole("button", { name: "Reply in a thread" }).first().click();
  await owner.getByRole("textbox", { name: "Reply in the thread" }).fill("Somewhere with a lake.");
  await owner.keyboard.press("Enter");
  await owner.getByLabel("Thread messages").getByText("Somewhere with a lake.").waitFor();
  await owner.getByLabel("Thread messages").getByText("Sending…").waitFor({ state: "detached" });
  await shot(owner, "thread");
  // The thread takes the room's place in the card; closing it brings the
  // room back.
  await owner.getByRole("button", { name: "Close threads" }).click();
  await owner.getByRole("log").getByText("Hi Eva, chat is up on the box.", { exact: true }).waitFor();
  await eva.getByRole("log").getByRole("button", { name: /1 reply/ }).waitFor();
  await eva.getByRole("button", { name: "Threads", exact: true }).click();
  await eva.getByLabel("Threads in this room").getByText("Somewhere with a lake.").waitFor();
  await shot(eva, "thread-list");
  await eva.getByRole("button", { name: "Close threads" }).click();
  await owner.getByRole("main").locator("input[type=file]").first().setInputFiles(PICTURE);
  // Eva's copy waits blurred, and nothing of it is fetched until she
  // presses it.
  const hidden = eva.getByRole("button", { name: /^Picture from Matus, hidden/ }).last();
  await hidden.waitFor();
  await eva.waitForTimeout(2000);
  if ((await eva.getByRole("log").locator("img[alt='salmon.png']").count()) !== 0) throw new Error("Matus's picture was fetched before Eva asked");
  await shot(eva, "picture-hidden");
  await hidden.click();
  await eva.getByRole("log").locator("img[src^='blob:']").last().waitFor();
  await owner.getByRole("log").getByText("Hi Eva, chat is up on the box.", { exact: true }).hover();
  await owner.getByRole("button", { name: "Edit", exact: true }).first().click();
  await owner.getByRole("textbox", { name: "Message Eva" }).fill("Hi Eva, chat is up on mattbox.");
  await owner.keyboard.press("Enter");
  await eva.getByRole("log").getByText("Hi Eva, chat is up on mattbox.").waitFor();
  await eva.getByRole("log").getByText("(edited)").waitFor();
  await eva.waitForTimeout(2000);
  await shot(eva, "picture-and-edit");

  // A second device reads the past with the recovery key.
  const second = await person("matus");
  await second.goto(`${ORIGIN}/chat/`);
  await second.getByRole("heading", { name: "Conversations" }).waitFor();
  await second.getByRole("button", { name: /Eva/ }).first().click();
  await second.getByRole("log").getByText(/no key for this message/).first().waitFor();
  await second.getByRole("button", { name: "Unlock this device to read encrypted messages" }).click();
  await second.getByRole("textbox", { name: "Recovery key" }).fill(key);
  await second.getByRole("button", { name: "Unlock", exact: true }).click();
  await second.getByRole("log").getByText("Hi Eva, chat is up on mattbox.").waitFor();
  await shot(second, "second-device");
  // A picture the handbook lacks is no reason to fail the test.
  if (Bun.env.LOSOS_HANDBOOK_SHOTS === "1") await handbook(key).catch((e) => console.error(`handbook: ${e}`));

  // Contacts, Mail and Cloud link a person as /chat/?with=<sign-in name>:
  // Eva's link to Matus opens the conversation they already have, not a
  // new one, and the link's parameter is gone once it has been used.
  const room = new URL(eva.url()).hash;
  if (!room.startsWith("#/room/")) throw new Error(`Eva is not in a room: ${eva.url()}`);
  await eva.goto(`${ORIGIN}/`);
  await eva.goto(`${ORIGIN}/chat/?with=matus`);
  await eva.getByRole("textbox", { name: "Message Matus" }).waitFor();
  await eva.getByRole("log").getByText("Hi Eva, chat is up on mattbox.").waitFor();
  const landed = new URL(eva.url());
  if (landed.hash !== room || landed.searchParams.has("with")) throw new Error(`?with= landed on ${eva.url()}, wanted ${room}`);

  // LosOS Cloud's share sheet links /chat/?share=<path>&text=…: Matus
  // picks the conversation, the file goes as an encrypted message and the
  // words wait in the composer.
  const PLAN = "Holiday plan.md";
  const planText = "# Holiday\n\n- Tatras in July\n- Book the cabin\n";
  const put = await owner.evaluate(
    async ([name, body]) => (await fetch(`/cloud/api/files/content?path=${encodeURIComponent(name)}`, { method: "PUT", body })).status,
    [PLAN, planText],
  );
  if (put < 200 || put >= 300) throw new Error(`could not put ${PLAN} into LosOS Cloud: ${put}`);
  await owner.goto(`${ORIGIN}/chat/?share=${encodeURIComponent(PLAN)}&text=${encodeURIComponent("The plan is in the file.")}`);
  const sendTo = owner.getByRole("dialog", { name: "Send 1 file" });
  await sendTo.getByRole("button", { name: /Eva/ }).first().waitFor();
  await film(owner, "share-from-cloud", `${HOST}/chat/`, "a file shared from LosOS Cloud: pick the conversation", "click", { eva: sendTo.getByRole("button", { name: /Eva/ }).first() });
  await shot(owner, "share-from-cloud");
  await sendTo.getByRole("button", { name: /Eva/ }).first().click();
  const draft = owner.getByRole("textbox", { name: "Message Eva" });
  await owner.waitForFunction(() => document.querySelector('textarea[aria-label="Message Eva"]')?.value === "The plan is in the file.");
  if (new URL(owner.url()).searchParams.has("share")) throw new Error(`the share link stayed: ${owner.url()}`);
  await draft.press("Enter");
  await eva.getByRole("log").getByText(PLAN).first().waitFor();
  await eva.getByRole("log").getByText("The plan is in the file.").waitFor();

  // Eva keeps the file: it lands in the Chat folder of her LosOS Cloud,
  // decrypted, and the card turns into a link that opens it there.
  await eva.getByRole("button", { name: `Save ${PLAN} to LosOS Cloud` }).last().click();
  await eva.getByRole("link", { name: `Open ${PLAN} from LosOS Cloud` }).last().waitFor();
  const kept = await eva.evaluate(async (name) => {
    const list = await (await fetch("/cloud/api/files?path=Chat")).json();
    const entry = list.entries.find((e) => e.name === name);
    return entry === undefined ? null : await (await fetch(`/cloud/api/files/content?path=${encodeURIComponent(entry.path)}`)).text();
  }, PLAN);
  if (kept !== planText) throw new Error(`Eva's LosOS Cloud has ${JSON.stringify(kept)}, wanted the plan`);
  await film(eva, "saved-to-cloud", `${HOST}/chat/`, "a file from chat kept in LosOS Cloud", "click", { open: eva.getByRole("link", { name: `Open ${PLAN} from LosOS Cloud` }).last() });
  await shot(eva, "saved-to-cloud");

  // And from the composer: Attach from LosOS Cloud.
  await owner.getByRole("button", { name: "Attach from LosOS Cloud" }).click();
  const picker = owner.locator("[data-slot=cloud-picker]");
  await picker.locator(`[data-entry="${PLAN}"]`).click();
  await film(owner, "attach-from-cloud", `${HOST}/chat/`, "attach from LosOS Cloud in the composer", "click", { send: picker.locator("[data-action=pick]") });
  await shot(owner, "attach-from-cloud");
  await picker.locator("[data-action=pick]").click();
  await eva.waitForFunction((name) => [...document.querySelectorAll('[role="log"] p')].filter((p) => p.textContent === name).length >= 2, PLAN);

  await safety(owner, eva);
}

/* Pictures and private details (backend/src/pictures.rs,
 * components/hidden-picture.tsx, components/details.tsx): the account's
 * section, an administrator marking Eva as a child, the nudity warning on
 * a picture received and one about to be sent, card and IBAN numbers drawn
 * black, the report dialog, and the adult mark that allows "No one". */
async function safety(owner, eva) {
  const api = (page, path, body) =>
    page.evaluate(
      async ([path, body]) => {
        const r = await fetch(path, body === undefined ? {} : { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify(body) });
        return { status: r.status, body: await r.json().catch(() => null) };
      },
      [path, body],
    );

  // "No one" needs the adult mark, and lososd itself says so.
  const early = await api(eva, "/api/pictures", { blur: "nobody" });
  if (early.status !== 403 || early.body?.code !== "adultRequired") throw new Error(`"No one" without the mark: ${JSON.stringify(early)}`);

  // Eva's account: every picture hidden, the warnings, no age mark.
  await eva.goto(`${ORIGIN}/settings/account`);
  const section = eva.getByTestId("account-pictures");
  await section.waitFor();
  await section.scrollIntoViewIfNeeded();
  if ((await eva.getByTestId("pictures-age").textContent())?.trim() === "") throw new Error("no age row");
  await shot(eva, "account-pictures");

  // Matus marks Eva as a child under Users.
  await owner.goto(`${ORIGIN}/settings/users`);
  const row = owner.locator('[data-testid=users-person][data-person="eva"]');
  await row.getByTestId("users-age-edit").click();
  await owner.getByTestId("users-age-choice").getByText("Child", { exact: true }).click();
  await shot(owner, "users-age");
  await owner.getByTestId("users-age-save").click();
  await row.getByTestId("users-age").getByText(/child/i).waitFor();
  await shot(owner, "users-age-child");
  const marked = await api(eva, "/api/pictures");
  if (marked.body?.child !== true || marked.body?.nudityCheck !== true || marked.body?.detailsCheck !== true) {
    throw new Error(`Eva marked as a child: ${JSON.stringify(marked.body)}`);
  }
  const quiet = await api(eva, "/api/pictures", { nudityCheck: false });
  if (quiet.status !== 403 || quiet.body?.code !== "childAccount") throw new Error(`a child turning the warning off: ${JSON.stringify(quiet)}`);
  await eva.reload();
  await section.waitFor();
  await section.scrollIntoViewIfNeeded();
  for (const name of ["Warn me about nudity", "Black out private details"]) {
    const toggle = section.getByRole("switch", { name });
    if (!(await toggle.isChecked()) || (await toggle.isEnabled())) throw new Error(`${name} is not held on for a child`);
  }
  await shot(eva, "account-pictures-child");

  // Matus sends a red square, a green one and a message with a card number
  // and an IBAN. His own page asks first about the numbers.
  await owner.goto(`${ORIGIN}/chat/`);
  await owner.getByRole("button", { name: /Eva/ }).first().click();
  const composer = owner.getByRole("textbox", { name: "Message Eva" });
  await composer.waitFor();
  const files = owner.getByRole("main").locator("input[type=file]").first();
  await files.setInputFiles(RED);
  await owner.getByRole("log").locator("img[alt='red.png']").waitFor();
  await files.setInputFiles(GREEN);
  await owner.getByRole("log").locator("img[alt='green.png']").waitFor();
  const PAY = "Pay the cabin to SK31 1200 0000 1987 4263 7541, or use my card 4242 4242 4242 4242.";
  await composer.fill(PAY);
  await owner.keyboard.press("Enter");
  const asked = owner.getByRole("dialog", { name: /private details/ });
  await asked.waitFor();
  const named = (await asked.getByTestId("details-found").innerText()).split("\n").map((s) => s.trim());
  if (JSON.stringify(named) !== JSON.stringify(["IBAN ending 7541", "Card number ending 4242"])) throw new Error(`the details named: ${JSON.stringify(named)}`);
  await shot(owner, "details-send-anyway");
  await asked.getByRole("button", { name: "Send anyway" }).click();
  await owner.getByRole("log").locator("[data-detail=iban]").last().waitFor();

  // Eva's page: the pictures wait blurred; she always shows Matus's.
  await eva.goto(`${ORIGIN}/chat/`);
  await eva.getByRole("button", { name: /Matus/ }).first().click();
  const log = eva.getByRole("log");
  await log.locator("[data-detail=card]").last().waitFor();
  await log.getByRole("button", { name: /^Picture from Matus, hidden/ }).last().waitFor();
  await shot(eva, "pictures-hidden-child");
  await log.getByRole("button", { name: "Always show pictures from Matus" }).last().click();
  // Red is held behind the warning, green is shown.
  await log.locator("[data-nudity=flagged]").filter({ has: eva.locator("img[alt='red.png']") }).waitFor();
  await log.locator("img[alt='green.png']").waitFor();
  await eva.waitForFunction(() => {
    const green = document.querySelector("[role=log] img[alt='green.png']");
    return green !== null && green.closest("[data-nudity]") === null;
  });
  // The numbers are not in the page at all until pressed: not in the log,
  // and not in the conversation list's last line either.
  const text = await eva.locator("body").innerText();
  if (text.includes("4242 4242") || text.includes("1987 4263")) throw new Error("a card or IBAN number is in the page unpressed");
  await log.locator("[data-detail=card]").last().scrollIntoViewIfNeeded();
  // The pointer off the log, so no message's quick reactions cover the bars.
  await eva.mouse.move(4, 4);
  await shot(eva, "nudity-and-details");
  await log.locator("[data-detail=iban]").last().click();
  await log.getByText("SK31 1200 0000 1987 4263 7541").waitFor();
  await eva.mouse.move(4, 4);
  await shot(eva, "details-shown");
  const trusted = await api(eva, "/api/pictures");
  if (!trusted.body?.trusted?.includes("@matus:mattbox.local")) throw new Error(`Eva's trusted senders: ${JSON.stringify(trusted.body)}`);

  // Eva tries to send the red square: her page asks, and she does not.
  const evaFiles = eva.getByRole("main").locator("input[type=file]").first();
  await evaFiles.setInputFiles(RED);
  const ask = eva.getByRole("dialog", { name: /may show nudity/ });
  await ask.waitFor();
  await shot(eva, "send-anyway");
  await ask.getByRole("button", { name: "Don't send" }).click();
  await eva.waitForTimeout(5000);
  if ((await owner.getByRole("log").locator("img[alt='red.png']").count()) !== 1) throw new Error("Eva's red square went to Matus");

  // Report: how to reach the police and hotlines, and the box's moderators.
  await log.getByText(/^Pay the cabin to/).last().click({ button: "right" });
  await eva.locator('[data-slot="context-menu"]').first().getByRole("menuitem", { name: "Report" }).click();
  const report = eva.getByTestId("report-dialog");
  await report.waitFor();
  await report.getByTestId("report-handbook").waitFor();
  await shot(eva, "report");
  await eva.getByTestId("report-send").click();
  await eva.getByText("The moderators have your report", { exact: true }).waitFor();
  await report.waitFor({ state: "hidden" });
  // Matus moderates: the report names the room and the event, never the text.
  const queue = await api(owner, "/api/moderation");
  const chatReport = (queue.body?.reports ?? []).find((r) => r.app === "chat" && r.state === "open");
  if (!chatReport || !/^![^/]+\/\$/.test(chatReport.ref)) throw new Error(`the chat report: ${JSON.stringify(queue.body?.reports)}`);
  if (JSON.stringify(chatReport).includes("Pay the cabin")) throw new Error("the report carries the message's text");

  // Matus marks Eva as an adult: "No one" is hers to choose.
  await owner.goto(`${ORIGIN}/settings/users`);
  await row.getByTestId("users-age-edit").click();
  await owner.getByTestId("users-age-choice").getByText("Adult", { exact: true }).click();
  await owner.getByTestId("users-age-save").click();
  await row.getByTestId("users-age").getByText(/adult/i).waitFor();
  await eva.goto(`${ORIGIN}/settings/account`);
  await section.waitFor();
  await section.scrollIntoViewIfNeeded();
  await eva.getByTestId("pictures-blur").getByText("No one", { exact: true }).click();
  await eva.waitForFunction(async () => (await (await fetch("/api/pictures")).json()).blur === "nobody");
  await shot(eva, "account-pictures-adult");
}

// Thrown at the very end, after the pictures of the failure are saved:
// an uncaught error is what makes Bun exit non-zero.
let failed = false;
try {
  if (phase === "off") await off();
  else if (phase === "on") await on();
  else throw new Error(`usage: chat-browser.mjs off|on <out>`);
  if (problems.length > 0) throw new Error(`the page broke its policy:\n${problems.join("\n")}`);
} catch (error) {
  let i = 0;
  for (const context of browser.contexts()) {
    for (const page of context.pages()) await page.screenshot({ path: `${out}/failed-${phase}-${i++}.png` }).catch(() => {});
  }
  console.error(problems.join("\n"));
  console.error(network.filter((l) => !/ 200 /.test(l)).slice(-60).join("\n"));
  console.error(network.slice(-60).join("\n"));
  console.error(error);
  failed = true;
} finally {
  await Bun.write(`${out}/network-${phase}.log`, network.join("\n") + "\n");
  await Bun.write(stepsFile, JSON.stringify(steps, null, 2));
  await Bun.write(
    `${out}/film/manifest.json`,
    JSON.stringify({ feature: "chat", viewport: { width: 1280, height: 900 }, dpr: 2, test: "losos-chat", steps }, null, 2),
  );
  await browser.close();
}
if (failed) throw new Error(`the ${phase} phase failed, see above`);
