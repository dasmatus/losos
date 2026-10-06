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

import assert from 'node:assert';
import { launch, runner, serve } from './harness.mjs';

const { origin, close: closeServer } = await serve();
const browser = await launch();

/* One page wired to a box in a given state. `claimed` is the whole point: it
 * is what decides between the wizard and the key prompt. */
async function open({ claimed, tls = false, path = '/', ready = true, waitingFor = null }) {
  const page = await browser.newPage({ viewport: { width: 1280, height: 900 }, locale: 'en-US' });
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
    /* `ready` is a function or a value: a function lets a test flip the box
     * from installing to ready between two polls. */
    const isReady = typeof ready === 'function' ? ready() : ready;
    return route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({ claimed, ready: isReady, waitingFor: isReady ? null : waitingFor }),
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

const { check, finish } = runner();

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

/* Walk step 2 for real: Continue past the certificate, type a password twice,
 * submit. Returns the page, every request the page sent, and the body text
 * after lososd (stubbed) has answered. */
async function claimThroughStepTwo(password = 'correct horse battery staple') {
  const { page, errors } = await open({ claimed: false });
  const requests = [];
  page.on('request', (req) => requests.push({ method: req.method(), url: new URL(req.url()).pathname }));

  await page.getByRole('button', { name: /^Continue$/ }).click();
  await page.locator('input[name="new-password"]').fill(password);
  await page.locator('input[name="confirm-password"]').fill(password);
  await page.getByRole('button', { name: /^Set the password$/ }).click();
  await page.waitForTimeout(300);

  const text = await page.locator('body').innerText();
  return { page, errors, requests, text };
}

/* The bug the wizard shipped with: step 2 posted the first password to the
 * token-gated /api/set-password, and on a fresh box nothing holds a token, so
 * every first run ended in a 401 on this step. The public claim route is the
 * one that both sets the password and hands out the token; this is the only
 * check that proves step 2 uses it. */
await check('step 2 claims the box through the public claim route and keeps the token', async () => {
  const { page, errors, requests, text } = await claimThroughStepTwo();
  const claims = requests.filter((r) => r.method === 'POST' && r.url === '/api/setup/claim');
  assert.strictEqual(claims.length, 1, `expected one claim POST, saw ${JSON.stringify(requests)}`);
  assert.ok(
    !requests.some((r) => r.method === 'POST' && r.url === '/api/set-password'),
    'a fresh box has no token to send to /api/set-password; step 2 must claim instead',
  );
  const token = await page.evaluate(() => window.sessionStorage.getItem('losos-token'));
  assert.strictEqual(token, '0'.repeat(64), 'the token from the claim reply was not stored');
  assert.ok(/Password set/i.test(text), `step 2 did not report the password as set; page errors: ${JSON.stringify(errors)}\n${text}`);
  assert.ok(/notshared/.test(text), 'the account name lososd echoed back is not shown');
  await page.close();
});

/* The key is released exactly once, in the claim reply, and the sign-in
 * dialog on every later visit asks for it. A wizard that stored it in session
 * storage and showed it to nobody made closing the tab a permanent lockout. */
await check('step 2 shows the admin key from the claim, since nothing else ever will', async () => {
  const { page, text } = await claimThroughStepTwo();
  assert.ok(text.includes('0'.repeat(64)), `the admin key is not on screen after the claim:\n${text}`);
  assert.ok(/admin key/i.test(text), 'the key is shown without saying what it is');
  await page.close();
});

await check('claiming the box mid-wizard does not end the wizard', async () => {
  // Step 2 stores the token and flips the claim flag. A shell keyed on either
  // would swap the app in here and step 3, the recovery code, would never show.
  const { page, text } = await claimThroughStepTwo();
  assert.ok(
    /Trust this box|sign in|Recovery code|Password set/i.test(text),
    'the wizard vanished once a token existed, so step 3 would never be shown',
  );
  assert.ok(!/Your board/i.test(text), 'the overview replaced the wizard mid-flow');
  await page.close();
});


/* What the recorded install demo showed: on a fresh box the Nextcloud pod is
 * still running `occ maintenance:install` when the owner reaches step 2, and
 * the claim failed into "command failed; see the lososd journal". lososd now
 * says `ready: false` with a reason, and the step must wait on that rather
 * than let the owner submit into it. */
await check('step 2 waits with the reason on screen while the box is not ready, and never submits', async () => {
  const { page, errors } = await open({
    claimed: false,
    ready: false,
    waitingFor: 'Nextcloud is still installing itself. This happens once, on the first boot, and takes a few minutes.',
  });
  const requests = [];
  page.on('request', (req) => requests.push({ method: req.method(), url: new URL(req.url()).pathname }));
  await page.getByRole('button', { name: /^Continue$/ }).click();
  await page.waitForTimeout(400);
  const text = await page.locator('body').innerText();
  assert.ok(/finish starting/i.test(text), `no waiting panel while not ready:\n${text}`);
  assert.ok(/still installing itself/.test(text), 'lososd\'s reason is not shown to the owner');
  assert.ok(await page.locator('input[name="new-password"]').isDisabled(), 'the password field is enabled while the box cannot take one');
  assert.ok(await page.getByRole('button', { name: /^Set the password$/ }).isDisabled(), 'the submit is enabled while the box cannot take a password');
  assert.ok(!requests.some((r) => r.method === 'POST' && r.url === '/api/setup/claim'), 'a claim was sent while not ready');
  assert.deepStrictEqual(errors, []);
  await page.close();
});

/* The whole point: nothing to click. The poll notices the box is ready and
 * the form opens on its own, then the claim goes through as before. */
await check('step 2 opens on its own once the box reports ready, then claims', async () => {
  /* A flag rather than a poll count: the shell asks the same route once to
   * pick wizard-or-app before step 2 ever mounts, and the number of times it
   * does so is not this test's business. */
  let installed = false;
  const { page, errors } = await open({
    claimed: false,
    ready: () => installed,
    waitingFor: 'Nextcloud is still installing itself.',
  });
  await page.getByRole('button', { name: /^Continue$/ }).click();
  await page.waitForTimeout(300);
  assert.ok(await page.locator('input[name="new-password"]').isDisabled(), 'expected to start out waiting');
  installed = true;
  // The next poll comes after the 5 s interval; nothing is clicked.
  await page.locator('input[name="new-password"]:not([disabled])').waitFor({ timeout: 8000 });
  const text = await page.locator('body').innerText();
  assert.ok(/You can set the password now/i.test(text), `no "ready" line after the wait:\n${text}`);
  assert.ok(!/finish starting/i.test(text), 'the waiting panel is still up after the box became ready');
  await page.locator('input[name="new-password"]').fill('correct horse battery staple');
  await page.locator('input[name="confirm-password"]').fill('correct horse battery staple');
  await page.getByRole('button', { name: /^Set the password$/ }).click();
  await page.waitForTimeout(300);
  assert.ok(/Password set/i.test(await page.locator('body').innerText()), 'the claim after the wait did not go through');
  assert.deepStrictEqual(errors, []);
  await page.close();
});

/* The race: the last poll said ready, the submit landed a moment after the
 * pod went into maintenance. lososd answers 503 with the reason; the step
 * must read that as "not yet", not as a failure that stops the wizard. */
await check('a 503 from the claim itself sends step 2 back to waiting', async () => {
  const { page } = await open({ claimed: false, ready: true });
  await page.route('**/api/setup/claim', async (route) => {
    if (route.request().method() !== 'POST') return route.fallback();
    return route.fulfill({
      status: 503,
      contentType: 'application/json',
      headers: { 'Retry-After': '10' },
      body: JSON.stringify({ error: 'Nextcloud is in maintenance mode right now.', ready: false, waitingFor: 'Nextcloud is in maintenance mode right now.' }),
    });
  });
  await page.getByRole('button', { name: /^Continue$/ }).click();
  await page.locator('input[name="new-password"]').fill('correct horse battery staple');
  await page.locator('input[name="confirm-password"]').fill('correct horse battery staple');
  await page.getByRole('button', { name: /^Set the password$/ }).click();
  await page.waitForTimeout(400);
  const text = await page.locator('body').innerText();
  assert.ok(/not ready for the password yet/i.test(text), `the 503 is not explained:\n${text}`);
  assert.ok(/maintenance mode/.test(text), 'lososd\'s reason from the 503 is not shown');
  assert.ok(await page.getByRole('button', { name: /^Set the password$/ }).isDisabled(), 'the submit stayed enabled after a 503');
  assert.ok(!/stopped accepting|see the lososd journal/i.test(text), 'the 503 reads as a hard failure');
  await page.close();
});

/* Take 6 of the recorded install demo (2026-10-05): the claim's occ run
 * outlived the proxy's 60 s, the browser got a 504 with an HTML body, and
 * lososd finished anyway — box claimed, admin key in a reply nobody received.
 * lososd now answers the same password again for a while after a claim; the
 * step has to ask again on a lost reply rather than show "HTTP 504" over a
 * box the owner in fact just claimed. */
await check('a claim whose reply was lost is asked again, and the second answer is kept', async () => {
  const { page } = await open({ claimed: false, ready: true });
  let posts = 0;
  await page.route('**/api/setup/claim', async (route) => {
    if (route.request().method() !== 'POST') return route.fallback();
    posts += 1;
    if (posts === 1) {
      return route.fulfill({
        status: 504,
        contentType: 'text/html',
        body: '<html><body><h1>504 Gateway Time-out</h1><hr><center>nginx</center></body></html>',
      });
    }
    return route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({ claimed: true, user: 'notshared', token: '7'.repeat(64), replayed: true }),
    });
  });
  await page.getByRole('button', { name: /^Continue$/ }).click();
  await page.locator('input[name="new-password"]').fill('correct horse battery staple');
  await page.locator('input[name="confirm-password"]').fill('correct horse battery staple');
  await page.getByRole('button', { name: /^Set the password$/ }).click();
  await page.waitForTimeout(1500);
  let text = await page.locator('body').innerText();
  assert.ok(!/HTTP 504/.test(text), `the lost reply was shown as an error:\n${text}`);
  assert.strictEqual(posts, 1, 'the retry must wait, not hammer');
  await page.waitForTimeout(5000);
  text = await page.locator('body').innerText();
  assert.strictEqual(posts, 2, `expected the claim to be asked again once, saw ${posts}`);
  assert.ok(/notshared/.test(text), `the second answer did not finish the step:\n${text}`);
  assert.strictEqual(
    await page.evaluate(() => sessionStorage.getItem('losos-token')),
    '7'.repeat(64),
    'the token from the replayed reply was not kept',
  );
  await page.close();
});

