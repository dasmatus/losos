// The box's frame on a real box, from a real browser: nix/tests/cloud-shell.nix
// runs this in its laptop VM against the box's own nginx, LosOS Login,
// lososd and LosOS Cloud.
//
//   node cloud-shell.mjs BASE NAME PASSWORD OUT [--ca FILE] [--dav-user U --dav-password P]
//
// It signs in through LosOS Login and, on each address in PAGES (admin pages,
// LosOS Cloud's places, a document open in LosOS Office), measures the top
// bar, the logo and the sidebar twice: as soon as the sidebar is drawn and
// again once the page has settled. On each it presses Ctrl+Shift+P and types
// a colon, and inside the document it presses Ctrl+Shift+P with the caret in
// the text. In LosOS Cloud it uploads a file through the Upload button's file
// chooser, and has curl put a file in over WebDAV while the page watches,
// without a reload. Everything it saw goes to OUT/report.json, with a picture
// of each page as NN-slug.png; the test script asserts on the report, so the
// same run takes the "before" pictures on a box without the fix.
//
// The window is 1280 x 900, light, en-US, reduced motion.

import { execFileSync } from "node:child_process";
import { createHash, X509Certificate } from "node:crypto";
import { mkdirSync, writeFileSync } from "node:fs";
import { readFileSync } from "node:fs";
import { join } from "node:path";

const { chromium } = await import(process.env.PLAYWRIGHT_CORE ?? "playwright-core");

const VIEW = { width: 1280, height: 900 };

const [base, name, password, out, ...rest] = process.argv.slice(2);
const opt = (key, fallback) => {
  const at = rest.indexOf(`--${key}`);
  return at < 0 ? fallback : rest[at + 1];
};
const ca = opt("ca", "/etc/losos-box.pem");
const davUser = opt("dav-user", name);
const davPassword = opt("dav-password", password);
mkdirSync(out, { recursive: true });

function spkiPin(pemFile) {
  const cert = new X509Certificate(readFileSync(pemFile));
  const der = cert.publicKey.export({ type: "spki", format: "der" });
  return createHash("sha256").update(der).digest("base64");
}

const browser = await chromium.launch({
  args: [`--ignore-certificate-errors-spki-list=${spkiPin(ca)}`],
});
const context = await browser.newContext({
  viewport: VIEW,
  colorScheme: "light",
  locale: "en-US",
  reducedMotion: "reduce",
});
let page = await context.newPage();
function prepare() {
  page.setDefaultTimeout(300_000);
  page.setDefaultNavigationTimeout(300_000);
  page.on("pageerror", (e) => report.errors.push(String(e)));
}

const report = { pages: [], palette: [], upload: null, live: null, errors: [] };
prepare();

/** A page again if the last one went away (it is in the report). */
async function reopen() {
  if (!page.isClosed()) return;
  report.errors.push("the page closed");
  page = await context.newPage();
  prepare();
}

// LosOS Login, as its page does it, from inside its page.
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
if (status !== 200) throw new Error(`sign-in refused: ${status}`);
// The first-run tour stays shut, so it is not in every picture.
await context.addInitScript(() => window.localStorage.setItem("losos-tour", "seen"));

const nav = () => page.getByRole("navigation", { name: "Sections", exact: true });
const box = async (loc) => {
  const b = await loc.boundingBox().catch(() => null);
  return b === null ? null : { x: Math.round(b.x), y: Math.round(b.y), w: Math.round(b.width), h: Math.round(b.height) };
};

/** Where the frame is: the bar's row, the logo, the sidebar and its links. */
async function frame() {
  const bar = page.locator("[data-slot=app-bar] > div").first();
  const logo = page.locator("[data-slot=app-bar] img").first();
  const side = nav();
  const visible = (await side.count()) > 0 && (await side.isVisible());
  return {
    bar: await box(bar),
    logo: await box(logo),
    sidebar: visible ? await box(side) : null,
    // The sections, not what is open under the current one.
    links: visible ? (await side.locator("[data-slot=sidebar-menu-button]").allInnerTexts()).map((l) => l.trim()).filter(Boolean) : [],
    // What the bar holds besides the logo and the menus: an app's name,
    // a search field.
    barText: ((await page.locator("[data-slot=app-bar]").first().innerText().catch(() => "")) || "").replace(/\s+/g, " ").trim(),
    barSearch: await page.locator("[data-slot=app-bar] input").count(),
  };
}

async function settle() {
  await page.waitForLoadState("networkidle").catch(() => undefined);
  await page.waitForFunction(
    () => ![...document.querySelectorAll("[data-slot=spinner]")].some((el) => el.getBoundingClientRect().width > 0),
    undefined,
    { timeout: 120_000 },
  ).catch(() => undefined);
  await page.waitForTimeout(1500);
}

const palette = () => page.getByRole("combobox", { name: /^Search pages/ });

