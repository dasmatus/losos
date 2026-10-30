// Real-browser render check: bundles tests/browser-entry.tsx (React included),
// mounts it in headless chromium via playwright, and asserts computed styles
// so broken tokens.css/losos.css wiring fails loudly. renderToStaticMarkup in
// render.test.tsx can't see CSS at all; this test exists for that gap.
//
// Needs `bun run build` first (reads dist/styles.css) and a browsers dir:
//   PLAYWRIGHT_BROWSERS_PATH=$(nix build --print-out-paths --no-link \
//     nixpkgs#playwright-driver.browsers)
// The nix check (nix/tests/design-system.nix) sets all of this up; keep the
// `playwright` devDependency version identical to nixpkgs' playwright-driver.
import { chromium } from 'playwright';

const bundle = await Bun.build({
  entrypoints: [`${import.meta.dir}/browser-entry.tsx`],
  target: 'browser',
  format: 'iife',
  define: { 'process.env.NODE_ENV': '"production"' },
});
if (!bundle.success) throw new AggregateError(bundle.logs, 'bundling tests/browser-entry.tsx failed');

const css = await Bun.file(`${import.meta.dir}/../dist/styles.css`).text();
// Inlined below, so a `</script` inside the bundle must not end the tag
// (esbuild escaped it on its own; Bun.build does not).
const js = (await bundle.outputs[0].text()).replaceAll('</script', '<\\/script');

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
  if (got !== want) throw new Error(`${sel} { ${prop} }: got ${got}, want ${want}`);
}

// The showcase must mount fully (React render errors surface as pageerror,
// not as a setContent failure).
if (pageErrors.length > 0) throw new Error(`page errors during render: ${JSON.stringify(pageErrors)}`);
if (!(await page.$('.auth-card'))) throw new Error('auth card not mounted');

await page.screenshot({ path: `${import.meta.dir}/../dist/.browser-test.png`, fullPage: true });
await browser.close();
console.log(`browser render tests: OK (${cases.length} computed-style assertions)`);
