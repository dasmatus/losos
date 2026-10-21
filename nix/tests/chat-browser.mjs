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
  // What the network did, printed when a step does not happen.
  page.on("response", (r) => {
    if (r.status() >= 400) network.push(`${name}: ${r.status()} ${r.request().method()} ${r.url()}`);
  });
  page.on("requestfailed", (r) => network.push(`${name}: failed ${r.method()} ${r.url()} ${r.failure()?.errorText}`));
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
  await eva.getByRole("log").getByText("Hi Eva, chat is up on the box.", { exact: true }).hover();
  await eva.getByRole("button", { name: "React", exact: true }).click();
  await eva.getByRole("menuitem", { name: "👍" }).click();
  await owner.getByRole("log").getByRole("button", { name: /👍/ }).waitFor();
  await owner.getByRole("log").getByText("Nice. Can we plan the holiday here?").hover();
  await owner.getByRole("button", { name: "Reply in a thread" }).first().click();
  await owner.getByRole("textbox", { name: "Reply in the thread" }).fill("Somewhere with a lake.");
  await owner.keyboard.press("Enter");
  await owner.getByLabel("Thread messages").getByText("Somewhere with a lake.").waitFor();
  await owner.getByLabel("Thread messages").getByText("Sending…").waitFor({ state: "detached" });
  await shot(owner, "thread");
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
  console.error(network.slice(-80).join("\n"));
  console.error(error);
  process.exitCode = 1;
} finally {
  writeFileSync(stepsFile, JSON.stringify(steps, null, 2));
  writeFileSync(
    `${out}/film/manifest.json`,
    JSON.stringify({ feature: "chat", viewport: { width: 1280, height: 900 }, dpr: 2, test: "losos-chat", steps }, null, 2),
  );
  await browser.close();
}
