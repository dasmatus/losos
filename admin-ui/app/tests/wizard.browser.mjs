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
 * Needs `bun run build` first and a browsers dir:
 *   PLAYWRIGHT_BROWSERS_PATH=$(nix build --print-out-paths --no-link \
 *     nixpkgs#playwright-driver.browsers)
 * nix/tests/admin-ui.nix sets all of that up; keep the `playwright` devDependency
 * pinned to the same version as nixpkgs' playwright-driver.
 */

import assert from './assert.mjs';
import { launch, runner, serve } from './harness.mjs';

const { origin, close: closeServer } = await serve();
const browser = await launch();

/* One page wired to a box in a given state. `claimed` is the whole point: it
 * is what decides between the wizard and the key prompt. */
async function open({
  claimed,
  tls = false,
  path = '/',
  ready = true,
  waitingFor = null,
  /* An origin to pretend the page is served from, with every request to it
   * answered by the real harness server. The harness listens on 127.0.0.1
   * with a port, and some of what the wizard shows depends on the page NOT
   * having one — a port is what a VM's forward looks like. */
  at = null,
  /* `tpm.enable`'s running value in GET /api/options, or null to leave the
   * route to the catch-all (no document). */
  tpm = null,
  viewport = { width: 1280, height: 900 },
}) {
  const page = await browser.newPage({ viewport, locale: 'en-US' });
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));

  if (at !== null) {
    await page.route(`${at}/**`, async (route) => {
      const url = new URL(route.request().url());
      /* A check closes its page once it has seen what it came for, often
       * with a request still in flight here. Closing disposes the fetched
       * response, fulfilling it then throws, and that rejection ended the
       * whole run. Nothing waits for such a request, so it is dropped. */
      try {
        const response = await route.fetch({ url: origin + url.pathname + url.search });
        await route.fulfill({ response });
      } catch (e) {
        if (!page.isClosed() && !/has been (disposed|closed)/.test(String(e))) throw e;
      }
    });
  }

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

  if (tpm !== null) {
    await page.route('**/api/options', (route) =>
      route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({
          available: true,
          version: 1,
          stray: [],
          excluded: {},
          options: [
            {
              name: 'tpm.enable',
              group: 'tpm',
              editor: { kind: 'bool' },
              nixType: 'boolean',
              description: '',
              default: true,
              defaultText: 'true',
              current: tpm,
              danger: false,
              fixed: 'installer',
              readOnly: true,
            },
          ],
        }),
      }),
    );
  }

  await page.route('**/setup/state.json', (route) =>
    route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({
        hostName: 'mattbox',
        fqdn: 'mattbox.local',
        /* What nginx substitutes per request: the box's LAN address. */
        address: BOX_ADDRESS,
        tls,
        /* The shape modules/setup.nix writes, installers included. */
        certificate: tls
          ? {
              url: '/setup/losos-ca.crt',
              fingerprint: 'sha256:' + 'ab'.repeat(32),
              fingerprintDisplay: 'AB:'.repeat(31) + 'AB',
              expires: '2028-01-01T00:00:00Z',
              install: { sh: '/setup/trust.sh', ps1: '/setup/trust.ps1' },
            }
          : null,
      }),
    }),
  );

  await page.goto((at ?? origin) + path, { waitUntil: 'networkidle' });
  return { page, errors };
}

/* The address modules/setup.nix fills into state.json: the box's own, as
 * the request reached it. */
const BOX_ADDRESS = '192.168.122.56';

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
    !/Sign in to this box|spare admin key/i.test(text),
    'a fresh box must never ask for the admin key: nothing on the appliance prints it',
  );
  await page.close();
});

await check('a box that already has an owner asks to sign in, not to set up', async () => {
  const { page } = await open({ claimed: true });
  const text = await page.locator('body').innerText();
  assert.ok(/Sign in to this box/i.test(text), 'expected the sign-in prompt on a claimed box');
  assert.ok(
    !/Trust this box/i.test(text),
    'showing setup to a returning owner reads as "this box has been wiped"',
  );
  await page.close();
});

/* The one-line installer: the line names the box by the IP address the box
 * reports (state.json `address`, which nginx fills in per request), on plain
 * http, whatever name the page itself was opened on — `mattbox.local` here,
 * which the next computer over may not resolve. The Windows switch has to
 * change both the fetch command and the script. */
