// A plain test page for the canvas module: one Lab, driven from here the way
// the React Lab drives it, rendered by Bevy. Query parameters:
//   setup=two-sites|web|star|bus|office|three   view=logical|physical
//   theme=dark   defer=click|idle|<ms>   msaa=1|4   backend=webgpu|webgl2
//   labels=0
import { loadRender, delay, pickBackend, webgpuFailed } from './loader.js';

const $ = (s) => document.querySelector(s);
const q = new URLSearchParams(location.search);
const logEl = $('#log');
const lines = [];
function log(msg) {
  lines.push(msg);
  if (lines.length > 40) lines.shift();
  logEl.textContent = lines.join('\n');
}
// The React Lab's palette (canvas.tsx's PALETTE_TOKENS), resolved, in its
// own shape: {theme, palette}. The canvas reads the oklch() protocol
// colours itself.
const TOKENS = [
  'ground', 'surface', 'sunk', 'ink', 'muted', 'faint', 'line', 'hair', 'accent', 'accent-wash', 'ok', 'warn', 'crit',
  'lab-copper', 'lab-fiber', 'lab-wan', 'lab-wifi', 'lab-grid', 'lab-room', 'lab-room-edge', 'lab-mark', 'lab-led-off',
  'lab-p-dhcp', 'lab-p-arp', 'lab-p-mdns', 'lab-p-dns', 'lab-p-http', 'lab-p-icmp', 'lab-p-rathole', 'lab-p-rke2', 'lab-p-lososd',
  'lab-kit', 'lab-kit-face', 'lab-kit-edge', 'lab-hole', 'lab-rack', 'lab-rack-edge', 'lab-rack-face', 'lab-screen',
  'lab-rack-bay', 'lab-rack-ink', 'lab-screen-off', 'lab-screen-tile',
];
function themeJson() {
  const cs = getComputedStyle(document.documentElement);
  const palette = Object.fromEntries(TOKENS.map((k) => [k, cs.getPropertyValue('--' + k).trim()]));
  return JSON.stringify({ theme: document.documentElement.dataset.theme === 'dark' ? 'dark' : 'light', palette });
}

// Three built-in setups side by side in one world, for the frame-time test.
function threeSetups(mod) {
  const scratch = new mod.Lab(2, Date.now(), 0);
  const devices = [], links = [];
  ['two-sites', 'web', 'office'].forEach((key, i) => {
    scratch.load_scenario(key);
    const doc = JSON.parse(JSON.parse(scratch.export_setup()).text);
    for (const d of doc.devices) devices.push({ ...d, id: `s${i}${d.id}`, x: d.x + i * 1500, px: null, py: null });
    for (const l of doc.links) links.push({ ...l, a: { ...l.a, dev: `s${i}${l.a.dev}` }, b: { ...l.b, dev: `s${i}${l.b.dev}` } });
  });
  scratch.free();
  return JSON.stringify({ format: 'losos-lab-setup', version: 1, name: 'Three setups', devices, links });
}

const timings = { pageStart: performance.timeOrigin, navToScript: performance.now() };
window.__lab = { timings, events: [] };

