/* What a browser actually shows the owner of a box that has just been plugged
 * in, and of one that has not.
 *
 * `tsc --noEmit` says the code is well typed and says nothing about which of
 * the two screens a new owner lands on, which is the only thing that matters
 * here: the page used to open on a dialog asking for an admin key that nothing
 * on the appliance prints, so a box out of the carton could not be set up at
 * all. That failure typechecked perfectly.
 *
 * Runs against the real production bundle from `vite build`, served over
 * http://127.0.0.1 so the page is a secure context (localhost is one by
 * specification, whatever the box is doing). lososd is not here — it owns
 * org.losos1 on the system bus and the policy for that ships in a NixOS module
 * — so the API is stubbed with page.route(), which also lets one run flip the
 * box between claimed and unclaimed without any state to reset.
 *
 * Needs `npm run build` first and a browsers dir:
 *   PLAYWRIGHT_BROWSERS_PATH=$(nix build --print-out-paths --no-link \
 *     nixpkgs#playwright-driver.browsers)
 * tests/admin-ui.nix sets all of that up; keep the `playwright` devDependency
 * pinned to the same version as nixpkgs' playwright-driver.
 */

import { createServer } from 'node:http';
import { readFile } from 'node:fs/promises';
import { extname, join, normalize } from 'node:path';
import assert from 'node:assert';
import { chromium } from 'playwright';

const DIST = 'dist';
const TYPES = {
  '.html': 'text/html; charset=utf-8',
  '.js': 'text/javascript; charset=utf-8',
  '.css': 'text/css; charset=utf-8',
  '.json': 'application/json',
  '.svg': 'image/svg+xml',
};

/* Serves dist/ with an SPA fallback, the same shape nginx gives the admin
 * location (`try_files $uri /index.html`). Without the fallback a deep link
 * 404s and the test would be asserting against an error page. */
const server = createServer((req, res) => {
  const url = new URL(req.url, 'http://127.0.0.1');
  const rel = normalize(url.pathname).replace(/^(\.\.[/\\])+/, '');
  const send = async (file) => {
    const body = await readFile(join(DIST, file));
    res.writeHead(200, { 'Content-Type': TYPES[extname(file)] ?? 'application/octet-stream' });
    res.end(body);
  };
  send(rel === '/' ? 'index.html' : rel).catch(() => send('index.html').catch(() => {
    res.writeHead(404);
    res.end('not found');
  }));
});

await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
const origin = `http://127.0.0.1:${server.address().port}`;

const browser = await chromium.launch({ chromiumSandbox: false });

/* One page wired to a box in a given state. `claimed` is the whole point: it
 * is what decides between the wizard and the key prompt. */
async function open({ claimed, tls = false, path = '/' }) {
  const page = await browser.newPage({ viewport: { width: 1280, height: 900 } });
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));

  /* Registered FIRST, and that order is load-bearing: when several routes
   * match, Playwright uses the one registered LAST. With this catch-all added
   * afterwards it shadowed /api/setup/claim, the page read `claimed:
   * undefined`, and the unclaimed case passed for the wrong reason while the
   * claimed case failed outright. Specific routes go below it. */
  await page.route('**/api/**', (route) =>
    route.fulfill({ status: 200, contentType: 'application/json', body: '{}' }),
  );

  await page.route('**/api/setup/claim', async (route) => {
    if (route.request().method() === 'POST') {
      return route.fulfill({
        status: claimed ? 409 : 200,
        contentType: 'application/json',
        body: JSON.stringify(
          claimed
            ? { error: 'this box has already been set up' }
            : { claimed: true, user: 'notshared', token: '0'.repeat(64) },
        ),
      });
    }
    return route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({ claimed }),
    });
  });

  await page.route('**/setup/state.json', (route) =>
    route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({
        hostName: 'mattbox',
        fqdn: 'mattbox.local',
        tls,
        certificate: tls
          ? { sha256: 'AA:BB', notAfter: '2028-01-01T00:00:00Z', url: '/setup/losos.crt' }
          : null,
      }),
    }),
  );

  await page.goto(origin + path, { waitUntil: 'networkidle' });
  return { page, errors };
}

const failures = [];
const check = async (name, fn) => {
  try {
    await fn();
    console.log(`  ok   ${name}`);
  } catch (e) {
    failures.push(`${name}: ${e.message}`);
    console.log(`  FAIL ${name}\n       ${e.message}`);
  }
};

