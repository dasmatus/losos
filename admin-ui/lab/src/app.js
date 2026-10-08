// ── UI state and rendering ─────────────────────────────────────────────
const UI = {
  view: 'logical', tool: 'select', placeType: null, linkKind: null, connectFrom: null,
  sel: null, tab: 'status', cat: 'losos', scenario: 'two-sites',
  cam: { logical: { x: 0, y: 0, k: 1 }, physical: { x: 0, y: 0, k: 1 } },
  linkBorn: new Map(),
};
const $ = (s, el = document) => el.querySelector(s);
const NS = 'http://www.w3.org/2000/svg';
const svg = () => $('#canvas');
let PLATE = '';

function toast(msg, ms = 3600) {
  let t = $('.toast'); if (!t) { t = document.createElement('div'); t.className = 'toast'; t.setAttribute('role', 'status'); document.body.appendChild(t); }
  t.textContent = msg; t.hidden = false; clearTimeout(t._h); t._h = setTimeout(() => (t.hidden = true), ms);
}
function changed(opts = {}) {
  UI.dirty = true;
  evaluate();
  trafficForChange();
  renderAll(opts);
}
function renderAll(opts = {}) {
  renderTop(); renderCanvas(); renderTray();
  if (!opts.keepSide) renderSide(); else refreshSideLive();
  renderSimBar(); queueEvRender();
}

// ── Top bar ────────────────────────────────────────────────────────────
function renderTop() {
  for (const b of document.querySelectorAll('[data-view]')) b.setAttribute('aria-pressed', String(b.dataset.view === UI.view));
  for (const b of document.querySelectorAll('[data-mode]')) b.setAttribute('aria-pressed', String(b.dataset.mode === SIM.mode));
  $('#scenario').value = UI.scenario;
  const eng = $('#engine');
  const E = typeof ENGINE !== 'undefined' ? ENGINE : { available: false, reason: 'not loaded' };
  // The badge names the engine; the why is in its tooltip, so the bar never
  // squeezes the view and mode switches beside it.
  const where = E.libvirt ? esc(E.libvirt.label) + (E.wasm ? ', qemu-wasm as the fallback' : '') : 'x86_64 guests in qemu-wasm';
  eng.innerHTML = E.available
    ? `<span class="dot ok"></span> <b>Emulated</b> · ${where}`
    : `<span class="dot warn"></span> <b>Simulated consoles</b>`;
  eng.title = !E.available ? E.reason
    : (E.libvirt ? 'Guests run under libvirt on this computer through losos-registrar lab, one transient domain per powered device. ' : '')
      + (E.wasm ? 'qemu-system-x86_64 compiled to WebAssembly (ktock/qemu-wasm) runs any guest libvirt cannot. ' : '')
      + 'LosOS devices boot the LosOS stand-in guest; routers, switches and access points boot Netzgeräte Betriebssystem.';
}

