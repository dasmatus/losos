// ── The world: devices, links, and what LosOS would make of them ─────────
const TYPES = {
  box: { label: 'LosOS box', short: 'Box', cat: 'losos', ports: [['eth0', 'eth']], endpoint: true, emulate: 'box',
    blurb: 'A mini-PC running LosOS: Nextcloud and Forgejo, the admin UI on :80.' },
  'edge-local': { label: 'Edge gateway', short: 'Gateway', cat: 'losos', ports: [['eth0', 'eth']], endpoint: true, emulate: 'edge-local',
    blurb: 'A local edge (spoke): registrar, rathole server, LAN advert, optional mesh.' },
  'edge-official': { label: 'Official edge', short: 'Official edge', cat: 'losos', ports: [['eth0', 'eth']], endpoint: true, emulate: 'edge-official',
    blurb: 'An official edge (hub) on a VPS, certified by the LosOS root key.' },
  laptop: { label: 'Laptop · LosOS Desktop', short: 'Laptop', cat: 'end', ports: [['eth0', 'eth'], ['wlan0', 'wifi']], endpoint: true,
    blurb: 'A laptop running LosOS Desktop (derisk), with the Danube browser.' },
  router: { label: 'Router', short: 'Router', cat: 'net', ports: [['wan', 'eth'], ['lan1', 'eth'], ['lan2', 'eth'], ['lan3', 'eth'], ['lan4', 'eth']],
    emulate: 'router', blurb: 'Home or office router: DHCP and NAT for one LAN. Runs Netzgeräte Betriebssystem.' },
  switch: { label: 'Switch', short: 'Switch', cat: 'net', ports: [1, 2, 3, 4, 5, 6, 7, 8].map(i => ['p' + i, 'eth']),
    emulate: 'switch', blurb: 'Eight-port Ethernet switch with a management address. Runs Netzgeräte Betriebssystem.' },
  ap: { label: 'Wi-Fi access point', short: 'Access point', cat: 'net', ports: [['eth0', 'eth'], ['wifi', 'wifi-ap']],
    emulate: 'ap', blurb: 'Bridges wireless clients onto its Ethernet segment. Runs Netzgeräte Betriebssystem.' },
  bus: { label: 'Coax bus', short: 'Bus', cat: 'net', ports: [1, 2, 3, 4, 5, 6, 7, 8].map(i => ['t' + i, 'eth']),
    blurb: 'One shared cable with a tap per device and a terminator at each end: every frame reaches every tap.' },
  internet: { label: 'Internet', short: 'Internet', cat: 'net', ports: [1, 2, 3, 4, 5, 6, 7, 8].map(i => ['wan' + i, 'eth']),
    blurb: 'The public internet: routers\' WAN ports and VPSes attach here.' },
};
const SITES = {
  home: { name: 'Home', sub: 'Flat in Bratislava' },
  office: { name: 'Office', sub: 'Company floor' },
  dc: { name: 'Datacenter', sub: 'VPS rack, Falkenstein' },
  isp: { name: 'Internet', sub: 'ISP uplinks' },
};
const LINK_KINDS = {
  auto: { label: 'Automatic', hint: 'Picks the cable and the first free ports.' },
  copper: { label: 'Copper', hint: 'Ethernet patch cable.' },
  fiber: { label: 'Fiber', hint: 'Fiber run between buildings or racks.' },
  wifi: { label: 'Wireless', hint: 'Associate a laptop with an access point.' },
};

const DEFAULT_CFG = {
  box: () => ({ proxy: true, tenantOnHub: true, joinMesh: false, shareCompute: false }),
  'edge-local': () => ({ advertise: true, openEnrolment: true, cluster: true, uplink: true, zone: 'acme.losos.cfd' }),
  'edge-official': () => ({ certified: true, domain: 'losos.cfd', cluster: true, acceptsRelay: true }),
  laptop: () => ({}),
  router: () => ({ subnet: '' }),
  switch: () => ({}), bus: () => ({}), ap: () => ({ ssid: 'losos-lab' }), internet: () => ({}),
};
const DEFAULT_NAME = { box: 'mattbox', 'edge-local': 'acme-gw', 'edge-official': 'edge', laptop: 'matus-laptop', router: 'router', switch: 'switch', bus: 'bus', ap: 'ap', internet: 'internet' };
const DEFAULT_SITE = { 'edge-official': 'dc', internet: 'isp' };

let world = { devices: [], links: [], seq: 1 };

