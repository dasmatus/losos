/* Shared plumbing for the browser checks: a static server for dist/ with the
 * SPA fallback nginx gives the admin location, a chromium launcher, and a tiny
 * check runner. Each *.browser.mjs file is its own process so one file's
 * failure cannot hide another's. */

import { createServer } from 'node:http';
import { readFile } from 'node:fs/promises';
import { extname, join, normalize } from 'node:path';
import { chromium } from 'playwright';

const DIST = 'dist';
const TYPES = {
  '.html': 'text/html; charset=utf-8',
  '.js': 'text/javascript; charset=utf-8',
  '.css': 'text/css; charset=utf-8',
  '.json': 'application/json',
  '.svg': 'image/svg+xml',
};

/* Serves dist/ with an SPA fallback (`try_files $uri /index.html`). Without it
 * a deep link 404s and the test would be asserting against an error page. */
export async function serve() {
  const server = createServer((req, res) => {
    const url = new URL(req.url, 'http://127.0.0.1');
    const rel = normalize(url.pathname).replace(/^(\.\.[/\\])+/, '');
    const send = async (file) => {
      const body = await readFile(join(DIST, file));
      res.writeHead(200, { 'Content-Type': TYPES[extname(file)] ?? 'application/octet-stream' });
      res.end(body);
    };
    send(rel === '/' ? 'index.html' : rel).catch(() =>
      send('index.html').catch(() => {
        res.writeHead(404);
        res.end('not found');
      }),
    );
  });
  await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
  return {
    origin: `http://127.0.0.1:${server.address().port}`,
    close: () => new Promise((resolve) => server.close(resolve)),
  };
}

/* LOSOS_CHROMIUM points at a chromium binary when the browsers Playwright
 * expects are not installed (a dev machine or CI image with a different
 * revision). Unset, Playwright uses PLAYWRIGHT_BROWSERS_PATH as before, which
 * is what tests/admin-ui.nix relies on. */
export function launch() {
  const executablePath = process.env.LOSOS_CHROMIUM || undefined;
  return chromium.launch({ chromiumSandbox: false, executablePath });
}

export function runner() {
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
  /* A check kept for a feature that is not reachable yet. It prints so a run
   * shows what it is not covering, rather than the coverage going quietly. */
  const skip = (name, reason) => {
    console.log(`  skip ${name} (${reason})`);
  };
  const finish = () => {
    if (failures.length > 0) {
      console.error(`\n${failures.length} failed:\n${failures.map((f) => `  - ${f}`).join('\n')}`);
      process.exit(1);
    }
    console.log('\nall browser checks passed');
  };
  return { check, skip, finish };
}
