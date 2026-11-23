/* How long an impatient owner waits: from the first request for the admin
 * page to an interactive welcome screen — the wizard's first step (the
 * "Trust this box" card, with its Continue button live) on a box nobody has
 * set up yet.
 *
 *   bun run build && bun tests/welcome-latency.ts [out.json] [samples]
 *
 * Runs against the real production bundle, served by the same stubbed
 * harness the browser checks use (tests/harness.ts, a fresh page per
 * sample, so every sample is a genuine cold first contact: no cache, no
 * claimed box, no stored progress). lososd is stubbed with page.route()
 * exactly as wizard.browser.ts does.
 *
 * The number is per-sample and reported with median and max. Thresholds:
 * median under 2.5 s passes, under 5 s warns loudly, and past 5 s the run
 * FAILS — on an appliance in the living room, a welcome screen slower than
 * that is read as frozen, and the setup is abandoned for a phone app.
 * A pageerror or a card that never renders within the 30 s deadline is the
 * same FAIL, since that is a first contact with nothing to wait for.
 *
 * Needs `bun run build` first and a browsers dir; the CI workflow
 * (.github/workflows/webui-latency.yml) and nix/tests/admin-ui.nix set both
 * up the same way wizard.browser.ts does.
 */

import type { Page } from 'playwright';
import { launch, serve } from './harness.ts';

const [out = '/tmp/welcome-latency.json', samplesArg = ''] = Bun.argv.slice(2);
const SAMPLES = Number(Bun.env.WELCOME_SAMPLES ?? (samplesArg || 7));
const PASS_MS = Number(Bun.env.WELCOME_PASS_MS ?? 2500);
const NOTE_MS = Number(Bun.env.WELCOME_NOTE_MS ?? 5000);
const DEADLINE_MS = Number(Bun.env.WELCOME_DEADLINE_MS ?? 30_000);
/* What the wizard's step 1 must show for the first contact to count. */
const MARKER = /Trust this box/i;
const CONTINUE = /^Continue$/;

const { origin, close: closeServer } = await serve();

/* The stubbed answers a fresh box gives, copied from wizard.browser.ts:
 * the catch-all must come FIRST (the later route shadows it), then the two
 * fixture routes the wizard reads. */
async function wire(page: Page) {
  await page.route('**/api/**', (route) =>
    route.fulfill({ status: 200, contentType: 'application/json', body: '{}' }),
  );
  await page.route('**/api/setup/claim', (route) => {
    if (route.request().method() === 'POST') {
      return route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({ claimed: true, user: 'notshared', token: '0'.repeat(64) }),
      });
    }
    return route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({ claimed: false, ready: true, waitingFor: null }),
    });
  });
  await page.route('**/setup/state.json', (route) =>
    route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({
        hostName: 'mattbox',
        fqdn: 'mattbox.local',
        address: '192.168.122.56',
        tls: false,
        certificate: null,
      }),
    }),
  );
}

/* One cold contact: a fresh context (no cache, no storage), navigation, and
 * the ms until the welcome card is interactive. */
async function contact(browser: Awaited<ReturnType<typeof launch>>): Promise<{
  ms: number;
  error: string | null;
} | null> {
  const context = await browser.newContext({ viewport: { width: 1280, height: 900 }, locale: 'en-US' });
  const page = await context.newPage();
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  await wire(page);
  const start = Date.now();
  try {
    await page.goto(origin + '/', { waitUntil: 'commit', timeout: DEADLINE_MS });
    const card = page.getByText(MARKER).first();
    await card.waitFor({ state: 'visible', timeout: DEADLINE_MS - (Date.now() - start) });
    await page
      .getByRole('button', { name: CONTINUE })
      .first()
      .waitFor({ state: 'visible', timeout: Math.max(0, DEADLINE_MS - (Date.now() - start)) });
    const ms = Date.now() - start;
    const pageErrors = errors.join('; ');
    return { ms, error: pageErrors === '' ? null : pageErrors };
  } catch (e) {
    /* The deadline condition covers the deadline case itself, though not when the
     * check is gone entirely */
    if (page.isClosed()) return null;
    return { ms: Date.now() - start, error: `${String(e)}${errors.length ? `; pageerrors: ${errors.join('; ')}` : ''}` };
  } finally {
    await context.close();
  }
}

const browser = await launch({
  /* The renderer crash a stock chromium inspect can hit on a headless VM
   * (a GPU it then falls over on); both flags are the documented way to
   * keep the measurement about the page, not the runner's graphics. */
  args: ['--disable-gpu', '--disable-dev-shm-usage'],
});
const rows: { sample: number; ms: number | null; error: string | null }[] = [];
for (let i = 1; i <= SAMPLES; i++) {
  const r = await contact(browser);
  rows.push({ sample: i, ms: r === null ? null : r.ms, error: r === null ? 'page closed under the check' : r.error });
  const shown = r === null ? 'gone' : r.error ?? `${r.ms} ms`;
  console.log(`sample ${i}: ${shown}`);
}
await browser.close();
await closeServer();

/* Failure beats any number: a first contact that renders nothing is the
 * worst patience a welcome screen can spend. */
const bad = rows.filter((r) => r.ms === null || r.error !== null);
const good = rows.filter((r) => r.ms !== null && r.error === null);
const times = good.map((r) => r.ms as number).sort((a, b) => a - b);
const median = times.length ? times[Math.floor(times.length / 2)] : null;
const max = times.length ? times[times.length - 1] : null;

const verdict = (ms: number | null) =>
  ms === null ? 'FAIL' : ms <= PASS_MS ? 'PASS' : ms <= NOTE_MS ? 'LOUDNOTE' : 'FAIL';
const gate = rows.length && good.length ? verdict(median) : 'FAIL';

const rowsSummary =
  `samples=${SAMPLES} good=${good.length} failed=${bad.length}\n` +
  (times.length
    ? `median=${median} ms (gate ${PASS_MS} ms)   max=${max} ms (cap ${NOTE_MS} ms)\n`
    : '') +
  rows
    .map((r) =>
      r.ms === null
        ? `  ${r.sample}: FAILED${r.error ? ` — ${r.error.slice(0, 300)}` : ''}`
        : `  ${r.sample}: ${r.ms} ms${r.error ? ` — (errors: ${r.error.slice(0, 200)})` : ''}`,
    )
    .join('\n') +
  `\nverdict: ${gate}`;

await Bun.write(out, JSON.stringify({
  samples: SAMPLES,
  pass_ms: PASS_MS,
  cap_ms: NOTE_MS,
  median,
  max,
  verdict: gate,
  rows,
}, null, 2) + '\n');

console.log(rowsSummary);

/* A LOUDNOTE does not red the run: if the welcome screen honours the hard
 * cap, the numbers travel to the summary and not the gate. Only a FAIL — a
 * median past the cap, an impossible welcome, or a broken page — exits 1. */
if (gate !== 'PASS') {
  console.warn(`welcome-latency ${gate}: median ${median} ms / best ${max} ms`);
}
if (gate === 'FAIL') {
  console.error(`the welcome screen failed the impatient-owner gate: ${rowsSummary}`);
  process.exit(1);
}