function newDevice(type, x, y, opts = {}) {
  const id = 'd' + (world.seq++);
  let base = opts.name || DEFAULT_NAME[type];
  let name = base, n = 2;
  while (world.devices.some(d => d.name === name)) name = base + n++;
  const dev = {
    id, type, name, x, y,
    site: opts.site || DEFAULT_SITE[type] || 'home',
    px: opts.px ?? null, py: opts.py ?? null,
    power: opts.power ?? true,
    cfg: Object.assign(DEFAULT_CFG[type](), opts.cfg || {}),
    mac: '52:54:00:4c:' + (world.seq).toString(16).padStart(2, '0') + ':' + (Math.floor(Math.random() * 200) + 16).toString(16),
  };
  world.devices.push(dev);
  return dev;
}
const dev = (id) => world.devices.find(d => d.id === id);
const byName = (name) => world.devices.find(d => d.name === name);
function portKind(d, p) { const e = TYPES[d.type].ports.find(x => x[0] === p); return e && e[1]; }
function linksOf(d) { return world.links.filter(l => l.a.dev === d.id || l.b.dev === d.id); }
function portUsed(d, p) { return world.links.some(l => (l.a.dev === d.id && l.a.port === p) || (l.b.dev === d.id && l.b.port === p)); }
function otherEnd(l, id) { return l.a.dev === id ? l.b : l.a; }

function freePort(d, want) {
  for (const [p, k] of TYPES[d.type].ports) {
    if (want === 'wifi') { if (k === 'wifi-ap') return p; if (k === 'wifi' && !portUsed(d, p)) return p; continue; }
    if (k === 'eth' && !portUsed(d, p)) return p;
  }
  return null;
}
function connect(aId, bId, kind = 'auto', aPort, bPort) {
  const A = dev(aId), B = dev(bId);
  if (!A || !B || A === B) return { error: 'Pick two different devices.' };
  let wifi = kind === 'wifi';
  if (kind === 'auto') {
    const aw = TYPES[A.type].ports.some(p => p[1] === 'wifi-ap'), bw = TYPES[B.type].ports.some(p => p[1] === 'wifi-ap');
    const aCli = TYPES[A.type].ports.some(p => p[1] === 'wifi'), bCli = TYPES[B.type].ports.some(p => p[1] === 'wifi');
    if ((aw && bCli && !freePort(B, 'eth')) || (bw && aCli && !freePort(A, 'eth'))) wifi = true;
    if ((aw && bCli) || (bw && aCli)) wifi = wifi || (A.type === 'laptop' || B.type === 'laptop');
  }
  if (wifi) {
    const ap = TYPES[A.type].ports.some(p => p[1] === 'wifi-ap') ? A : TYPES[B.type].ports.some(p => p[1] === 'wifi-ap') ? B : null;
    const cli = ap === A ? B : A;
    if (!ap) return { error: 'A wireless link needs an access point at one end.' };
    const cp = freePort(cli, 'wifi');
    if (!cp || portKind(cli, cp) !== 'wifi') return { error: `${cli.name} has no free wireless card.` };
    if (world.links.some(l => (l.a.dev === cli.id || l.b.dev === cli.id) && l.kind === 'wifi')) return { error: `${cli.name} is already on Wi-Fi.` };
    const l = { id: 'l' + (world.seq++), a: { dev: ap.id, port: 'wifi' }, b: { dev: cli.id, port: cp }, kind: 'wifi' };
    world.links.push(l); return { link: l };
  }
  const ap_ = aPort || freePort(A, 'eth'), bp_ = bPort || freePort(B, 'eth');
  if (!ap_) return { error: `${A.name} has no free Ethernet port.` };
  if (!bp_) return { error: `${B.name} has no free Ethernet port.` };
  if (world.links.some(l => (l.a.dev === A.id && l.b.dev === B.id) || (l.a.dev === B.id && l.b.dev === A.id))) {
    // a second cable between the same two boxes is legal, but almost always a slip
  }
  let k = kind === 'auto' || kind === 'copper' ? 'copper' : kind;
  if (A.type === 'internet' || B.type === 'internet') k = kind === 'fiber' ? 'fiber' : 'wan';
  const l = { id: 'l' + (world.seq++), a: { dev: A.id, port: ap_ }, b: { dev: B.id, port: bp_ }, kind: k };
  world.links.push(l);
  return { link: l };
}
function removeDevice(id) { world.links = world.links.filter(l => l.a.dev !== id && l.b.dev !== id); world.devices = world.devices.filter(d => d.id !== id); }
function removeLink(id) { world.links = world.links.filter(l => l.id !== id); }

