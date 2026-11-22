/* The television: a set gets the 10-foot view, a desk does not, and the
 * arrows, Back and Home move a ring you can see. Against dist/, like the
 * other browser checks.
 *
 *   bun run build && bun tests/tv.browser.ts
 *
 * Set TV_ARTIFACTS=1 to also write 1080p, 4K and a short remote recording
 * under /tmp/losos-tv-artifacts.
 */

import type { Page, Route } from 'playwright';
import assert from './assert.ts';
import { launch, runner, serve, shellRoutes } from './harness.ts';

const json = (route: Route, status: number, body: unknown) =>
  route.fulfill({ status, contentType: 'application/json', body: JSON.stringify(body) });

const { origin, close: closeServer } = await serve();
const browser = await launch();
const { check, finish } = runner();
const ARTIFACTS = '/tmp/losos-tv-artifacts';

async function stub(page: Page, signedIn = true) {
  await shellRoutes(page);
  await page.route('**/status.php', (route) => json(route, 200, { installed: true }));
  await page.route('**/api/health', (route) => json(route, 200, { ok: true }));
  await page.route('**/api/settings', (route) =>
    json(route, 200, {
      hostName: 'mattbox',
      sharingMyStorage: false,
      sharingLocally: false,
      gpuEnable: false,
      proxyEnable: false,
      clusterEnable: false,
      shareCompute: false,
      computeWindowStart: '23:00',
      computeWindowEnd: '07:00',
      hardeningApparmor: false,
      hardeningMalloc: false,
      hardeningNosmt: false,
      hardeningUsbguard: false,
    }),
  );
  await page.route('**/api/storage', (route) =>
    json(route, 200, { totalBytes: 480e9, usedBytes: 131e9, reserveBytes: 48e9 }),
  );
  await page.route('**/api/status', (route) => json(route, 200, { state: 'idle', progress: 0, message: '' }));
  if (!signedIn) await page.route('**/api/me', (route) => json(route, 401, { error: 'unauthorized' }));
}

async function openTv(width: number, height: number, signedIn = true) {
  const page = await browser.newPage({ viewport: { width, height }, deviceScaleFactor: 1 });
  await page.addInitScript(() => {
    (globalThis as { __LOSOS_FORCE_PLATFORM__?: string }).__LOSOS_FORCE_PLATFORM__ = 'tv';
  });
  await stub(page, signedIn);
  await page.goto(origin + '/', { waitUntil: 'networkidle' });
  await page.getByTestId('tv-view').waitFor();
  await page.locator('[data-tv-item="health"]').waitFor();
  return page;
}

await check('a desk stays a desk', async () => {
  const page = await browser.newPage({ viewport: { width: 1920, height: 1080 } });
  await stub(page);
  await page.goto(origin + '/', { waitUntil: 'networkidle' });
  await page.getByTestId('desktop').waitFor();
  assert.equal(await page.getByTestId('tv-view').count(), 0);
  if (Bun.env.TV_ARTIFACTS === '1') {
    await Bun.$`mkdir -p ${ARTIFACTS}`;
    await page.screenshot({ path: `${ARTIFACTS}/before-1080p.png` });
  }
  await page.close();
});

await check('a set opens the television by itself', async () => {
  const context = await browser.newContext({
    viewport: { width: 1920, height: 1080 },
    userAgent: 'Mozilla/5.0 (SMART-TV; Linux; Tizen 6.5) AppleWebKit/537.36 Chrome/120.0.0.0',
  });
  const page = await context.newPage();
  await stub(page);
  await page.goto(origin + '/', { waitUntil: 'networkidle' });
  await page.getByTestId('tv-view').waitFor();
  assert.equal(await page.getByTestId('desktop').count(), 0);
  assert.equal(await page.locator('[data-slot="app-bar"]').count(), 0);
  await context.close();
});

await check('Off keeps the desk even on a set', async () => {
  const context = await browser.newContext({
    viewport: { width: 1920, height: 1080 },
    userAgent: 'Mozilla/5.0 (SMART-TV; Linux; Tizen 6.5) AppleWebKit/537.36 Chrome/120.0.0.0',
  });
  const page = await context.newPage();
  await page.addInitScript(() => window.localStorage.setItem('losos-tv', 'off'));
  await stub(page);
  await page.goto(origin + '/', { waitUntil: 'networkidle' });
  await page.getByTestId('desktop').waitFor();
  assert.equal(await page.getByTestId('tv-view').count(), 0);
  await context.close();
});

