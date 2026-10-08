/* LosOS Lab (/lab/, the second page of this bundle) under its own policy.
 *
 * The real dist/ bundle and the real simulator (src/lab/core-pkg, built by
 * `npm run lab:core`), served with LAB_CSP the way nginx serves /lab/. Every
 * page here fails on a CSP violation and on an uncaught error, so a library
 * that injects a <style> or a component that writes a style attribute turns
 * this red before it reaches a box. What a person does with the Lab is
 * walked once each: a built-in setup, This box signed in and signed out,
 * placing, dragging, wiring and deleting a device, Save then Open, a
 * simulation step, the physical view, a console, and the phone layout.
 *
 * The libvirt checks stand a fake `losos-registrar lab` behind /api/lab/
 * (page.route for the HTTP half, page.routeWebSocket for the console and
 * NIC sockets), the way lososd relays the real one on a box. The virt-rpc
 * checks add the relay path: a ticket, and a fake libvirt daemon behind the
 * relay socket that the page's WebAssembly client really drives, under the
 * same CSP (so the client's own module and its 'wasm-unsafe-eval' are
 * covered too).
 */

import assert from 'node:assert';
import { readFile } from 'node:fs/promises';
import { launch, runner, serve } from './harness.mjs';

const TOKEN = 'b'.repeat(64);
const { origin, close: closeServer } = await serve({ csp: true });
const browser = await launch();
const { check, finish } = runner();

const json = (route, status, body) =>
  route.fulfill({ status, contentType: 'application/json', body: JSON.stringify(body) });

const SETTINGS = {
  sharingMyStorage: false,
  nextcloudMode: 'container',
  forgejoMode: 'container',
  hostName: 'mattbox',
  https: false,
  gpuEnable: false,
  apachePort: 80,
  proxyEnable: false,
  clusterEnable: false,
  shareCompute: false,
  computeWindowStart: '23:00',
  computeWindowEnd: '07:00',
};
const EDGE = {
  reachable: true,
  official: true,
  edges: [{ name: 'edge demo', url: 'http://edge.local:8443', source: 'lan', official: true }],
  lanSearched: true,
  configuredUrl: 'https://losos-edge.dasmat.us',
  checkedAt: 1760000000,
};

/* A Lab page: the API refuses without the key, as lososd does. `signedIn`
 * puts the key in this tab's sessionStorage, as the admin page leaves it. */
async function open({ viewport = { width: 1440, height: 900 }, locale = 'en-US', signedIn = false, scheme = 'light', routes = null } = {}) {
  const page = await browser.newPage({ viewport, locale, colorScheme: scheme, acceptDownloads: true });
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  await page.addInitScript(() => {
    window.__violations = [];
    document.addEventListener('securitypolicyviolation', (e) =>
      window.__violations.push(`${e.violatedDirective} ${e.blockedURI}`),
    );
  });
  if (signedIn) await page.addInitScript((t) => window.sessionStorage.setItem('losos-token', t), TOKEN);
  const authed = (route) => route.request().headers()['authorization'] === `Bearer ${TOKEN}`;
  await page.route('**/api/**', (route) => json(route, 401, { error: 'unauthorized' }));
  await page.route('**/api/settings', (route) => (authed(route) ? json(route, 200, SETTINGS) : json(route, 401, {})));
  await page.route('**/api/edge', (route) => (authed(route) ? json(route, 200, EDGE) : json(route, 401, {})));
  if (routes) await routes(page, authed);
  await page.goto(origin + '/lab/');
  await page.getByTestId('lab').waitFor({ timeout: 15000 });
  await page.getByTestId('lab-splash').waitFor({ state: 'detached', timeout: 15000 });
  const clean = async () => {
    const violations = await page.evaluate(() => window.__violations);
    assert.deepStrictEqual(violations, [], `CSP violations: ${violations.join(', ')}`);
    assert.deepStrictEqual(errors, [], `page errors: ${errors.join(' | ')}`);
  };
  return { page, clean };
}

