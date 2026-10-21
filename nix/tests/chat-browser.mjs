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
import { createRequire } from "node:module";
import { mkdirSync, readFileSync, writeFileSync, existsSync } from "node:fs";

const { chromium } = createRequire(import.meta.url)(process.env.CHAT_PLAYWRIGHT ?? "playwright");

const [phase, out] = process.argv.slice(2);
const HOST = process.env.CHAT_HOST ?? "mattbox.local";
const ORIGIN = `https://${HOST}`;
const PROXY = process.env.CHAT_PROXY ?? "socks5://127.0.0.1:1080";
const PICTURE = process.env.CHAT_PICTURE;
const PEOPLE = {
  matus: process.env.CHAT_MATUS_PASSWORD,
  eva: process.env.CHAT_EVA_PASSWORD,
};
mkdirSync(`${out}/film`, { recursive: true });
mkdirSync(`${out}/pr`, { recursive: true });
const stepsFile = `${out}/film/steps.json`;
const steps = existsSync(stepsFile) ? JSON.parse(readFileSync(stepsFile, "utf8")) : [];

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
  steps.push({ file, url, title, action, targets: boxes });
}

async function shot(page, name) {
  await settle(page);
  await page.screenshot({ path: `${out}/pr/${name}.png` });
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
  await owner.getByRole("log").getByRole("button", { name: /👍/ }).waitFor();
  await owner.getByRole("log").getByText("Nice. Can we plan the holiday here?").hover();
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
}

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
  process.exitCode = 1;
} finally {
  writeFileSync(`${out}/network-${phase}.log`, network.join("\n") + "\n");
  writeFileSync(stepsFile, JSON.stringify(steps, null, 2));
  writeFileSync(
    `${out}/film/manifest.json`,
    JSON.stringify({ feature: "chat", viewport: { width: 1280, height: 900 }, dpr: 2, test: "losos-chat", steps }, null, 2),
  );
  await browser.close();
}