// ── Evaluation ───────────────────────────────────────────────────────────
// Everything below is a pure function of the topology, the switches and the
// power buttons. The rules are LosOS's own (wiki/Mesh.md, Edge-Federation.md,
// Master-Proxy.md); the packets drawn on the canvas are derived from them.
const isGear = (d) => ['router', 'switch', 'bus', 'ap', 'internet'].includes(d.type);
const powered = (d) => d && d.power;
function linkUp(l) { return powered(dev(l.a.dev)) && powered(dev(l.b.dev)); }

function ipToInt(ip) { return ip.split('.').reduce((a, b) => (a << 8) + (+b), 0) >>> 0; }
function intToIp(n) { return [24, 16, 8, 0].map(s => (n >>> s) & 255).join('.'); }

let NET = null;
function evaluate() {
  const parent = new Map();
  const key = (d, p) => d + ':' + p;
  const find = (k) => { while (parent.get(k) !== k) { parent.set(k, parent.get(parent.get(k))); k = parent.get(k); } return k; };
  const union = (a, b) => { const ra = find(a), rb = find(b); if (ra !== rb) parent.set(ra, rb); };
  for (const d of world.devices) for (const [p] of TYPES[d.type].ports) parent.set(key(d.id, p), key(d.id, p));
  for (const l of world.links) if (linkUp(l)) union(key(l.a.dev, l.a.port), key(l.b.dev, l.b.port));
  for (const d of world.devices) {
    if (!d.power) continue;
    const ps = TYPES[d.type].ports.map(p => p[0]);
    if (d.type === 'switch' || d.type === 'bus' || d.type === 'ap' || d.type === 'internet') ps.slice(1).forEach(p => union(key(d.id, ps[0]), key(d.id, p)));
    if (d.type === 'router') ps.slice(2).forEach(p => union(key(d.id, 'lan1'), key(d.id, p)));
  }
  // segments
  const segs = new Map();
  const segOf = (d, p) => find(key(d, p));
  const getSeg = (k) => { if (!segs.has(k)) segs.set(k, { id: k, members: [], routers: [], internet: false, kind: 'isolated' }); return segs.get(k); };
  for (const d of world.devices) {
    if (!d.power) continue;
    if (d.type === 'internet') { const s = getSeg(segOf(d.id, 'wan1')); s.kind = 'internet'; s.internet = true; s.cloud = d.id; }
    if (d.type === 'router') { const s = getSeg(segOf(d.id, 'lan1')); s.routers.push(d.id); }
  }
  for (const s of segs.values()) if (s.kind !== 'internet' && s.routers.length) { s.kind = 'lan'; s.router = s.routers[0]; }
  // active interface per endpoint (Ethernet beats Wi-Fi), and the router's WAN
  const iface = new Map();
  for (const d of world.devices) {
    if (!d.power) continue;
    if (TYPES[d.type].endpoint) {
      for (const [p] of TYPES[d.type].ports) {
        const l = world.links.find(l => linkUp(l) && ((l.a.dev === d.id && l.a.port === p) || (l.b.dev === d.id && l.b.port === p)));
        if (l) { iface.set(d.id, { port: p, seg: segOf(d.id, p) }); break; }
      }
    }
    if (d.type === 'router') {
      const l = world.links.find(l => linkUp(l) && ((l.a.dev === d.id && l.a.port === 'wan') || (l.b.dev === d.id && l.b.port === 'wan')));
      if (l) iface.set(d.id, { port: 'wan', seg: segOf(d.id, 'wan') });
    }
  }
  for (const [id, f] of iface) { const s = getSeg(f.seg); if (!s.members.includes(id)) s.members.push(id); }
  // subnets per router, in a stable order
  const routers = world.devices.filter(d => d.type === 'router');
  const subnet = new Map();
  routers.forEach((r, i) => subnet.set(r.id, r.cfg.subnet || ['192.168.1', '10.10.0', '192.168.50', '172.16.4'][i % 4] || ('10.' + (20 + i) + '.0')));
  // addresses: internet segment first (public), then LANs outward (a router's
  // WAN may itself sit on another router's LAN: double NAT works too)
  const addr = new Map();
  let pub = 0;
  const internetSegs = [...segs.values()].filter(s => s.kind === 'internet');
  for (const s of internetSegs) {
    for (const id of s.members) {
      const d = dev(id);
      const ip = d.type === 'edge-official' ? '49.12.34.' + (56 + pub++) : d.type === 'router' ? '85.216.' + (120 + routers.indexOf(d)) + '.' + (17 + routers.indexOf(d) * 3) : '49.12.40.' + (10 + pub++);
      addr.set(id, { ip, mask: 24, gw: '', src: 'public' });
    }
  }
  const hasInternet = (segId, depth = 0) => {
    const s = segs.get(segId); if (!s || depth > 6) return false;
    if (s.kind === 'internet') return true;
    if (s.kind !== 'lan') return false;
    const r = dev(s.router); const w = iface.get(r.id);
    return !!(w && hasInternet(w.seg, depth + 1));
  };
  for (let pass = 0; pass < 4; pass++) {
    for (const s of segs.values()) {
      if (s.kind !== 'lan') continue;
      const r = dev(s.router); const sn = subnet.get(r.id);
      let n = 100;
      addr.set(r.id + '#lan', { ip: sn + '.1', mask: 24, src: 'router' });
      for (const id of s.members) {
        if (id === s.router) continue;
        addr.set(id, { ip: sn + '.' + (n++), mask: 24, gw: sn + '.1', src: 'dhcp', router: r.id });
      }
    }
  }
  let ll = 20;
  for (const s of segs.values()) {
    if (s.kind !== 'isolated') continue;
    for (const id of s.members) if (!addr.has(id)) addr.set(id, { ip: '169.254.' + (10 + (ll >> 8)) + '.' + (ll++ & 255), mask: 16, gw: '', src: 'link-local' });
  }
  const internet = new Map();
  for (const [id, f] of iface) internet.set(id, hasInternet(f.seg));
  // network gear: the segment its own OS sits on, and its management address
  const gearSeg = new Map(), mgmt = new Map();
  let mg = 2, gl = 200;
  for (const d of world.devices) {
    if (!d.power || !['router', 'switch', 'ap'].includes(d.type)) continue;
    const k = segOf(d.id, d.type === 'router' ? 'lan1' : TYPES[d.type].ports[0][0]);
    gearSeg.set(d.id, k);
    const s = segs.get(k);
    if (d.type === 'router') { const a = addr.get(d.id + '#lan'); if (a) mgmt.set(d.id, { ip: a.ip, mask: 24 }); continue; }
    if (s && s.kind === 'lan') mgmt.set(d.id, { ip: subnet.get(s.router) + '.' + (mg++), mask: 24, gw: subnet.get(s.router) + '.1' });
    else mgmt.set(d.id, { ip: '169.254.' + (gl++) + '.1', mask: 16 });
  }

  NET = { segs, iface, addr, internet, subnet, gearSeg, mgmt };
  NET.segOf = (id) => iface.get(id)?.seg;
  NET.segsOf = (id) => [iface.get(id)?.seg, gearSeg.get(id)].filter(Boolean);
  NET.sameSeg = (a, b) => { const B = NET.segsOf(b); return NET.segsOf(a).some(x => B.includes(x)); };
  NET.ip = (id) => addr.get(id)?.ip;
  NET.hub = () => world.devices.find(d => d.type === 'edge-official' && d.power && internet.get(d.id));
  computeLosos();
  return NET;
}