const devices = (page) => page.locator('[data-testid=lab-canvas-wrap] g.node[data-dev], [data-testid=lab-canvas-wrap] g.hw[data-dev]').count();
const links = (page) => page.locator('[data-testid=lab-canvas-wrap] [data-link]').count();
const node = (page, name) => page.locator(`[data-testid=lab-canvas-wrap] [data-dev][aria-label^="${name},"]`);

await check('loads under the lab CSP, simulated, signed out says so', async () => {
  const { page, clean } = await open();
  await page.getByText('Sign in on the admin page first, then open the lab').waitFor();
  assert.strictEqual(await page.getByTestId('setup-picker').locator('option[value=this-box-problem]').innerText(), 'This box: sign in on the admin page first');
  assert.match(await page.getByTestId('engine-badge').innerText(), /Simulated consoles/);
  assert.ok((await devices(page)) >= 5, 'the default setup draws its devices');
  const picker = page.getByTestId('setup-picker');
  assert.strictEqual(await picker.inputValue(), 'two-sites');
  await clean();
  await page.close();
});

await check('every built-in setup loads and draws', async () => {
  const { page, clean } = await open();
  const picker = page.getByTestId('setup-picker');
  const keys = await picker.locator('option:not([disabled])').evaluateAll((os) => os.map((o) => o.value).filter(Boolean));
  assert.ok(keys.length >= 3, `built-ins listed: ${keys.join(', ')}`);
  for (const key of keys) {
    await picker.selectOption(key);
    await page.waitForTimeout(150);
    assert.strictEqual(await picker.inputValue(), key);
    if (key !== 'empty') assert.ok((await devices(page)) >= 1, `${key} draws something`);
  }
  await clean();
  await page.close();
});

await check('This box: signed in, the lab draws this box from /api', async () => {
  const { page, clean } = await open({ signedIn: true });
  const picker = page.getByTestId('setup-picker');
  assert.strictEqual(await picker.inputValue(), 'this-box');
  assert.strictEqual(await page.getByText('Sign in on the admin page first, then open the lab').count(), 0);
  assert.ok((await node(page, 'mattbox').count()) === 1, 'the box carries its hostName');
  await clean();
  await page.close();
});

await check('place from the tray, drag, connect, delete with the keyboard', async () => {
  const { page, clean } = await open();
  const wrap = await page.getByTestId('lab-canvas-wrap').boundingBox();
  const before = await devices(page);
  const linksBefore = await links(page);

  // place: a Box from the tray, dropped on empty canvas
  await page.locator('[data-testid=tray-items] [data-type=box]').click();
  await page.mouse.click(wrap.x + 420, wrap.y + wrap.height - 90);
  assert.strictEqual(await devices(page), before + 1, 'one device placed');
  const ids = await page.locator('[data-testid=lab-canvas-wrap] g.node[data-dev]').evaluateAll((ns) => ns.map((n) => n.getAttribute('data-dev')));
  const added = ids[ids.length - 1];
  const fresh = page.locator(`[data-testid=lab-canvas-wrap] g.node[data-dev="${added}"]`);
  await page.keyboard.press('Escape');

  // drag it
  const a = await fresh.boundingBox();
  await page.mouse.move(a.x + a.width / 2, a.y + a.height / 2);
  await page.mouse.down();
  await page.mouse.move(a.x + a.width / 2 + 40, a.y + a.height / 2 - 30, { steps: 6 });
  await page.mouse.move(a.x + a.width / 2 + 80, a.y + a.height / 2 - 60, { steps: 6 });
  await page.mouse.up();
  const b = await fresh.boundingBox();
  assert.ok(Math.abs(b.x - a.x - 80) < 6 && Math.abs(b.y - a.y + 60) < 6, `dragged by (80,-60): (${b.x - a.x}, ${b.y - a.y})`);

  // connect it to the home router with the first cable in the tray
  await page.getByRole('button', { name: 'Connections' }).click();
  await page.locator('[data-testid=tray-items] [data-link-kind]').first().click();
  await fresh.click();
  const port = page.getByTestId('connect-port');
  if (await port.count()) await port.selectOption({ index: 0 });
  await node(page, 'home-router').click();
  assert.strictEqual(await links(page), linksBefore + 1, 'one cable laid');
  await page.keyboard.press('Escape');

  // select and delete with the keyboard: the device and its cable go
  await fresh.click();
  await page.getByTestId('inspector-device').waitFor();
  await page.keyboard.press('Delete');
  assert.strictEqual(await devices(page), before, 'device removed');
  assert.strictEqual(await links(page), linksBefore, 'its cable went with it');
  await clean();
  await page.close();
});

