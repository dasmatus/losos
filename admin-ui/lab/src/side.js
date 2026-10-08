// ── Inspector ─────────────────────────────────────────────────────────
function tabsFor(d) {
  const t = [['status', 'Status'], ['config', 'Config'], ['physical', 'Physical']];
  if (consoleable(d)) t.splice(1, 0, ['console', 'Console']);
  if (d.type === 'laptop') t.unshift(['desktop', 'Desktop']);
  return t;
}
function renderSide() {
  const side = $('#side');
  if (!UI.sel) document.getElementById('app').classList.remove('conwide');
  side.innerHTML = '';
  if (!UI.sel || (UI.sel.kind === 'dev' && !dev(UI.sel.id)) || (UI.sel.kind === 'link' && !world.links.find(l => l.id === UI.sel.id))) { UI.sel = null; side.appendChild(emptySide()); return; }
  if (UI.sel.kind === 'link') { side.appendChild(linkSide(world.links.find(l => l.id === UI.sel.id))); return; }
  const d = dev(UI.sel.id);
  const head = document.createElement('div'); head.className = 'side-head';
  head.innerHTML = `${iconSvg(d.type, 44)}<div style="min-width:0"><h2>${esc(d.name)}</h2><div class="sub">${esc(TYPES[d.type].label)} · ${esc(SITES[d.site].name)} · ${d.power ? 'on' : 'off'}</div></div><span class="spacer"></span><button class="btn small" id="pwrBtn" title="Power">${d.power ? 'Power off' : 'Power on'}</button>`;
  side.appendChild(head);
  $('#pwrBtn', head).onclick = () => togglePower(d.id);
  const tabs = document.createElement('div'); tabs.className = 'tabs'; tabs.setAttribute('role', 'tablist');
  const list = tabsFor(d);
  if (!list.some(t => t[0] === UI.tab)) UI.tab = list[0][0];
  // a serial console wants 72 columns: the inspector widens while it shows one
  const app = document.getElementById('app'), wantWide = UI.tab === 'console';
  if (app.classList.contains('conwide') !== wantWide) { app.classList.toggle('conwide', wantWide); requestAnimationFrame(() => { fitView(); renderCanvas(); }); }
  for (const [k, v] of list) { const b = document.createElement('button'); b.setAttribute('role', 'tab'); b.textContent = v; b.setAttribute('aria-selected', String(UI.tab === k)); b.onclick = () => { UI.tab = k; renderSide(); }; tabs.appendChild(b); }
  side.appendChild(tabs);
  const pane = document.createElement('div'); pane.className = 'pane'; pane.id = 'pane';
  side.appendChild(pane);
  if (UI.tab === 'status') statusPane(pane, d);
  if (UI.tab === 'config') configPane(pane, d);
  if (UI.tab === 'physical') physicalPane(pane, d);
  if (UI.tab === 'console') consolePane(pane, d);
  if (UI.tab === 'desktop') desktopPane(pane, d);
}
function refreshSideLive() { if (UI.sel?.kind === 'dev' && UI.tab === 'status') { const p = $('#pane'); if (p) { p.innerHTML = ''; statusPane(p, dev(UI.sel.id)); } } else if (UI.tab !== 'console' && UI.tab !== 'desktop') renderSide(); }
function emptySide() {
  const e = document.createElement('div'); e.className = 'empty';
  const sc = SCENARIOS[UI.scenario] || { name: UI.fileName || 'My setup', blurb: 'Opened from a setup file. Save downloads it again with your changes.' };
  e.innerHTML = `<h3>${esc(sc.name)}</h3><p>${esc(sc.blurb)}</p>
  <ol><li>Click a device to see what LosOS made of the network: its address, the edges it found, its tunnel and public name.</li>
  <li>Open the laptop's <b>Desktop</b> tab and browse to a box by its <code>.local</code> name, or by its public name from another site.</li>
  <li>Switch to <b>Simulation</b> to step through every DHCP, mDNS, HTTP and tunnel packet, like Packet Tracer's event list.</li>
  <li>Pull a cable, power off the gateway or the official edge, and watch the boxes pick a new path.</li></ol>
  <p><b>What is real here.</b> ${typeof ENGINE !== 'undefined' && ENGINE.available ? `Each powered box and edge boots a real x86_64 Linux guest ${ENGINE.libvirt ? 'under libvirt (' + esc(ENGINE.libvirt.label) + ')' : 'under qemu-wasm'}; its Ethernet frames cross the simulated switches, and the routers answer DHCP, ARP and ping.` : 'In this viewer the consoles are simulated, because qemu-wasm needs a cross-origin isolated page. The standalone build boots a real x86_64 guest per device.'} The LosOS behaviour (edge discovery, the path rule, enrolment, relaying, the lanOnly guard) is modelled on the rules in the LosOS wiki.</p>`;
  return e;
}
function linkSide(l) {
  const e = document.createElement('div'); e.className = 'pane';
  const A = dev(l.a.dev), B = dev(l.b.dev);
  e.innerHTML = `<div class="side-head" style="padding:0 0 10px"><div><h2>${esc(LINK_KINDS[l.kind]?.label || (l.kind === 'wan' ? 'WAN line' : l.kind))} link</h2><div class="sub">${esc(A.name)} ${esc(l.a.port)} ↔ ${esc(B.name)} ${esc(l.b.port)}</div></div></div>
  <dl class="kv"><dt>State</dt><dd>${linkState(l) === 'up' ? '<span class="tag ok">up</span>' : '<span class="tag crit">down</span> one end is off'}</dd><dt>Medium</dt><dd>${l.kind === 'wifi' ? `802.11, SSID ${esc(dev(l.a.dev).cfg.ssid || 'losos-lab')}` : l.kind === 'fiber' ? '1000BASE-LX' : l.kind === 'wan' ? 'ISP access line' : '1000BASE-T, Cat6'}</dd></dl>
  <button class="btn" id="delLink">Remove this ${l.kind === 'wifi' ? 'association' : 'cable'}</button>`;
  setTimeout(() => { const b = $('#delLink'); if (b) b.onclick = () => { removeLink(l.id); UI.sel = null; changed(); }; });
  return e;
}
function statusPane(p, d) {
  const a = NET.addr.get(d.id) || (d.type === 'router' ? NET.addr.get(d.id + '#lan') : null);
  const f = NET.iface.get(d.id);
  let h = `<dl class="kv">`;
  if (d.type === 'router') {
    const w = NET.addr.get(d.id);
    h += `<dt>LAN</dt><dd class="mono">${esc(NET.subnet.get(d.id))}.0/24 · gateway .1</dd><dt>DHCP</dt><dd>${d.power ? 'serving .100–.199' : 'off'}</dd><dt>WAN</dt><dd class="mono">${w ? esc(w.ip) + ' (' + esc(w.src) + ')' : 'not connected'}</dd><dt>NAT</dt><dd>outbound only: nothing on the internet can open a connection to this LAN</dd>`;
  } else if (TYPES[d.type].endpoint) {
    h += `<dt>Interface</dt><dd class="mono">${f ? esc(f.port) : '—'}</dd><dt>Address</dt><dd class="mono">${a ? `${esc(a.ip)}/${a.mask} <span class="tag">${esc(a.src)}</span>` : 'none'}</dd>`;
    if (a?.gw) h += `<dt>Gateway</dt><dd class="mono">${esc(a.gw)}</dd>`;
    h += `<dt>Internet</dt><dd>${NET.internet.get(d.id) ? '<span class="tag ok">yes</span>' : '<span class="tag">no</span>'}</dd>`;
    if (a && a.src !== 'public') h += `<dt>mDNS name</dt><dd class="mono">${esc(d.name)}.local</dd>`;
    if (typeof ENGINE !== 'undefined' && ENGINE.available && TYPES[d.type].emulate) h += `<dt>Guest</dt><dd>${ENGINE.running(d.id) ? `<span class="tag ok">${esc(ENGINE.backend(d.id))}, running</span>` : 'not running'}</dd>`;
  } else {
    h += `<dt>Ports</dt><dd>${TYPES[d.type].ports.length}</dd><dt>In use</dt><dd>${linksOf(d).length}</dd>`;
  }
  h += `</dl>`;
  if (d.type === 'box') {
    const st = NET.losos.box.get(d.id);
    h += `<div class="sect">Edges in reach</div>`;
    if (!st.edges.length) h += `<div class="status-card"><div class="h"><span class="dot warn"></span>No edge proxy found</div><p>Tried mDNS <code>_losos-edge._tcp</code> on this network${d.cfg.proxy ? ' and the configured address' : ''}. Turning sharing on is refused (409 edgeRequired); the box's own apps don't care.</p></div>`;
    for (const e of st.edges) h += `<div class="status-card"><div class="h"><span style="color:var(--${e.official ? 'ok' : 'warn'})">${e.official ? '✓' : '⚠'}</span>${esc(nm(e.id))} <span class="tag">${e.source === 'lan' ? 'found on LAN' : 'configured'}</span>${st.path?.id === e.id ? '<span class="tag acc">path</span>' : ''}</div><p class="mono">${esc(e.url)}</p>${e.official ? '' : '<p>Not official: buying on the market and selling this box\'s spare storage and compute are not possible through it.</p>'}</div>`;
    h += `<div class="sect">Tunnel and names</div><dl class="kv">
      <dt>Path</dt><dd>${st.path ? esc(nm(st.path.id)) + ` <span class="tag">${st.path.source === 'lan' ? 'local edge first' : 'official edge second'}</span>` : 'none'}</dd>
      <dt>Tunnel</dt><dd>${tunnelTag(st)}</dd>
      <dt>Public name</dt><dd class="mono">${st.publicName ? 'https://' + esc(st.publicName) : '—'}</dd>
      <dt>Mesh</dt><dd>${esc(st.mesh)}</dd><dt>Market</dt><dd>${st.market === 'available' ? '<span class="tag ok">available</span>' : esc(st.market)}</dd></dl>`;
    h += `<button class="btn" id="scanNow">Scan for edges now</button>`;
  }
  if (d.type === 'edge-local') {
    const sp = NET.losos.spoke.get(d.id);
    h += `<div class="sect">Gateway</div><dl class="kv"><dt>LAN advert</dt><dd>${d.cfg.advertise ? '_losos-edge._tcp, enrol=' + (d.cfg.openEnrolment ? 'open' : 'closed') : 'off'}</dd>
      <dt>Uplink</dt><dd>${sp.uplink === 'up' ? '<span class="tag ok">up</span> to ' + esc(nm(sp.hub)) : esc(sp.uplink)}</dd><dt>Zone</dt><dd class="mono">${esc(d.cfg.zone)}</dd>
      <dt>Enrolled</dt><dd>${sp.enrolled.map(nm).map(esc).join(', ') || '—'}</dd><dt>Relayed</dt><dd class="mono">${sp.relayed.map(b => esc(NET.losos.box.get(b).publicName)).join('<br>') || '—'}</dd>
      <dt>Mesh</dt><dd>${d.cfg.cluster ? 'rke2 server · ' + (sp.cluster.map(nm).map(esc).join(', ') || 'no nodes') : 'off'}</dd></dl>`;
  }
  if (d.type === 'edge-official') {
    const hb = NET.losos.hub.get(d.id);
    h += `<div class="sect">Official edge</div><dl class="kv"><dt>Certificate</dt><dd>${d.cfg.certified ? '<span class="tag ok">signed by the LosOS root</span>' : '<span class="tag warn">not signed</span>'}</dd><dt>Registrar</dt><dd class="mono">register.${esc(d.cfg.domain)}</dd>
      <dt>Tenants</dt><dd>${hb.tenants.map(nm).map(esc).join(', ') || '—'}</dd><dt>Relayed</dt><dd class="mono">${hb.relays.map(b => esc(NET.losos.box.get(b).publicName)).join('<br>') || '—'}</dd><dt>Mesh</dt><dd>${hb.cluster.map(nm).map(esc).join(', ') || '—'}</dd></dl>`;
  }
  if (d.type === 'laptop') h += `<p class="help">Open the <b>Desktop</b> tab to browse with Danube or use the terminal.</p>`;
  p.innerHTML = h;
  const sb = $('#scanNow', p); if (sb) sb.onclick = () => { startFlow(flowScan(d.id), { title: 'scan' }); toast('Scanning: mDNS on the LAN, then the configured address.'); };
}
function tunnelTag(st) {
  if (st.tunnel === 'enrolled' || st.tunnel === 'registered') return `<span class="tag ok">${st.tunnel}</span>`;
  if (st.tunnel === 'refused') return `<span class="tag crit">refused</span> ${esc(st.reason)}`;
  if (st.tunnel === 'stopped') return `<span class="tag warn">stopped</span> ${esc(st.reason)}`;
  return 'off';
}
function configPane(p, d) {
  const rows = [];
  const tog = (key, label, help) => rows.push(`<div class="row"><label for="cfg-${key}">${label}</label><input type="checkbox" class="toggle" id="cfg-${key}" data-key="${key}" ${d.cfg[key] ? 'checked' : ''}></div>${help ? `<p class="help">${help}</p>` : ''}`);
  const txt = (key, label, help) => rows.push(`<div class="row"><label for="cfg-${key}">${label}</label><input type="text" id="cfg-${key}" data-key="${key}" value="${esc(d.cfg[key] || '')}" spellcheck="false"></div>${help ? `<p class="help">${help}</p>` : ''}`);
  rows.push(`<div class="row"><label for="cfg-name">Host name</label><input type="text" id="cfg-name" value="${esc(d.name)}" spellcheck="false"></div>`);
  rows.push(`<div class="row"><label for="cfg-site">Location</label><select id="cfg-site">${Object.entries(SITES).map(([k, v]) => `<option value="${k}" ${d.site === k ? 'selected' : ''}>${v.name}</option>`).join('')}</select></div>`);
  if (d.type === 'box') {
    rows.push('<div class="sect">losos.proxy</div>');
    tog('proxy', 'Master proxy (<code>losos.proxy.enable</code>)', 'Tunnel to an edge so the box is reachable from outside without opening a port.');
    tog('tenantOnHub', 'Listed as a tenant on the official edge', 'The hub only routes ids in <code>losos.edge.tenants</code>.');
    rows.push('<div class="sect">Mesh pane</div>');
    tog('joinMesh', 'Join the mesh', 'Refused with 409 edgeRequired while no edge answers.');
    tog('shareCompute', 'Share compute when I sleep');
  }
  if (d.type === 'edge-local') {
    rows.push('<div class="sect">losos.edge</div>');
    tog('advertise', 'Advertise on the LAN (<code>lan.advertise</code>)', 'Publishes <code>_losos-edge._tcp</code> over mDNS.');
    tog('openEnrolment', 'Open enrolment (<code>lan.openEnrolment</code>)', 'Unknown boxes enrol trust-on-first-use. Never on a VPS.');
    tog('cluster', 'Mesh control plane (<code>cluster.enable</code>)', 'An rke2 server with Longhorn for this site.');
    tog('uplink', 'Uplink to the official edge (<code>uplink.enable</code>)');
    txt('zone', 'Relay zone', 'Boxes become <code>&lt;box&gt;.&lt;zone&gt;</code> on the hub.');
  }
  if (d.type === 'edge-official') {
    rows.push('<div class="sect">losos.edge</div>');
    tog('certified', 'Certificate signed by the LosOS root', 'Without it the edge answers the identity challenge but is not official: no market.');
    tog('acceptsRelay', 'Tenant <code>acme</code> may relay (<code>relayZone</code>)');
    tog('cluster', 'Mesh control plane');
    txt('domain', 'Public domain');
  }
  if (d.type === 'router') txt('subnet', 'LAN prefix', 'Three octets, for example <code>192.168.1</code>. Empty picks one.');
  if (d.type === 'ap') txt('ssid', 'SSID');
  rows.push(`<div class="sect">Device</div><button class="btn" id="delDev">Remove ${esc(d.name)}</button>`);
  p.innerHTML = rows.join('');
  p.querySelectorAll('input.toggle').forEach(i => i.onchange = () => { d.cfg[i.dataset.key] = i.checked; changed({ keepSide: true }); toast(`${d.name}: ${i.dataset.key} ${i.checked ? 'on' : 'off'}.`); });
  p.querySelectorAll('input[type=text][data-key]').forEach(i => i.onchange = () => { d.cfg[i.dataset.key] = i.value.trim(); changed({ keepSide: true }); });
  $('#cfg-name', p).onchange = (e) => { const v = e.target.value.trim().toLowerCase().replace(/[^a-z0-9-]/g, '-').slice(0, 30); if (!v || world.devices.some(x => x !== d && x.name === v)) { e.target.value = d.name; return toast('Pick a host name no other device uses.'); } d.name = v; e.target.value = v; changed({ keepSide: true }); renderSide(); };
  $('#cfg-site', p).onchange = (e) => { d.site = e.target.value; d.px = null; renderAll(); };
  $('#delDev', p).onclick = () => { removeDevice(d.id); UI.sel = null; changed(); };
}
function physicalPane(p, d) {
  const hw = hwDrawing(d, portStateFn(d));
  const pad = 14;
  p.innerHTML = `<svg class="front" viewBox="${-pad} ${-pad} ${hw.w + pad * 2} ${hw.h + pad * 2}" role="img" aria-label="Front panel of ${esc(d.name)}">${hw.svg}</svg>
    <p class="help" style="margin-top:6px">Click the power LED or use the button above. Port LEDs: green link, amber negotiating, dark none.</p>
    <div class="sect">Ports</div><dl class="kv">${TYPES[d.type].ports.map(([port, k]) => {
      const ls = world.links.filter(l => (l.a.dev === d.id && l.a.port === port) || (l.b.dev === d.id && l.b.port === port));
      return `<dt class="mono">${port}</dt><dd>${ls.length ? ls.map(l => { const o = otherEnd(l, d.id); return `${esc(nm(o.dev))} ${esc(o.port)} <span class="tag ${linkState(l) === 'up' ? 'ok' : 'crit'}">${linkState(l)}</span>`; }).join('<br>') : `<span style="color:var(--muted)">${k === 'wifi-ap' ? 'no clients' : 'empty'}</span>`}</dd>`;
    }).join('')}</dl><dl class="kv"><dt>MAC</dt><dd class="mono">${d.mac}</dd><dt>Location</dt><dd>${esc(SITES[d.site].name)}, ${esc(SITES[d.site].sub)}</dd></dl>`;
  p.querySelector('svg').addEventListener('click', (e) => { const pw = e.target.closest('[data-power]'); if (pw) togglePower(d.id); });
}
function consolePane(p, d) {
  p.classList.add('flush');
  const head = document.createElement('div'); head.className = 'term-head';
  const emu = typeof ENGINE !== 'undefined' && ENGINE.available;
  const backend = emu ? ENGINE.backend(d.id) : '';
  head.innerHTML = backend
    ? `<span class="dot ${ENGINE.running(d.id) ? 'ok' : ''}"></span><b>${esc(backend)}</b><span>x86_64 guest, serial console${ENGINE.running(d.id) ? '' : ' · not running'}</span>`
    : `<span class="dot warn"></span><b>Simulated console</b><span>answers from the model</span>`;
  p.appendChild(head);
  if (emu) { ENGINE.attach(d.id, p); return; }
  termFor(d.id).attach(p);
}

