// The demo's whole flow, from the page, against a byte relay: open, version,
// create (paused, autodestroy), console, resume, read to the prompt, type,
// destroy. A file of its own: the Lab's CSP and an IWA's allow no inline script.
import init, { VirtClient } from './pkg/losos_lab_virt.js';

const q = new URLSearchParams(location.search);
const logEl = document.getElementById('log'), termEl = document.getElementById('term'), verdict = document.getElementById('verdict');
const t0 = performance.now();
const ms = () => String(Math.round(performance.now() - t0)).padStart(6);
const log = (msg, cls = 'ok') => { const li = document.createElement('li'); li.className = cls; li.textContent = `[${ms()} ms] ${msg}`; logEl.append(li); console.log(li.textContent); };
// ANSI colour codes off, carriage returns folded, for a plain <pre>.
const plain = (s) => s.replace(/\x1b\[[0-9;?]*[A-Za-z]/g, '').replace(/\r\n/g, '\n').replace(/\r/g, '');
const esc = (s) => s.replace(/[<>&'"]/g, (c) => `&#${c.charCodeAt(0)};`);

let text = '';
async function run() {
  await init();
  log(`wasm loaded; Direct Sockets here: ${VirtClient.directSocketsAvailable()}`);
  // ?target=tcp://127.0.0.1:16509 uses Direct Sockets (an Isolated Web App
  // only). ?helper=http://127.0.0.1:8095/lab/v1/ goes through
  // `losos-registrar lab`'s relay: a single-use ticket from POST
  // virt-ticket, offered as the `ticket.<hex>` subprotocol (this page's
  // origin must be one of the helper's --origin values). Otherwise a
  // WebSocket to ws-relay.mjs beside this page.
  let target = q.get('target');
  let protocols;
  if (!target && q.get('helper')) {
    const helper = new URL(q.get('helper'), location.href);
    const r = await fetch(new URL('virt-ticket', helper), { method: 'POST' });
    if (!r.ok) throw new Error(`virt-ticket: HTTP ${r.status} ${await r.text()}`);
    const { ticket } = await r.json();
    const ws = new URL('virt', helper); ws.protocol = helper.protocol === 'https:' ? 'wss:' : 'ws:';
    target = ws.href;
    protocols = ['losos-lab', `ticket.${ticket}`];
    log('POST virt-ticket: a single-use ticket from the helper');
  }
  if (!target) {
    const ws = new URL(q.get('ws') || '/virt', location.href); ws.protocol = location.protocol === 'https:' ? 'wss:' : 'ws:';
    if (q.get('token')) ws.searchParams.set('token', q.get('token'));
    target = ws.href;
  }
  const virt = await VirtClient.connect(target, protocols);
  log(`connected: ${target.replace(/token=[^&]*/, 'token=…')}`);
  const uri = q.get('uri') || 'qemu:///system';
  await virt.open(uri);
  const v = await virt.version();
  log(`CONNECT_OPEN ${uri}: QEMU ${v.hypervisor}, libvirt ${v.library}`);

  const name = q.get('name') || `lab-wasm-${Math.floor(Math.random() * 1e6)}`;
  const emulator = q.get('emulator') ? `<emulator>${esc(q.get('emulator'))}</emulator>` : '';
  const dir = esc(q.get('guest'));
  const xml = `<domain type='qemu'><name>${name}</name><memory unit='MiB'>96</memory><vcpu>1</vcpu>
<os><type arch='x86_64' machine='pc'>hvm</type><kernel>${dir}/bzImage</kernel>
<cmdline>console=ttyS0 root=/dev/vda ro rootwait loglevel=4 no_timer_check losos.host=${name} losos.role=pc</cmdline></os>
<on_poweroff>destroy</on_poweroff><devices>${emulator}
<disk type='file' device='disk'><driver name='qemu' type='raw'/><source file='${dir}/rootfs.bin'/><target dev='vda' bus='virtio'/><readonly/></disk>
<serial type='pty'><target port='0'/></serial><console type='pty'><target type='serial' port='0'/></console>
<memballoon model='none'/></devices></domain>`;
  const dom = await virt.createDomain(xml, VirtClient.PAUSED | VirtClient.AUTODESTROY);
  log(`DOMAIN_CREATE_XML (paused, autodestroy): ${dom.name} id ${dom.id} ${dom.uuid}`);
  const con = await virt.console(name);
  log('DOMAIN_OPEN_CONSOLE: stream open');
  await virt.resume(name);
  log('DOMAIN_RESUME');

  const reader = con.readable.getReader();
  const dec = new TextDecoder();
  const until = async (pred, what, ms) => {
    const end = performance.now() + ms;
    while (!pred(text)) {
      if (performance.now() > end) throw new Error(`no ${what} within ${ms} ms`);
      const { value, done } = await reader.read();
      if (done) throw new Error('console ended');
      text += dec.decode(value, { stream: true });
      termEl.textContent = plain(text);
    }
  };
  await until((t) => t.endsWith('~# '), 'shell prompt', 120000);
  log(`console: shell prompt after ${text.length} bytes`);
  con.write('uname -sm; echo lab-$((6*7))\r');
  await until((t) => /lab-42\r\n/.test(t), 'command output', 30000);
  log('console: typed "uname -sm; echo lab-$((6*7))", guest answered');
  if (q.get('leave')) {
    // Walk away without destroying: AUTODESTROY ends the guest with the connection.
    verdict.textContent = 'left running; close the tab to end it';
    document.body.dataset.result = 'left';
    return;
  }
  await virt.destroy(name);
  log(`DOMAIN_DESTROY ${name}`);
  let gone = false;
  try { await virt.lookup(name); } catch (e) { gone = /no domain|not found/i.test(String(e.message)); }
  log(`DOMAIN_LOOKUP_BY_NAME afterwards: ${gone ? 'no such domain (transient, gone)' : 'still there'}`, gone ? 'ok' : 'bad');
  await virt.close();
  log('CONNECT_CLOSE');
  verdict.textContent = gone ? 'PASS: created, console read and written, destroyed, all from wasm' : 'FAIL';
  document.body.dataset.result = gone ? 'pass' : 'fail';
}
run().catch((e) => { log(`error: ${e && e.message || e}`, 'bad'); verdict.textContent = 'FAIL: ' + (e && e.message || e); document.body.dataset.result = 'fail'; });