await check('Save, then Open the file: the same setup comes back', async () => {
  const { page, clean } = await open();
  // change something first, so the saved file is not a built-in
  await page.locator('[data-testid=tray-items] [data-type=box]').click();
  const wrap = await page.getByTestId('lab-canvas-wrap').boundingBox();
  await page.mouse.click(wrap.x + 420, wrap.y + wrap.height - 90);
  await page.keyboard.press('Escape');
  const saved = await devices(page);
  const [download] = await Promise.all([page.waitForEvent('download'), page.getByRole('button', { name: 'Save' }).click()]);
  assert.match(download.suggestedFilename(), /\.llf$/);
  const text = await readFile(await download.path(), 'utf8');
  JSON.parse(text);

  await page.getByTestId('setup-picker').selectOption('two-sites');
  assert.strictEqual(await devices(page), saved - 1);
  await page.getByTestId('open-file').setInputFiles({ name: download.suggestedFilename(), mimeType: 'application/json', buffer: Buffer.from(text) });
  await page.waitForTimeout(200);
  assert.strictEqual(await devices(page), saved, 'the saved device came back');
  assert.match(await page.getByTestId('setup-picker').locator('option:checked').innerText(), /^File: /);

  // a file over the 1 MiB cap is refused, the setup stays
  await page.getByTestId('open-file').setInputFiles({ name: 'big.llf', mimeType: 'application/json', buffer: Buffer.alloc((1 << 20) + 1, 32) });
  await page.waitForTimeout(200);
  assert.strictEqual(await devices(page), saved);
  await clean();
  await page.close();
});

await check('Simulation: Step advances the clock and logs events', async () => {
  const { page, clean } = await open();
  await page.getByRole('radio', { name: 'Simulation' }).click();
  await page.getByTestId('sim-step').waitFor();
  const status0 = await page.getByTestId('sim-status').innerText();
  for (let i = 0; i < 6; i++) await page.getByTestId('sim-step').click();
  const status1 = await page.getByTestId('sim-status').innerText();
  assert.notStrictEqual(status1, status0, `status moved: ${status0} → ${status1}`);
  assert.ok((await page.locator('[data-testid=event-list] tbody tr').count()) > 0, 'events listed');
  await page.getByRole('radio', { name: 'Realtime' }).click();
  await page.getByTestId('sim-step').waitFor({ state: 'detached' });
  await clean();
  await page.close();
});

await check('the physical view draws rooms and hardware', async () => {
  const { page, clean } = await open();
  await page.getByRole('radio', { name: 'Physical' }).click();
  await page.locator('[data-testid=lab-canvas-wrap] g.hw[data-dev]').first().waitFor();
  assert.ok((await page.locator('[data-testid=lab-canvas-wrap] .room').count()) >= 3, 'rooms drawn');

  // drag mattbox from the home into the office: it changes room
  const box = page.locator('[data-testid=lab-canvas-wrap] g.hw[data-dev][aria-label^="mattbox,"]');
  const office = await page.locator('[data-testid=lab-canvas-wrap] [data-room=office]').boundingBox();
  const a = await box.boundingBox();
  await page.mouse.move(a.x + 20, a.y + 12);
  await page.mouse.down();
  await page.mouse.move(office.x + office.width * 0.6, office.y + office.height * 0.7, { steps: 12 });
  await page.mouse.up();
  await page.getByText(/^mattbox moved to /).waitFor();
  const b = await box.boundingBox();
  assert.ok(b.x > office.x && b.y > office.y, 'drawn inside the office');

  // the connect tool takes the port that was clicked on the hardware
  await page.getByRole('button', { name: 'Connections' }).click();
  await page.locator('[data-testid=tray-items] [data-link-kind]').first().click();
  const router = page.locator('[data-testid=lab-canvas-wrap] g.hw[data-dev][aria-label^="office-router,"]');
  // lan4 sits at (80, 36) of the 100×46 drawing, drawn 1.3× inside a halo 5 wider each side
  const r = await router.locator('rect.hw-halo').boundingBox();
  const k = r.width / (100 * 1.3 + 10);
  await page.mouse.click(r.x + (5 + 80 * 1.3) * k, r.y + (5 + 36 * 1.3) * k);
  const sw = page.locator('[data-testid=lab-canvas-wrap] g.hw[data-dev][aria-label^="office-switch,"]');
  const swr = await sw.locator('rect.hw-halo').boundingBox();
  await page.mouse.click(swr.x + swr.width * 0.08, swr.y + swr.height * 0.5);
  await page.getByText(/^Connected office-router lan4 to office-switch p\d/).waitFor({ timeout: 5000 }).catch(async (e) => {
    const said = await page.locator('[data-tone], [role=status]').allInnerTexts();
    throw new Error(e.message.split('\n')[0] + ' — on screen: ' + said.join(' | '));
  });
  await clean();
  await page.close();
});