await check('step 1 offers a one-line install command naming the box by its IP address', async () => {
  const { page, errors } = await open({ claimed: false, tls: true, at: 'http://mattbox.local' });
  const base = `http://${BOX_ADDRESS}`;
  const line = page.getByTestId('trust-command-line');
  const text = (await line.innerText()).trim();
  assert.strictEqual(
    text,
    `curl -fsSL ${base}/setup/trust.sh | sh`,
    `unexpected command; page errors: ${JSON.stringify(errors)}`,
  );
  const body = await page.locator('body').innerText();
  assert.ok(/Read the script first/i.test(body), 'the script must be offered to read before running');
  assert.ok(/Or by hand/i.test(body), 'the download stays as the manual route');
  const readLink = page.getByRole('link', { name: /Read the script first/i });
  assert.strictEqual(await readLink.getAttribute('href'), `${base}/setup/trust.sh`);

  await page.getByRole('radio', { name: 'Windows' }).click();
  assert.strictEqual(
    (await line.innerText()).trim(),
    `irm ${base}/setup/trust.ps1 | iex`,
    'the Windows choice must switch both the fetcher and the script',
  );
  assert.strictEqual(await readLink.getAttribute('href'), `${base}/setup/trust.ps1`);
  await page.close();
});

/* Behind a port forward (a VM's `localhost:8080`, or this harness) the
 * address the box sees on its side is not reachable from the page's, so the
 * page's own host and port are what the line has to carry. */
await check("behind a port forward the command keeps the page's own host and port", async () => {
  const { page } = await open({ claimed: false, tls: true });
  const text = (await page.getByTestId('trust-command-line').innerText()).trim();
  assert.strictEqual(text, `curl -fsSL ${origin}/setup/trust.sh | sh`);
  await page.close();
});

await check('a box without the installers offers the download only', async () => {
  const { page } = await open({ claimed: false, tls: true });
  // Re-stub with no `install` field, as a box on older software answers.
  await page.route('**/setup/state.json', (route) =>
    route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({
        hostName: 'mattbox',
        fqdn: 'mattbox.local',
        address: BOX_ADDRESS,
        tls: true,
        certificate: {
          url: '/setup/losos-ca.crt',
          fingerprint: 'sha256:' + 'ab'.repeat(32),
          fingerprintDisplay: 'AB:'.repeat(31) + 'AB',
          expires: '2028-01-01T00:00:00Z',
        },
      }),
    }),
  );
  await page.reload({ waitUntil: 'networkidle' });
  assert.strictEqual(await page.getByTestId('trust-command').count(), 0);
  const body = await page.locator('body').innerText();
  assert.ok(/Get the certificate/i.test(body), 'the download must still be there');
  assert.ok(!/Or by hand/i.test(body), '"by hand" makes no sense when there is no other way');
  await page.close();
});

/* The bug this exists for: step 1 predicted from the BOX's tls flag while step
 * 2 decided from the BROWSER's secure context, so on localhost step 1 promised
 * "the next step will not offer a passkey" and step 2 offered it. Both now read
 * passkeysPossibleHere(). The assertion is the agreement, not either value,
 * because which way it goes depends on where the test runs. */