// ── Canvas: shared camera, logical and physical renderers ───────────────
function camera() { return UI.cam[UI.view]; }
function toWorld(ev) {
  const r = svg().getBoundingClientRect(); const c = camera();
  return { x: (ev.clientX - r.left - c.x) / c.k, y: (ev.clientY - r.top - c.y) / c.k };
}
function fitView() {
  const el = svg(); if (!el) return;
  const r = el.getBoundingClientRect(); if (!r.width) return;
  if (UI.view === 'logical') {
    if (!world.devices.length) { UI.cam.logical = { x: 0, y: 0, k: 1 }; return; }
    const xs = world.devices.map(d => d.x), ys = world.devices.map(d => d.y);
    const minX = Math.min(...xs) - 90, maxX = Math.max(...xs) + 90, minY = Math.min(...ys) - 70, maxY = Math.max(...ys) + 80;
    // The legend sits along the bottom edge: keep the devices above it.
    const h = Math.max(120, r.height - 44);
    const k = Math.min(1.25, Math.min(r.width / (maxX - minX), h / (maxY - minY)));
    UI.cam.logical = { k, x: (r.width - (maxX - minX) * k) / 2 - minX * k, y: (h - (maxY - minY) * k) / 2 - minY * k };
  } else {
    const W = 1240, H = 730;
    const k = Math.min(r.width / W, r.height / H) * 0.98;
    UI.cam.physical = { k, x: (r.width - W * k) / 2, y: (r.height - H * k) / 2 };
  }
}
function renderCanvas() {
  const el = svg(); const c = camera();
  const hint = $('#hint');
  const ht = UI.tool === 'place' ? `Click on the ${UI.view} view to place a ${TYPES[UI.placeType].label}. Esc cancels.`
    : UI.tool === 'connect' ? (UI.connectFrom ? `Now click the device to connect ${nm(UI.connectFrom)} to.` : `${LINK_KINDS[UI.linkKind].label} cable: click the first device. Esc cancels.`)
    : UI.tool === 'delete' ? 'Click a device or a cable to remove it. Esc cancels.' : '';
  hint.hidden = !ht; hint.textContent = ht;
  el.innerHTML = '';
  const defs = document.createElementNS(NS, 'defs');
  defs.innerHTML = `<pattern id="grid" width="40" height="40" patternUnits="userSpaceOnUse"><path d="M40 0H0V40" fill="none" class="gridline"/></pattern>`;
  el.appendChild(defs);
  const vp = document.createElementNS(NS, 'g');
  vp.setAttribute('id', 'vp');
  vp.setAttribute('transform', `translate(${c.x} ${c.y}) scale(${c.k})`);
  el.appendChild(vp);
  if (UI.view === 'logical') renderLogical(vp); else renderPhysical(vp);
  const pl = document.createElementNS(NS, 'g'); pl.setAttribute('id', 'pdus'); vp.appendChild(pl);
  const rb = document.createElementNS(NS, 'path'); rb.setAttribute('id', 'rubber'); rb.setAttribute('class', 'rubber'); vp.appendChild(rb);
  $('#legend').innerHTML = UI.view === 'logical'
    ? `<span><i style="border-color:var(--wire)"></i>Copper</span><span><i style="border-color:var(--fiber)"></i>Fiber</span><span><i style="border-color:var(--wan);border-top-style:dashed"></i>WAN</span><span><i style="border-color:var(--wifi);border-top-style:dotted"></i>Wi-Fi</span><span><span class="dot ok"></span>link up</span><span><span class="dot crit"></span>down</span>`
    : `<span>Click a power LED to switch a device on or off. Drag hardware between rooms to move it.</span>`;
  drawPdus();
}
function linkState(l) {
  if (!linkUp(l)) return 'down';
  const born = UI.linkBorn.get(l.id);
  if (born && performance.now() - born < 1400) return 'wait';
  return 'up';
}
function portStateFn(d) {
  return (p) => {
    const l = world.links.find(l => (l.a.dev === d.id && l.a.port === p) || (l.b.dev === d.id && l.b.port === p));
    return l ? linkState(l) : 'none';
  };
}
function el(tag, attrs = {}, parent) { const e = document.createElementNS(NS, tag); for (const k in attrs) e.setAttribute(k, attrs[k]); if (parent) parent.appendChild(e); return e; }