await check('a box console answers, config editors are typed', async () => {
  const { page, clean } = await open();
  await node(page, 'mattbox').click();
  await page.getByRole('tab', { name: 'Console' }).click();
  const term = page.getByTestId('sim-terminal');
  await term.click();
  const len0 = (await term.innerText()).length;
  await page.keyboard.type('help');
  await page.keyboard.press('Enter');
  await page.waitForTimeout(100);
  assert.ok((await term.innerText()).length > len0 + 20, 'help printed something');
  await page.getByRole('tab', { name: 'Config' }).click();
  const pane = page.getByTestId('pane-config');
  assert.ok((await pane.getByRole('switch').count()) > 0, 'booleans are switches');
  assert.ok((await pane.locator('input[type=text], input:not([type])').count()) > 0, 'strings are inputs');
  await clean();
  await page.close();
});

await check('the chrome speaks Slovak and German, device blurbs stay English', async () => {
  for (const [locale, logical, sim] of [
    ['sk-SK', 'Logický', 'Simulácia'],
    ['de-DE', 'Logisch', 'Simulation'],
  ]) {
    const { page, clean } = await open({ locale });
    await page.getByRole('radio', { name: logical }).waitFor();
    await page.getByRole('radio', { name: sim }).waitFor();
    await clean();
    await page.close();
  }
});

await check('dark theme: the canvas follows the house tokens', async () => {
  const { page, clean } = await open({ scheme: 'dark' });
  const [ground, room] = await page.evaluate(() => {
    const cs = getComputedStyle(document.documentElement);
    return [cs.getPropertyValue('--ground').trim(), cs.getPropertyValue('--lab-room').trim()];
  });
  assert.strictEqual(ground, '#0a0e12');
  assert.ok(room.length > 0, '--lab-room resolves');
  await clean();
  await page.close();
});

await check('phone width: no sideways scroll, the inspector is a sheet', async () => {
  const { page, clean } = await open({ viewport: { width: 390, height: 844 } });
  const overflow = await page.evaluate(() => document.documentElement.scrollWidth - window.innerWidth);
  assert.ok(overflow <= 0, `page scrolls sideways by ${overflow}px`);
  await node(page, 'mattbox').click();
  await page.getByTestId('inspector-sheet').waitFor();
  await page.getByTestId('pane-status').or(page.getByTestId('inspector-device')).first().waitFor();
  await page.keyboard.press('Escape');
  await page.getByTestId('inspector-sheet').waitFor({ state: 'detached' });
  await clean();
  await page.close();
});

/* libvirt's remote protocol as far as the page's client drives it (program
 * 0x20008086: AUTH_LIST, CONNECT_OPEN, DOMAIN_CREATE_XML, LOOKUP_BY_NAME,
 * OPEN_CONSOLE, RESUME, DESTROY, CONNECT_CLOSE), behind the relay socket.
 * A KVM domain is refused the way a machine without KVM refuses it, so the
 * client's fallback to plain QEMU is walked too. */