await check('step 1 does not promise something step 2 contradicts', async () => {
  const { page } = await open({ claimed: false, tls: false });
  const step1 = await page.locator('body').innerText();
  const promisedNoPasskey = /next step will not offer a passkey/i.test(step1);

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
 * mid-flow, before the owner has copied the spare admin key that step 2 shows
 * exactly once. The owner would never see it and would not know. */
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
async function claimThroughStepTwo(password = 'Correct-horse battery staple 1', { tpm = null } = {}) {
  const { page, errors } = await open({ claimed: false, tpm });
  const requests = [];
  page.on('request', (req) => requests.push({ method: req.method(), url: new URL(req.url()).pathname }));

  await page.getByRole('button', { name: /^Continue$/ }).click();
  await page.locator('input[name="username"]').fill('matus');
  await page.locator('input[name="new-password"]').fill(password);
  await page.locator('input[name="confirm-password"]').fill(password);
  await page.getByRole('button', { name: /^Set the password$/ }).click();
  await page.waitForTimeout(300);

  const text = await page.locator('body').innerText();
  return { page, errors, requests, text };
}

/* A box installed without a TPM keeps its disk key on the boot partition.
 * The wizard says so once the claim has given it a token to ask with, and
 * not before; a box with a TPM, or one serving no option document, says
 * nothing. */
await check('the wizard warns about a missing TPM once the password is set', async () => {
  const notice = (page) => page.locator('[data-notice="no-tpm"]');
  const before = await open({ claimed: false, tpm: false });
  await before.page.getByRole('button', { name: /^Continue$/ }).waitFor();
  assert.equal(await notice(before.page).count(), 0, 'a TPM warning before the claim');
  await before.page.close();

  const keyfile = await claimThroughStepTwo(undefined, { tpm: false });
  await notice(keyfile.page).waitFor({ timeout: 3000 }).catch(() => assert.fail('no TPM warning after the claim on a keyfile box'));
  assert.match(await notice(keyfile.page).innerText(), /This box has no TPM chip/);
  assert.equal(
    await notice(keyfile.page).getByRole('link', { name: 'What a TPM does' }).getAttribute('href'),
    '/handbook/reference/tpm/',
  );
  assert.deepEqual(keyfile.errors, []);
  await keyfile.page.close();

  for (const tpm of [true, null]) {
    const { page } = await claimThroughStepTwo(undefined, { tpm });
    await page.waitForTimeout(300);
    assert.equal(await notice(page).count(), 0, `a TPM warning with tpm ${tpm}`);
    await page.close();
  }
});

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

/* The key is released exactly once, in the claim reply. It is the spare now
 * — the password unlocks the admin pages — but the spare is still the only
 * way in while LosOS Cloud is not running, and a wizard that stored it in
 * session storage and showed it to nobody made that a permanent lockout. */
await check('step 2 shows the spare admin key from the claim, since nothing else ever will', async () => {
  const { page, text } = await claimThroughStepTwo();
  assert.ok(text.includes('0'.repeat(64)), `the admin key is not on screen after the claim:\n${text}`);
  assert.ok(/spare admin key/i.test(text), 'the key is shown without saying what it is for');
  assert.ok(/you sign in to these admin pages on LosOS Login/i.test(text), 'step 2 does not say LosOS Login is the way in now');
  await page.close();
});

/* The four rules are on screen before a character is typed and tick off
 * as they are met, so a refusal never names a rule the owner has not seen.
 * One password now opens the admin pages too, which is why there are four
 * rules and not one; four is also why they still fit on screen. */
await check('step 2 shows the four password rules up front and ticks them as they are met', async () => {
  const { page } = await open({ claimed: false });
  await page.getByRole('button', { name: /^Continue$/ }).click();
  const rules = page.locator('[data-testid="password-rules"] li');
  assert.strictEqual(await rules.count(), 4, 'expected exactly four rules');
  const met = async () => rules.evaluateAll((items) => items.map((li) => li.dataset.met));
  assert.deepStrictEqual(await met(), ['false', 'false', 'false', 'false'], 'rules ticked before anything was typed');
  const field = page.locator('input[name="new-password"]');
  await field.fill('correct horse battery staple');
  assert.deepStrictEqual(await met(), ['true', 'false', 'false', 'false'], 'only the length should be met');
  await field.fill('Correct horse battery staple');
  assert.deepStrictEqual(await met(), ['true', 'true', 'false', 'false'], 'length and cases should be met');
  // A space is not a symbol: the fourth rule stays open until a real one.
  await field.fill('Correct horse battery staple 1');
  assert.deepStrictEqual(await met(), ['true', 'true', 'true', 'false'], 'a space must not count as a symbol');
  await field.fill('Correct-horse battery staple 1');
  assert.deepStrictEqual(await met(), ['true', 'true', 'true', 'true'], 'all four should be met');
  // The server's refusal has a local twin: a password that misses a rule is
  // refused next to the field, before any request, naming the rule.
  await page.locator('input[name="username"]').fill('matus');
  await field.fill('correct horse battery staple');
  await page.locator('input[name="confirm-password"]').fill('correct horse battery staple');
  await page.getByRole('button', { name: /^Set the password$/ }).click();
  await page.waitForTimeout(200);
  const text = await page.locator('body').innerText();
  assert.ok(/upper-case letter/i.test(text), `the missing rule is not named:\n${text}`);
  await page.close();
});

await check('claiming the box mid-wizard does not end the wizard', async () => {
  // Step 2 stores the token and flips the claim flag. A shell keyed on either
  // would swap the app in here, and the spare admin key would never be seen.
  const { page, text } = await claimThroughStepTwo();
  assert.ok(
    /Trust this box|sign in|spare admin key|Password set/i.test(text),
    'the wizard vanished once a token existed, before the spare key was shown',
  );
  assert.ok(!/Your board/i.test(text), 'the overview replaced the wizard mid-flow');
  await page.close();
});

await check('a reload after the claim comes back to step 2 with the spare key, not to the app', async () => {
  // The tab holds the token now, so without the wizard's own note the shell
  // would open the overview and the once-shown key would be gone.
  const { page } = await claimThroughStepTwo();
  await page.reload({ waitUntil: 'networkidle' });
  await page.waitForTimeout(300);
  const text = await page.locator('body').innerText();
  assert.ok(text.includes('0'.repeat(64)), `the spare key is gone after a reload:\n${text}`);
  assert.ok(!/Your board/i.test(text), 'a reload mid-setup opened the overview');
  await page.close();
});

/* When lososd says `ready: false` with a reason, step 2 waits on it rather
 * than let the owner submit into a box that cannot take a password yet. */
await check('step 2 waits with the reason on screen while the box is not ready, and never submits', async () => {
  const { page, errors } = await open({
    claimed: false,
    ready: false,
    waitingFor: 'LosOS Login is still starting. This happens once, on the first boot, and takes a minute.',
  });
  const requests = [];
  page.on('request', (req) => requests.push({ method: req.method(), url: new URL(req.url()).pathname }));
  await page.getByRole('button', { name: /^Continue$/ }).click();
  await page.waitForTimeout(400);
  const text = await page.locator('body').innerText();
  assert.ok(/finish starting/i.test(text), `no waiting panel while not ready:\n${text}`);
  assert.ok(/LosOS Login is still starting/.test(text), 'lososd\'s reason is not shown to the owner');
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
    waitingFor: 'LosOS Login is still starting.',
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
  await page.locator('input[name="username"]').fill('matus');
  await page.locator('input[name="new-password"]').fill('Correct-horse battery staple 1');
  await page.locator('input[name="confirm-password"]').fill('Correct-horse battery staple 1');
  await page.getByRole('button', { name: /^Set the password$/ }).click();
  await page.waitForTimeout(300);
  assert.ok(/Password set/i.test(await page.locator('body').innerText()), 'the claim after the wait did not go through');
  assert.deepStrictEqual(errors, []);
  await page.close();
});

/* The race: the last poll said ready, the submit landed a moment after the
 * box stopped being ready. lososd answers 503 with the reason; the step
 * must read that as "not yet", not as a failure that stops the wizard. */
/* On a busy box the first GET /api/setup/claim can take twelve seconds, and
 * until it answered step 2's form used to be open, with no panel, as if the
 * box were ready. A password typed then sat greyed out behind the waiting
 * panel that followed. Before the first answer the form is disabled and
 * nothing is shown. */
await check('step 2 keeps the form disabled until the first readiness answer, without flashing the panel', async () => {
  const { page } = await open({ claimed: false, ready: true });
  let release;
  const held = new Promise((r) => { release = r; });
  // Registered after open()'s routes, so it wins: the first answer waits
  // until the test lets it go.
  await page.route('**/api/setup/claim', async (route) => {
    if (route.request().method() !== 'GET') return route.fallback();
    await held;
    return route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify({ claimed: false, ready: true, waitingFor: null }) });
  });
  await page.getByRole('button', { name: /^Continue$/ }).click();
  const pw = page.locator('input[name="new-password"]');
  await pw.waitFor({ timeout: 5000 });
  assert.ok(await pw.isDisabled(), 'the form was open before the box had answered');
  assert.ok(await page.getByRole('button', { name: /^Set the password$/ }).isDisabled(), 'the submit was enabled before the box had answered');
  let text = await page.locator('body').innerText();
  assert.ok(!/finish starting/i.test(text), `the waiting panel flashed before any answer:\n${text}`);
  release();
  await page.locator('input[name="new-password"]:not([disabled])').waitFor({ timeout: 5000 });
  text = await page.locator('body').innerText();
  assert.ok(!/finish starting/i.test(text), 'the waiting panel is up on a ready box');
  await page.close();
});