// A bus is drawn as the cable it is: a backbone through its icon, with each
// device dropping onto it at its own x.
function busSpan(b) {
  const xs = linksOf(b).map(l => dev(otherEnd(l, b.id).dev).x);
  return { x1: Math.min(b.x - 120, ...xs) - 30, x2: Math.max(b.x + 120, ...xs) + 30 };
}
function anchor(d, other) {
  if (d.type !== 'bus' || !other) return d;
  const s = busSpan(d);
  return { x: Math.max(s.x1 + 20, Math.min(s.x2 - 20, other.x)), y: d.y };
}
function renderLogical(vp) {
  el('rect', { x: -4000, y: -4000, width: 8000, height: 8000, fill: 'url(#grid)' }, vp);
  const L = el('g', {}, vp), N = el('g', {}, vp);
  for (const b of world.devices.filter(d => d.type === 'bus')) {
    const s = busSpan(b);
    el('line', { x1: s.x1, y1: b.y, x2: s.x2, y2: b.y, class: 'busline' + (b.power ? '' : ' off') }, L);
    for (const x of [s.x1, s.x2]) el('rect', { x: x - 5, y: b.y - 9, width: 10, height: 18, rx: 2.5, class: 'busterm' }, L);
  }
  for (const l of world.links) {
    const A = anchor(dev(l.a.dev), dev(l.b.dev)), B = anchor(dev(l.b.dev), dev(l.a.dev));
    const sel = UI.sel?.kind === 'link' && UI.sel.id === l.id;
    el('line', { x1: A.x, y1: A.y, x2: B.x, y2: B.y, class: `link ${l.kind}${sel ? ' sel' : ''}` }, L);
    const hit = el('line', { x1: A.x, y1: A.y, x2: B.x, y2: B.y, class: 'link-hit', 'data-link': l.id }, L);
    hit.appendChild(document.createElementNS(NS, 'title')).textContent = `${dev(l.a.dev).name} ${l.a.port} ↔ ${dev(l.b.dev).name} ${l.b.port} (${l.kind})`;
    const dx = B.x - A.x, dy = B.y - A.y, len = Math.hypot(dx, dy) || 1, ux = dx / len, uy = dy / len;
    const st = linkState(l);
    for (const [P, end, s] of [[A, l.a, 1], [B, l.b, -1]]) {
      if (dev(end.dev).type === 'bus') { el('circle', { cx: P.x, cy: P.y, r: 4, class: 'led ' + (st === 'up' ? 'up' : st === 'wait' ? 'wait' : 'down') }, L); continue; }
      const lx = P.x + ux * s * 40, ly = P.y + uy * s * 40;
      el('circle', { cx: lx, cy: ly, r: 4.5, class: 'led ' + (st === 'up' ? 'up' : st === 'wait' ? 'wait' : 'down') }, L);
      if (len > 150) {
        const t = el('text', { x: P.x + ux * s * 62 - uy * 9, y: P.y + uy * s * 62 + ux * 9 + 3, class: 'portlabel', 'text-anchor': 'middle' }, L);
        t.textContent = end.port;
      }
    }
  }
  for (const d of world.devices) {
    const sel = UI.sel?.kind === 'dev' && UI.sel.id === d.id;
    const g = el('g', { class: 'node' + (sel ? ' sel' : ''), 'data-dev': d.id, transform: `translate(${d.x} ${d.y})`, tabindex: 0, role: 'button', 'aria-label': `${d.name}, ${TYPES[d.type].label}` }, N);
    el('rect', { x: -44, y: -36, width: 88, height: 98, rx: 10, class: 'halo' }, g);
    const ic = el('g', { transform: 'translate(-32 -32)', opacity: d.power ? 1 : 0.45 }, g);
    ic.innerHTML = ICON[d.type];
    const t = el('text', { y: 38, 'text-anchor': 'middle', class: 'label' }, g); t.textContent = d.name;
    const ip = d.type === 'router' ? (NET.addr.get(d.id + '#lan')?.ip || NET.addr.get(d.id)?.ip) : NET.addr.get(d.id)?.ip;
    const sub = el('text', { y: 52, 'text-anchor': 'middle', class: 'sublabel' }, g);
    sub.textContent = !d.power ? 'off' : ip ? ip : TYPES[d.type].endpoint ? 'no address' : '';
    const badge = nodeBadge(d);
    if (badge) {
      const bg = el('g', { transform: 'translate(22 -30)' }, g);
      el('circle', { r: 8, class: 'badge-bg' }, bg);
      el('circle', { r: 4.5, fill: badge.color }, bg);
      bg.appendChild(document.createElementNS(NS, 'title')).textContent = badge.title;
    }
    if (typeof ENGINE !== 'undefined' && ENGINE.running(d.id)) {
      const vm = el('g', { transform: 'translate(-40 -32)' }, g);
      el('rect', { width: 26, height: 14, rx: 3, fill: 'var(--ok)' }, vm);
      const tt = el('text', { x: 13, y: 10.5, 'text-anchor': 'middle', 'font-size': 9, fill: '#fff', 'font-weight': 700 }, vm); tt.textContent = 'VM';
    }
  }
}
function nodeBadge(d) {
  if (!d.power) return null;
  if (d.type === 'box') {
    const st = NET.losos.box.get(d.id);
    if (!st.up) return { color: 'var(--faint)', title: 'No address' };
    if (st.tunnel === 'refused') return { color: 'var(--crit)', title: 'Tunnel refused: ' + st.reason };
    if (!st.path) return { color: 'var(--warn)', title: 'No edge in reach' };
    return { color: st.path.source === 'lan' ? 'var(--accent)' : 'var(--ok)', title: `Edge: ${nm(st.path.id)}${st.publicName ? ' · ' + st.publicName : ' · LAN only'}` };
  }
  if (d.type === 'edge-local') { const sp = NET.losos.spoke.get(d.id); return sp.uplink === 'up' ? { color: 'var(--ok)', title: 'Uplink to the official edge is up' } : { color: d.cfg.uplink ? 'var(--warn)' : 'var(--faint)', title: 'Uplink: ' + sp.uplink }; }
  if (d.type === 'edge-official') return { color: d.cfg.certified ? 'var(--ok)' : 'var(--warn)', title: d.cfg.certified ? 'Certificate signed by the LosOS root' : 'Not certified: not official' };
  return null;
}