function fakeLibvirt(ws, seen) {
  const PROG = 0x20008086;
  const u32 = (...v) => {
    const b = Buffer.alloc(4 * v.length);
    v.forEach((x, i) => b.writeUInt32BE(x >>> 0, 4 * i));
    return b;
  };
  const str = (s) => {
    const b = Buffer.from(s);
    return Buffer.concat([u32(b.length), b, Buffer.alloc((4 - (b.length % 4)) % 4)]);
  };
  const readStr = (p, off) => {
    const n = p.readUInt32BE(off);
    return [p.subarray(off + 4, off + 4 + n).toString(), off + 4 + n + ((4 - (n % 4)) % 4)];
  };
  const packet = (proc, type, serial, status, body = Buffer.alloc(0)) =>
    Buffer.concat([u32(28 + body.length, PROG, 1, proc, type, serial, status), body]);
  const dom = (name) => Buffer.concat([str(name), Buffer.alloc(16, 0xab), u32(1)]);
  // virNetMessageError: code, domain, message, level, dom, str1..3, int1, int2, net
  const error = (message) => Buffer.concat([u32(1, 10, 1), str(message), u32(2, 0, 0, 0, 0, 0, 0, 0)]);
  let buf = Buffer.alloc(0);
  let consoleSerial = 0;
  ws.onMessage((m) => {
    buf = Buffer.concat([buf, typeof m === 'string' ? Buffer.from(m) : Buffer.from(m)]);
    while (buf.length >= 4 && buf.length >= buf.readUInt32BE(0)) {
      const p = buf.subarray(0, buf.readUInt32BE(0));
      buf = buf.subarray(p.length);
      if (p.readUInt32BE(4) !== PROG) continue;
      const proc = p.readInt32BE(12);
      const type = p.readUInt32BE(16);
      const serial = p.readUInt32BE(20);
      const body = p.subarray(28);
      if (type === 3) {
        seen.typed += body.toString();
        continue;
      }
      seen.procs.push(proc);
      let ret = Buffer.alloc(0);
      if (proc === 66) ret = u32(1, 0); // AUTH_LIST: none
      if (proc === 1) seen.uri = body.readUInt32BE(0) ? readStr(body, 4)[0] : null;
      if (proc === 10) {
        const [xml, off] = readStr(body, 0);
        seen.xmls.push(xml);
        seen.flags = body.readUInt32BE(off);
        if (xml.startsWith("<domain type='kvm'>")) {
          ws.send(packet(proc, 1, serial, 1, error("unsupported configuration: domain type 'kvm' is not available here")));
          continue;
        }
        seen.name = /<name>([^<]*)<\/name>/.exec(xml)[1];
        ret = dom(seen.name);
      }
      if (proc === 23) ret = dom(readStr(body, 0)[0]);
      if (proc === 201) consoleSerial = serial;
      ws.send(packet(proc, 1, serial, 0, ret));
      if (proc === 28) ws.send(packet(201, 3, consoleSerial, 2, Buffer.from('Welcome to the LosOS stand-in\r\n\x1b[32mmattbox is ready\x1b[0m\r\n# ')));
    }
  });
}

/* A fake helper: hello, guests, and the two sockets per guest. `refuse`
 * makes POST /guests answer as libvirt refusing the domain. `virsh: false`
 * is a helper whose virsh is missing, and `virt` adds the relay path: the
 * ticket, the relay socket (fakeLibvirt) and the page guest's NIC socket. */
