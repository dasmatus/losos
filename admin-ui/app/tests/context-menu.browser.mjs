/* The right-click menu (components/context-menu.tsx): what each place on
 * the admin page offers, how it opens from the mouse, the keyboard and a
 * long press, where it lands, and that a text field keeps the browser's
 * own menu.
 *
 * Same arrangement as the other files: the real dist/ bundle, the API
 * stubbed with page.route(), the page under the appliance CSP. */

import assert from './assert.mjs';
import { launch, runner, serve } from './harness.mjs';

const TOKEN = 'a'.repeat(64);
const { origin, close: closeServer } = await serve({ csp: true });
const browser = await launch();
const { check, finish } = runner();

const json = (route, status, body) =>
  route.fulfill({ status, contentType: 'application/json', body: JSON.stringify(body) });

async function open({ path = '/', viewport = { width: 1280, height: 900 }, touch = false } = {}) {
  const context = await browser.newContext({ viewport, locale: 'en-US', hasTouch: touch, isMobile: touch });
  const page = await context.newPage();
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  await page.addInitScript(() => {
    window.__violations = [];
    document.addEventListener('securitypolicyviolation', (e) => window.__violations.push(e.violatedDirective));
    // What the page did with the last right click, read after every handler.
    window.addEventListener('contextmenu', (e) => {
      window.__lastMenu = { prevented: e.defaultPrevented };
    });
  });
  await page.route('**/api/**', (route) => json(route, 200, {}));
  await page.route('**/api/setup/claim', (route) => json(route, 200, { claimed: true }));
  await page.route('**/api/pins', (route) => json(route, 200, { pins: [], limit: 24 }));
  await page.route('**/nextcloud/status.php', (route) => json(route, 200, { installed: true }));
  await page.route('**/forgejo/', (route) => route.fulfill({ status: 200, contentType: 'text/html', body: '' }));
  await page.addInitScript((t) => window.sessionStorage.setItem('losos-token', t), TOKEN);
  await page.goto(origin + path, { waitUntil: 'networkidle' });
  const atLoad = await page.evaluate(() => window.__violations);
  assert.deepEqual(atLoad, [], `CSP violations at load: ${JSON.stringify(atLoad)}`);
  const done = async () => {
    const violations = (await page.evaluate(() => window.__violations)).slice(atLoad.length);
    assert.deepEqual(violations, [], `the menu added CSP violations: ${JSON.stringify(violations)}`);
    assert.deepEqual(errors, [], 'uncaught errors');
    await context.close();
  };
  return { page, done };
}

const menu = (page) => page.locator('[data-slot="context-menu"]').first();
const items = (page) => menu(page).locator('[role^="menuitem"]');
const texts = async (page) => (await items(page).allInnerTexts()).map((s) => s.replace(/\s+/g, ' ').trim());

await check('empty space offers the page: Back, Forward, Reload, search, the theme and the handbook', async () => {
  const { page, done } = await open();
  const heading = page.getByRole('main').locator('h1, h2').first();
  await heading.waitFor();
  const box = await page.getByRole('main').boundingBox();
  await page.mouse.click(box.x + box.width - 8, box.y + 8, { button: 'right' });
  await menu(page).waitFor();
  assert.equal(await menu(page).getAttribute('role'), 'menu');
  assert.equal(await menu(page).getAttribute('aria-label'), 'This page');
  const got = await texts(page);
  assert.deepEqual(got.map((s) => s.split(' ')[0]), ['Search', 'Back', 'Forward', 'Reload', 'Appearance', 'Handbook']);
  // The keys beside an item are the real ones, for screen readers too.
  const search = items(page).filter({ hasText: 'Search the box' });
  assert.equal(await search.getAttribute('aria-keyshortcuts'), 'Control+Shift+P');
  assert.match(await search.innerText(), /Ctrl\s*⇧\s*P/);
  assert.equal(await menu(page).locator('[role=separator]').count(), 2);
  assert.equal(await page.evaluate(() => window.__lastMenu.prevented), true);
  await page.keyboard.press('Escape');
  await menu(page).waitFor({ state: 'detached' });
  await done();
});

await check('the first menu on a page grows in from its starting style, like every later one', async () => {
  const { page, done } = await open();
  await page.getByRole('main').locator('h1, h2').first().waitFor();
  // What the popup looks like the moment it is in the page.
  await page.evaluate(() => {
    window.__firstMenu = new Promise((found) => {
      new MutationObserver((_, observer) => {
        const popup = document.querySelector('[data-slot="context-menu"]');
        if (popup === null) return;
        observer.disconnect();
        found({ starting: popup.hasAttribute('data-starting-style'), opacity: getComputedStyle(popup).opacity });
      }).observe(document.body, { childList: true, subtree: true });
    });
  });
  const box = await page.getByRole('main').boundingBox();
  await page.mouse.click(box.x + box.width - 8, box.y + 8, { button: 'right' });
  const first = await page.evaluate(() => window.__firstMenu);
  assert.deepEqual(first, { starting: true, opacity: '0' }, 'the first menu skipped its way in');
  await page.keyboard.press('Escape');
  await menu(page).waitFor({ state: 'detached' });
  await done();
});