console.log('admin-ui browser checks');

await check('a box nobody has set up opens the wizard, not a key prompt', async () => {
  const { page, errors } = await open({ claimed: false });
  const text = await page.locator('body').innerText();
  assert.ok(
    /Trust this box/i.test(text),
    `expected the wizard's first step; page errors: ${JSON.stringify(errors)}`,
  );
  assert.ok(
    !/Unlock this box/i.test(text),
    'a fresh box must never ask for the admin key: nothing on the appliance prints it',
  );
  await page.close();
});

await check('a box that already has an owner asks to sign in, not to set up', async () => {
  const { page } = await open({ claimed: true });
  const text = await page.locator('body').innerText();
  assert.ok(/Unlock this box/i.test(text), 'expected the key prompt on a claimed box');
  assert.ok(
    !/Trust this box/i.test(text),
    'showing setup to a returning owner reads as "this box has been wiped"',
  );
  await page.close();
});

/* The bug this exists for: step 1 predicted from the BOX's tls flag while step
 * 2 decided from the BROWSER's secure context, so on localhost step 1 promised
 * "the passkey option will not be offered" and step 2 offered it. Both now read
 * passkeysPossibleHere(). The assertion is the agreement, not either value,
 * because which way it goes depends on where the test runs. */
await check('step 1 does not promise something step 2 contradicts', async () => {
  const { page } = await open({ claimed: false, tls: false });
  const step1 = await page.locator('body').innerText();
  const promisedNoPasskey = /passkey option on the next step will not be offered/i.test(step1);

  const secure = await page.evaluate(
    () => window.isSecureContext && typeof window.PublicKeyCredential !== 'undefined',
  );
  assert.ok(
    !(promisedNoPasskey && secure),
    'step 1 said passkeys will not be offered, but this browser can create them, so step 2 will offer one',
  );
  await page.close();
});

/* Guards the unslop pass. An em dash in the body copy is the tell that pulled
 * the whole UI's prose in for a rewrite; the placeholder dash that means "no
 * value" lives in a readout, not in a sentence, so this only looks at prose
 * that has a space on both sides of the dash. */
await check('no em dash reappears in the wizard copy', async () => {
  const { page } = await open({ claimed: false });
  const text = await page.locator('body').innerText();
  const offenders = text.split('\n').filter((line) => / — /.test(line));
  assert.deepStrictEqual(
    offenders,
    [],
    `em dash used as a sentence connector:\n${offenders.join('\n')}`,
  );
  await page.close();
});

/* Setup is not something a URL can step around.
 *
 * Two ways it could be, and both are real. A deep link straight to a settings
 * route on a box nobody has claimed would be a half-configured page reached
 * before the owner exists. And the wizard tears itself down halfway if the
 * shell watches the claim flag rather than the wizard: step 2 claims the box
 * and stores the token, so a gate keyed on "is it claimed" flips to the app
 * mid-flow and skips step 3, the recovery code, which is the one thing that has
 * to leave the box. The owner would never see it and would not know. */
for (const path of ['/settings', '/settings/reset', '/storage', '/mesh', '/apps']) {
  await check(`an unclaimed box shows setup at ${path}, not the page`, async () => {
    const { page } = await open({ claimed: false, path });
    const text = await page.locator('body').innerText();
    assert.ok(
      /Trust this box/i.test(text),
      `${path} reached the app on a box with no owner`,
    );
    await page.close();
  });
}

await check('claiming the box mid-wizard does not end the wizard', async () => {
  const { page } = await open({ claimed: false });
  // What step 2 does on success: the token lands in session storage and the
  // claim flag flips. A shell keyed on either would swap the app in here.
  await page.evaluate(() => {
    window.sessionStorage.setItem('losos-token', '0'.repeat(64));
    window.dispatchEvent(new StorageEvent('storage', { key: 'losos-token' }));
  });
  await page.waitForTimeout(200);
  const text = await page.locator('body').innerText();
  assert.ok(
    /Trust this box|sign in|Recovery code/i.test(text),
    'the wizard vanished once a token existed, so step 3 would never be shown',
  );
  assert.ok(!/Your board/i.test(text), 'the overview replaced the wizard mid-flow');
  await page.close();
});

await browser.close();
server.close();

if (failures.length > 0) {
  console.error(`\n${failures.length} failed:\n${failures.map((f) => `  - ${f}`).join('\n')}`);
  process.exit(1);
}
console.log('\nall browser checks passed');