function fakeHelper({ refuse = false, gear = false, virsh = true, virt = false } = {}) {
  const seen = { posts: [], protocols: [], typed: '', deleted: [], tickets: 0, procs: [], xmls: [], nicOpened: false };
  const routes = async (page, authed) => {
    const hello = virsh
      ? { available: true, virsh: true, domainType: 'kvm', label: 'KVM via libvirt', reason: null }
      : { available: false, virsh: false, domainType: 'qemu', label: 'QEMU via libvirt (no KVM)', reason: 'libvirt did not answer: virsh: No such file or directory' };
    await page.route('**/api/lab/hello', (route) =>
      authed(route)
        ? json(route, 200, {
            ...hello,
            images: { kernel: true, rootfs: true, gear, dir: '/srv/lab guest' },
            maxGuests: 8,
            memoryMiB: 96,
            ...(virt ? { virt: { available: true, socket: '/run/libvirt/virtqemud-sock', reason: null } } : {}),
          })
        : json(route, 401, {}),
    );
    if (virt) {
      await page.route('**/api/lab/virt-ticket', (route) => {
        if (!authed(route) || route.request().method() !== 'POST') return json(route, 401, {});
        seen.tickets++;
        return json(route, 200, {
          ticket: 'aa'.repeat(16),
          expiresIn: 30,
          uri: 'qemu:///system?socket=/run/libvirt/virtqemud-sock',
          nic: { guest: 'virt-c0ffee', domain: 'losos-lab-virt-c0ffee', ticket: '11ff', helperPort: 40001, qemuPort: 40002 },
        });
      });
      await page.routeWebSocket(/\/api\/lab\/virt$/, (ws) => fakeLibvirt(ws, seen));
    }
    await page.route('**/api/lab/guests', (route) => {
      if (!authed(route)) return json(route, 401, {});
      seen.posts.push(route.request().postDataJSON());
      return refuse
        ? json(route, 502, { error: 'libvirt refused the guest: no KVM here' })
        : json(route, 201, { guest: 'd1-abc123', domain: 'losos-lab-d1-abc123', ticket: '00ff', nics: 1, label: 'KVM via libvirt' });
    });
    await page.route('**/api/lab/guests/*', (route) => {
      seen.deleted.push(route.request().method() + ' ' + new URL(route.request().url()).pathname);
      return json(route, 200, {});
    });
    await page.routeWebSocket(/\/api\/lab\/ws\//, (ws) => {
      if (ws.url().endsWith('/console')) {
        ws.send(Buffer.from('Welcome to the LosOS stand-in\r\n\x1b[32mmattbox is ready\x1b[0m\r\n# '));
        ws.onMessage((m) => {
          seen.typed += typeof m === 'string' ? m : Buffer.from(m).toString();
        });
      }
    });
    // after routeWebSocket, whose own init script replaces WebSocket too
    await page.addInitScript(() => {
      const WS = window.WebSocket;
      window.__wsProtocols = [];
      window.WebSocket = class extends WS {
        constructor(url, protocols) {
          window.__wsProtocols.push([String(url), [].concat(protocols ?? [])]);
          super(url, protocols);
        }
      };
    });
  };
  return { seen, routes };
}

await check('libvirt: the box relays a helper, a guest boots there and its console is the byte terminal', async () => {
  const helper = fakeHelper();
  const { page, clean } = await open({ signedIn: true, routes: helper.routes });
  assert.match(await page.getByTestId('engine-badge').innerText(), /Emulated.*KVM via libvirt/s);
  assert.match(await page.locator('[data-testid=tray-items] [data-type=box]').innerText(), /boots under libvirt/);
  await node(page, 'mattbox').click();
  await page.getByRole('tab', { name: 'Console' }).click();
  assert.match(await page.getByTestId('console-head').innerText(), /KVM via libvirt/);
  await page.getByRole('button', { name: 'Boot the x86_64 guest (KVM via libvirt)' }).click();
  const term = page.getByTestId('guest-terminal');
  await term.waitFor();
  await page.waitForFunction(() => /mattbox is ready/.test(document.querySelector('[data-testid=guest-terminal]')?.textContent ?? ''));
  assert.strictEqual(await term.locator('.c-32').first().innerText(), 'mattbox is ready', 'ANSI green is a house class');

  const post = helper.seen.posts[0];
  assert.deepStrictEqual(Object.keys(post).sort(), ['cmdline', 'id', 'macs', 'name', 'role']);
  assert.strictEqual(post.name, 'mattbox');
  assert.strictEqual(post.role, 'box');
  assert.match(post.cmdline, /^console=ttyS0 .*losos\.host=mattbox/);
  assert.strictEqual(post.macs.length, 1);
  const protocols = await page.evaluate(() => window.__wsProtocols);
  assert.deepStrictEqual(
    protocols.map(([u, p]) => [new URL(u).pathname, p]),
    [
      ['/api/lab/ws/d1-abc123/console', ['losos-lab', 'ticket.00ff']],
      ['/api/lab/ws/d1-abc123/nic/0', ['losos-lab', 'ticket.00ff']],
    ],
  );

  await term.click();
  await page.keyboard.type('ls');
  await page.keyboard.press('Enter');
  await page.waitForTimeout(150);
  assert.strictEqual(helper.seen.typed, 'ls\r');
  assert.match(await page.getByTestId('console-head').innerText(), /KVM via libvirt/);
  assert.match(await page.getByTestId('pane-status').or(page.getByRole('tab', { name: 'Status' })).first().innerText(), /./);

  // a router needs gear.bin, which this helper lacks: its console stays simulated
  await page.locator('[data-testid=lab-canvas-wrap] g.node[data-dev][aria-label$=", Router"]').first().click();
  await page.getByRole('tab', { name: 'Console' }).click();
  await page.getByTestId('sim-terminal').waitFor();

  // powering the box off deletes its domain
  await page.getByTestId('setup-picker').selectOption('two-sites');
  await page.waitForTimeout(150);
  assert.deepStrictEqual(helper.seen.deleted, ['DELETE /api/lab/guests/d1-abc123']);
  await clean();
  await page.close();
});

