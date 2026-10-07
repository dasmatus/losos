/* The recorder for demo/edge-lan/two-boxes.nix: photographs the Mesh pane of
 * both boxes at every step the VM script announces.
 *
 *   LOSOS_RECORD_DIR=<dir> node demo/edge-lan/record.mjs
 *
 * The VM script (run by demo/edge-lan/record.sh) writes <dir>/box{1,2}.token
 * and .port once the boxes are up, then appends one JSON line per step to
 * <dir>/timeline.jsonl and waits for <dir>/ack-<n>. For each step this opens
 * http://127.0.0.1:<port>/mesh on both boxes — the real nginx and the real
 * SPA, signed in with the box's admin key — waits for the pane to settle on
 * the edge state the step implies, writes <dir>/<nn>-<step>-<box>.png, and
 * acknowledges. The step's caption lands in <dir>/timeline.jsonl for
 * compose.sh. Needs the app's node_modules (admin-ui/app, after `npm ci`;
 * LOSOS_APP_DIR points elsewhere) for playwright;
 * LOSOS_CHROMIUM points at a chromium binary when Playwright's own is not
 * installed. */
import { existsSync, readFileSync, writeFileSync, appendFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

// playwright comes from the app's node_modules (LOSOS_APP_DIR, default
// admin-ui/app in this checkout), wherever this script is run from.
const appDir = process.env.LOSOS_APP_DIR || join(dirname(fileURLToPath(import.meta.url)), '..', '..', 'admin-ui', 'app');
const { chromium } = await import(pathToFileURL(join(appDir, 'node_modules', 'playwright', 'index.mjs')).href);

const dir = process.env.LOSOS_RECORD_DIR;
if (!dir) throw new Error('set LOSOS_RECORD_DIR');
const boxes = ['box1', 'box2'];
const want = { A1: 'none', A2: 'none', A3: 'none', A4: 'none', B1: 'found', B2: 'found', B5: 'none', B6: 'found', B8: 'found', end: 'found' };
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const log = (...a) => console.log(new Date().toISOString().slice(11, 19), ...a);

async function waitFor(path, timeoutMs = 60 * 60 * 1000) {
  const t0 = Date.now();
  while (!existsSync(path)) {
    if (Date.now() - t0 > timeoutMs) throw new Error(`gave up waiting for ${path}`);
    await sleep(1000);
  }
}

const browser = await chromium.launch({ chromiumSandbox: false, executablePath: process.env.LOSOS_CHROMIUM || undefined });

async function openMesh(box) {
  await waitFor(join(dir, `${box}.port`));
  await waitFor(join(dir, `${box}.token`));
  const port = readFileSync(join(dir, `${box}.port`), 'utf8').trim();
  const token = readFileSync(join(dir, `${box}.token`), 'utf8').trim();
  const page = await browser.newPage({ viewport: { width: 1280, height: 900 }, locale: 'en-US' });
  await page.addInitScript((t) => {
    window.sessionStorage.setItem('losos-token', t);
    window.localStorage.setItem('losos-theme', 'light');
  }, token);
  await page.goto(`http://127.0.0.1:${port}/mesh`, { waitUntil: 'networkidle', timeout: 120000 });
  return page;
}

async function shoot(n, step, box) {
  const page = await openMesh(box);
  const state = want[step];
  try {
    if (state) await page.locator(`[data-edge="${state}"]`).waitFor({ timeout: 90000 });
  } catch (e) {
    log(`${box}: pane did not reach data-edge=${state} for ${step}; photographing as is`);
  }
  if (step === 'B8') {
    // The warning sign beside the unsigned edge: hover it so the tooltip
    // listing what that edge cannot do is in the picture.
    const sign = page.getByRole('button', { name: /Not an official LosOS edge/ }).first();
    if (await sign.count()) {
      await sign.hover();
      await sleep(800);
    }
  }
  await sleep(600);
  const file = join(dir, `${String(n).padStart(2, '0')}-${step}-${box}.png`);
  await page.screenshot({ path: file });
  await page.close();
  return file;
}

let done = 0;
log('waiting for the VM script');
for (;;) {
  const tl = join(dir, 'timeline.jsonl');
  const lines = existsSync(tl) ? readFileSync(tl, 'utf8').split('\n').filter(Boolean) : [];
  if (lines.length > done) {
    const ev = JSON.parse(lines[done]);
    log(`step ${ev.n} ${ev.step}: ${ev.caption}`);
    const files = [];
    for (const box of boxes) {
      try {
        files.push(await shoot(ev.n, ev.step, box));
      } catch (e) {
        log(`${box}: ${e.message}`);
      }
    }
    appendFileSync(join(dir, 'shots.jsonl'), JSON.stringify({ ...ev, files }) + '\n');
    writeFileSync(join(dir, `ack-${ev.n}`), '');
    done += 1;
    if (ev.step === 'end') break;
  } else {
    if (existsSync(join(dir, 'vm-finished'))) break;
    await sleep(1000);
  }
}
await browser.close();
log('recorder done');