await check('a 503 from the claim itself sends step 2 back to waiting', async () => {
  const { page } = await open({ claimed: false, ready: true });
  await page.route('**/api/setup/claim', async (route) => {
    if (route.request().method() !== 'POST') return route.fallback();
    return route.fulfill({
      status: 503,
      contentType: 'application/json',
      headers: { 'Retry-After': '10' },
      body: JSON.stringify({ error: 'LosOS Login is restarting right now.', ready: false, waitingFor: 'LosOS Login is restarting right now.' }),
    });
  });
  await page.getByRole('button', { name: /^Continue$/ }).click();
  await page.locator('input[name="username"]').fill('matus');
  await page.locator('input[name="new-password"]').fill('Correct-horse battery staple 1');
  await page.locator('input[name="confirm-password"]').fill('Correct-horse battery staple 1');
  await page.getByRole('button', { name: /^Set the password$/ }).click();
  await page.waitForTimeout(400);
  const text = await page.locator('body').innerText();
  assert.ok(/not ready for the password yet/i.test(text), `the 503 is not explained:\n${text}`);
  assert.ok(/restarting right now/.test(text), 'lososd\'s reason from the 503 is not shown');
  assert.ok(await page.getByRole('button', { name: /^Set the password$/ }).isDisabled(), 'the submit stayed enabled after a 503');
  assert.ok(!/stopped accepting|see the lososd journal/i.test(text), 'the 503 reads as a hard failure');
  await page.close();
});