await check('libvirt: a refused guest says so once and keeps the simulated console', async () => {
  const helper = fakeHelper({ refuse: true });
  const { page, clean } = await open({ signedIn: true, routes: helper.routes });
  await node(page, 'mattbox').click();
  await page.getByRole('tab', { name: 'Console' }).click();
  await page.getByRole('button', { name: /^Boot the x86_64 guest/ }).click();
  // .first(): the toast's live region repeats the words for a moment
  await page.getByText('KVM via libvirt could not start mattbox: libvirt refused the guest: no KVM here. Its console stays simulated.').first().waitFor();
  await page.getByTestId('sim-terminal').waitFor();
  assert.strictEqual(helper.seen.posts.length, 1);
  await clean();
  await page.close();
});

await check('libvirt: signed out, the box copy does not ask the helper', async () => {
  const helper = fakeHelper();
  let asked = 0;
  const { page, clean } = await open({
    routes: async (p, authed) => {
      await helper.routes(p, authed);
      await p.route('**/api/lab/hello', (route) => {
        asked++;
        return json(route, 401, {});
      });
    },
  });
  assert.strictEqual(asked, 0);
  assert.match(await page.getByTestId('engine-badge').innerText(), /Simulated consoles/);
  const tip = await page.getByTestId('engine-badge').locator('xpath=..').getAttribute('aria-label');
  assert.match(tip ?? '', /libvirt: sign in on the admin page first\./);
  await clean();
  await page.close();
});

