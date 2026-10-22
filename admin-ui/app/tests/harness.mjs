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
  '.png': 'image/png',
  '.jpg': 'image/jpeg',
  '.wasm': 'application/wasm',
};

/* The admin vhost's Content-Security-Policy (modules/containers.nix,
 * `adminCsp`), up to frame-src, which the board needs for /widget-frame/.
 * LosOS Diagrams at /diagrams runs under it too.
 * A check that passes `csp: true` to serve() gets its pages under it, so a
 * library that styles itself with an injected <style> element, or a
 * component that sets a style attribute, fails here before it fails on the
 * box. */
export const ADMIN_CSP = [
  "default-src 'none'",
  "script-src 'self' 'wasm-unsafe-eval'",
  "style-src 'self'",
  "img-src 'self' data: blob:",
  "font-src 'self'",
  "connect-src 'self'",
  "frame-src 'self'",
].join('; ');

/* The widget frame's own policy (modules/containers.nix, `widgetFrameCsp`),
 * served for /widget-frame/ alone the way nginx's header map does it. A
 * widget written by hand runs there under `sandbox="allow-scripts"`; the
 * admin page keeps the strict policy above. Mirror any change here. */
export const WIDGET_FRAME_CSP = [
  "default-src 'none'",
  "script-src 'unsafe-inline' 'unsafe-eval'",
  "style-src 'unsafe-inline'",
  "img-src data: https: http:",
  "font-src data: https: http:",
  "media-src data: https: http:",
  "connect-src https: http:",
  "frame-ancestors 'self'",
  "base-uri 'none'",
  "form-action 'none'",
].join('; ');

/* A mail message's HTML (backend/src/http.rs MAIL_FRAME_CSP, the same
 * text as nginx's mail arm in modules/containers.nix). Mirror it here. */
export const MAIL_FRAME_CSP = [
  'sandbox allow-popups allow-popups-to-escape-sandbox',
  "default-src 'none'",
  "style-src 'unsafe-inline'",
  'img-src data:',
  'font-src data:',
  "frame-ancestors 'self'",
  "base-uri 'none'",
  "form-action 'none'",
].join('; ');

/* A company edge's manage page (/manage/, served by the edge itself:
 * PAGE_CSP in edge/registrar/src/server/manage.rs). Mirror it here. */
export const MANAGE_CSP = [
  "default-src 'none'",
  "script-src 'self'",
  "style-src 'self'",
  "img-src 'self' data:",
  "font-src 'self'",
  "connect-src 'self'",
  "manifest-src 'self'",
  "base-uri 'none'",
  "form-action 'none'",
  "frame-ancestors 'none'",
].join('; ');

/* The admin shell around a screen: someone signed in with LosOS Login (an
 * administrator, so every sidebar entry shows) on a claimed box, and empty
 * answers for what the shell reads on the side (pins, the look). Call it
 * before a check's own routes, which then win. /api/lab/* stays refused,
 * as lososd refuses it without the key, so Diagrams' probes find no helper. */
export const SHELL_ME = {
  name: 'matus',
  displayName: 'Matus',
  groups: ['losos-admins'],
  admin: true,
  spareKey: false,
  sections: ['storage', 'lab'],
  shared: [],
};
export async function shellRoutes(page, me = SHELL_ME) {
  const json = (route, status, body) =>
    route.fulfill({ status, contentType: 'application/json', body: JSON.stringify(body) });
  await page.route('**/api/**', (route) =>
    new URL(route.request().url()).pathname.startsWith('/api/lab')
      ? json(route, 401, { error: 'unauthorized' })
      : json(route, 200, {}),
  );
  await page.route('**/api/setup/claim', (route) => json(route, 200, { claimed: true }));
  await page.route('**/api/me', (route) => json(route, 200, me));
}

/* Serves dist/ with an SPA fallback (`try_files $uri /index.html`). Without it
 * a deep link 404s and the test would be asserting against an error page. */
export async function serve({ csp = false } = {}) {
  const server = createServer((req, res) => {
    const url = new URL(req.url, 'http://127.0.0.1');
    const rel = normalize(url.pathname).replace(/^(\.\.[/\\])+/, '');
    const send = async (file) => {
      const body = await readFile(join(DIST, file));
      const policy = file.startsWith('/widget-frame/')
        ? WIDGET_FRAME_CSP
        : file.startsWith('/manage/') || file === 'manage/index.html'
          ? MANAGE_CSP
          : ADMIN_CSP;
      res.writeHead(200, {
        'Content-Type': TYPES[extname(file)] ?? 'application/octet-stream',
        ...(csp ? { 'Content-Security-Policy': policy } : {}),
      });
      res.end(body);
    };
    // A directory gets its index, as nginx's `index` directive gives it.
    const file = rel === '/' ? 'index.html' : rel.endsWith('/') ? rel + 'index.html' : rel;
    // The edge's /manage/, LosOS Cloud's /cloud/ and a knowledge base's /kb/
    // are pages of their own with their own fallback, as their nginx
    // locations are; /diagrams is the SPA's.
    const page = ['manage', 'cloud', 'kb'].find((p) => rel.startsWith(`/${p}/`) || rel === `/${p}`);
    const fallback = page === undefined ? 'index.html' : `${page}/index.html`;
    send(file).catch(() =>
      send(fallback).catch(() => {
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
 * is what nix/tests/admin-ui.nix relies on. */
export function launch({ args = [] } = {}) {
  const executablePath = process.env.LOSOS_CHROMIUM || undefined;
  return chromium.launch({ chromiumSandbox: false, executablePath, args });
}

export function runner() {
  const failures = [];
  // LOSOS_ONLY=<text> runs only the checks whose name contains it.
  const only = process.env.LOSOS_ONLY;
  const check = async (name, fn) => {
    if (only && !name.includes(only)) return;
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