// physical
const HWK = 1.3;
const ROOMS = { home: { x: 20, y: 20, w: 590, h: 360 }, office: { x: 630, y: 20, w: 590, h: 360 }, isp: { x: 20, y: 400, w: 590, h: 310 }, dc: { x: 630, y: 400, w: 590, h: 310 } };
function physPos(d) {
  if (d.px == null) {
    const same = world.devices.filter(x => x.site === d.site && x.px != null);
    const gear = isGear(d);
    let px = gear ? 40 : 60, py = gear ? 50 : 250;
    for (let i = 0; i < 20; i++) { if (!same.some(x => Math.abs(x.px - px) < 110 && Math.abs(x.py - py) < 60)) break; px += 130; if (px > 470) { px = 40; py += gear ? 70 : -80; } }
    d.px = px; d.py = py;
  }
  const R = ROOMS[d.site] || ROOMS.home;
  return { x: R.x + d.px, y: R.y + d.py };
}
function renderPhysical(vp) {
  el('rect', { x: -4000, y: -4000, width: 8000, height: 8000, fill: 'var(--ground)' }, vp);
  for (const [k, R] of Object.entries(ROOMS)) {
    el('rect', { x: R.x, y: R.y, width: R.w, height: R.h, rx: 6, class: 'room', 'data-room': k }, vp);
    const t = el('text', { x: R.x + 14, y: R.y + 22, class: 'room-title' }, vp); t.textContent = SITES[k].name;
    const s = el('text', { x: R.x + 14, y: R.y + 37, class: 'room-sub' }, vp); s.textContent = SITES[k].sub;
    // furniture
    if (k === 'home') {
      el('rect', { x: R.x + 20, y: R.y + 108, width: 300, height: 8, rx: 2, class: 'furn' }, vp);
      el('rect', { x: R.x + 20, y: R.y + R.h - 46, width: R.w - 40, height: 12, rx: 3, class: 'furn' }, vp);
      el('rect', { x: R.x + 40, y: R.y + R.h - 34, width: 8, height: 30, class: 'furn' }, vp);
      el('rect', { x: R.x + R.w - 48, y: R.y + R.h - 34, width: 8, height: 30, class: 'furn' }, vp);
      const l = el('text', { x: R.x + 24, y: R.y + 130, class: 'room-sub' }, vp); l.textContent = 'hallway shelf';
      const l2 = el('text', { x: R.x + 24, y: R.y + R.h - 52, class: 'room-sub' }, vp); l2.textContent = 'desk';
    }
    if (k === 'office') {
      el('rect', { x: R.x + 22, y: R.y + 38, width: 184, height: 196, rx: 3, class: 'rack' }, vp);
      for (let u = 0; u < 12; u++) el('line', { x1: R.x + 28, x2: R.x + 200, y1: R.y + 46 + u * 15.5, y2: R.y + 46 + u * 15.5, stroke: '#3a454f', 'stroke-width': .6 }, vp);
      const l = el('text', { x: R.x + 26, y: R.y + 250, class: 'room-sub' }, vp); l.textContent = 'network cabinet, 12U';
      el('rect', { x: R.x + 220, y: R.y + R.h - 46, width: R.w - 240, height: 12, rx: 3, class: 'furn' }, vp);
      const l2 = el('text', { x: R.x + 224, y: R.y + R.h - 52, class: 'room-sub' }, vp); l2.textContent = 'desks';
    }
    if (k === 'dc') {
      el('rect', { x: R.x + 22, y: R.y + 38, width: 184, height: 250, rx: 3, class: 'rack' }, vp);
      for (let u = 0; u < 15; u++) el('line', { x1: R.x + 28, x2: R.x + 200, y1: R.y + 46 + u * 15.5, y2: R.y + 46 + u * 15.5, stroke: '#3a454f', 'stroke-width': .6 }, vp);
      const l = el('text', { x: R.x + 220, y: R.y + 60, class: 'room-sub' }, vp); l.textContent = 'rented VPS, public IPv4, 1 Gbit/s';
    }
  }
  const C = el('g', {}, vp), H = el('g', {}, vp);
  // Wi-Fi ranges first
  for (const d of world.devices.filter(d => d.type === 'ap' && d.power)) { const p = physPos(d); el('circle', { cx: p.x + 30 * HWK, cy: p.y + 22 * HWK, r: 125, class: 'range' }, C); }
  const portAbs = (d, port) => { const p = physPos(d); const hw = hwDrawing(d, () => 'none'); const pp = hw.ports[port] || [hw.w / 2, hw.h]; return { x: p.x + pp[0] * HWK, y: p.y + pp[1] * HWK }; };
  for (const l of world.links) {
    const A = dev(l.a.dev), B = dev(l.b.dev);
    const a = portAbs(A, l.a.port), b = portAbs(B, l.b.port);
    const sel = UI.sel?.kind === 'link' && UI.sel.id === l.id;
    if (l.kind === 'wifi') {
      el('path', { d: `M${a.x} ${a.y}L${b.x} ${b.y}`, class: 'link wifi' + (sel ? ' sel' : ''), fill: 'none' }, C);
      continue;
    }
    const dist = Math.hypot(b.x - a.x, b.y - a.y);
    const same = A.site === B.site;
    const sag = same ? 30 + dist * 0.18 : 40;
    const d = `M${a.x} ${a.y}C${a.x} ${a.y + sag} ${b.x} ${b.y + sag} ${b.x} ${b.y}`;
    el('path', { d, class: `cable ${same ? l.kind === 'wan' ? 'copper' : l.kind : 'wan'}`, 'stroke-width': sel ? 5 : 3 }, C);
    const hit = el('path', { d, class: 'link-hit', 'data-link': l.id }, C);
    hit.appendChild(document.createElementNS(NS, 'title')).textContent = `${A.name} ${l.a.port} ↔ ${B.name} ${l.b.port}`;
    const mx = (a.x + b.x) / 2, my = (a.y + b.y) / 2 + sag * 0.75;
    const t = el('text', { x: mx, y: my + 12, class: 'meters', 'text-anchor': 'middle' }, C);
    t.textContent = same ? `${Math.max(0.5, dist * 0.02).toFixed(1)} m ${l.kind === 'fiber' ? 'fiber' : 'Cat6'}` : A.site === 'dc' || B.site === 'dc' ? 'datacenter uplink' : 'ISP line';
  }
  for (const d of world.devices) {
    const p = physPos(d);
    const hw = hwDrawing(d, portStateFn(d));
    const sel = UI.sel?.kind === 'dev' && UI.sel.id === d.id;
    const g = el('g', { class: 'hw' + (sel ? ' sel' : ''), 'data-dev': d.id, transform: `translate(${p.x} ${p.y})` }, H);
    const racked = (d.site === 'office' || d.site === 'dc') && d.px < 190;
    el('rect', { x: -5, y: -5, width: hw.w * HWK + 10, height: hw.h * HWK + 10, rx: 6, class: 'hw-halo' }, g);
    const inner = el('g', { transform: `scale(${HWK})` }, g); inner.innerHTML = hw.svg;
    const t = racked || isGear(d)
      ? el('text', { x: hw.w * HWK + 8, y: hw.h * HWK / 2 + 4, class: 'label' }, g)
      : el('text', { x: hw.w * HWK / 2, y: hw.h * HWK + 15, 'text-anchor': 'middle', class: 'label' }, g);
    t.textContent = d.name;
    if (racked && !isGear(d)) t.setAttribute('x', 184 - d.px + 30);
  }
}