await check('virt-rpc: no virsh, the page drives libvirt through the relay; the client loads only then', async () => {
  const helper = fakeHelper({ virsh: false, virt: true });
  const fetched = [];
  const { page, clean } = await open({
    signedIn: true,
    routes: async (p, authed) => {
      p.on('request', (r) => fetched.push(new URL(r.url()).pathname));
      await helper.routes(p, authed);
    },
  });
  assert.match(await page.getByTestId('engine-badge').innerText(), /Emulated.*libvirt via WebAssembly/s);
  assert.match(await page.locator('[data-testid=tray-items] [data-type=box]').innerText(), /boots under libvirt/);
  assert.deepStrictEqual(fetched.filter((u) => /losos_lab_virt/.test(u)), [], 'the client is not fetched with the page');

  await node(page, 'mattbox').click();
  await page.getByRole('tab', { name: 'Console' }).click();
  assert.match(await page.getByTestId('console-head').innerText(), /libvirt via WebAssembly/);
  await page.getByRole('button', { name: 'Boot the x86_64 guest (libvirt via WebAssembly)' }).click();
  const term = page.getByTestId('guest-terminal');
  await term.waitFor();
  await page.waitForFunction(() => /mattbox is ready/.test(document.querySelector('[data-testid=guest-terminal]')?.textContent ?? ''));
  assert.ok(fetched.some((u) => /losos_lab_virt.*\.wasm$/.test(u)), 'the client came when the guest started');

  // the ticket, then one relayed connection: open, KVM refused, QEMU created
  // paused and autodestroy, its console, resume
  assert.strictEqual(helper.seen.tickets, 1);
  assert.strictEqual(helper.seen.uri, 'qemu:///system', 'the socket parameter stays with the helper');
  assert.deepStrictEqual(helper.seen.procs, [66, 1, 10, 10, 23, 201, 23, 28]);
  assert.strictEqual(helper.seen.flags, 3, 'PAUSED | AUTODESTROY');
  const xml = helper.seen.xmls[1];
  assert.match(helper.seen.xmls[0], /^<domain type='kvm'>/);
  assert.match(xml, /^<domain type='qemu'>/);
  assert.strictEqual(helper.seen.name, 'losos-lab-virt-c0ffee', 'the name the helper handed out');
  assert.match(xml, /<kernel>\/srv\/lab guest\/bzImage<\/kernel>/);
  assert.match(xml, /<source file='\/srv\/lab guest\/rootfs.bin'\/>/);
  assert.match(xml, /<cmdline>console=ttyS0 .*losos\.host=mattbox/);
  assert.match(xml, /<mac address='[0-9a-f:]{17}'\/>\n\s*<source address='127.0.0.1' port='40001'>\n\s*<local address='127.0.0.1' port='40002'\/>/);
  assert.match(xml, /<serial type='pty'>/);
  assert.doesNotMatch(xml, /<serial type='tcp'>/);
  const protocols = await page.evaluate(() => window.__wsProtocols);
  assert.deepStrictEqual(
    protocols.map(([u, p]) => [new URL(u).pathname, p]),
    [
      ['/api/lab/virt', ['losos-lab', 'ticket.' + 'aa'.repeat(16)]],
      ['/api/lab/ws/virt-c0ffee/nic/0', ['losos-lab', 'ticket.11ff']],
    ],
  );
  assert.match(await page.getByTestId('console-head').innerText(), /QEMU via libvirt from WebAssembly \(no KVM\)/);

  // typing goes down the console stream
  await term.click();
  await page.keyboard.type('ls');
  await page.keyboard.press('Enter');
  await page.waitForTimeout(150);
  assert.strictEqual(helper.seen.typed, 'ls\r');

  // powering off destroys the domain over the relay and hands back the card
  await page.getByTestId('setup-picker').selectOption('two-sites');
  await page.waitForTimeout(300);
  assert.ok(helper.seen.procs.includes(12), `DOMAIN_DESTROY sent: ${helper.seen.procs}`);
  assert.deepStrictEqual(helper.seen.deleted, ['DELETE /api/lab/guests/virt-c0ffee']);
  await clean();
  await page.close();
});

await check('virt-rpc: beside virsh it is the fallback, and a helper without the relay leaves it out quietly', async () => {
  const both = fakeHelper({ virt: true });
  let { page, clean } = await open({ signedIn: true, routes: both.routes });
  assert.match(await page.getByTestId('engine-badge').innerText(), /KVM via libvirt, libvirt via WebAssembly as the fallback/);
  await clean();
  await page.close();

  // an older helper (no `virt` in hello): only the virsh path, no complaint
  const old = fakeHelper();
  ({ page, clean } = await open({ signedIn: true, routes: old.routes }));
  const badge = await page.getByTestId('engine-badge').innerText();
  assert.match(badge, /KVM via libvirt/);
  assert.doesNotMatch(badge, /WebAssembly/);
  await clean();
  await page.close();

  // no helper answering at all: simulated, and the reason is said once
  ({ page, clean } = await open({
    signedIn: true,
    routes: async (p) => {
      await p.route('**/api/lab/hello', (route) => route.abort());
    },
  }));
  assert.match(await page.getByTestId('engine-badge').innerText(), /Simulated consoles/);
  const tip = (await page.getByTestId('engine-badge').locator('xpath=..').getAttribute('aria-label')) ?? '';
  assert.match(tip, /libvirt: the box did not answer\./);
  assert.doesNotMatch(tip, /virt-rpc/);
  await clean();
  await page.close();
});

await browser.close();
await closeServer();
finish();
