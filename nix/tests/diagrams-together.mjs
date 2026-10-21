// Two people on two boxes edit one diagram: Matus on mattbox shares a
// .canvas file with Eva on kitchenbox, she opens it from Shared with you,
// and each sees the other's cards, pointer and selection as they happen.
//
// nix/tests/diagrams-together.nix runs it in its laptop VM:
//
//   node diagrams-together.mjs CONFIG OUT
//
// CONFIG is a JSON file:
//
//   { "path": "Diagrams/launch.canvas", "fileId": 3,
//     "owner": { "base": "https://mattbox.local", "name": "matus", "password": "…" },
//     "guest": { "base": "https://kitchenbox.local", "name": "eva", "password": "…" },
//     "certs": ["/etc/losos-mattbox.pem", "/etc/losos-kitchenbox.pem"] }
//
// The browser trusts each box's certificate by its public key, WebSockets
// included; Node's own requests trust NODE_EXTRA_CA_CERTS.
//
// OUT collects one PNG per window, 1440 x 900, light, en-US, as the other
// Diagrams pictures are. A page error or a refusal by the page's security
// policy fails the run.

import { createHash, X509Certificate } from "node:crypto";
import { mkdirSync, readFileSync } from "node:fs";
import { join } from "node:path";

const VIEW = { width: 1440, height: 900 };
const WAIT = 180_000;

const { chromium } = await import(process.env.PLAYWRIGHT_CORE ?? "playwright-core");
const [cfgFile, out] = process.argv.slice(2);
const cfg = JSON.parse(readFileSync(cfgFile, "utf8"));
mkdirSync(out, { recursive: true });

const problems = [];

function spkiPin(pemFile) {
  const cert = new X509Certificate(readFileSync(pemFile));
  const der = cert.publicKey.export({ type: "spki", format: "der" });
  return createHash("sha256").update(der).digest("base64");
}

async function window(browser, who, box) {
  const context = await browser.newContext({
    viewport: VIEW,
    colorScheme: "light",
    locale: "en-US",
    timezoneId: "Europe/Bratislava",
    reducedMotion: "reduce",
  });
  context.setDefaultNavigationTimeout(WAIT);
  context.setDefaultTimeout(WAIT);
  await context.addInitScript(() => {
    document.addEventListener("securitypolicyviolation", (e) => {
      console.error(`policy refused ${e.violatedDirective}: ${e.blockedURI || "inline"}`);
    });
  });
  const page = await context.newPage();
  page.on("pageerror", (e) => problems.push(`${box}: ${e}`));
  page.on("console", (m) => {
    if (m.type() === "error" && m.text().startsWith("policy refused")) problems.push(`${box}: ${m.text()}`);
  });
  const r = await page.request.post(`${who.base}/auth/api/firstfactor`, {
    data: { username: who.name, password: who.password, keepMeLoggedIn: false, targetURL: `${who.base}/diagrams/` },
  });
  if (!r.ok()) throw new Error(`${box}: sign-in refused: ${r.status()} ${await r.text()}`);
  return { page, box, base: who.base, who };
}

const card = (p, label) => p.locator(`[data-testid=diagram] [data-node][aria-label="${label}"]`);
const faces = (p) => p.locator("[data-testid=live-people] [data-face]");

async function ready(p) {
  await p.locator("[data-testid=lab-splash]").waitFor({ state: "detached" });
  await card(p, "Back up the old NAS").waitFor();
}

/** A point on the canvas, from its top-left corner. */
async function at(p, x, y) {
  const box = await p.getByTestId("diagram-wrap").boundingBox();
  return { x: box.x + x, y: box.y + y };
}

async function shot(w, name) {
  await w.page.waitForLoadState("networkidle").catch(() => {});
  await w.page.evaluate(() => document.fonts.ready.then(() => true));
  // No toast over the picture.
  await w.page.waitForFunction(() => !document.querySelector("[data-sonner-toast]"), undefined, { timeout: WAIT });
  await w.page.waitForTimeout(400);
  await w.page.screenshot({ path: join(out, `${name}.png`), timeout: WAIT });
  console.log(`captured ${name}.png`);
}

const browser = await chromium.launch({
  args: ["--disable-gpu", `--ignore-certificate-errors-spki-list=${cfg.certs.map(spkiPin).join(",")}`],
});
try {
  const m = await window(browser, cfg.owner, "mattbox");
  const e = await window(browser, cfg.guest, "kitchenbox");
  const guest = `${cfg.guest.name}@${new URL(cfg.guest.base).hostname}`;

  // Matus shares the diagram with Eva, who can edit it.
  const shared = await m.page.request.post(`${m.base}/cloud/api/mesh/shares`, {
    data: { fileId: cfg.fileId, to: guest, perm: "edit" },
  });
  if (!shared.ok()) throw new Error(`sharing: ${shared.status()} ${await shared.text()}`);

  // Eva takes it from Shared with you; a diagram opens in Diagrams.
  await e.page.goto(`${e.base}/cloud/shared`);
  const name = cfg.path.slice(cfg.path.lastIndexOf("/") + 1);
  const offer = e.page.locator("[data-incoming-share]").filter({ hasText: name });
  await offer.locator("[data-accept]").click();
  await offer.locator("[data-open]").click();
  await e.page.waitForURL(/\/diagrams\/\?share=/);
  await ready(e.page);

  await m.page.goto(`${m.base}/diagrams/?canvas=svg&file=${encodeURIComponent(cfg.path)}`);
  await ready(m.page);
  for (const w of [m, e]) await faces(w.page).nth(1).waitFor();

  // Eva writes a card; Matus sees it come.
  await e.page.locator("[data-testid=tray-items] [data-card=text]").click();
  let p = await at(e.page, 560, 600);
  await e.page.mouse.click(p.x, p.y);
  await e.page.keyboard.type("The van is booked for Friday");
  await e.page.keyboard.press("Escape");
  await card(m.page, "The van is booked for Friday").waitFor();

  // Matus picks the decision; Eva points at the database.
  await card(m.page, "Everything copied?").click();
  p = await at(m.page, 300, 470);
  await m.page.mouse.move(p.x, p.y);
  await card(e.page, "LosOS Cloud").click();
  p = await at(e.page, 760, 360);
  await e.page.mouse.move(p.x, p.y, { steps: 4 });
  await m.page.locator(`[data-peer="${guest}"]`).getByText("Eva").waitFor();
  await m.page.locator(`[data-peer="${guest}"] .dg-peer-sel`).waitFor();
  await e.page.locator('[data-peer^="matus@"] .dg-peer-sel').waitFor();

  await shot(m, "together-matus");
  await shot(e, "together-eva");

  if (problems.length > 0) throw new Error(problems.join("\n"));
} finally {
  await browser.close();
}