/* On a box still warming up the claim's occ run once outlived the proxy's
 * 60 s: the browser got a 504 with an HTML body, and lososd finished
 * anyway — box claimed, admin key in a reply nobody received. lososd now
 * answers the same password again for a while after a claim; the step has to
 * ask again on a lost reply rather than show "HTTP 504" over a box the owner
 * in fact just claimed. */
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
  await page.locator('input[name="username"]').fill('matus');
  await page.locator('input[name="new-password"]').fill('Correct-horse battery staple 1');
  await page.locator('input[name="confirm-password"]').fill('Correct-horse battery staple 1');
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
  await page.locator('input[name="username"]').fill('matus');
  await page.locator('input[name="new-password"]').fill('Correct-horse battery staple 1');
  await page.locator('input[name="confirm-password"]').fill('Correct-horse battery staple 1');
  await page.getByRole('button', { name: /^Set the password$/ }).click();
  await page.waitForTimeout(6000);
  const text = await page.locator('body').innerText();
  assert.ok(/already been set up/.test(text), `the 409's sentence is not shown:\n${text}`);
  assert.strictEqual(posts, 1, `a refused claim must not be retried, saw ${posts} posts`);
  await page.close();
});

/* Steps 1 and 2, up to the Continue that opens step 3. */
async function throughStepTwo(page) {
  await page.getByRole('button', { name: /^Continue$/ }).click();
  await page.locator('input[name="username"]').fill('matus');
  await page.locator('input[name="new-password"]').fill('Correct-horse battery staple 1');
  await page.locator('input[name="confirm-password"]').fill('Correct-horse battery staple 1');
  await page.getByRole('button', { name: /^Set the password$/ }).click();
  await page.getByText('notshared').first().waitFor({ timeout: 5000 });
  await page.getByRole('button', { name: /^Continue$/ }).click();
}

/* Step 3 on a new box: the wizard signs the owner in to LosOS Login with the
 * password step 2 set, from this page (POST /auth/api/firstfactor), and the
 * tab swaps the spare key for that session. LosOS Login starts only after
 * the claim, so its first answers are nginx's 502. Once the tab is the
 * person's, the shell draws the desktop behind the card, out of reach, and
 * "Let's go" grows it into the window. LosOS Login's cookie is https only,
 * so these pages are served as https://mattbox.local. */