// PDUs: envelopes riding the logical links
function drawPdus(force) {
  const layer = $('#pdus'); if (!layer) return;
  layer.innerHTML = '';
  if (UI.view !== 'logical') return;
  const f = SIM.mode === 'simulation' && !SIM.playing && !force ? 1 : SIM.frameFrac;
  for (const a of SIM.active) {
    if (!SIM.filters[a.proto]) continue;
    const i = Math.max(0, a.i - 1);
    // the PDU is between hops[i] and hops[i+1] while moving, else sitting at hops[a.i]
    const fd = dev(a.hops[a.i === 0 ? 0 : a.i - 1]), td = dev(a.hops[a.i]);
    if (!fd || !td) continue;
    const t = a.i === 0 ? 0 : f;
    // through a bus, a frame runs along the cable from one drop to the next
    let x, y;
    if (td.type === 'bus' || fd.type === 'bus') {
      const bus = td.type === 'bus' ? td : fd, other = td.type === 'bus' ? fd : td;
      const p = anchor(bus, other);
      const prevHop = dev(a.hops[a.i - 2]);
      const start = fd.type === 'bus' && prevHop ? anchor(bus, prevHop) : fd.type === 'bus' ? p : fd;
      const end = td.type === 'bus' ? p : td;
      if (fd.type === 'bus') { // along the cable, then down the drop
        if (t < .5) { x = start.x + (p.x - start.x) * t * 2; y = bus.y; } else { x = p.x + (end.x - p.x) * (t - .5) * 2; y = p.y + (end.y - p.y) * (t - .5) * 2; }
      } else { x = start.x + (end.x - start.x) * t; y = start.y + (end.y - start.y) * t; }
    } else { x = fd.x + (td.x - fd.x) * t; y = fd.y + (td.y - fd.y) * t; }
    const g = el('g', { class: 'pdu', transform: `translate(${x - 11} ${y - 8})` }, layer);
    el('rect', { width: 22, height: 15, rx: 2.5, fill: PROTO[a.proto].color, stroke: 'var(--surface)', 'stroke-width': 1.5 }, g);
    el('path', { d: 'M1.5 2L11 9l9.5-7', fill: 'none', stroke: 'rgba(255,255,255,.85)', 'stroke-width': 1.4 }, g);
    if (a.real) el('circle', { cx: 20, cy: 1, r: 3, fill: 'var(--ok)' }, g);
  }
}

