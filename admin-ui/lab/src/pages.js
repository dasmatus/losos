// ── What each device answers over HTTP ────────────────────────────────────
const esc = (s) => String(s).replace(/[&<>"]/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' }[c]));
function pageFor(r, u) {
  const d = dev(r.dev);
  const p = u.pathname;
  const viaTunnel = r.via === 'tunnel';
  const json = (status, obj) => ({ status, title: 'application/json', text: JSON.stringify(obj, null, 2), html: `<div class="page"><div class="pbody"><pre>${esc(JSON.stringify(obj, null, 2))}</pre></div></div>` });
  if (d.type === 'box') {
    const st = NET.losos.box.get(d.id);
    if (p.startsWith('/nextcloud')) return cloudPage(d, viaTunnel);
    if (p.startsWith('/forgejo')) return gitPage(d);
    if (p.startsWith('/api/edge')) {
      if (viaTunnel) return forbidden(d);
      return json(200, { path: st.path ? { name: nm(st.path.id), url: st.path.url, source: st.path.source } : null, edges: st.edges.map(e => ({ name: nm(e.id), url: e.url, source: e.source, official: e.official })) });
    }
    if (viaTunnel) return forbidden(d);
    return adminPage(d, st);
  }
  if (d.type === 'edge-local') {
    const sp = NET.losos.spoke.get(d.id);
    if (p.startsWith('/health')) return json(200, { status: 'ok' });
    return { status: 200, title: 'LosOS edge gateway', text: `losos-edge boxes\n${sp.enrolled.map(nm).join('\n') || '(none)'}`, html: `<div class="page"><div class="pbar"><img src="${PLATE}" alt=""><b>${esc(d.name)}</b><span class="muted">LosOS edge gateway</span></div><div class="pbody">
      <h1>Gateway for this network</h1>
      <div class="card"><b>Uplink to the official edge:</b> ${esc(sp.uplink)}${sp.uplink === 'up' ? ` · boxes are public under <code>${esc(d.cfg.zone)}</code>` : ''}</div>
      <div class="card"><b>Enrolled boxes</b> (${d.cfg.openEnrolment ? 'open enrolment, trust on first use' : 'closed enrolment'})<br>${sp.enrolled.map(b => `<code>${esc(nm(b))}</code>`).join(', ') || '<span class="muted">none yet</span>'}</div>
      <div class="card"><b>Mesh control plane</b>: ${d.cfg.cluster ? `rke2 server, ${sp.cluster.length} node(s): ${sp.cluster.map(nm).join(', ') || '—'}` : 'off'}</div>
      <p class="muted">The gateway is not official: boxes behind it can share storage and compute, but trade on the market only through an official edge.</p></div></div>` };
  }
  if (d.type === 'edge-official') {
    const h = NET.losos.hub.get(d.id);
    if (p.startsWith('/health')) return json(200, { status: 'ok' });
    if (p.startsWith('/identity')) return json(200, { cert: { name: `register.${d.cfg.domain}`, signedBy: d.cfg.certified ? 'LosOS root (keys/official-edge-root.pub)' : 'unknown', expires: '2027-04-01' }, nonceSignature: 'ed25519:…' });
    return json(200, { registrar: `register.${d.cfg.domain}`, tenants: h.tenants.map(nm), relayed: h.relays.map(b => NET.losos.box.get(b).publicName), mesh: h.cluster.map(nm) });
  }
  if (d.type === 'laptop') return { status: 0, error: 'refused', title: 'Connection refused', html: errPage('Unable to connect', `${esc(d.name)} runs no web server.`) };
  if (d.type === 'router') return { status: 200, title: 'Router', html: `<div class="page err"><h1>${esc(d.name)} · router</h1><p>LAN ${esc(NET.subnet.get(d.id))}.0/24, DHCP pool .100–.199. Sign-in page of the router's own firmware.</p></div>`, text: 'router admin' };
  return { status: 0, error: 'refused', title: 'Connection refused', html: errPage('Unable to connect', 'Nothing listens there.') };
}
function forbidden(d) {
  return { status: 403, title: 'Forbidden', text: '403 Forbidden', html: `<div class="page err"><h1>403 Forbidden</h1><p>nginx</p><hr><p class="muted">The admin pages of <b>${esc(d.name)}</b> answer on its own network only. Tunnel traffic reaches nginx from 127.0.0.1, and the <code>lanOnly</code> guard denies loopback on purpose, so nobody on the internet reaches the admin UI. Use <code>/nextcloud</code> from outside, or open <code>http://${esc(d.name)}.local/</code> from home.</p></div>` };
}
function errPage(h, p) { return `<div class="page err"><h1>${h}</h1><p>${p}</p></div>`; }
function errorPage(err, host) {
  const m = {
    mdns: ['Server not found', `<code>${esc(host)}</code> is an mDNS name. Only computers on the same network as the device can resolve it. From elsewhere, use the box's public name.`],
    nodns: ['No internet connection', 'This computer has no route to a DNS server. Connect it to a router with internet, or to an access point on one.'],
    nxdomain: ['This site can’t be reached', `DNS_PROBE_FINISHED_NXDOMAIN: no DNS record for <code>${esc(host)}</code>. A box gets a public name only once its tunnel is registered with an edge that is on the internet.`],
    timeout: ['The connection timed out', `<code>${esc(host)}</code> is a private address on another network. Routers do not forward inbound connections; that is what the tunnel through an edge is for.`],
    badurl: ['Invalid address', 'Type a host name or an address.'],
    offline: ['You are offline', 'The laptop has no network link. Plug in a cable or join Wi-Fi.'],
  }[err] || ['Error', err];
  return errPage(m[0], m[1]);
}
function adminPage(d, st) {
  const edgeRow = (e) => `<div class="card" style="display:flex;gap:8px;align-items:center"><span style="color:${e.official ? '#2e7357' : '#8c6512'};font-weight:700">${e.official ? '✓' : '⚠'}</span><div><b>${esc(nm(e.id))}</b> <span class="muted">${esc(e.url)}</span><br><span class="muted">${e.source === 'lan' ? 'Found on this network' : 'Configured address'}${e.official ? ' · official edge' : ' · can share storage and compute, cannot trade on the market'}</span></div></div>`;
  return { status: 200, title: 'LosOS', text: `LosOS admin · ${d.name}`, html: `<div class="page"><div class="pbar"><img src="${PLATE}" alt="LosOS"><b>${esc(d.name)}</b><span class="muted">Home</span><span style="margin-left:auto" class="muted">Signed in on the LAN</span></div><div class="pbody">
    <h1>Apps</h1>
    <div style="display:flex;gap:8px;flex-wrap:wrap"><div class="card" style="flex:1;min-width:120px"><b>LosOS cloud</b><br><span class="muted">/nextcloud</span></div><div class="card" style="flex:1;min-width:120px"><b>LosOS Git</b><br><span class="muted">/forgejo</span></div></div>
    <h1 style="margin-top:12px">Mesh</h1>
    ${st.edges.length ? st.edges.map(edgeRow).join('') : '<div class="card"><b>No edge proxy found.</b><br><span class="muted">Tried mDNS <code>_losos-edge._tcp</code> on this network and the configured address. Sharing stays off.</span></div>'}
    <div class="card"><b>Path</b>: ${st.path ? esc(nm(st.path.id)) + (st.path.source === 'lan' ? ' (local edge first)' : ' (official edge)') : 'none'}<br><b>Public address</b>: ${st.publicName ? `<code>https://${esc(st.publicName)}</code>` : '<span class="muted">none</span>'}<br><b>Mesh</b>: ${esc(st.mesh)} · <b>Market</b>: ${esc(st.market)}</div>
  </div></div>` };
}
function cloudPage(d, viaTunnel) {
  return { status: 200, title: 'LosOS cloud', text: '<title>LosOS cloud</title>', html: `<div class="page" style="background:linear-gradient(160deg,#0e6e7d,#0a4f5a);min-height:100%;color:#fff;padding:10px"><div class="login"><img src="${PLATE}" alt="" width="56" height="56"><h1 style="color:#fff">LosOS cloud</h1><p style="opacity:.85;font-size:12px">${esc(d.name)}${viaTunnel ? ' · through the edge tunnel' : ' · on this network'}</p><input aria-label="Account name" value="notshared"><input aria-label="Password" type="password" value="••••••••••••"><button class="go" type="button">Log in</button></div></div>` };
}
function gitPage(d) {
  return { status: 200, title: 'LosOS Git', text: '<title>LosOS Git</title>', html: `<div class="page"><div class="pbar"><img src="${PLATE}" alt=""><b>LosOS Git</b><span class="muted">${esc(d.name)}</span></div><div class="pbody"><h1>Explore</h1><div class="card"><b>notshared/losos-config</b> <span class="muted">private</span><br><span class="muted">Every apply, mode change and reset of this box is a commit here.</span></div></div></div>` };
}