const HTTPS_BOX = 'https://mattbox.local';
const SPARE_ME = { name: null, displayName: null, groups: [], admin: true, spareKey: true, sections: ['storage'], standalone: true };
const PERSON_ME = { name: 'matus', displayName: 'Matus', groups: ['losos-admins'], admin: true, spareKey: false, sections: ['storage'], standalone: true, shared: [] };

/* LosOS Login on a box: `health` answers per call (true is up), the sign-in
 * takes `password` and sets the session, and /api/me tells the session from
 * the spare key by the Authorization header, as nginx and lososd do. */
async function login(page, { health = () => true, password = 'Correct-horse battery staple 1', me = SPARE_ME } = {}) {
  const seen = { signIns: [], session: false };
  await page.route('**/auth/api/health', (route) =>
    health()
      ? route.fulfill({ status: 200, contentType: 'application/json', body: '{"status":"OK"}' })
      : route.fulfill({ status: 502, contentType: 'text/html', body: '<html><head><title>502 Bad Gateway</title></head><body>nginx</body></html>' }),
  );
  await page.route('**/auth/api/firstfactor', (route) => {
    const body = JSON.parse(route.request().postData() ?? '{}');
    seen.signIns.push(body);
    if (body.username !== 'matus' || body.password !== password) {
      return route.fulfill({ status: 401, contentType: 'application/json', body: '{"status":"KO","message":"Authentication failed. Check your credentials."}' });
    }
    seen.session = true;
    return route.fulfill({ status: 200, contentType: 'application/json', body: '{"status":"OK"}' });
  });
  await page.route('**/api/me', (route) => {
    const bearer = route.request().headers()['authorization'] !== undefined;
    if (bearer) return route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(me) });
    if (!seen.session) return route.fulfill({ status: 401, contentType: 'application/json', body: '{"error":"unauthorized"}' });
    return route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(PERSON_ME) });
  });
  return seen;
}

const desktopStage = (page) => page.getByTestId('desktop').getAttribute('data-stage');

await check('step 3 waits for LosOS Login, signs in with the password from step 2, and shows the desktop behind the card', async () => {
  const { page, errors } = await open({ claimed: false, ready: true, at: HTTPS_BOX });
  let calls = 0;
  const seen = await login(page, { health: () => ++calls > 2 });
  await throughStepTwo(page);

  // nginx's 502 while LosOS Login starts: a note, no sign-in yet, no desktop.
  await page.getByText('LosOS Login is still starting').waitFor({ timeout: 5000 });
  assert.equal(seen.signIns.length, 0, 'signed in before LosOS Login answered');
  assert.equal(await page.getByTestId('desktop').count(), 0, 'the desktop is up before anyone is signed in');
  assert.ok(await page.getByRole('button', { name: "Let's go" }).isDisabled(), "Let's go is live before the sign-in");

  // LosOS Login answers: one sign-in, with the name and password of step 2.
  await page.getByTestId('desktop-ready').waitFor({ timeout: 10_000 });
  assert.deepEqual(seen.signIns, [{ username: 'matus', password: 'Correct-horse battery staple 1', keepMeLoggedIn: false }]);
  assert.equal(await page.evaluate(() => sessionStorage.getItem('losos-token')), null, 'the tab kept the spare key after the session');
  assert.ok(!/Correct-horse/.test(await page.evaluate(() => JSON.stringify(sessionStorage))), 'the password reached storage');
  assert.match(await page.getByTestId('desktop-ready').innerText(), /signed in as matus/);

  // The real desktop behind the card, out of reach, the card at its foot.
  assert.equal(await desktopStage(page), 'preview');
  assert.equal(await page.getByTestId('desktop').evaluate((el) => el.inert), true, 'the preview can be clicked');
  assert.equal(await page.getByTestId('wizard').getAttribute('data-docked'), 'true');
  await page.getByTestId('signed-in-avatar').waitFor({ timeout: 5000 });
  assert.ok(!(await page.getByRole('button', { name: "Let's go" }).isDisabled()));
  assert.deepEqual(errors, []);
  await page.close();
});