/* A box someone else already claimed, or the same owner after the grace
 * window: lososd answers 409 with its sentence. That is an answer, so it is
 * shown once and not retried. */
await check('a 409 from the claim is shown and not asked again', async () => {
  const { page } = await open({ claimed: false, ready: true });
  let posts = 0;
  await page.route('**/api/setup/claim', async (route) => {
    if (route.request().method() !== 'POST') return route.fallback();
    posts += 1;
    return route.fulfill({
      status: 409,
      contentType: 'application/json',
      body: JSON.stringify({ error: 'this box has already been set up' }),
    });
  });
  await page.getByRole('button', { name: /^Continue$/ }).click();
  await page.locator('input[name="new-password"]').fill('correct horse battery staple');
  await page.locator('input[name="confirm-password"]').fill('correct horse battery staple');
  await page.getByRole('button', { name: /^Set the password$/ }).click();
  await page.waitForTimeout(6000);
  const text = await page.locator('body').innerText();
  assert.ok(/already been set up/.test(text), `the 409's sentence is not shown:\n${text}`);
  assert.strictEqual(posts, 1, `a refused claim must not be retried, saw ${posts} posts`);
  await page.close();
});

/* Step 4 on a new box, take 7 of the recorded install demo (2026-10-05): the
 * frame is opened a minute or two before the files app's web server is up,
 * so what it shows is nginx's "502 Bad Gateway" page. The old watcher read
 * "/nextcloud, not the login page" as a session and said "You are signed in"
 * over an error page nothing ever reloaded. The step now reloads the frame
 * until the files app answers, and counts only a page the app stamps with a
 * user as signed in. Take 8 then showed that 502 page in the frame for six
 * minutes; now the frame stays hidden, loading in the background behind a
 * quiet panel, until it holds a page of the app. */
