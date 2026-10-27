/* The recorder for VM tests that carry the hooks in nix/demo/recorder.py:
 * photographs each step's pages through the box's real nginx and admin UI.
 *
 *   LOSOS_RECORD_DIR=<dir> node nix/demo/record.mjs
 *
 * The VM script appends one JSON line per step to <dir>/timeline.jsonl,
 * each with a list of shots ({name, port, path, token?, wait?, local?}), and waits
 * for <dir>/ack-<n>. For each shot this opens http://127.0.0.1:<port><path>,
 * signed in with the admin key in <dir>/<token> when the shot names one,
 * waits for the `wait` selector, and writes <dir>/<nn>-<name>.png. The
 * step, its caption and its pictures land in <dir>/shots.jsonl.
 *
 * Needs the app's node_modules (admin-ui/app after `npm ci`, or
 * LOSOS_APP_DIR) for playwright. LOSOS_CHROMIUM points at a chromium binary
 * when Playwright's own is not installed. LOSOS_RECORD_WIDTH and
 * LOSOS_RECORD_HEIGHT set the window, 1280 by 860 by default; LOSOS_THEME
 * is light (default) or dark. */
import { existsSync, readFileSync, writeFileSync, appendFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const appDir = process.env.LOSOS_APP_DIR || join(dirname(fileURLToPath(import.meta.url)), '..', '..', 'admin-ui', 'app');
const { chromium } = await import(pathToFileURL(join(appDir, 'node_modules', 'playwright', 'index.mjs')).href);

const dir = process.env.LOSOS_RECORD_DIR;
if (!dir) throw new Error('set LOSOS_RECORD_DIR');
const viewport = {
  width: Number(process.env.LOSOS_RECORD_WIDTH || 1280),
  height: Number(process.env.LOSOS_RECORD_HEIGHT || 860),
};
const theme = process.env.LOSOS_THEME || 'light';
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const log = (...a) => console.log(new Date().toISOString().slice(11, 19), ...a);

const browser = await chromium.launch({ chromiumSandbox: false, executablePath: process.env.LOSOS_CHROMIUM || undefined });

async function shoot(n, shot) {
  const page = await browser.newPage({ viewport, locale: 'en-US' });
  const token = shot.token ? readFileSync(join(dir, shot.token), 'utf8').trim() : null;
  await page.addInitScript(
    ([t, th, local]) => {
      if (t) window.sessionStorage.setItem('losos-token', t);
      window.localStorage.setItem('losos-theme', th);
      for (const [k, v] of Object.entries(local)) window.localStorage.setItem(k, v);
    },
    [token, theme, shot.local || {}],
  );
  try {
    await page.goto(`http://127.0.0.1:${shot.port}${shot.path || '/'}`, { waitUntil: 'networkidle', timeout: 120000 });
    if (shot.wait) {
      try {
        await page.locator(shot.wait).first().waitFor({ timeout: 90000 });
      } catch {
        log(`${shot.name}: ${shot.wait} never appeared; photographing as is`);
      }
    }
    await sleep(800);
    const file = join(dir, `${String(n).padStart(2, '0')}-${shot.name}.png`);
    await page.screenshot({ path: file });
    return file;
  } finally {
    await page.close();
  }
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
    for (const shot of ev.shots || []) {
      try {
        files.push(await shoot(ev.n, shot));
      } catch (e) {
        log(`${shot.name}: ${e.message}`);
      }
    }
    appendFileSync(join(dir, 'shots.jsonl'), JSON.stringify({ n: ev.n, step: ev.step, caption: ev.caption, files }) + '\n');
    writeFileSync(join(dir, `ack-${ev.n}`), '');
    done += 1;
  } else {
    if (existsSync(join(dir, 'vm-finished'))) break;
    await sleep(1000);
  }
}
await browser.close();
log('recorder done');