await check("Let's go grows the desktop into the window, signed in, with the tour over it", async () => {
  const { page, errors } = await open({ claimed: false, ready: true, at: HTTPS_BOX });
  await login(page);
  await throughStepTwo(page);
  await page.getByTestId('desktop-ready').waitFor({ timeout: 10_000 });
  await page.getByRole('button', { name: "Let's go" }).click();
  // Growing: the wizard on its way out and out of reach.
  assert.equal(await desktopStage(page), 'live');
  const tour = page.getByTestId('tour');
  await tour.getByRole('heading', { name: 'Your apps, on your box' }).waitFor({ timeout: 5000 });
  assert.equal(await page.getByTestId('wizard').count(), 0, 'the wizard is still on the page');
  assert.equal(await desktopStage(page), null);
  assert.equal(await page.getByTestId('desktop').evaluate((el) => el.inert), false);
  await page.getByTestId('signed-in-avatar').waitFor();
  // Not closed yet, so a reload brings it back rather than losing it. The
  // reload lands on the desktop by the session: setup is over.
  await page.reload({ waitUntil: 'networkidle' });
  await tour.waitFor({ timeout: 5000 });
  assert.equal(await page.getByTestId('wizard').count(), 0, 'a reload went back into setup');
  await tour.getByRole('button', { name: 'Skip the tour' }).click();
  await tour.waitFor({ state: 'detached', timeout: 3000 });
  assert.deepEqual(errors, []);
  await page.close();
});

await check("on a phone the desktop's bottom bar stays inside the preview, and Let's go is in reach", async () => {
  const { page, errors } = await open({ claimed: false, ready: true, at: HTTPS_BOX, viewport: { width: 390, height: 844 } });
  await login(page);
  await throughStepTwo(page);
  await page.getByTestId('desktop-ready').waitFor({ timeout: 10_000 });
  const bar = page.locator('[data-slot="bottom-nav"]');
  await bar.waitFor({ timeout: 5000 });
  // Drawn in <body> it would sit over the card, live, while the rest is a picture.
  assert.equal(await bar.evaluate((el) => el.closest('[data-testid="desktop"]') !== null), true, 'the bottom bar escaped the preview');
  await page.getByRole('button', { name: "Let's go" }).click({ timeout: 3000 });
  await page.getByTestId('tour').waitFor({ timeout: 5000 });
  assert.equal(await desktopStage(page), null);
  assert.deepEqual(errors, []);
  await page.close();
});

await check('a tab reloaded on step 3 asks for the password once more, and says when it is wrong', async () => {
  const { page } = await open({ claimed: false, ready: true, at: HTTPS_BOX });
  let up = false;
  const seen = await login(page, { health: () => up });
  await throughStepTwo(page);
  await page.getByText('LosOS Login is still starting').waitFor({ timeout: 5000 });
  // The password was in memory only: the reload forgets it.
  await page.reload({ waitUntil: 'networkidle' });
  up = true;
  await page.getByText('Type it once more to sign in', { exact: false }).waitFor({ timeout: 10_000 });
  assert.equal(seen.signIns.length, 0);
  const field = page.getByLabel('Password', { exact: true });
  await field.fill('not the one');
  await page.getByRole('button', { name: 'Sign in', exact: true }).click();
  await page.getByText('Wrong name or password.').waitFor({ timeout: 5000 });
  assert.equal(await field.getAttribute('aria-invalid'), 'true');
  assert.equal(await page.evaluate(() => sessionStorage.getItem('losos-token')), '0'.repeat(64), 'a refused password lost the spare key');
  await field.fill('Correct-horse battery staple 1');
  await page.getByRole('button', { name: 'Sign in', exact: true }).click();
  await page.getByTestId('desktop-ready').waitFor({ timeout: 10_000 });
  assert.equal(seen.signIns.length, 2);
  assert.equal(await desktopStage(page), 'preview');
  await page.close();
});

await check('on plain http the tab stays signed in with the spare key, and says why', async () => {
  const { page } = await open({ claimed: false, ready: true });
  const seen = await login(page);
  await throughStepTwo(page);
  const note = page.getByTestId('desktop-spare-key');
  await note.waitFor({ timeout: 10_000 });
  assert.match(await note.innerText(), /over https only/);
  assert.equal(seen.signIns.length, 0, 'tried LosOS Login over http');
  assert.equal(await page.evaluate(() => sessionStorage.getItem('losos-token')), '0'.repeat(64));
  assert.equal(await desktopStage(page), 'preview');
  assert.ok(!(await page.getByRole('button', { name: "Let's go" }).isDisabled()));
  await page.close();
});

