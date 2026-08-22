// Real-browser render check: bundles tests/browser-entry.tsx (React included),
// mounts it in headless chromium via playwright, and asserts computed styles
// so broken tokens.css/losos.css wiring fails loudly. renderToStaticMarkup in
// render.test.tsx can't see CSS at all; this test exists for that gap.
//
// Needs `npm run build` first (reads dist/styles.css) and a browsers dir:
//   PLAYWRIGHT_BROWSERS_PATH=$(nix build --print-out-paths --no-link \
//     nixpkgs#playwright-driver.browsers)
// The nix check (tests/design-system.nix) sets all of this up; keep the
// `playwright` devDependency version identical to nixpkgs' playwright-driver.
import { build } from 'esbuild';
import { readFileSync } from 'node:fs';
import { chromium } from 'playwright';
import assert from 'node:assert';

await build({
  entryPoints: ['tests/browser-entry.tsx'],
  bundle: true,
  format: 'iife',
  jsx: 'automatic',
  outfile: 'dist/.browser-test.js',
});

const css = readFileSync('dist/styles.css', 'utf8');
const js = readFileSync('dist/.browser-test.js', 'utf8');

const browser = await chromium.launch({ chromiumSandbox: false });
const page = await browser.newPage({ viewport: { width: 1280, height: 720 } });
const pageErrors = [];
page.on('pageerror', (e) => pageErrors.push(String(e)));
await page.setContent(
  `<!doctype html><html><head><meta charset="utf-8"><style>${css}</style></head>` +
  `<body><div id="root"></div><script>${js}</script></body></html>`,
  { waitUntil: 'load' },
);
try {
  await page.waitForSelector('.topbar');
} catch (e) {
  // A React render error leaves the root empty and would otherwise surface
  // only as this timeout, so attach what actually went wrong.
  throw new Error(`showcase never mounted; page errors: ${JSON.stringify(pageErrors)}`, { cause: e });
}

async function computed(selector, prop) {
  return page.$eval(
    selector,
    (el, p) => getComputedStyle(el).getPropertyValue(p),
    prop,
  );
}

// One assertion per token-driven surface; expected values are the literals
// tokens.css defines (chromium reports them as rgb()).
const cases = [
  ['body', 'background-color', 'rgb(245, 247, 250)'],            // --ls-bg
  ['.topbar', 'background-color', 'rgb(28, 33, 40)'],            // --ls-ink
  ['.btn-primary', 'background-color', 'rgb(31, 111, 235)'],     // --ls-accent
  ['.dot[data-state="up"]', 'background-color', 'rgb(46, 204, 113)'],   // --ls-success
  ['.dot[data-state="down"]', 'background-color', 'rgb(229, 83, 75)'],  // --ls-danger
  ['.banner[data-kind="building"] .indbar', 'display', 'block'], // building reveals indbar
  ['.switch input:checked + .switch-track', 'background-color', 'rgb(46, 204, 113)'],
  ['.card', 'border-radius', '8px'],                             // --ls-radius-md
  ['.chip', 'border-radius', '6px'],                             // --ls-radius-sm
  ['.progress-card[data-kind="done"] .spinner', 'color', 'rgb(46, 204, 113)'],
];
for (const [sel, prop, want] of cases) {
  const got = (await computed(sel, prop)).trim();
  assert.strictEqual(got, want, `${sel} { ${prop} }`);
}

// The showcase must mount fully (React render errors surface as pageerror,
// not as a setContent failure).
assert.deepStrictEqual(pageErrors, [], 'page errors during render');
assert.ok(await page.$('.auth-card'), 'auth card mounted');

await page.screenshot({ path: 'dist/.browser-test.png', fullPage: true });
await browser.close();
console.log(`browser render tests: OK (${cases.length} computed-style assertions)`);
