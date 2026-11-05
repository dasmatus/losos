/* Shared plumbing for the browser checks: a static server for dist/ with the
 * SPA fallback nginx gives the admin location, a chromium launcher, and a tiny
 * check runner. Each *.browser.ts file is its own process so one file's
 * failure cannot hide another's. */

import { chromium, type Browser, type ElementHandle, type Locator, type Page, type Route } from 'playwright';

const DIST = 'dist';
const TYPES: Record<string, string> = {
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

/* LosOS Chat's policy (/chat/, modules/containers.nix `chatCsp`): the Lab's
 * 'wasm-unsafe-eval' for the end-to-end crypto, and blob: for decrypted
 * pictures and voice messages. Mirror nginx's /chat/ arm here. */
export const CHAT_CSP = [
  "default-src 'none'",
  "script-src 'self' 'wasm-unsafe-eval'",
  "style-src 'self'",
  "img-src 'self' data: blob:",
  "media-src 'self' blob:",
  "font-src 'self'",
  "connect-src 'self'",
  "frame-ancestors 'none'",
  "base-uri 'none'",
  "form-action 'none'",
].join('; ');

/* The app frame's own policy (modules/containers.nix, `appFrameCsp`), for
 * /app-frame/ alone. An app of the owner's runs there under
 * `sandbox="allow-scripts allow-forms"`: the runtime is the box's own
 * script ('self'), the app is an inline script the runtime adds. Mirror
 * any change here. */
export const APP_FRAME_CSP = [
  "default-src 'none'",
  "script-src 'self' 'unsafe-inline'",
  "style-src 'self' 'unsafe-inline'",
  "img-src data: blob: https: http:",
  "font-src 'self' data: https: http:",
  "media-src data: blob: https: http:",
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
export interface ShellMe {
  name: string;
  displayName: string;
  groups: string[];
  admin: boolean;
  spareKey: boolean;
  sections: string[];
  shared: string[];
}
export const SHELL_ME: ShellMe = {
  name: 'matus',
  displayName: 'Matus',
  groups: ['losos-admins'],
  admin: true,
  spareKey: false,
  sections: ['storage', 'lab'],
  shared: [],
};
export async function shellRoutes(page: Page, me: ShellMe = SHELL_ME): Promise<void> {
  const json = (route: Route, status: number, body: unknown) =>
    route.fulfill({ status, contentType: 'application/json', body: JSON.stringify(body) });
  await page.route('**/api/**', (route) =>
    new URL(route.request().url()).pathname.startsWith('/api/lab')
      ? json(route, 401, { error: 'unauthorized' })
      : json(route, 200, {}),
  );
  await page.route('**/api/setup/claim', (route) => json(route, 200, { claimed: true }));
  await page.route('**/api/me', (route) => json(route, 200, me));
}

/* WebSockets a check answers itself, as page.routeWebSocket would, but on the
 * page's own server: Playwright's mocked socket sends binary frames in a form
 * that never round-trips a Uint8Array there and back. `handler` gets each
 * connection from `page` whose ws:// URL `match` tests true, the last route
 * registered first, while the page is open; its connection has url(), send(),
 * onMessage(), onClose() and close(). Register before the page navigates: the
 * page tags its sockets with its own id from an init script, so two pages on
 * one path each reach their own handler. */
export interface FakeSocket {
  url(): string;
  send(message: string | Uint8Array): void;
  onMessage(handler: (message: string | Uint8Array) => void): void;
  onClose(handler: () => void): void;
  close(): void;
}
const SOCKETS: { page: string | null; match: RegExp; handler: (socket: FakeSocket) => void }[] = [];
const PAGE_IDS = new WeakMap<Page, string>();
const TAG = 'losos-check-page';
export async function routeWebSocket(page: Page, match: RegExp, handler: (socket: FakeSocket) => void): Promise<void> {
  if (!PAGE_IDS.has(page)) {
    const id = crypto.randomUUID();
    PAGE_IDS.set(page, id);
    await page.addInitScript(
      (arg: string[]) => {
        const [tag, id] = arg as [string, string];
        const Native = window.WebSocket;
        window.WebSocket = class extends Native {
          constructor(url: string | URL, protocols?: string | string[]) {
            const tagged = new URL(url, location.href);
            tagged.searchParams.set(tag, id);
            super(tagged, protocols);
          }
        } as typeof window.WebSocket;
      },
      [TAG, id],
    );
  }
  const route = { page: PAGE_IDS.get(page) ?? null, match, handler };
  SOCKETS.push(route);
  page.once('close', () => SOCKETS.splice(SOCKETS.indexOf(route), 1));
}

interface WsData {
  url: string;
  route: (typeof SOCKETS)[number];
  heard: ((message: string | Uint8Array) => void)[];
  closed: (() => void)[];
}

function upgrade(req: Request, server: Bun.Server<WsData>): Response | undefined {
  const tagged = new URL(req.url.replace(/^http/, 'ws'));
  const page = tagged.searchParams.get(TAG);
  tagged.searchParams.delete(TAG);
  const url = tagged.href;
  const route = SOCKETS.findLast((r) => r.page === page && r.match.test(url));
  if (route === undefined) return new Response('no socket here', { status: 404 });
  // The page names its subprotocols; the first is the one it gets.
  const protocol = req.headers.get('sec-websocket-protocol')?.split(',')[0]?.trim();
  const headers = new Headers();
  if (protocol) headers.set('Sec-WebSocket-Protocol', protocol);
  const data: WsData = { url, route, heard: [], closed: [] };
  if (server.upgrade(req, { headers, data })) return undefined;
  return new Response('not a WebSocket', { status: 400 });
}

const SOCKET_EVENTS: Bun.WebSocketHandler<WsData> = {
  open(ws) {
    const data = ws.data;
    data.route.handler({
      url: () => data.url,
      send: (message) => ws.send(message as string),
      onMessage: (f) => data.heard.push(f),
      onClose: (f) => data.closed.push(f),
      close: () => ws.close(),
    });
  },
  message(ws, message) {
    const m: string | Uint8Array = typeof message === 'string' ? message : new Uint8Array(message);
    // eslint-disable-next-line @typescript-eslint/no-non-null-assertion
    for (const f of ws.data.heard) f(m);
  },
  close(ws) {
    for (const f of ws.data.closed) f();
  },
};

/* Serves dist/ with an SPA fallback (`try_files $uri /index.html`). Without it
 * a deep link 404s and the test would be asserting against an error page. */
export interface ServeOptions {
  csp?: boolean;
  dist?: string;
}
export interface ServedApp {
  origin: string;
  close(): Promise<void>;
}
export async function serve({ csp = false, dist = DIST }: ServeOptions = {}): Promise<ServedApp> {
  const send = async (file: string) => {
    const body = Bun.file(`${dist}/${file.replace(/^\/+/, '')}`);
    if (!(await body.exists())) return null;
    const policy = file.startsWith('/widget-frame/')
      ? WIDGET_FRAME_CSP
      : file.startsWith('/app-frame/')
        ? APP_FRAME_CSP
        : file.startsWith('/manage/') || file === 'manage/index.html'
          ? MANAGE_CSP
          : file.startsWith('/chat/') || file === 'chat/index.html'
            ? CHAT_CSP
          : ADMIN_CSP;
    return new Response(body, {
      headers: {
        'Content-Type': TYPES[file.match(/\.[^./]*$/)?.[0] ?? ''] ?? 'application/octet-stream',
        ...(csp ? { 'Content-Security-Policy': policy } : {}),
      },
    });
  };
  const server = Bun.serve({
    hostname: '127.0.0.1',
    port: 0,
    websocket: SOCKET_EVENTS,
    async fetch(req, server) {
      if (req.headers.get('upgrade')?.toLowerCase() === 'websocket') return upgrade(req, server) ?? undefined;
      // URL parsing already resolves every `..`, so nothing leaves dist/.
      const rel = decodeURIComponent(new URL(req.url).pathname);
      // A directory gets its index, as nginx's `index` directive gives it.
      const file = rel === '/' ? 'index.html' : rel.endsWith('/') ? rel + 'index.html' : rel;
      // The edge's /manage/, LosOS Cloud's /cloud/, a knowledge base's /kb/,
      // LosOS Git's /git/, LosOS Wiki's /wiki/ and LosOS Chat's /chat/ are pages of their own with
      // their own fallback, as their nginx locations are; /diagrams is the SPA's.
      const page = ['manage', 'cloud', 'kb', 'git', 'wiki', 'chat'].find((p) => rel.startsWith(`/${p}/`) || rel === `/${p}`);
      const fallback = page === undefined ? 'index.html' : `${page}/index.html`;
      return (await send(file)) ?? (await send(fallback)) ?? new Response('not found', { status: 404 });
    },
  });
  return {
    origin: `http://127.0.0.1:${server.port}`,
    close: async () => server.stop(true),
  };
}

/* Playwright takes a binary answer in its own Buffer shape, and a Uint8Array
 * never answers. So the checks hand it a file instead: `stash` writes the
 * bytes to a fresh directory, for `route.fulfill({ path })`. */
const SCRATCH = `${Bun.env.TMPDIR ?? '/tmp'}/losos-checks-${crypto.randomUUID()}`;
let stashed = 0;
export async function stash(name: string, bytes: string | ArrayBuffer | ArrayBufferView): Promise<string> {
  const path = `${SCRATCH}/${++stashed}/${name}`;
  await Bun.write(path, bytes as string);
  return path;
}

/* Picks files in a file input (a locator or an element handle) the way a
 * person's choice reaches the page: Files in memory, then input and change.
 * setInputFiles takes the bytes in Playwright's own Buffer shape, and a path
 * makes a file Chromium reads from disk, which a request route then sees no
 * body of. Each file is { name, bytes, type? }, bytes a string or a
 * Uint8Array. */
export interface PickedFile {
  name: string;
  bytes: string | Uint8Array;
  type?: string;
}
interface SentFile {
  name: string;
  type: string;
  bytes: number[];
}
export async function pickFiles(input: Locator | ElementHandle<Element>, files: PickedFile[]): Promise<void> {
  const sent: SentFile[] = files.map(({ name, bytes, type = '' }) => ({
    name,
    type,
    bytes: [...(typeof bytes === 'string' ? new TextEncoder().encode(bytes) : bytes)],
  }));
  await (input as Locator).evaluate<Element | HTMLInputElement | null, SentFile[]>((el: Element, picked: SentFile[]) => {
    const list = new DataTransfer();
    for (const f of picked) list.items.add(new File([new Uint8Array(f.bytes)], f.name, { type: f.type }));
    (el as HTMLInputElement).files = list.files;
    el.dispatchEvent(new Event('input', { bubbles: true }));
    el.dispatchEvent(new Event('change', { bubbles: true }));
    return el;
  }, sent);
}

/* LOSOS_CHROMIUM points at a chromium binary when the browsers Playwright
 * expects are not installed (a dev machine or CI image with a different
 * revision). Unset, Playwright uses PLAYWRIGHT_BROWSERS_PATH as before, which
 * is what nix/tests/admin-ui.nix relies on. */
export function launch({ args = [] as string[] }: { args?: string[] } = {}): Promise<Browser> {
  const executablePath: string | undefined = Bun.env.LOSOS_CHROMIUM || undefined;
  return chromium.launch({ chromiumSandbox: false, executablePath, args });
}

export interface CheckRunner {
  check(name: string, fn: () => void | Promise<void>): Promise<void>;
  skip(name: string, reason: string): void;
  finish(): void;
}
export function runner(): CheckRunner {
  const failures: string[] = [];
  // LOSOS_ONLY=<text> runs only the checks whose name contains it.
  const only: string | undefined = Bun.env.LOSOS_ONLY;
  const check = async (name: string, fn: () => void | Promise<void>) => {
    if (only && !name.includes(only)) return;
    try {
      await fn();
      console.log(`  ok   ${name}`);
    } catch (e) {
      failures.push(`${name}: ${(e as Error).message}`);
      console.log(`  FAIL ${name}\n       ${(e as Error).message}`);
    }
  };
  /* A check kept for a feature that is not reachable yet. It prints so a run
   * shows what it is not covering, rather than the coverage going quietly. */
  const skip = (name: string, reason: string) => {
    console.log(`  skip ${name} (${reason})`);
  };
  const finish = () => {
    if (failures.length > 0) {
      // Thrown, not printed: an uncaught error is what makes Bun exit non-zero.
      throw new Error(`${failures.length} failed:\n${failures.map((f) => `  - ${f}`).join('\n')}`);
    }
    console.log('\nall browser checks passed');
  };
  return { check, skip, finish };
}
