// ── Setups as files: Save, Open, and the last one kept in this browser ──
// A setup file holds what a person placed (devices, where they stand, how
// they are set, which cables join which ports), never traffic or consoles.
// Opening one rebuilds it through newDevice and connect, so a hand-edited or
// foreign file can only produce what the tray could have produced.
const SETUP_FORMAT = 'losos-lab-setup';
const LAST_KEY = 'losos-lab-last';
let OPENED = null; // the file opened in this tab, so the picker can go back to it

function setupDoc() {
  const name = UI.scenario === 'file' || UI.scenario === 'last' ? UI.fileName : (SCENARIOS[UI.scenario] || {}).name;
  return {
    format: SETUP_FORMAT, version: 1, name: name || 'My setup',
    devices: world.devices.map(d => ({ id: d.id, type: d.type, name: d.name, site: d.site, x: d.x, y: d.y, px: d.px, py: d.py, power: d.power, cfg: d.cfg })),
    links: world.links.map(l => ({ a: l.a, b: l.b, kind: l.kind })),
  };
}

const num = (v, lo, hi) => typeof v === 'number' && isFinite(v) ? Math.max(lo, Math.min(hi, Math.round(v))) : null;

// Rebuilds a parsed document into the (already emptied) world. Returns what
// it had to leave out, so the toast can say so instead of failing silently.
function docProblem(doc) {
  if (!doc || doc.format !== SETUP_FORMAT || !Array.isArray(doc.devices) || !Array.isArray(doc.links)) return 'This is not a LosOS Lab setup file.';
  if (doc.version !== 1) return `This setup file is version ${String(doc.version).slice(0, 10)}; this Lab reads version 1.`;
  return null;
}
function buildFromDoc(doc) {
  const ids = new Map(); let skipped = 0;
  for (const s of doc.devices.slice(0, 200)) {
    if (!s || !Object.hasOwn(TYPES, s.type)) { skipped++; continue; }
    const base = DEFAULT_CFG[s.type](), cfg = {};
    for (const [k, v] of Object.entries(s.cfg || {})) if (Object.hasOwn(base, k) && typeof v === typeof base[k]) cfg[k] = v;
    const d = newDevice(s.type, num(s.x, -5000, 5000) ?? 400, num(s.y, -5000, 5000) ?? 300, {
      name: deviceName(s.name, DEFAULT_NAME[s.type]).slice(0, 40),
      site: Object.hasOwn(SITES, s.site) ? s.site : undefined,
      px: num(s.px, 0, 2000), py: num(s.py, 0, 2000), power: s.power !== false, cfg,
    });
    ids.set(String(s.id), d.id);
  }
  for (const s of doc.links.slice(0, 400)) {
    const a = s && s.a && dev(ids.get(String(s.a.dev))), b = s && s.b && dev(ids.get(String(s.b.dev)));
    if (!a || !b) { skipped++; continue; }
    let r;
    if (s.kind === 'wifi') r = connect(a.id, b.id, 'wifi');
    else {
      const ok = (d, p) => portKind(d, p) === 'eth' && !portUsed(d, p);
      if (!ok(a, s.a.port) || !ok(b, s.b.port)) { skipped++; continue; }
      r = connect(a.id, b.id, Object.hasOwn(LINK_KINDS, s.kind) ? s.kind : 'copper', s.a.port, s.b.port);
    }
    if (r.error) skipped++;
  }
  return skipped;
}

// Checked before anything is cleared, so a wrong file leaves the canvas be.
function openDoc(doc, key, label) {
  const problem = docProblem(doc);
  if (problem) { toast(problem, 6000); $('#scenario').value = UI.scenario; return null; }
  UI.fileName = String(doc.name || label).slice(0, 60);
  setFileOption(key, UI.fileName);
  let skipped = 0;
  startWorld(key, () => { skipped = buildFromDoc(doc); return null; });
  UI.dirty = false;
  if (skipped) toast(`Opened ${UI.fileName}. ${skipped} ${skipped === 1 ? 'part was' : 'parts were'} not valid and left out.`, 6000);
  return skipped;
}