// What lososd and the registrars would conclude.
function computeLosos() {
  const L = { box: new Map(), spoke: new Map(), hub: new Map() };
  const hubs = world.devices.filter(d => d.type === 'edge-official');
  const liveHub = hubs.find(h => h.power && NET.internet.get(h.id) && NET.ip(h.id));
  for (const s of world.devices.filter(d => d.type === 'edge-local')) {
    const st = { up: s.power && !!NET.ip(s.id), uplink: 'off', relayed: [], enrolled: [], cluster: [] };
    if (st.up && s.cfg.uplink) {
      if (!NET.internet.get(s.id)) st.uplink = 'no internet';
      else if (!liveHub) st.uplink = 'hub unreachable';
      else if (!liveHub.cfg.acceptsRelay) st.uplink = 'refused by hub';
      else { st.uplink = 'up'; st.hub = liveHub.id; }
    }
    L.spoke.set(s.id, st);
  }
  for (const h of hubs) L.hub.set(h.id, { up: h.power && !!NET.ip(h.id) && NET.internet.get(h.id), tenants: [], relays: [], cluster: [] });
  for (const b of world.devices.filter(d => d.type === 'box')) {
    const st = { up: b.power && !!NET.ip(b.id), edges: [], path: null, tunnel: 'off', publicName: null, mesh: 'off', market: 'unavailable', reason: '' };
    if (st.up) {
      for (const e of world.devices.filter(d => d.type === 'edge-local')) {
        if (e.power && e.cfg.advertise && NET.sameSeg(b.id, e.id) && NET.ip(e.id)) st.edges.push({ id: e.id, source: 'lan', official: false, url: `http://${e.name}.local:8443` });
      }
      if (b.cfg.proxy && NET.internet.get(b.id) && liveHub) {
        st.edges.push({ id: liveHub.id, source: 'configured', official: !!liveHub.cfg.certified, url: `https://register.${liveHub.cfg.domain}` });
      }
      st.path = st.edges[0] || null;
      const official = st.edges.find(e => e.official);
      st.market = official ? 'available' : 'unavailable (noOfficialEdge)';
      if (b.cfg.proxy && st.path) {
        const e = dev(st.path.id);
        if (st.path.source === 'lan') {
          const sp = L.spoke.get(e.id);
          if (e.cfg.openEnrolment) {
            st.tunnel = 'enrolled'; sp.enrolled.push(b.id);
            if (sp.uplink === 'up') { st.publicName = `${b.name}.${e.cfg.zone}`; sp.relayed.push(b.id); L.hub.get(sp.hub).relays.push(b.id); }
          } else { st.tunnel = 'refused'; st.reason = 'gateway has closed enrolment'; }
        } else {
          if (b.cfg.tenantOnHub) { st.tunnel = 'registered'; st.publicName = `${b.name}.${e.cfg.domain}`; L.hub.get(e.id).tenants.push(b.id); }
          else { st.tunnel = 'refused'; st.reason = 'not a tenant of the official edge'; }
        }
      } else if (b.cfg.proxy && !st.path) { st.tunnel = 'stopped'; st.reason = 'no edge in reach (/run/losos/edge-none)'; }
      if (b.cfg.joinMesh) {
        if (!st.path) st.mesh = 'refused: 409 edgeRequired';
        else {
          const e = dev(st.path.id);
          if (e.cfg.cluster) { st.mesh = 'joined'; st.meshEdge = e.id; (e.type === 'edge-local' ? L.spoke.get(e.id) : L.hub.get(e.id)).cluster.push(b.id); }
          else st.mesh = 'edge runs no mesh control plane';
        }
      }
    }
    L.box.set(b.id, st);
  }
  NET.losos = L;
}