await check('the theme is a submenu the arrow keys reach, and picking Dark switches the page', async () => {
  const { page, done } = await open();
  const box = await page.getByRole('main').boundingBox();
  await page.mouse.click(box.x + box.width - 8, box.y + 8, { button: 'right' });
  await menu(page).waitFor();
  for (let i = 0; i < 5; i += 1) await page.keyboard.press('ArrowDown');
  assert.match(await page.evaluate(() => document.activeElement?.textContent ?? ''), /Appearance/);
  await page.keyboard.press('ArrowRight');
  const sub = page.locator('[data-slot="context-menu"][aria-label="Appearance"]');
  await sub.waitFor();
  assert.deepEqual(
    (await sub.getByRole('menuitemradio').allInnerTexts()).map((s) => s.trim()),
    ['Match the browser', 'Light', 'Dark'],
  );
  assert.equal(await sub.getByRole('menuitemradio', { name: 'Match the browser' }).getAttribute('aria-checked'), 'true');
  await sub.getByRole('menuitemradio', { name: 'Dark' }).click();
  await page.waitForFunction(() => document.documentElement.dataset.theme === 'dark');
  await menu(page).waitFor({ state: 'detached' });
  await done();
});

await check('a text field keeps the browser’s own menu, for spelling and paste', async () => {
  const { page, done } = await open();
  await page.keyboard.press('Control+Shift+P');
  const field = page.getByRole('combobox', { name: /^Search pages/ });
  await field.waitFor();
  await field.click({ button: 'right' });
  await page.waitForTimeout(200);
  assert.equal(await page.locator('[data-slot="context-menu"]').count(), 0);
  assert.equal(await page.evaluate(() => window.__lastMenu.prevented), false);
  await done();
});

await check('picked text offers Copy and a search for it, which opens the palette with the words', async () => {
  const { page, done } = await open();
  const heading = page.getByRole('main').locator('h1').first();
  await heading.waitFor();
  await heading.selectText();
  const words = (await page.evaluate(() => window.getSelection().toString())).trim();
  assert.ok(words.length > 0);
  await heading.click({ button: 'right' });
  await menu(page).waitFor();
  const got = await texts(page);
  assert.match(got[0], /^Copy/);
  assert.match(got[1], /^Search for "/);
  assert.equal(await items(page).first().getAttribute('aria-keyshortcuts'), 'Control+C');
  await items(page).nth(1).click();
  const field = page.getByRole('combobox', { name: /^Search pages/ });
  await field.waitFor();
  assert.equal(await field.inputValue(), words);
  await done();
});

await check('a link offers a new tab and its address, before the page', async () => {
  const { page, done } = await open();
  const link = page.getByRole('navigation', { name: 'Sections', exact: true }).getByRole('link').first();
  await link.click({ button: 'right' });
  await menu(page).waitFor();
  const got = await texts(page);
  assert.deepEqual(got.slice(0, 2), ['Open in new tab', 'Copy link']);
  assert.ok(got.includes('Reload Ctrl R'), JSON.stringify(got));
  const opened = page.context().waitForEvent('page');
  await items(page).first().click();
  const tab = await opened;
  assert.equal(new URL(tab.url()).pathname, new URL(await link.evaluate((a) => a.href)).pathname);
  await done();
});

await check('Shift+F10 opens the menu under what has the focus, on its first item, and Escape gives the focus back', async () => {
  const { page, done } = await open();
  const tile = page.getByRole('main').getByRole('listitem').getByRole('link').first();
  await tile.waitFor();
  await tile.focus();
  // The page behind a modal menu is hidden from the accessibility tree, so
  // the tile is measured first.
  const at = await tile.boundingBox();
  const name = (await tile.innerText()).split('\n')[0].trim();
  await page.keyboard.press('Shift+F10');
  await menu(page).waitFor();
  assert.equal(await menu(page).getAttribute('aria-label'), name);
  await page.waitForFunction(() => document.activeElement?.getAttribute('role') === 'menuitem');
  assert.equal(await page.evaluate(() => document.activeElement.textContent.trim()), 'Open in new tab');
  const shown = await menu(page).boundingBox();
  assert.ok(shown.y >= at.y + at.height - 2, `menu at ${shown.y}, tile ends at ${at.y + at.height}`);
  await page.keyboard.press('ArrowDown');
  assert.match(await page.evaluate(() => document.activeElement.textContent), /Pin to sidebar/);
  await page.keyboard.press('Escape');
  await menu(page).waitFor({ state: 'detached' });
  await page.waitForFunction(() => document.activeElement?.tagName === 'A');
  await done();
});

await check('at the window’s edge the menu turns back into the window', async () => {
  const { page, done } = await open({ viewport: { width: 900, height: 640 } });
  await page.getByRole('main').waitFor();
  await page.mouse.click(895, 635, { button: 'right' });
  await menu(page).waitFor();
  await page.waitForTimeout(200);
  const b = await menu(page).boundingBox();
  assert.ok(b.x >= 0 && b.y >= 0 && b.x + b.width <= 900 && b.y + b.height <= 640, JSON.stringify(b));
  await done();
});

await check('a long press on a touch screen opens it once, and lifting the finger, however late, does not follow the link', async () => {
  const { page, done } = await open({ viewport: { width: 390, height: 844 }, touch: true });
  const tile = page.getByRole('main').getByRole('listitem').getByRole('link').first();
  await tile.waitFor();
  const before = page.url();
  const b = await tile.boundingBox();
  const cdp = await page.context().newCDPSession(page);
  const point = [{ x: b.x + b.width / 2, y: b.y + b.height / 2 }];
  await cdp.send('Input.dispatchTouchEvent', { type: 'touchStart', touchPoints: point });
  // Held well past the press itself: the lift still belongs to the menu.
  await page.waitForTimeout(1800);
  await cdp.send('Input.dispatchTouchEvent', { type: 'touchEnd', touchPoints: [] });
  await menu(page).waitFor();
  assert.equal(await page.locator('[data-slot="context-menu"]').count(), 1);
  assert.equal(page.url(), before);
  await done();
});

await browser.close();
await closeServer();
finish();