/** Whether Ctrl+Shift+P and a colon open the palette here. */
async function tryPalette(where, focus) {
  const result = { where, chord: false, colon: false };
  if (focus !== undefined) await focus();
  else await page.mouse.click(VIEW.width - 20, VIEW.height - 20);
  await page.keyboard.press("Control+Shift+P");
  result.chord = await palette().waitFor({ timeout: 10_000 }).then(() => true, () => false);
  if (result.chord) {
    await page.keyboard.press("Escape");
    await palette().waitFor({ state: "detached", timeout: 10_000 }).catch(() => undefined);
  }
  if (focus === undefined) {
    await page.mouse.click(VIEW.width - 20, VIEW.height - 20);
    await page.keyboard.type(":");
    result.colon = await palette().waitFor({ timeout: 10_000 }).then(() => true, () => false);
    if (result.colon) {
      await page.keyboard.press("Escape");
      await palette().waitFor({ state: "detached", timeout: 10_000 }).catch(() => undefined);
    }
  }
  report.palette.push(result);
  console.log("palette", JSON.stringify(result));
}

let shot = 0;
async function visit(slug, path, { waitFor, palette: keys = true } = {}) {
  await page.goto(`${base}${path}`);
  // As soon as there is a sidebar: what a person sees first.
  await nav().waitFor({ timeout: 120_000 }).catch(() => undefined);
  const first = await frame();
  if (waitFor !== undefined) await page.locator(waitFor).first().waitFor({ timeout: 300_000 });
  await settle();
  const settled = await frame();
  const file = `${String(++shot).padStart(2, "0")}-${slug}.png`;
  await page.screenshot({ path: join(out, file) });
  report.pages.push({ slug, path, file, first, settled });
  console.log(slug, JSON.stringify(settled));
  if (keys) await tryPalette(slug);
}

// ── The admin pages and LosOS Cloud ─────────────────────────────────────────

await visit("overview", "/");
await visit("storage", "/storage");
await visit("about", "/settings/about");
await visit("cloud-files", "/cloud/files", { waitFor: "[data-entry]" });
await visit("cloud-recent", "/cloud/recent");
await visit("cloud-notes", "/cloud/notes");
await visit("cloud-photos", "/cloud/photos");

// ── A document in LosOS Office ──────────────────────────────────────────────

try {
  await page.goto(`${base}/cloud/files`);
  await page.locator("[data-new=doc]").first().click();
  await page.waitForURL(/\/cloud\/office\/\d+/);
  const docPath = new URL(page.url()).pathname;
  await visit("office-document", docPath, { waitFor: "[data-markdown-office] [contenteditable=true]", palette: false });
  await tryPalette("office-document-text", async () => {
    await page.locator("[data-markdown-office] [contenteditable=true]").first().click();
  });
  await tryPalette("office-document");
} catch (e) {
  report.errors.push(`office: ${e}`);
}

// ── Upload through the file chooser ─────────────────────────────────────────

{
  await reopen();
  await page.goto(`${base}/cloud/files`);
  await page.locator("[data-entry]").first().waitFor();
  // Larger than nginx's 10 MB default, which the sign-in check once held
  // every upload to.
  const bytes = Buffer.alloc(24_000_000, 7);
  const chooser = page.waitForEvent("filechooser");
  await page.getByRole("button", { name: "Upload", exact: true }).first().click();
  const picked = await chooser;
  await picked.setFiles({ name: "picked-in-browser.bin", mimeType: "application/octet-stream", buffer: bytes });
  const listed = await page
    .locator('[data-entry="picked-in-browser.bin"]')
    .waitFor({ timeout: 300_000 })
    .then(() => true, () => false);
  const panel = (await page.locator("[data-uploads]").innerText().catch(() => "")).replace(/\s+/g, " ").trim();
  report.upload = { listed, panel };
  await page.screenshot({ path: join(out, `${String(++shot).padStart(2, "0")}-upload.png`) });
  console.log("upload", JSON.stringify(report.upload));
}

// ── A file put in over WebDAV appears without a reload ──────────────────────

{
  await reopen();
  await page.goto(`${base}/cloud/files`);
  await page.locator("[data-entry]").first().waitFor();
  await page.evaluate(() => {
    window.__sameDocument = true;
  });
  const dav = `${base}/remote.php/webdav/from-webdav.txt`;
  execFileSync("curl", ["-sf", "--cacert", ca, "-u", `${davUser}:${davPassword}`, "-T", "-", dav], {
    input: "copied in by a file manager\n",
  });
  const listed = await page
    .locator('[data-entry="from-webdav.txt"]')
    .waitFor({ timeout: 60_000 })
    .then(() => true, () => false);
  const sameDocument = await page.evaluate(() => window.__sameDocument === true);
  report.live = { listed, sameDocument };
  await page.screenshot({ path: join(out, `${String(++shot).padStart(2, "0")}-webdav-live.png`) });
  console.log("live", JSON.stringify(report.live));
}

writeFileSync(join(out, "report.json"), JSON.stringify(report, null, 2) + "\n");
await browser.close();