// The picker gets one extra entry for whatever is open from a file.
function setFileOption(key, name) {
  const sel = $('#scenario');
  let o = sel.querySelector(`option[value="${key}"]`);
  if (!o) { o = document.createElement('option'); o.value = key; sel.appendChild(o); }
  o.textContent = (key === 'last' ? 'Last setup: ' : 'File: ') + name;
  sel.value = key;
}

async function saveSetup() {
  const doc = setupDoc();
  const blob = new Blob([JSON.stringify(doc, null, 2) + '\n'], { type: 'application/json' });
  const filename = deviceName(doc.name, 'setup').slice(0, 32).replace(/-+$/, '') + '.losos-lab.json';
  // Inside a claude.ai artifact a plain download link does nothing; the
  // viewer's own save prompt does. Everywhere else (the box) the link.
  const dl = window.claude && typeof window.claude.use === 'function' ? await window.claude.use('downloads').catch(() => null) : null;
  if (dl) {
    try { await dl.save({ filename, data: blob }); toast(`Saved ${doc.name} as ${filename}.`); }
    catch (e) { if (e && e.code !== 'declined') toast('This viewer cannot save files.', 6000); }
    return;
  }
  const a = document.createElement('a');
  a.href = URL.createObjectURL(blob);
  a.download = filename;
  document.body.appendChild(a); a.click(); a.remove();
  setTimeout(() => URL.revokeObjectURL(a.href), 1000);
  toast(`Saved ${doc.name} as ${filename}.`);
}

function openSetupFile(file) {
  if (!file) return;
  if (file.size > 1 << 20) { toast('That file is too big to be a setup.', 6000); return; }
  file.text().then(text => {
    let doc;
    try { doc = JSON.parse(text); } catch { toast('This is not a LosOS Lab setup file.', 6000); return; }
    const skipped = openDoc(doc, 'file', file.name.replace(/\.losos-lab\.json$|\.json$/, ''));
    if (skipped !== null) {
      OPENED = doc;
      if (!skipped) toast(`Opened ${UI.fileName}.`);
      UI.dirty = true; // keep it as the last setup too
    }
  });
}

// Whatever was on the canvas when the tab closed comes back as "Last setup".
// Storage can be missing (private window, blocked site data): then nothing
// is kept and nothing breaks.
function readLast() { try { const t = localStorage.getItem(LAST_KEY); return t ? JSON.parse(t) : null; } catch { return null; } }
function keepLast() {
  if (!UI.dirty) return;
  UI.dirty = false;
  const doc = setupDoc();
  if (UI.scenario !== 'file' && UI.scenario !== 'last') doc.name = 'Edited ' + doc.name;
  try { localStorage.setItem(LAST_KEY, JSON.stringify(doc)); } catch { /* storage blocked */ }
}

function initFiles() {
  $('#saveSetup').onclick = saveSetup;
  $('#openSetup').onclick = () => $('#openFile').click();
  $('#openFile').onchange = (e) => { openSetupFile(e.target.files[0]); e.target.value = ''; };
  const last = readLast();
  if (last && last.format === SETUP_FORMAT) {
    const o = document.createElement('option'); o.value = 'last'; o.textContent = 'Last setup: ' + String(last.name || 'My setup').slice(0, 60);
    $('#scenario').appendChild(o);
  }
  setInterval(keepLast, 2000);
  window.addEventListener('pagehide', keepLast);
}
function pickScenario(key) {
  if (key === 'last') { const last = readLast(); if (openDoc(last, 'last', 'My setup') === null) loadScenario('two-sites'); return; }
  if (key === 'file') { if (OPENED) openDoc(OPENED, 'file', UI.fileName); return; }
  loadScenario(key);
}