// ── Canvas interaction ──────────────────────────────────────────────────
let drag = null;
function onDown(ev) {
  if (ev.button !== 0) return;
  const t = ev.target;
  const pw = t.closest && t.closest('[data-power]');
  if (pw) { togglePower(pw.dataset.power); ev.stopPropagation(); return; }
  const nodeEl = t.closest && t.closest('[data-dev]');
  const linkEl = t.closest && t.closest('[data-link]');
  const w = toWorld(ev);
  if (UI.tool === 'place') {
    let site = 'home';
    if (UI.view === 'physical') { for (const [k, R] of Object.entries(ROOMS)) if (w.x >= R.x && w.x <= R.x + R.w && w.y >= R.y && w.y <= R.y + R.h) site = k; }
    const opts = { site: UI.placeType === 'edge-official' && UI.view !== 'physical' ? 'dc' : site };
    let lx = w.x, ly = w.y;
    if (UI.view === 'physical') { const R = ROOMS[site]; opts.px = Math.round(w.x - R.x - 40); opts.py = Math.round(w.y - R.y - 20); lx = 200 + world.devices.length * 60; ly = 300; }
    const d = newDevice(UI.placeType, Math.round(lx), Math.round(ly), opts);
    UI.sel = { kind: 'dev', id: d.id }; UI.tab = d.type === 'laptop' ? 'desktop' : 'config';
    if (!ev.shiftKey) { UI.tool = 'select'; UI.placeType = null; }
    changed(); return;
  }
  if (nodeEl) {
    const id = nodeEl.dataset.dev;
    if (UI.tool === 'delete') { removeDevice(id); if (UI.sel?.id === id) UI.sel = null; changed(); return; }
    if (UI.tool === 'connect') {
      if (!UI.connectFrom) { UI.connectFrom = id; renderCanvas(); return; }
      const r = connect(UI.connectFrom, id, UI.linkKind);
      if (r.error) toast(r.error); else { UI.linkBorn.set(r.link.id, performance.now()); setTimeout(() => renderCanvas(), 1500); toast(`Connected ${nm(r.link.a.dev)} ${r.link.a.port} to ${nm(r.link.b.dev)} ${r.link.b.port}.`); }
      UI.connectFrom = null; if (!ev.shiftKey) { UI.tool = 'select'; UI.linkKind = null; }
      changed(); return;
    }
    const d = dev(id);
    const wasSel = UI.sel?.kind === 'dev' && UI.sel.id === id;
    UI.sel = { kind: 'dev', id };
    if (!wasSel) { const T = d.type; if (!['status', 'config', 'physical', 'console', 'desktop'].includes(UI.tab) || (UI.tab === 'desktop' && T !== 'laptop') || (UI.tab === 'console' && !consoleable(d))) UI.tab = 'status'; renderSide(); }
    drag = { kind: 'node', id, sx: ev.clientX, sy: ev.clientY, ox: UI.view === 'logical' ? d.x : d.px, oy: UI.view === 'logical' ? d.y : d.py, site: d.site, moved: false };
    renderCanvas(); renderTray();
    svg().setPointerCapture(ev.pointerId); return;
  }
  if (linkEl) {
    const id = linkEl.dataset.link;
    if (UI.tool === 'delete') { removeLink(id); changed(); return; }
    UI.sel = { kind: 'link', id }; renderCanvas(); renderSide(); return;
  }
  drag = { kind: 'pan', sx: ev.clientX, sy: ev.clientY, ox: camera().x, oy: camera().y, moved: false };
  svg().setPointerCapture(ev.pointerId);
}
function onMove(ev) {
  if (UI.tool === 'connect' && UI.connectFrom && UI.view === 'logical') {
    const a = dev(UI.connectFrom), w = toWorld(ev);
    const rb = $('#rubber'); if (rb) rb.setAttribute('d', `M${a.x} ${a.y}L${w.x} ${w.y}`);
  }
  if (!drag) return;
  const dx = ev.clientX - drag.sx, dy = ev.clientY - drag.sy;
  if (Math.abs(dx) + Math.abs(dy) > 3) drag.moved = true;
  if (drag.kind === 'pan') { camera().x = drag.ox + dx; camera().y = drag.oy + dy; const vp = $('#vp'); const c = camera(); vp.setAttribute('transform', `translate(${c.x} ${c.y}) scale(${c.k})`); return; }
  const d = dev(drag.id); const k = camera().k;
  if (UI.view === 'logical') { d.x = Math.round(drag.ox + dx / k); d.y = Math.round(drag.oy + dy / k); }
  else { d.px = Math.round(drag.ox + dx / k); d.py = Math.round(drag.oy + dy / k); }
  renderCanvas();
}
function onUp(ev) {
  if (drag && drag.kind === 'node' && UI.view === 'physical' && drag.moved) {
    const d = dev(drag.id); const p = physPos(d);
    for (const [k, R] of Object.entries(ROOMS)) {
      if (p.x + 40 >= R.x && p.x + 40 <= R.x + R.w && p.y + 20 >= R.y && p.y + 20 <= R.y + R.h && k !== d.site) {
        d.px = Math.round(p.x - R.x); d.py = Math.round(p.y - R.y); d.site = k; toast(`${d.name} moved to ${SITES[k].name}.`); renderSide();
      }
    }
    const R = ROOMS[d.site]; d.px = Math.max(0, Math.min(R.w - 60, d.px)); d.py = Math.max(30, Math.min(R.h - 40, d.py));
    renderCanvas();
  }
  if (drag && drag.kind === 'node' && drag.moved) UI.dirty = true;
  if (drag && drag.kind === 'pan' && !drag.moved && UI.tool === 'select') { UI.sel = null; renderCanvas(); renderSide(); }
  drag = null;
}
function onWheel(ev) {
  ev.preventDefault();
  const r = svg().getBoundingClientRect(); const c = camera();
  const mx = ev.clientX - r.left, my = ev.clientY - r.top;
  const k2 = Math.max(0.3, Math.min(2.5, c.k * Math.exp(-ev.deltaY * 0.0015)));
  c.x = mx - (mx - c.x) * (k2 / c.k); c.y = my - (my - c.y) * (k2 / c.k); c.k = k2;
  renderCanvas();
}
function zoomBy(f) { const r = svg().getBoundingClientRect(); onWheel({ preventDefault() {}, clientX: r.left + r.width / 2, clientY: r.top + r.height / 2, deltaY: -Math.log(f) / 0.0015 }); }
function togglePower(id) {
  const d = dev(id); d.power = !d.power;
  if (typeof ENGINE !== 'undefined' && ENGINE.available && TYPES[d.type].emulate) { if (d.power) ENGINE.start(d.id); else ENGINE.stop(d.id); }
  const t = TERMS.get(id); if (t) { if (d.power) { evaluate(); t.println('\x1b[90m[sim] power on\x1b[0m'); t.println(banner(d).join('\n')); } else t.println('\x1b[90m[sim] power off\x1b[0m'); }
  toast(`${d.name} powered ${d.power ? 'on' : 'off'}.`);
  changed();
}