await check('the ring is visible and the arrows, Back and Home move it', async () => {
  const page = await openTv(1920, 1080);
  const health = page.locator('[data-tv-item="health"]');
  await health.focus();
  const outline = await health.evaluate((el) => getComputedStyle(el).outlineWidth);
  assert.equal(outline, '4px', `the focus ring is ${outline}`);
  const pad = await page.locator('.tv-root').evaluate((el) => parseFloat(getComputedStyle(el).paddingTop));
  assert.ok(pad >= 48, `overscan padding ${pad}px`);
  await page.keyboard.press('ArrowRight');
  assert.equal(await page.evaluate(() => document.activeElement?.getAttribute('data-tv-item')), 'storage');
  await page.keyboard.press('Enter');
  await page.getByTestId('tv-detail').waitFor();
  await page.keyboard.press('Escape');
  await page.getByTestId('tv-home').waitFor();
  await page.keyboard.press('ArrowRight');
  await page.keyboard.press('Enter');
  await page.getByTestId('tv-detail').waitFor();
  await page.keyboard.press('Home');
  await page.getByTestId('tv-home').waitFor();
  assert.equal(await page.locator('[data-slot="app-bar"]').count(), 0);
  if (Bun.env.TV_ARTIFACTS === '1') {
    await health.focus();
    await page.screenshot({ path: `${ARTIFACTS}/after-1080p.png` });
  }
  await page.close();
});

await check('4K draws larger type than 1080p, still inside the overscan', async () => {
  const hd = await openTv(1920, 1080);
  const hdSize = await hd.locator('.tv-title').evaluate((el) => parseFloat(getComputedStyle(el).fontSize));
  const hdPad = await hd.locator('.tv-root').evaluate((el) => parseFloat(getComputedStyle(el).paddingLeft));
  await hd.close();
  const uhd = await openTv(3840, 2160);
  const uhdSize = await uhd.locator('.tv-title').evaluate((el) => parseFloat(getComputedStyle(el).fontSize));
  const uhdPad = await uhd.locator('.tv-root').evaluate((el) => parseFloat(getComputedStyle(el).paddingLeft));
  assert.ok(uhdSize > hdSize * 1.5, `4K title ${uhdSize}px should dwarf 1080p ${hdSize}px`);
  assert.ok(uhdPad >= 64, `4K side margin ${uhdPad}px`);
  assert.ok(hdPad >= 64, `1080p side margin ${hdPad}px`);
  if (Bun.env.TV_ARTIFACTS === '1') {
    await uhd.locator('[data-tv-item="health"]').focus();
    await uhd.screenshot({ path: `${ARTIFACTS}/after-4k.png` });
  }
  await uhd.close();
});

await check('Back from sign-in returns home, and a typed character is not Back', async () => {
  const page = await openTv(1920, 1080, false);
  await page.locator('[data-tv-item="storage"]').focus();
  await page.keyboard.press('Enter');
  await page.getByTestId('tv-signin').waitFor();
  const name = page.locator('[data-tv-item="signin-name"]');
  await name.fill('matus');
  await name.press('Backspace');
  await page.getByTestId('tv-signin').waitFor();
  assert.equal(await name.inputValue(), 'matu');
  await name.fill('');
  await page.keyboard.press('Escape');
  await page.getByTestId('tv-home').waitFor();
  await page.close();
});

if (Bun.env.TV_ARTIFACTS === '1') {
  await check('record a remote moving the ring', async () => {
    const context = await browser.newContext({
      viewport: { width: 1920, height: 1080 },
      recordVideo: { dir: ARTIFACTS, size: { width: 1280, height: 720 } },
    });
    const page = await context.newPage();
    await page.addInitScript(() => {
      (globalThis as { __LOSOS_FORCE_PLATFORM__?: string }).__LOSOS_FORCE_PLATFORM__ = 'tv';
    });
    await stub(page);
    await page.goto(origin + '/', { waitUntil: 'networkidle' });
    await page.locator('[data-tv-item="health"]').focus();
    await page.waitForTimeout(800);
    await page.keyboard.press('ArrowRight');
    await page.waitForTimeout(800);
    await page.keyboard.press('Enter');
    await page.getByTestId('tv-detail').waitFor();
    await page.waitForTimeout(900);
    await page.keyboard.press('Escape');
    await page.getByTestId('tv-home').waitFor();
    await page.waitForTimeout(600);
    await page.keyboard.press('ArrowDown');
    await page.waitForTimeout(700);
    await page.keyboard.press('ArrowRight');
    await page.waitForTimeout(800);
    await page.keyboard.press('Home');
    await page.waitForTimeout(700);
    const video = page.video();
    await page.close();
    await context.close();
    const saved = await video?.path();
    if (saved) await Bun.write(`${ARTIFACTS}/remote.webm`, Bun.file(saved));
  });
}

finish();
await browser.close();
await closeServer();
