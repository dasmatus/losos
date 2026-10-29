/* The recorder for nix/demo/box-edge-box/demo.nix: at every step the VM
 * script announces, photographs the Mesh pane of both boxes and, at steps 3
 * and 5, the edge's manage page, all through the VMs' real nginx and admin
 * UI forwarded to this machine.
 *
 *   LOSOS_RECORD_DIR=<dir> bun nix/demo/box-edge-box/record.mjs
 *
 * record.sh runs it beside the VM script. The script writes
 * <dir>/{boxa,boxb,edge}.{token,key,port}, appends one JSON line per step
 * to <dir>/timeline.jsonl and waits for <dir>/ack-<n>; this writes
 * <dir>/<nn>-<box>-mesh.png and <dir>/<nn>-edge-manage.png, appends the
 * files to <dir>/shots.jsonl and acknowledges. THEME=dark for the dark
 * pictures; LOSOS_CHROMIUM for a chromium Playwright did not install;
 * LOSOS_APP_DIR for the admin-ui/app whose node_modules has playwright. */

const exists = (file) => Bun.file(file).exists();
const join = (...parts) => parts.join('/');
/* One line more at the end of a file. */
async function append(path, line) {
  const file = Bun.file(path);
  await Bun.write(file, ((await file.exists()) ? await file.text() : '') + line);
}

const appDir = Bun.env.LOSOS_APP_DIR || join(import.meta.dir, '..', '..', '..', 'admin-ui', 'app');
const { chromium } = await import(join(appDir, 'node_modules', 'playwright', 'index.mjs'));
const dir = Bun.env.LOSOS_RECORD_DIR;
if (!dir) throw new Error('set LOSOS_RECORD_DIR');
const theme = Bun.env.THEME || 'light';
const want = { 1: 'none', 2: 'found', 3: 'found', 4: 'found', 5: 'found', 6: 'found', 7: 'none' };
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const log = (...a) => console.log(new Date().toISOString().slice(11, 19), ...a);
const browser = await chromium.launch({ chromiumSandbox: false, executablePath: Bun.env.LOSOS_CHROMIUM || undefined });

async function mesh(n, step, box) {
  const port = (await Bun.file(join(dir, `${box}.port`)).text()).trim();
  const token = (await Bun.file(join(dir, `${box}.token`)).text()).trim();
  const page = await browser.newPage({ viewport: { width: 1280, height: 860 }, locale: 'en-US' });
  await page.addInitScript(([t, th]) => {
    window.sessionStorage.setItem('losos-token', t);
    window.localStorage.setItem('losos-theme', th);
  }, [token, theme]);
  await page.goto(`http://127.0.0.1:${port}/mesh`, { waitUntil: 'networkidle', timeout: 120000 });
  try {
    await page.locator(`[data-edge="${want[step]}"]`).first().waitFor({ timeout: 90000 });
  } catch {
    log(`${box}: no data-edge=${want[step]} at step ${step}`);
  }
  await sleep(1500);
  const file = join(dir, `${String(n).padStart(2, '0')}-${box}-mesh.png`);
  await page.screenshot({ path: file });
  await page.close();
  return file;
}

async function edgePage(n) {
  const key = (await Bun.file(join(dir, 'edge.key')).text()).trim();
  const page = await browser.newPage({ viewport: { width: 1280, height: 860 }, locale: 'en-US' });
  await page.addInitScript((th) => window.localStorage.setItem('losos-theme', th), theme);
  await page.goto('http://127.0.0.1:18443/manage/', { waitUntil: 'networkidle', timeout: 120000 });
  await page.locator('input').first().fill(key);
  await page.locator('button[type=submit]').first().click();
  await page.getByText('boxb').first().waitFor({ timeout: 60000 }).catch(() => log('edge page: boxb not listed'));
  await sleep(1500);
  const file = join(dir, `${String(n).padStart(2, '0')}-edge-manage.png`);
  await page.screenshot({ path: file });
  await page.close();
  return file;
}

let done = 0;
for (;;) {
  const tl = join(dir, 'timeline.jsonl');
  const lines = await exists(tl) ? (await Bun.file(tl).text()).split('\n').filter(Boolean) : [];
  if (lines.length > done) {
    const ev = JSON.parse(lines[done]);
    log(`step ${ev.n}: ${ev.caption}`);
    const files = [];
    for (const box of ['boxa', 'boxb']) {
      try { files.push(await mesh(ev.n, ev.step, box)); } catch (e) { log(`${box}: ${e.message}`); }
    }
    if (['3', '5'].includes(ev.step)) {
      try { files.push(await edgePage(ev.n)); } catch (e) { log(`edge: ${e.message}`); }
    }
    await append(join(dir, 'shots.jsonl'), JSON.stringify({ ...ev, files }) + '\n');
    await Bun.write(join(dir, `ack-${ev.n}`), '');
    done += 1;
    if (ev.step === '7') break;
  } else {
    if (await exists(join(dir, 'vm-finished'))) break;
    await sleep(1000);
  }
}
await browser.close();
log('recorder done');