// ── Tray ──────────────────────────────────────────────────────────────
const CATS = { losos: 'LosOS', net: 'Network', end: 'End devices', links: 'Connections' };
function renderTray() {
  const cats = $('#trayCats'); cats.innerHTML = '';
  for (const [k, v] of Object.entries(CATS)) { const b = document.createElement('button'); b.textContent = v; b.setAttribute('aria-pressed', String(UI.cat === k)); b.onclick = () => { UI.cat = k; renderTray(); }; cats.appendChild(b); }
  const items = $('#trayItems'); items.innerHTML = '';
  if (UI.cat === 'links') {
    for (const [k, v] of Object.entries(LINK_KINDS)) {
      const b = document.createElement('button'); b.className = 'titem'; b.setAttribute('aria-pressed', String(UI.tool === 'connect' && UI.linkKind === k));
      const stroke = k === 'fiber' ? 'var(--fiber)' : k === 'wifi' ? 'var(--wifi)' : k === 'auto' ? 'var(--ink)' : 'var(--wire)';
      b.innerHTML = `<svg viewBox="0 0 64 56"><path d="M8 44C24 44 18 12 56 12" fill="none" stroke="${stroke}" stroke-width="3.5" ${k === 'wifi' ? 'stroke-dasharray="2 6" stroke-linecap="round"' : ''}/>${k === 'auto' ? '<path d="M44 34l4-8 4 8-4 8z" fill="var(--accent)"/>' : ''}</svg><span>${v.label}</span><small>${v.hint}</small>`;
      b.onclick = () => { UI.tool = 'connect'; UI.linkKind = k; UI.connectFrom = null; UI.placeType = null; renderTray(); renderCanvas(); };
      items.appendChild(b);
    }
    const del = document.createElement('button'); del.className = 'titem'; del.setAttribute('aria-pressed', String(UI.tool === 'delete'));
    del.innerHTML = `<svg viewBox="0 0 64 56"><path d="M20 16l24 24M44 16L20 40" stroke="var(--crit)" stroke-width="4" stroke-linecap="round"/></svg><span>Delete</span><small>Remove a device or cable.</small>`;
    del.onclick = () => { UI.tool = UI.tool === 'delete' ? 'select' : 'delete'; renderTray(); renderCanvas(); };
    items.appendChild(del);
    return;
  }
  for (const [k, T] of Object.entries(TYPES)) {
    if (T.cat !== UI.cat) continue;
    const b = document.createElement('button'); b.className = 'titem'; b.setAttribute('aria-pressed', String(UI.tool === 'place' && UI.placeType === k));
    b.innerHTML = `${iconSvg(k, 48)}<span>${T.short}</span><small>${T.cat === 'losos' ? (typeof ENGINE !== 'undefined' && ENGINE.available ? (ENGINE.libvirt ? 'boots under libvirt' : 'boots in qemu-wasm') : 'LosOS device') : T.cat === 'end' ? 'LosOS Desktop' : 'generic gear'}</small>`;
    b.title = T.blurb;
    b.onclick = () => { UI.tool = 'place'; UI.placeType = k; UI.connectFrom = null; renderTray(); renderCanvas(); };
    items.appendChild(b);
  }
}
function consoleable(d) { return ['box', 'edge-local', 'edge-official', 'router', 'switch', 'ap'].includes(d.type); }

window.addEventListener('keydown', (e) => {
  if (e.target.closest && e.target.closest('input, textarea, select, .term, .xterm')) return;
  if (e.key === 'Escape') { UI.tool = 'select'; UI.placeType = null; UI.linkKind = null; UI.connectFrom = null; renderTray(); renderCanvas(); }
  if ((e.key === 'Delete' || e.key === 'Backspace') && UI.sel) {
    if (UI.sel.kind === 'dev') removeDevice(UI.sel.id); else removeLink(UI.sel.id);
    UI.sel = null; changed(); e.preventDefault();
  }
});