await check('step 4 hides the frame and keeps reloading it until the files app answers, and signs in only on a real session', async () => {
  const { page } = await open({ claimed: false, ready: true });
  let hits = 0;
  await page.route('**/nextcloud', async (route) => {
    hits += 1;
    if (hits <= 2) {
      return route.fulfill({ status: 502, contentType: 'text/html', body: '<html><head><title>502 Bad Gateway</title></head><body><center><h1>502 Bad Gateway</h1></center><hr><center>nginx</center></body></html>' });
    }
    if (hits === 3) {
      // The login page: the app's token on <head>, no user.
      return route.fulfill({ status: 200, contentType: 'text/html', body: '<html><head data-requesttoken="tok"><title>Login</title></head><body><form><input id="user"><input id="password" type="password"><button id="go" type="button" onclick="location.href=\'/nextcloud/apps/files/\'">Log in</button></form></body></html>' });
    }
    return route.fulfill({ status: 200, contentType: 'text/html', body: '<html><head data-requesttoken="tok"><title>Files</title></head><body>should not be asked again</body></html>' });
  });
  await page.route('**/nextcloud/apps/files/', (route) =>
    route.fulfill({ status: 200, contentType: 'text/html', body: '<html><head data-requesttoken="tok" data-user="notshared"><title>Files</title></head><body><div id="app-content">files</div></body></html>' }),
  );
  await page.route('**/api/recovery', (route) =>
    route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify({ code: 'c5b6add9-e47a-43d5-87be-9b45e4a81441', minted: true }) }),
  );
  // Through steps 1 to 3.
  await page.getByRole('button', { name: /^Continue$/ }).click();
  await page.locator('input[name="new-password"]').fill('correct horse battery staple');
  await page.locator('input[name="confirm-password"]').fill('correct horse battery staple');
  await page.getByRole('button', { name: /^Set the password$/ }).click();
  await page.getByText('notshared').first().waitFor({ timeout: 5000 });
  await page.getByRole('button', { name: /^Continue$/ }).click();
  await page.getByRole('button', { name: 'Copy' }).click();
  await page.getByRole('button', { name: /^Continue$/ }).click();
  await page.locator('iframe').first().waitFor({ state: 'attached', timeout: 5000 });

  // On the 502: not signed in, the frame is not on screen (the error page
  // loads behind a panel that says the app is starting), the new-tab link
  // that would open the same error page is withheld, and the frame reloads.
  await page.waitForTimeout(1500);
  let text = await page.locator('body').innerText();
  assert.ok(!/You are signed in/.test(text), `an nginx error page counted as a session:\n${text}`);
  assert.ok(/still starting/.test(text), `the starting note is missing:\n${text}`);
  assert.ok(!(await page.locator('iframe').first().isVisible()), 'the frame showed the nginx error page');
  assert.ok(!(await page.getByRole('link', { name: /new tab/ }).isVisible()), 'the new-tab link was offered onto an error page');
  await page.waitForFunction(() => {
    const f = document.querySelector('iframe');
    return !!f && !!f.contentDocument && !!f.contentDocument.querySelector('#user');
  }, null, { timeout: 20_000 });
  assert.ok(hits >= 3, `the frame was not reloaded until the app answered (hits=${hits})`);
  // The login page itself is not a session either; the note is gone and the
  // frame is now on screen, with the new-tab link beside it.
  await page.waitForTimeout(1200);
  text = await page.locator('body').innerText();
  assert.ok(!/You are signed in/.test(text), 'the login page counted as a session');
  assert.ok(!/still starting/.test(text), 'the starting note stayed after the app answered');
  assert.ok(await page.locator('iframe').first().isVisible(), 'the frame stayed hidden after the app answered');
  assert.ok(await page.getByRole('link', { name: /new tab/ }).isVisible(), 'the new-tab link is missing once the app answered');
  assert.ok(await page.getByRole('button', { name: /^Finish/ }).isDisabled(), 'Finish enabled before any sign-in');
  // "Sign in" inside the frame: the app renders a page stamped with the user.
  await page.frameLocator('iframe').first().locator('#go').click();
  await page.getByText('You are signed in').waitFor({ timeout: 10_000 });
  assert.ok(!(await page.getByRole('button', { name: /^Finish/ }).isDisabled()), 'Finish still disabled after signing in');
  await page.close();
});

await browser.close();
await closeServer();
finish();
