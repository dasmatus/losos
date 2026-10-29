// Pictures of Notes and a knowledge base on a real box, from a real browser.
//
// Chromium (Playwright) signs in through LosOS Login as the owner, opens the
// kitchen garden vault in LosOS Cloud's Notes, reads, maps and graphs it,
// types into a note, publishes the folder as a knowledge base and reads that
// site signed out. One full-window PNG per state, with a manifest that says
// where each picture was taken and where the things on it are, as
// nix/tests/cloud-shots.mjs does. nix/tests/notes.nix runs it in its laptop
// VM against the box's own nginx; the same pictures serve the pull request
// and the launch film.
//
//   node notes-shots.mjs BASE NAME PASSWORD OUT [--before] [--commit SHA]
//                        [--test NAME] [--ca FILE]
//
// --before takes the same places on the box with knowledge bases off: a
// note as the editor showed it before Notes, the folder's menu and the
// site's address.

import { createHash, X509Certificate } from "node:crypto";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const VIEW = { width: 1280, height: 900 };
const VAULT = "Kitchen garden";

const { chromium } = await import(process.env.PLAYWRIGHT_CORE ?? "playwright-core");

function spkiPin(pemFile) {
  const cert = new X509Certificate(readFileSync(pemFile));
  const der = cert.publicKey.export({ type: "spki", format: "der" });
  return createHash("sha256").update(der).digest("base64");
}

