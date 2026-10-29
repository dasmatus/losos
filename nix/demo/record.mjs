/* The recorder for VM tests that carry the hooks in nix/demo/recorder.py:
 * photographs each step's pages through the box's real nginx and admin UI.
 *
 *   LOSOS_RECORD_DIR=<dir> bun nix/demo/record.mjs
 *
 * The VM script appends one JSON line per step to <dir>/timeline.jsonl,
 * each with a list of shots ({name, port, path, token?, wait?, local?}), and waits
 * for <dir>/ack-<n>. For each shot this opens http://127.0.0.1:<port><path>,
 * signed in with the admin key in <dir>/<token> when the shot names one,
 * waits for the `wait` selector, and writes <dir>/<nn>-<name>.png. The
 * step, its caption and its pictures land in <dir>/shots.jsonl.
 *
 * Needs the app's node_modules (admin-ui/app after `bun install`, or
 * LOSOS_APP_DIR) for playwright. LOSOS_CHROMIUM points at a chromium binary
 * when Playwright's own is not installed. LOSOS_RECORD_WIDTH and
 * LOSOS_RECORD_HEIGHT set the window, 1280 by 860 by default; LOSOS_THEME
 * is light (default) or dark. */

const exists = (file) => Bun.file(file).exists();
const join = (...parts) => parts.join('/');
/* One line more at the end of a file. */
async function append(path, line) {
  const file = Bun.file(path);
  await Bun.write(file, ((await file.exists()) ? await file.text() : '') + line);
}

const appDir = Bun.env.LOSOS_APP_DIR || join(import.meta.dir, '..', '..', 'admin-ui', 'app');
const { chromium } = await import(join(appDir, 'node_modules', 'playwright', 'index.mjs'));

const dir = Bun.env.LOSOS_RECORD_DIR;
if (!dir) throw new Error('set LOSOS_RECORD_DIR');
const viewport = {
  width: Number(Bun.env.LOSOS_RECORD_WIDTH || 1280),
  height: Number(Bun.env.LOSOS_RECORD_HEIGHT || 860),
};
const theme = Bun.env.LOSOS_THEME || 'light';
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const log = (...a) => console.log(new Date().toISOString().slice(11, 19), ...a);

const browser = await chromium.launch({ chromiumSandbox: false, executablePath: Bun.env.LOSOS_CHROMIUM || undefined });

async function shoot(n, shot) {
  const page = await browser.newPage({ viewport, locale: 'en-US' });
  const token = shot.token ? (await Bun.file(join(dir, shot.token)).text()).trim() : null;
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
  const lines = await exists(tl) ? (await Bun.file(tl).text()).split('\n').filter(Boolean) : [];
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
    await append(join(dir, 'shots.jsonl'), JSON.stringify({ n: ev.n, step: ev.step, caption: ev.caption, files }) + '\n');
    await Bun.write(join(dir, `ack-${ev.n}`), '');
    done += 1;
  } else {
    if (await exists(join(dir, 'vm-finished'))) break;
    await sleep(1000);
  }
}
await browser.close();
log('recorder done');
