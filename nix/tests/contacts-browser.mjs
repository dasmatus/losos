// The browser half of nix/tests/contacts.nix: LosOS Contacts in Chromium,
// and the pictures of it.
//
//   node contacts-browser.mjs before <out>  the admin pages before Contacts
//   node contacts-browser.mjs off    <out>  the admin pages with sharing off
//   node contacts-browser.mjs on     <out>  the book and the apps that use it
//
// As in nix/tests/chat-browser.mjs the box is reached through a SOCKS proxy
// on the LAN, so pages load at https://mattbox.local/. Pictures go to
// <out>/pr/. It exits non-zero at the first step that does not happen,
// after a picture of every open page.
import { createRequire } from "node:module";
import { mkdirSync, readFileSync } from "node:fs";

const { chromium } = createRequire(import.meta.url)(process.env.CONTACTS_PLAYWRIGHT ?? "playwright");

const [phase, out] = process.argv.slice(2);
const HOST = process.env.CONTACTS_HOST ?? "mattbox.local";
const ORIGIN = `https://${HOST}`;
const PROXY = process.env.CONTACTS_PROXY ?? "socks5://127.0.0.1:1080";
const OLD_CARD = process.env.CONTACTS_OLD_CARD;
mkdirSync(`${out}/pr`, { recursive: true });

const browser = await chromium.launch({ chromiumSandbox: false, proxy: { server: PROXY } });
const problems = [];
const pages = [];

async function person(name, password) {
  const context = await browser.newContext({
    viewport: { width: 1280, height: 860 },
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
  const owner = await person("matus", process.env.CONTACTS_MATUS_PASSWORD);
  await owner.goto(`${ORIGIN}/`);
  await owner.getByRole("link", { name: "Chat", exact: true }).waitFor();
  await shot(owner, "sidebar-before");
  await mailTo(owner, "mail-to-before");
  await chatPicker(owner, "chat-picker-before");
}

async function off() {
  const owner = await person("matus", process.env.CONTACTS_MATUS_PASSWORD);
  await owner.goto(`${ORIGIN}/`);
  await owner.getByRole("link", { name: "Overview", exact: true }).waitFor();
  await owner.waitForTimeout(3000);
  if ((await contactsLink(owner).count()) !== 0) throw new Error("Contacts is in the sidebar with sharing off");
  await shot(owner, "sidebar-sharing-off");
}

async function on() {
  const owner = await person("matus", process.env.CONTACTS_MATUS_PASSWORD);
  await owner.goto(`${ORIGIN}/`);
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
  await importer.locator('input[type="file"]').setInputFiles({
    name: "phone.vcf",
    mimeType: "text/vcard",
    buffer: readFileSync(OLD_CARD),
  });
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

  // The contacts room is not a conversation.
  if ((await owner.getByRole("button", { name: /^Contacts/ }).count()) !== 0) {
    throw new Error("the contacts room is in the room list");
  }
}

try {
  await { before, off, on }[phase]();
  if (problems.length > 0) throw new Error(`the page reported: ${problems.join("; ")}`);
} catch (error) {
  for (const [i, page] of pages.entries()) {
    await page.screenshot({ path: `${out}/pr/failed-${phase}-${i}.png` }).catch(() => {});
  }
  console.error(error);
  process.exitCode = 1;
} finally {
  await browser.close();
}