// ── LosOS Desktop on the laptop ─────────────────────────────────────────
const DESK = new Map();
function deskState(id) { if (!DESK.has(id)) DESK.set(id, { app: 'danube', open: ['danube'], url: '', html: '', history: [], loading: false, signedIn: false, err: '', notes: '' }); return DESK.get(id); }
// LosOS Desktop as its docs show it: the systemd-homed sign-in screen, then
// derisk's overview (workspaces along the top, window tiles, and the clock,
// suggested apps, services, calendar and notes beside them). Danube is the
// browser; the Terminal runs the simulator's shell.
const DESK_APPS = {
  danube: { name: 'Danube', icon: '<svg viewBox="0 0 24 24" width="18" height="18"><circle cx="12" cy="12" r="9.5" fill="#1e88c7"/><path d="M3.5 13.5c3-2.6 6-2.6 8.5 0s5.6 2.6 8.5 0" fill="none" stroke="#fff" stroke-width="1.8" stroke-linecap="round"/><path d="M4.5 8.5c2.6-2 5.2-2 7.5 0s5 2 7.5 0" fill="none" stroke="#bfe6fb" stroke-width="1.4" stroke-linecap="round"/></svg>' },
  term: { name: 'Terminal', icon: '<svg viewBox="0 0 24 24" width="18" height="18"><rect x="2.5" y="4" width="19" height="16" rx="2.5" fill="#26303d" stroke="#8a97a6"/><path d="M6 9l3 3-3 3M11 15h6" fill="none" stroke="#a3e635" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/></svg>' },
  files: { name: 'Files', icon: '<svg viewBox="0 0 24 24" width="18" height="18"><rect x="3" y="5" width="18" height="15" rx="2" fill="#c9cdd2"/><rect x="3" y="5" width="18" height="5" rx="2" fill="#e8763c"/><rect x="9" y="13" width="6" height="2.5" rx="1" fill="#6b737c"/></svg>' },
  bazaar: { name: 'Bazaar', icon: '<svg viewBox="0 0 24 24" width="18" height="18"><path d="M5 9h14l-1.2 11H6.2z" fill="#4a90d9"/><path d="M8.5 9V7a3.5 3.5 0 0 1 7 0v2" fill="none" stroke="#cfe3f7" stroke-width="1.8"/></svg>' },
};
function deskClock(now) { return { hm: now.toTimeString().slice(0, 5), day: now.toLocaleDateString('en-GB', { weekday: 'short', day: 'numeric', month: 'short' }).replace(',', '') }; }
function calendarHtml(now) {
  const y = now.getFullYear(), m = now.getMonth(), first = (new Date(y, m, 1).getDay() + 6) % 7, days = new Date(y, m + 1, 0).getDate();
  let h = '<div class="cal">' + ['M', 'T', 'W', 'T', 'F', 'S', 'S'].map(x => `<i>${x}</i>`).join('');
  for (let i = 0; i < first; i++) h += '<span></span>';
  for (let d = 1; d <= days; d++) h += `<span${d === now.getDate() ? ' class="today"' : ''}>${d}</span>`;
  return h + '</div>';
}
function desktopPane(p, d) {
  p.classList.add('flush');
  const s = deskState(d.id);
  const f = NET.iface.get(d.id);
  const a = NET.addr.get(d.id);
  const now = new Date(CLOCK.t);
  const ck = deskClock(now);
  const wide = document.getElementById('app').classList.contains('deskwide');
  const wrap = document.createElement('div'); wrap.className = 'desk' + (wide ? ' wide' : '');
  if (!d.power) { wrap.innerHTML = '<div class="desk-off">The laptop is off. Press its power button on the Physical view, or Power on above.</div>'; p.appendChild(wrap); return; }
  const wifiAp = dev(world.links.find(l => l.kind === 'wifi' && (l.a.dev === d.id || l.b.dev === d.id))?.a.dev);
  const net = f ? (f.port === 'wlan0' ? `Wi-Fi ${esc(wifiAp?.cfg.ssid || '')}` : 'Wired') + (a ? ' · ' + a.ip : '') : 'Offline';
  const expand = `<button class="dk-expand" id="dWide" aria-label="${wide ? 'Shrink' : 'Expand'} the laptop screen" title="${wide ? 'Shrink' : 'Expand'} the laptop screen">${wide ? '⤡' : '⤢'}</button>`;
  if (!s.signedIn) {
    wrap.classList.add('login');
    wrap.innerHTML = `${expand}<div class="dk-login"><div class="big">${ck.hm}</div><div class="date">${ck.day}</div><div class="user">matus</div>
      <input id="dPw" type="password" aria-label="Password" placeholder="${s.err ? 'Sorry, try again' : 'Password'}" autocomplete="off">
      <div class="${s.err ? 'bad' : 'hint'}">${s.err ? 'Password incorrect or not sufficient for authentication of user matus.' : 'Enter your password to log in'}</div>
      <div class="homed">systemd-homed · LUKS home for matus</div></div>`;
    p.appendChild(wrap);
    const pw = $('#dPw', wrap);
    pw.onkeydown = (e) => { if (e.key !== 'Enter') return; if (!pw.value) { s.err = 'empty'; renderSide(); return; } s.err = ''; s.signedIn = true; renderSide(); };
    $('#dWide', wrap).onclick = toggleDeskWide;
    setTimeout(() => pw.focus({ preventScroll: true }), 30);
    return;
  }
  const app = DESK_APPS[s.app];
  const tile = (k) => `<button class="mini${k === s.app ? ' on' : ''}" data-app="${k}" title="${DESK_APPS[k].name}">${DESK_APPS[k].icon}</button>`;
  wrap.innerHTML = `<div class="desk-bar"><span class="sq" aria-hidden="true"></span><span class="search">Search or ask…</span><span class="cur">${app.icon}${app.name}</span><span class="win">Window</span>
      <span class="net">${f ? '<svg viewBox="0 0 16 16" width="13" height="13"><path d="M1.5 6a9.5 9.5 0 0 1 13 0M4 8.6a6 6 0 0 1 8 0M6.4 11.1a2.4 2.4 0 0 1 3.2 0" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round"/><circle cx="8" cy="13.4" r="1.1" fill="currentColor"/></svg>' : ''}${net}</span><span class="wsn" title="Workspaces">⯌ ${s.open.length}</span>${expand}</div>
    <div class="desk-body"><div class="desk-tiles"><div class="ws"><div class="on">${s.open.map(tile).join('')}</div><div class="plus" aria-hidden="true">+</div></div>
      <div class="dwin" id="dwin"></div></div>
      <div class="side-wid">
        <div class="w-clock"><div class="clock">${ck.hm}</div><div class="muted">${ck.day}</div></div>
        <div><div class="wt">Suggested</div><div class="apps">${Object.entries(DESK_APPS).map(([k, v]) => `<button data-app="${k}">${v.icon}<span>${v.name}</span></button>`).join('')}</div></div>
        <div><div class="wt">Services</div><div class="muted">All user services running ✓</div></div>
        <div class="w-cal"><div class="wt">${ck.day}</div>${calendarHtml(now)}</div>
        <div class="w-notes"><div class="wt">Notes</div><textarea id="dNotes" aria-label="Notes" placeholder="Write something down">${esc(s.notes)}</textarea></div>
      </div></div>`;
  p.appendChild(wrap);
  $('#dWide', wrap).onclick = toggleDeskWide;
  const notes = $('#dNotes', wrap); notes.oninput = () => { s.notes = notes.value; };
  wrap.querySelectorAll('[data-app]').forEach(b => b.onclick = () => {
    const k = b.dataset.app;
    if (k === 'files' || k === 'bazaar') { toast(`${DESK_APPS[k].name} is part of LosOS Desktop; the simulator only runs Danube and the Terminal.`); return; }
    s.app = k; if (!s.open.includes(k)) s.open.push(k); renderSide();
  });
  const win = $('#dwin', wrap);
  if (s.app === 'term') {
    win.innerHTML = '<div class="dwin-head"><b>Terminal</b><span class="muted">matus@' + esc(d.name) + '</span></div>';
    const t = termFor(d.id); t.prompt = () => `matus@${d.name}:~$ `;
    const host = document.createElement('div'); host.className = 'dk-term'; win.appendChild(host); t.attach(host); return;
  }
  const suggestions = suggestUrls(d.id);
  win.innerHTML = `<div class="dwin-head"><button id="dBack" aria-label="Back">‹</button><button id="dReload" aria-label="Reload">↻</button><input id="dUrl" aria-label="Address" list="dSugg" value="${esc(s.url)}" placeholder="Type a box name, like mattbox.local"><datalist id="dSugg">${suggestions.map(u => `<option value="${esc(u)}">`).join('')}</datalist></div><div class="dwin-body" id="dBody"></div>`;
  const body = $('#dBody', win);
  if (s.loading) body.innerHTML = `<div class="page err"><p>Loading ${esc(s.url)}…</p><p class="muted">${SIM.mode === 'simulation' ? 'Simulation mode: press Play or Step below to move the packets.' : ''}</p></div>`;
  else if (s.html) body.innerHTML = s.html;
  else body.innerHTML = `<div class="page err"><h1>Danube</h1><p>Try one of these from this laptop:</p>${suggestions.map(u => `<p><a href="#" data-go="${esc(u)}"><code>${esc(u)}</code></a></p>`).join('')}</div>`;
  body.querySelectorAll('[data-go]').forEach(x => x.onclick = (e) => { e.preventDefault(); navigate(d.id, x.dataset.go); });
  const urlIn = $('#dUrl', win);
  urlIn.onkeydown = (e) => { if (e.key === 'Enter') navigate(d.id, urlIn.value.trim()); };
  $('#dReload', win).onclick = () => s.url && navigate(d.id, s.url, true);
  $('#dBack', win).onclick = () => { s.history.pop(); const prev = s.history.pop(); if (prev) navigate(d.id, prev); };
}
function toggleDeskWide() { document.getElementById('app').classList.toggle('deskwide'); fitView(); renderCanvas(); renderSide(); }
function suggestUrls(id) {
  const out = [];
  for (const b of world.devices.filter(x => x.type === 'box')) {
    out.push(`http://${b.name}.local/`);
    const st = NET.losos.box.get(b.id);
    if (st?.publicName) { out.push(`https://${st.publicName}/nextcloud`); out.push(`https://${st.publicName}/`); }
  }
  for (const e of world.devices.filter(x => x.type === 'edge-local')) out.push(`http://${e.name}.local:8443/`);
  const hub = world.devices.find(x => x.type === 'edge-official'); if (hub) out.push(`https://register.${hub.cfg.domain}/health`);
  return [...new Set(out)].slice(0, 8);
}
function navigate(id, url, reload) {
  const s = deskState(id);
  if (!url) return;
  if (!/^\w+:\/\//.test(url)) url = 'http://' + url;
  s.url = url; s.loading = true; if (!reload) s.history.push(url);
  if (!NET.iface.get(id)) { s.loading = false; s.html = errorPage('offline', ''); renderSide(); return; }
  renderSide();
  httpFlow(id, url, (r) => {
    s.loading = false;
    s.html = r.error ? errorPage(r.error, r.host) : r.html;
    if (UI.sel?.id === id && UI.tab === 'desktop') renderSide();
  });
}

// ── Simulation bar and event list ───────────────────────────────────────
let simBarKey = '';
function renderSimBar() {
  const bar = $('#simbar'); if (!bar) return;
  const sim = SIM.mode === 'simulation';
  const folded = $('.simpanel').classList.contains('folded');
  const queued = SIM.flows.length + SIM.active.length;
  const status = `t=${(SIM.tick * 0.25).toFixed(2)} s · ${queued} in flight`;
  const key = [SIM.mode, SIM.playing, folded, CLOCK.speed, JSON.stringify(SIM.filters)].join('|');
  if (key === simBarKey && $('#simStatus', bar)) { $('#simStatus', bar).textContent = status; return; }
  simBarKey = key;
  const n = nextTimer();
  bar.innerHTML = `<button class="btn small" id="sFold" aria-label="${folded ? 'Show' : 'Hide'} the event list">${folded ? '▴' : '▾'}</button><span class="title">${sim ? 'Simulation' : 'Realtime'}</span>
    <span class="clock mono" title="Lab clock. LosOS's timers fire on it."><span id="simClock">${clockText()}</span></span>
    ${sim ? `<button class="btn small" id="sPlay">${SIM.playing ? 'Pause' : 'Play'}</button><button class="btn small" id="sStep">Step ▸</button><button class="btn small" id="sReset">Reset</button><label class="speed">Speed <input type="range" id="sSpeed" min="0.25" max="4" step="0.25" value="${SIM.speed}"></label>`
      : `<div class="seg mini" role="group" aria-label="Clock speed">${SPEEDS.map(v => `<button data-speed="${v}" aria-pressed="${CLOCK.speed === v}">${v === 3600 ? '1 h/s' : v === 600 ? '10 min/s' : v + '×'}</button>`).join('')}</div><button class="btn small" id="sSkip" title="Jump to just before ${n.e.title} at ${n.e.at}">⏭ ${n.e.at}</button><button class="btn small" id="sReset">Clear</button>`}
    <span style="font-size:12px;color:var(--muted)" class="mono" id="simStatus">${status}</span>
    <span class="spacer"></span><div class="filters">${Object.entries(PROTO).map(([k, v]) => `<button class="chip" data-f="${k}" aria-pressed="${SIM.filters[k]}"><i style="background:${v.color}"></i>${v.label}</button>`).join('')}</div>`;
  const b = (id) => $('#' + id, bar);
  if (sim) {
    b('sPlay').onclick = () => { SIM.playing = !SIM.playing; renderSimBar(); if (SIM.playing) ensureRunning(); };
    b('sStep').onclick = () => { SIM.playing = false; simStep(); };
    b('sSpeed').oninput = (e) => { SIM.speed = +e.target.value; };
  } else {
    bar.querySelectorAll('[data-speed]').forEach(x => x.onclick = () => { CLOCK.speed = +x.dataset.speed; renderSimBar(); });
    b('sSkip').onclick = () => { skipToNextTimer(); simBarKey = ''; renderSimBar(); };
  }
  b('sReset').onclick = () => { simReset(); };
  b('sFold').onclick = () => { $('.simpanel').classList.toggle('folded'); renderSimBar(); fitView(); renderCanvas(); };
  bar.querySelectorAll('[data-f]').forEach(c => c.onclick = () => { SIM.filters[c.dataset.f] = !SIM.filters[c.dataset.f]; renderSimBar(); queueEvRender(); drawPdus(); });
}
let evQueued = false;
function queueEvRender() { if (evQueued) return; evQueued = true; requestAnimationFrame(() => { evQueued = false; renderEvents(); }); }
function renderEvents() {
  const box = $('#evlist'); if (!box) return;
  const rows = SIM.log.filter(e => SIM.filters[e.proto]).slice(SIM.mode === 'simulation' ? -300 : -60);
  if (!rows.length) { box.innerHTML = `<div class="empty" style="padding:10px 14px">${SIM.mode === 'simulation' ? 'No events yet. Queued traffic waits for Play or Step.' : 'Traffic appears here as devices boot, scan for edges and talk.'}</div>`; return; }
  const last = rows.length - 1;
  box.innerHTML = `<table><thead><tr><th>Time (s)</th><th>Last device</th><th>At device</th><th>Type</th><th>Info</th></tr></thead><tbody>${rows.map((e, i) => `<tr class="${i === last ? 'cur' : ''}"><td class="mono">${e.t}</td><td>${esc(nm(e.last))}</td><td>${esc(nm(e.at))}</td><td><span class="pill"><i style="background:${PROTO[e.proto].color}"></i>${PROTO[e.proto].label}</span>${e.real ? '<span class="real">guest frame</span>' : ''}</td><td class="info">${esc(e.info)}${e.fail ? ' <span class="tag crit">dropped</span>' : ''}</td></tr>`).join('')}</tbody></table>`;
  box.scrollTop = box.scrollHeight;
}