await check('a box without LosOS Login opens with the spare key', async () => {
  const { page } = await open({ claimed: false, ready: true, at: HTTPS_BOX });
  const seen = await login(page, { me: { ...SPARE_ME, standalone: false } });
  await throughStepTwo(page);
  const note = page.getByTestId('desktop-spare-key');
  await note.waitFor({ timeout: 10_000 });
  assert.match(await note.innerText(), /no LosOS Login/);
  assert.equal(seen.signIns.length, 0);
  await page.close();
});

/* ── The shadcn conversion ─────────────────────────────────────────────
 * The notes in the wizard are shadcn Alerts, the OS picker a Toggle Group
 * and the password form two Fields; see tests/app.browser.mjs for the
 * same checks on the signed-in half. */

await check('the wizard notes are Alerts, each with a title and an icon', async () => {
  // The box has a certificate but this page reached it over plain http, which
  // is what the harness serves: the warning that the page is not encrypted.
  const { page } = await open({ claimed: false, tls: true });
  const alerts = page.locator('[data-slot="alert"]');
  assert.ok((await alerts.count()) >= 1, 'step 1 on an unencrypted page shows no alert');
  const unencrypted = alerts.filter({ hasText: 'not encrypted yet' });
  assert.equal(await unencrypted.getAttribute('role'), 'alert');
  assert.equal(await unencrypted.getAttribute('data-variant'), 'warn');
  assert.equal(await unencrypted.locator('[data-slot="alert-title"]').count(), 1);
  assert.equal(await unencrypted.locator('> svg').count(), 1, 'the tone icon is not the alert\'s first child');
  await page.close();
});

await check('the OS picker is a Toggle Group: arrow keys switch the command too', async () => {
  const { page } = await open({ claimed: false, tls: true, at: 'http://mattbox.local' });
  const picker = page.getByRole('radiogroup', { name: /operating system|which computer/i });
  await picker.waitFor();
  const unix = picker.getByRole('radio', { name: /macOS/ });
  assert.equal(await unix.getAttribute('aria-checked'), 'true');
  await unix.focus();
  await page.keyboard.press('ArrowRight');
  assert.equal(await picker.getByRole('radio', { name: 'Windows' }).getAttribute('aria-checked'), 'true');
  assert.match((await page.getByTestId('trust-command-line').innerText()).trim(), /^irm .*trust\.ps1 \| iex$/);
  await page.close();
});

await check('the form is the sign-in name, the password and its repeat as three Fields, and a refusal marks both passwords invalid with one announced error', async () => {
  const { page } = await open({ claimed: false });
  await page.getByRole('button', { name: /^Continue$/ }).click();
  const all = page.locator('form [data-slot="field"]');
  assert.equal(await all.count(), 3, 'expected the name, the password and its repeat as three fields');
  assert.equal(await all.first().locator('input[name="username"]').count(), 1, 'the sign-in name is not the first field');
  const fields = page.locator('form [data-slot="field"]:has(input[type="password"])');
  const group = fields.first().locator('[data-slot="input-group"]');
  assert.equal(await group.locator('input[name="new-password"]').count(), 1, 'the password is not inside the Input Group');
  assert.equal(await group.getByRole('button', { name: /show the password/i }).count(), 1, 'the eye is not inside the field');
  assert.equal(await fields.first().locator('[data-testid="password-rules"]').count(), 1, 'the four rules left the field');
  await page.locator('input[name="username"]').fill('matus');
  await page.locator('input[name="new-password"]').fill('correct horse battery staple');
  await page.locator('input[name="confirm-password"]').fill('correct horse battery staple');
  await page.getByRole('button', { name: /^Set the password$/ }).click();
  await page.waitForTimeout(200);
  assert.equal(await page.locator('form [data-slot="field"][data-invalid="true"]').count(), 2, 'both fields should be marked invalid');
  const error = page.locator('form [data-slot="field-error"]');
  assert.equal(await error.count(), 1, 'one error line, not one per field');
  assert.equal(await error.getAttribute('role'), 'alert');
  assert.match(await error.innerText(), /upper-case letter/i);
  await page.close();
});

await browser.close();
await closeServer();
finish();