class Shots {
  constructor(base, out, meta) {
    this.host = new URL(base).hostname;
    this.out = out;
    this.meta = meta;
    this.steps = [];
    mkdirSync(out, { recursive: true });
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
        if ([...document.querySelectorAll("[data-slot=spinner], [role=status] [data-slot=spinner]")].some(seen)) return false;
        if (document.querySelector("[data-sonner-toast]")) return false;
        // An embed still loading says so in its own words.
        if ([...document.querySelectorAll(".note-embed")].some((e) => !e.hasAttribute("data-embed-of") && !e.classList.contains("embed-missing"))) return false;
        return true;
      },
      undefined,
      { timeout: 120_000 },
    );
    await p.evaluate(() => {
      document.documentElement.style.caretColor = "transparent";
    });
    await p.waitForTimeout(400);
  }

  async take(p, slug, title, action = { kind: "none" }, targets = {}) {
    await this.settle(p);
    const file = `${String(this.steps.length + 1).padStart(2, "0")}-${slug}.png`;
    // A frame at 2x on an emulated machine can take minutes, a dialog above
    // the page most of all.
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

  save() {
    const manifest = { feature: "notes", viewport: VIEW, dpr: 2, ...this.meta, steps: this.steps };
    writeFileSync(join(this.out, "manifest.json"), JSON.stringify(manifest, null, 2) + "\n");
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

const vaultUrl = (base, note, extra = "") =>
  `${base}/cloud/notes?${new URLSearchParams({ vault: VAULT, ...(note ? { note } : {}) }).toString()}${extra}`;

async function waitNote(p, path) {
  await p.waitForSelector(`[data-note="${path}"] .note-body h1`);
}

async function after(s, page, anon, base) {
  const p = page;
  // Notes on a box with no notes yet: the page makes the Notes folder and
  // offers the first note, rather than loading for ever.
  await p.goto(`${base}/cloud/notes`);
  await p.waitForSelector("[data-notes-empty] [data-new-note]", { timeout: 120_000 });
  await s.take(p, "first-note", "Notes on a box with no notes yet", { kind: "none" }, {
    empty: p.locator("[data-notes-empty]"),
  });

  await p.goto(`${base}/cloud/files`);
  const folder = p.locator(`[data-entry="${VAULT}"]`);
  await folder.waitFor();
  await s.take(p, "files", "the owner's files, with a folder of notes", { kind: "click", target: "notes" }, {
    folder,
    notes: p.getByRole("link", { name: "Notes", exact: true }),
  });

  await folder.click({ button: "right" });
  await p.waitForSelector('[data-slot="context-menu"]');
  const open = p.locator('[data-slot="context-menu"] [data-item="folder-notes-open"]');
  await s.take(p, "folder-menu", "a right click on the folder", { kind: "click", target: "open" }, {
    open,
    publish: p.locator('[data-slot="context-menu"] [data-item="folder-kb-publish"]'),
  });
  await open.click();
  await waitNote(p, "Home.md");
  const tomatoes = p.locator('.note-body a.wikilink[data-note-path="Tomatoes.md"]').first();
  await s.take(p, "home", "the vault's first note, rendered", { kind: "click", target: "tomatoes" }, {
    tomatoes,
    tree: p.locator("[data-vault-tree]"),
    side: p.locator("[data-note-side]"),
    callout: p.locator(".callout").first(),
  });

  await tomatoes.click();
  await waitNote(p, "Tomatoes.md");
  const map = p.locator('[data-view="map"]');
  await s.take(p, "note", "a note with an embedded section, a folded callout and its backlinks", { kind: "click", target: "map" }, {
    map,
    embed: p.locator(".note-embed").first(),
    backlinks: p.locator('[data-side="backlinks"]'),
    graph: p.locator('[data-side="graph"]'),
  });

  await map.click();
  await p.waitForSelector("[data-mindmap] svg");
  const graphButton = p.locator("[data-graph-button]");
  await s.take(p, "mind-map", "the note as a mind map", { kind: "click", target: "graph" }, {
    mindmap: p.locator("[data-mindmap]"),
    graph: graphButton,
  });

  await graphButton.click();
  await p.waitForSelector("[data-vault-graph] [data-graph] canvas");
  const tags = p.locator('[data-graph-option="tags"]');
  await s.take(p, "graph", "every note and link in the vault", { kind: "click", target: "tags" }, {
    graph: p.locator("[data-vault-graph] [data-graph]"),
    tags,
  });
  await tags.click();
  await s.take(p, "graph-tags", "the graph with tags", { kind: "none" }, { graph: p.locator("[data-vault-graph] [data-graph]") });

  // Typing into a note: a link, completed from the vault's names.
  await p.goto(vaultUrl(base, "Planting calendar.md", "&view=edit"));
  await p.waitForSelector("[data-note-editor] [data-testid=collab-editor][data-ready]");
  await p.waitForSelector('[data-note-editor] [data-connection="live"]');
  await p.waitForSelector("[data-note-preview] .note-body h1");
  const editor = p.locator("[data-testid=collab-editor] .cm-content");
  await s.take(p, "edit", "the note's text beside the note", { kind: "click", target: "editor" }, {
    editor: p.locator("[data-testid=collab-editor]"),
    preview: p.locator("[data-note-preview]"),
  });
  await editor.click();
  await p.keyboard.press("Control+End");
  await p.keyboard.press("Enter");
  await p.keyboard.press("Enter");
  let typed = "";
  for (const ch of "Feed with [[Comp") {
    typed += ch;
    await p.keyboard.type(ch);
    await s.take(p, `type-${typed.length}`, `typing “${typed}”`, { kind: "type", target: "editor" }, {
      editor: p.locator("[data-testid=collab-editor]"),
    });
  }
  await p.waitForSelector(".cm-tooltip-autocomplete");
  await p.keyboard.press("Enter");
  await p.waitForFunction(() =>
    [...document.querySelectorAll("[data-note-preview] a.wikilink")].some((a) => a.getAttribute("data-note-path") === "Compost.md" && a.closest("p")?.textContent?.startsWith("Feed with")),
  );
  await s.take(p, "linked", "the link written, and drawn beside it", { kind: "click", target: "read" }, {
    read: p.locator('[data-view="read"]'),
    preview: p.locator("[data-note-preview]"),
  });

  // Search inside the notes, a word at a time.
  await p.goto(vaultUrl(base, "Home.md"));
  await waitNote(p, "Home.md");
  const field = p.locator("[data-notes-search]");
  await field.click();
  typed = "";
  for (const ch of "ladybirds") {
    typed += ch;
    await field.press(ch);
    if (typed.length >= 3) await p.waitForSelector("[data-search-hits]");
    await s.take(p, `search-${typed}`, `searching for “${typed}”`, { kind: "type", target: "search" }, { search: field });
  }
  const hit = p.locator("[data-search-hits] a").first();
  await hit.click();
  await waitNote(p, "Pests/Aphids.md");
  await s.take(p, "found", "the note the search found", { kind: "none" });

  // A .note: MDX components beside Obsidian's syntax.
  await p.goto(vaultUrl(base, "Harvest.note"));
  await waitNote(p, "Harvest.note");
  await s.take(p, "note-mdx", "a .note, its callout, tabs and details drawn by losos-mdx", { kind: "none" }, {
    callout: p.locator("aside.callout").first(),
    tabs: p.locator(".mdx-tabs"),
    details: p.locator("details.mdx-details"),
  });

  // A .md note turned into a .note.
  await p.goto(vaultUrl(base, "Compost.md"));
  await waitNote(p, "Compost.md");
  const convert = p.locator("[data-convert-note]");
  await s.take(p, "convert", "a .md note, with Convert to .note", { kind: "click", target: "convert" }, { convert });
  await convert.click();
  await p.waitForFunction(() => new URLSearchParams(location.search).get("note") === "Compost.note");
  await waitNote(p, "Compost.note");
  await s.take(p, "converted", "the same note as a .note", { kind: "none" });

  // Publish the folder.
  await p.goto(`${base}/cloud/files`);
  await folder.waitFor();
  await folder.click({ button: "right" });
  const publish = p.locator('[data-slot="context-menu"] [data-item="folder-kb-publish"]');
  await publish.waitFor();
  await s.take(p, "publish-menu", "publishing the folder", { kind: "click", target: "publish" }, { publish });
  await publish.click();
  const dialog = p.locator("[data-publish-dialog]");
  await dialog.locator("[data-publish]").waitFor();
  await s.take(p, "publish-dialog", "who may read it", { kind: "click", target: "submit" }, {
    submit: dialog.locator("[data-publish]"),
    who: dialog.locator('[data-audience="anyone"]'),
  });
  await dialog.locator("[data-publish]").click();
  await dialog.locator("[data-site-url]").waitFor();
  await p.waitForFunction(() => !document.querySelector("[data-sonner-toast]"), undefined, { timeout: 30_000 }).catch(() => {});
  await s.take(p, "published", "the site's address", { kind: "none" }, { url: dialog.locator("[data-site-url]") });
  const address = (await dialog.locator("[data-site-url] a").textContent())?.trim() ?? "";
  if (!address.endsWith("/kb/kitchen-garden/")) throw new Error(`unexpected site address ${address}`);

  // The site, signed out.
  await anon.goto(address);
  await waitNote(anon, "Home.md");
  const kbTomatoes = anon.locator('.note-body a.wikilink[data-note-path="Tomatoes.md"]').first();
  await s.take(anon, "kb-home", "the knowledge base, read without signing in", { kind: "click", target: "tomatoes" }, {
    tomatoes: kbTomatoes,
    contents: anon.locator("[data-kb-contents]"),
  });
  if ((await anon.textContent("[data-vault-tree]"))?.includes("Private diary")) throw new Error("a private note is on the site");
  await kbTomatoes.click();
  await waitNote(anon, "Tomatoes.md");
  await s.take(anon, "kb-note", "a page of the knowledge base", { kind: "click", target: "graph" }, {
    graph: anon.locator('[data-view="graph"]'),
  });
  await anon.locator('[data-view="graph"]').click();
  await anon.waitForSelector("[data-vault-graph] [data-graph] canvas");
  await s.take(anon, "kb-graph", "the knowledge base's graph", { kind: "none" });
}

async function before(s, page, anon, base) {
  const p = page;
  // A note as it opened before Notes: its text in the editor.
  const ids = await p.evaluate(async (vault) => {
    const r = await fetch(`/cloud/api/files?path=${encodeURIComponent(vault)}`);
    const j = await r.json();
    return Object.fromEntries(j.entries.map((e) => [e.name, e.id]));
  }, VAULT);
  for (const [name, slug, title] of [
    ["Home.md", "home", "a note as plain text in the editor"],
    ["Tomatoes.md", "note", "a longer note as plain text"],
  ]) {
    await p.goto(`${base}/cloud/edit/${ids[name]}`);
    await p.waitForSelector("[data-testid=collab-editor][data-ready]");
    await p.waitForSelector('[data-connection="live"]');
    await s.take(p, slug, title);
  }
  await p.goto(`${base}/cloud/files/${encodeURIComponent(VAULT)}`);
  await p.waitForSelector('[data-entry="Home.md"]');
  await s.take(p, "folder", "the folder as a list of files");
  await p.goto(`${base}/cloud/files`);
  const folder = p.locator(`[data-entry="${VAULT}"]`);
  await folder.waitFor();
  await folder.click({ button: "right" });
  await p.waitForSelector('[data-slot="context-menu"]');
  await s.take(p, "folder-menu", "the folder's menu with knowledge bases off");
  await anon.goto(`${base}/kb/kitchen-garden/`);
  await anon.waitForSelector("[data-kb-missing]");
  await s.take(anon, "kb", "the site's address with knowledge bases off");
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
    console.error("usage: notes-shots.mjs BASE NAME PASSWORD OUT [--before] [--commit SHA] [--test NAME] [--ca FILE]");
    process.exit(2);
  }
  return { base: pos[0], name: pos[1], password: pos[2], out: pos[3], opts };
}

const { base, name, password, out, opts } = parse(process.argv.slice(2));
const browser = await chromium.launch({
  args: [`--ignore-certificate-errors-spki-list=${spkiPin(opts.ca ?? "/etc/losos-box.pem")}`],
});
const look = {
  viewport: VIEW,
  deviceScaleFactor: 2,
  colorScheme: "dark",
  locale: "en-US",
  timezoneId: "Europe/Bratislava",
  reducedMotion: "reduce",
};
const page = await (await browser.newContext(look)).newPage();
// A reader of the published site: another browser, never signed in.
const anon = await (await browser.newContext(look)).newPage();
// Every wait has room for an emulated machine.
page.setDefaultTimeout(120_000);
anon.setDefaultTimeout(120_000);
await signIn(page, base, name, password);

// What the admin page logs on a plain load (Sonner's refused stylesheet):
// Notes and the site may log nothing beyond it.
const baseline = new Set();
const problems = [];
let collecting = false;
for (const p of [page, anon]) {
  p.on("console", (m) => {
    if (m.type() !== "error") return;
    // A site that is not served answers 404, which is what the page asks
    // to learn that it is missing.
    if (/status of 404/.test(m.text()) && /\/kb\/api\/[^/]+\/site$/.test(m.location().url)) return;
    if (collecting) {
      if (!baseline.has(m.text())) problems.push(m.text());
    } else baseline.add(m.text());
  });
  p.on("pageerror", (e) => problems.push(String(e)));
}
await page.goto(`${base}/cloud/files`);
await page.waitForLoadState("networkidle");
collecting = true;

const s = new Shots(base, out, { commit: opts.commit ?? "", test: opts.test ?? "" });
try {
  if (opts.before) await before(s, page, anon, base);
  else await after(s, page, anon, base);
} catch (err) {
  await page.screenshot({ path: join(out, "failed-owner.png") }).catch(() => {});
  await anon.screenshot({ path: join(out, "failed-reader.png") }).catch(() => {});
  throw err;
} finally {
  s.save();
}
await browser.close();
if (problems.length > 0) {
  console.error(`the pages reported errors:\n${problems.join("\n")}`);
  process.exit(1);
}