// ── Name resolution and reachability, for the browser, curl and ping ─────
function resolveName(fromId, host) {
  const L = NET.losos;
  host = host.toLowerCase();
  if (/^\d+\.\d+\.\d+\.\d+$/.test(host)) {
    for (const [id, a] of NET.addr) {
      if (a.ip !== host) continue;
      const realId = id.split('#')[0];
      if (id.endsWith('#lan')) { if (NET.segOf(fromId) && NET.segs.get(NET.segOf(fromId))?.router === realId) return { dev: realId, via: 'lan' }; continue; }
      if (NET.sameSeg(fromId, realId)) return { dev: realId, via: 'lan' };
      if (a.src === 'public' && NET.internet.get(fromId)) return { dev: realId, via: 'internet' };
    }
    return { error: 'timeout' };
  }
  if (host.endsWith('.local')) {
    const label = host.slice(0, -6);
    const t = world.devices.find(d => d.name.toLowerCase() === label && TYPES[d.type].endpoint);
    if (t && NET.sameSeg(fromId, t.id) && NET.ip(t.id)) return { dev: t.id, via: 'lan', mdns: true };
    return { error: 'mdns' };
  }
  if (!NET.internet.get(fromId)) return { error: 'nodns' };
  const hub = world.devices.find(d => d.type === 'edge-official' && L.hub.get(d.id)?.up && (host === 'register.' + d.cfg.domain || host === 'edge.' + d.cfg.domain || host === d.cfg.domain));
  if (hub) return { dev: hub.id, via: 'internet' };
  for (const [bid, st] of L.box) {
    if (st.publicName === host) {
      const viaSpoke = st.path?.source === 'lan' ? st.path.id : null;
      const hubId = viaSpoke ? L.spoke.get(viaSpoke).hub : st.path.id;
      return { dev: bid, via: 'tunnel', hub: hubId, spoke: viaSpoke };
    }
  }
  return { error: 'nxdomain' };
}

// Device-level hop path between two devices, through network gear only.
function hops(a, b) {
  if (a === b) return [a];
  const prev = new Map([[a, null]]);
  const q = [a];
  while (q.length) {
    const cur = q.shift();
    for (const l of linksOf(dev(cur))) {
      if (!linkUp(l)) continue;
      const n = otherEnd(l, cur).dev;
      if (prev.has(n)) continue;
      prev.set(n, cur);
      if (n === b) { const path = [b]; let c = cur; while (c) { path.unshift(c); c = prev.get(c); } return path; }
      if (isGear(dev(n))) q.push(n);
    }
  }
  return null;
}