async function main() {
  if (q.get('theme') === 'dark') document.documentElement.dataset.theme = 'dark';
  await delay(q.get('defer'), $('#load'));
  timings.loadStart = performance.now();
  const backend = await pickBackend(q.get('backend'));
  timings.backendPicked = performance.now();
  window.__lab.backend = backend;
  $('#backend').textContent = backend === 'svg' ? 'no GPU: SVG canvas' : backend === 'webgpu' ? 'WebGPU' : 'WebGL2';
  console.info(`LosOS Lab canvas: ${backend}`);
  if (backend === 'svg') {
    // The React Lab draws its SVG canvas here; this page has none.
    $('#status').textContent = 'neither WebGPU nor WebGL2: the Lab falls back to its SVG canvas';
    return;
  }
  const { mod, wasm } = await loadRender(`./pkg/${backend}/losos_lab_render.js`);
  timings.moduleReady = performance.now();
  const lab = new mod.Lab(1, Date.now(), -new Date().getTimezoneOffset());
  const load = (key) => {
    const r = key === 'three'
      ? JSON.parse(lab.import_setup(threeSetups(mod), 'three.llf'))
      : JSON.parse(lab.load_scenario(key));
    if (r.error) log('error: ' + r.error);
  };
  load(q.get('setup') || 'two-sites');
  $('#setup').value = q.get('setup') || 'two-sites';
  const canvas = mod.LabCanvas.attach(lab, '#lab-canvas');
  timings.attached = performance.now();
  canvas.set_theme(themeJson());
  if (q.get('msaa')) canvas.set_msaa(Number(q.get('msaa')));
  if (q.get('labels') === '0') canvas.set_labels(false);
  Object.assign(window.__lab, { mod, wasm, lab, canvas });

  const on = (name, f) => canvas.on(name, (json) => {
    const v = JSON.parse(json);
    window.__lab.events.push([name, v]);
    if (!['hover', 'camera', 'move'].includes(name)) log(`${name} ${json}`);
    if (f) f(v);
  });
  on('frame', (v) => {
    timings.firstFrame = v.at;
    $('#status').textContent = `first frame ${Math.round(v.at - timings.loadStart)} ms after load start`;
    // The adapter Bevy actually got (wgpu's backend name and the adapter).
    const a = JSON.parse(canvas.stats()).adapter;
    if (a) {
      $('#backend').title = `${a.backend}: ${a.name}`;
      console.info(`LosOS Lab canvas: Bevy on ${canvas.backend()}, adapter ${a.backend} "${a.name}"`);
    }
  });
  on('render-error', () => {
    if (backend !== 'webgpu') return;
    webgpuFailed();
    const u = new URL(location.href);
    u.searchParams.delete('backend');
    location.replace(u);
  });
  on('select');
  on('hover', (v) => { $('#lab-canvas').title = v?.title || ''; });
  on('moved');
  on('move');
  on('camera');
  on('viewport');
  on('context');
  on('place', (v) => {
    const r = JSON.parse(lab.add_device(v.type, v.x, v.y, JSON.stringify(v.opts)));
    if (r.error) log('error: ' + r.error); else canvas.set_selection(JSON.stringify({ kind: 'device', id: r.device.id }));
    if (!v.shift) setTool('select');
  });
  on('delete', (v) => {
    const r = v.kind === 'device' ? lab.remove_device(v.id) : lab.remove_link(v.id);
    log(r);
  });
  on('power', (v) => {
    const d = JSON.parse(lab.snapshot()).world.devices.find((x) => x.id === v.id);
    log(lab.set_power(v.id, !d.power));
  });
  on('request-port-pick', (v) => {
    // The React Lab asks; this page takes the first free port on each side.
    log(lab.connect(v.a, v.b, v.kind, v.aPorts[0], v.bPorts[0]));
    if (!v.shift) setTool('select');
  });

  const setTool = (t) => {
    canvas.set_tool(t);
    for (const b of document.querySelectorAll('[data-tool]')) b.setAttribute('aria-pressed', String(b.dataset.tool === t));
  };
  const setView = (v) => {
    canvas.set_view(v);
    for (const b of document.querySelectorAll('[data-view]')) b.setAttribute('aria-pressed', String(b.dataset.view === v));
  };
  for (const b of document.querySelectorAll('[data-tool]')) b.addEventListener('click', () => setTool(b.dataset.tool));
  for (const b of document.querySelectorAll('[data-view]')) b.addEventListener('click', () => setView(b.dataset.view));
  $('#setup').addEventListener('change', (e) => { load(e.target.value); canvas.fit(); });
  $('#fit').addEventListener('click', () => canvas.fit());
  $('#zin').addEventListener('click', () => canvas.zoom_by(1.2));
  $('#zout').addEventListener('click', () => canvas.zoom_by(1 / 1.2));
  $('#theme').addEventListener('click', () => {
    const dark = document.documentElement.dataset.theme !== 'dark';
    if (dark) document.documentElement.dataset.theme = 'dark'; else delete document.documentElement.dataset.theme;
    $('#theme').textContent = dark ? 'Light' : 'Dark';
    canvas.set_theme(themeJson());
  });
  window.addEventListener('keydown', (e) => { if (e.key === 'Escape') setTool('select'); });
  setView(q.get('view') || 'logical');

  // The page's clock: the React Lab's store does the same every frame.
  let last = performance.now();
  let packets = 0;
  const raf = (window.__lab.raf = []);
  const frame = (t) => {
    const t0 = performance.now();
    const r = JSON.parse(lab.tick(t - last));
    raf.push([t - last, performance.now() - t0, r.packets.length]);
    if (raf.length > 600) raf.shift();
    last = t;
    packets = r.packets.length;
    window.__lab.packets = packets;
    requestAnimationFrame(frame);
  };
  requestAnimationFrame(frame);
}
main().catch((e) => { log('failed: ' + (e && e.stack || e)); $('#status').textContent = 'failed'; });
