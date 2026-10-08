// ── Packets: flows, the event list, realtime and simulation mode ─────────
const PROTO = {
  DHCP: { color: '#8e6fd1', label: 'DHCP' },
  ARP: { color: '#9aa34a', label: 'ARP' },
  mDNS: { color: '#2f9e8f', label: 'mDNS' },
  DNS: { color: '#4f86e0', label: 'DNS' },
  HTTP: { color: '#e8775a', label: 'HTTP(S)' },
  ICMP: { color: '#d4a72c', label: 'ICMP' },
  rathole: { color: '#2b6cb0', label: 'rathole/Noise' },
  rke2: { color: '#c05fa0', label: 'rke2 (mesh)' },
  lososd: { color: '#5c6a77', label: 'lososd' },
};

const SIM = {
  mode: 'realtime', playing: false, tick: 0, speed: 1, log: [], flows: [], active: [], filters: Object.fromEntries(Object.keys(PROTO).map(k => [k, true])),
  frameFrac: 0, lastTickAt: 0, seq: 0, cursor: -1,
};
// A flow is a list of stages; a stage is a list of PDUs that leave together.
// stage = [{proto, from, to, info, hops?}], plus an optional {after: fn}.
function pdu(proto, from, to, info, opt = {}) { return Object.assign({ proto, from, to, info }, opt); }
function startFlow(stages, opt = {}) {
  const f = { id: ++SIM.seq, stages: stages.filter(s => s && s.length !== 0), stage: -1, title: opt.title || '', done: opt.done };
  if (!f.stages.length) { f.done && f.done(); return; }
  SIM.flows.push(f);
  if (SIM.mode === 'realtime') ensureRunning();
  renderSimBar();
}
function spawnStage(f) {
  f.stage++;
  if (f.stage >= f.stages.length) { SIM.flows = SIM.flows.filter(x => x !== f); f.done && f.done(); return; }
  const st = f.stages[f.stage];
  for (const p of st) {
    if (p.local) { logEvent({ proto: p.proto, last: p.from, at: p.from, info: p.info, local: true }); continue; }
    const h = p.hops || hops(p.from, p.to);
    if (!h || h.length < 2) { logEvent({ proto: p.proto, last: p.from, at: p.from, info: p.info + ' · no route', fail: true }); continue; }
    SIM.active.push({ flow: f, proto: p.proto, info: p.info, hops: h, i: 0, real: p.real });
  }
  if (!SIM.active.some(a => a.flow === f)) spawnStage(f);
}
function stepTick() {
  SIM.tick++;
  if (SIM.mode === 'simulation') clockAdvance(250);
  // PDUs that reached their last hop finished animating during the last period
  SIM.active = SIM.active.filter(a => a.i < a.hops.length - 1);
  for (const f of [...SIM.flows]) if (!SIM.active.some(a => a.flow === f)) spawnStage(f);
  for (const a of SIM.active) {
    if (a.i < a.hops.length - 1) {
      a.i++;
      logEvent({ proto: a.proto, last: a.hops[a.i - 1], at: a.hops[a.i], info: a.info, real: a.real, final: a.i === a.hops.length - 1 });
    }
  }
  renderSimBar();
}
function logEvent(e) {
  e.t = (SIM.tick * 0.25).toFixed(2);
  SIM.log.push(e);
  if (SIM.log.length > 600) SIM.log.splice(0, SIM.log.length - 600);
  queueEvRender();
}
let runTimer = null;
function ensureRunning() {
  if (runTimer) return;
  SIM.lastTickAt = performance.now();
  const loop = () => {
    const now = performance.now();
    const period = SIM.mode === 'realtime' ? Math.max(40, 230 / Math.sqrt(CLOCK.speed)) : 1100 / SIM.speed;
    const busy = SIM.flows.length || SIM.active.length;
    if (SIM.mode === 'simulation' && !SIM.playing) { SIM.frameFrac = 0; drawPdus(); runTimer = null; return; }
    if (!busy) { SIM.frameFrac = 0; drawPdus(); runTimer = null; if (SIM.mode === 'simulation') { SIM.playing = false; renderSimBar(); } return; }
    if (now - SIM.lastTickAt >= period) { SIM.lastTickAt = now; stepTick(); }
    SIM.frameFrac = Math.min(1, (now - SIM.lastTickAt) / period);
    drawPdus();
    runTimer = requestAnimationFrame(loop);
  };
  runTimer = requestAnimationFrame(loop);
}
function simStep() {
  if (!SIM.flows.length && !SIM.active.length) return toast('Nothing queued. Power something on, browse from the laptop or ping from a console.');
  stepTick();
  // animate the hop just taken
  const t0 = performance.now();
  const anim = () => { SIM.frameFrac = Math.min(1, (performance.now() - t0) / 450); drawPdus(true); if (SIM.frameFrac < 1) requestAnimationFrame(anim); };
  requestAnimationFrame(anim);
}
function simReset() { SIM.flows = []; SIM.active = []; SIM.log = []; SIM.tick = 0; SIM.playing = false; drawPdus(); renderSimBar(); queueEvRender(); }

// ── Flows that LosOS behaviour produces ──────────────────────────────────
const nm = (id) => dev(id)?.name || id;
function bcastTo(from, info, proto, extra = []) {
  const seg = NET.segOf(from); if (!seg) return [];
  const s = NET.segs.get(seg);
  const targets = new Set([...s.members.filter(m => m !== from), ...(s.router ? [s.router] : []), ...extra]);
  return [...targets].map(t => pdu(proto, from, t, info));
}
function flowAddress(id) {
  const a = NET.addr.get(id); if (!a) return [];
  const d = dev(id);
  const st = [];
  if (a.src === 'dhcp') {
    st.push(bcastTo(id, `DHCPDISCOVER from ${d.mac}`, 'DHCP'));
    st.push([pdu('DHCP', a.router, id, `DHCPOFFER ${a.ip}/24, router ${a.gw}, dns ${a.gw}`)]);
    st.push([pdu('DHCP', id, a.router, `DHCPREQUEST ${a.ip}`)]);
    st.push([pdu('DHCP', a.router, id, `DHCPACK ${a.ip} lease 86400 s`)]);
  } else if (a.src === 'link-local') {
    st.push(bcastTo(id, `ARP probe for ${a.ip} (no DHCP server: IPv4 link-local)`, 'ARP'));
  }
  if (TYPES[d.type].endpoint && a.src !== 'public') st.push(bcastTo(id, `announce ${d.name}.local A ${a.ip}`, 'mDNS'));
  return st;
}
function dnsLeg(from, name) {
  const a = NET.addr.get(from);
  if (a && a.gw) return [[pdu('DNS', from, a.router, `query A ${name}`)], [pdu('DNS', a.router, from, `answer ${name}`)]];
  return [[pdu('DNS', from, from, `resolve ${name} (static resolver)`, { local: true })]];
}
function flowScan(bid) {
  const b = dev(bid), st = NET.losos.box.get(bid);
  if (!st || !st.up) return [];
  const stages = [];
  const lan = world.devices.filter(e => e.type === 'edge-local' && e.power && e.cfg.advertise && NET.sameSeg(bid, e.id));
  stages.push(bcastTo(bid, 'query PTR _losos-edge._tcp.local', 'mDNS'));
  if (lan.length) stages.push(lan.map(e => pdu('mDNS', e.id, bid, `answer ${e.name}._losos-edge._tcp url=http://${e.name}.local:8443 rathole=${e.name}.local:2333 enrol=${e.cfg.openEnrolment ? 'open' : 'closed'}`)));
  if (lan.length) { stages.push(lan.map(e => pdu('HTTP', bid, e.id, 'GET /health'))); stages.push(lan.map(e => pdu('HTTP', e.id, bid, '200 {"status":"ok"}'))); }
  const hubE = st.edges.find(e => e.source === 'configured');
  if (b.cfg.proxy && NET.internet.get(bid)) {
    const hub = NET.hub();
    if (hub) {
      stages.push(...dnsLeg(bid, 'register.' + hub.cfg.domain));
      stages.push([pdu('HTTP', bid, hub.id, 'GET https://register.' + hub.cfg.domain + '/health')]);
      stages.push([pdu('HTTP', hub.id, bid, '200 {"status":"ok"}')]);
      const nonce = Math.random().toString(16).slice(2, 10);
      stages.push([pdu('HTTP', bid, hub.id, `GET /identity?nonce=${nonce}`)]);
      stages.push([pdu('HTTP', hub.id, bid, hub.cfg.certified ? `200 cert signed by LosOS root, sig(nonce) ✓ → official` : '200 cert not signed by the LosOS root → not official')]);
    }
  }
  const path = st.path;
  stages.push([pdu('lososd', bid, bid, path ? `path = ${nm(path.id)} (${path.source === 'lan' ? 'local edge first' : 'official edge second'}), market ${st.market}` : 'path = none → /run/losos/edge-none, tunnel units stopped, sharing refused', { local: true })]);
  if (path && b.cfg.proxy) {
    stages.push([pdu('rathole', bid, path.id, `Noise_NK handshake to ${nm(path.id)}:2333 (pinned key)`)]);
    stages.push([pdu('rathole', path.id, bid, 'handshake ok, control channel open')]);
    stages.push([pdu('HTTP', bid, path.id, `POST /register {appliance_id:"${b.name}"}`)]);
    stages.push([pdu('HTTP', path.id, bid, st.tunnel === 'refused' ? `403 ${st.reason}` : st.tunnel === 'enrolled' ? '200 enrolled (trust on first use)' : `200 route ${st.publicName}`)]);
  }
  if (b.cfg.joinMesh) stages.push(...flowMeshStages(bid));
  return stages;
}
function flowMeshStages(bid) {
  const st = NET.losos.box.get(bid);
  if (!st.path) return [[pdu('lososd', bid, bid, 'join mesh refused: 409 edgeRequired (no edge in reach)', { local: true })]];
  const e = dev(st.path.id);
  if (!e.cfg.cluster) return [[pdu('lososd', bid, bid, `${e.name} runs no rke2 server; mesh stays off`, { local: true })]];
  return [[pdu('rke2', bid, e.id, `rke2 agent: join https://${e.name}:9345, node password from /etc/rancher`)], [pdu('rke2', e.id, bid, 'node registered; Longhorn replica scheduled')]];
}
function flowUplink(sid) {
  const s = dev(sid), sp = NET.losos.spoke.get(sid);
  if (!sp || sp.uplink !== 'up') return [];
  const hub = dev(sp.hub);
  return [
    ...dnsLeg(sid, 'register.' + hub.cfg.domain),
    [pdu('HTTP', sid, hub.id, `POST /relay {appliance_id:"${s.cfg.zone.split('.')[0]}", tenants:[${sp.relayed.map(nm).join(', ')}]}`)],
    [pdu('HTTP', hub.id, sid, `200 relayed under ${s.cfg.zone}`)],
    [pdu('rathole', sid, hub.id, 'uplink: Noise_NK handshake to edge:2333')],
    [pdu('rathole', hub.id, sid, 'uplink up: one service per relayed box')],
  ];
}

// Diff the old and new evaluation and send the traffic the change implies.
let PREV = null;
function snapshot() {
  const s = {};
  for (const d of world.devices) {
    const a = NET.addr.get(d.id);
    const b = NET.losos.box.get(d.id), sp = NET.losos.spoke.get(d.id);
    s[d.id] = { ip: a?.ip || '', box: b ? [b.path?.id, b.tunnel, b.publicName, b.mesh].join('|') : '', spoke: sp ? sp.uplink + '|' + sp.relayed.join(',') : '' };
  }
  return s;
}
function trafficForChange() {
  const now = snapshot();
  if (!PREV) { PREV = now; return; }
  const addrFlows = [];
  for (const d of world.devices) {
    const o = PREV[d.id] || {}, n = now[d.id];
    if (n.ip && n.ip !== o.ip && TYPES[d.type].endpoint) addrFlows.push(d.id);
  }
  for (const id of addrFlows) startFlow(flowAddress(id), { title: 'address ' + nm(id) });
  for (const d of world.devices) {
    const o = PREV[d.id] || {}, n = now[d.id];
    if (d.type === 'box' && n.box !== o.box && n.ip) startFlow(flowScan(d.id), { title: 'edge scan ' + d.name });
    if (d.type === 'edge-local' && n.spoke !== o.spoke) startFlow(flowUplink(d.id), { title: 'uplink ' + d.name });
  }
  PREV = now;
}

// HTTP from a client, through whatever the name resolves to.
function httpFlow(from, url, cb) {
  let u;
  try { u = new URL(/^\w+:\/\//.test(url) ? url : 'http://' + url); } catch { return cb({ error: 'badurl' }); }
  const host = u.hostname, path = u.pathname + u.search;
  const r = resolveName(from, host);
  const stages = [];
  if (host.endsWith('.local')) stages.push(bcastTo(from, `query A ${host}`, 'mDNS'));
  else if (!/^\d+\.\d+\.\d+\.\d+$/.test(host) && NET.internet.get(from)) stages.push(...dnsLeg(from, host));
  if (r.error) {
    if (host.endsWith('.local') && r.error === 'mdns') stages.push([pdu('mDNS', from, from, `no answer for ${host} on this network`, { local: true })]);
    return startFlow(stages, { done: () => cb({ error: r.error, host }) });
  }
  if (host.endsWith('.local')) stages.push([pdu('mDNS', r.dev, from, `answer ${host} A ${NET.ip(r.dev)}`)]);
  const resp = pageFor(r, u);
  if (r.via === 'tunnel') {
    const hub = r.hub, spoke = r.spoke, box = r.dev;
    stages.push([pdu('HTTP', from, hub, `GET ${u.protocol}//${host}${path} (TLS ends on ${nm(hub)})`)]);
    if (spoke) {
      stages.push([pdu('rathole', hub, spoke, `uplink service ${dev(spoke).cfg.zone.split('.')[0]}.${nm(box)}`, { hops: hops(spoke, hub)?.slice().reverse() })]);
      stages.push([pdu('rathole', spoke, box, `tunnel to ${nm(box)} → nginx :80`, { hops: hops(box, spoke)?.slice().reverse() })]);
      stages.push([pdu('rathole', box, spoke, `${resp.status} from nginx (client seen as 127.0.0.1)`)]);
      stages.push([pdu('rathole', spoke, hub, `${resp.status}`)]);
    } else {
      stages.push([pdu('rathole', hub, box, `tunnel to ${nm(box)} → nginx :80`, { hops: hops(box, hub)?.slice().reverse() })]);
      stages.push([pdu('rathole', box, hub, `${resp.status} from nginx (client seen as 127.0.0.1)`)]);
    }
    stages.push([pdu('HTTP', hub, from, `${resp.status} ${resp.title}`)]);
  } else {
    stages.push([pdu('HTTP', from, r.dev, `GET ${path}  Host: ${host}`)]);
    stages.push([pdu('HTTP', r.dev, from, `${resp.status} ${resp.title}`)]);
  }
  startFlow(stages, { title: 'GET ' + url, done: () => cb(resp) });
}
function pingFlow(from, host, count, out, done) {
  const r = resolveName(from, host);
  const stages = [];
  if (host.endsWith('.local')) stages.push(bcastTo(from, `query A ${host}`, 'mDNS'));
  else if (!/^\d+\.\d+\.\d+\.\d+$/.test(host) && NET.internet.get(from)) stages.push(...dnsLeg(from, host));
  if (r.error) { startFlow(stages, { done: () => { out(r.error === 'mdns' ? `ping: ${host}: Name or service not known (mDNS: not on this network)` : r.error === 'timeout' ? `PING ${host}: 100% packet loss (no route from this network)` : `ping: ${host}: Name or service not known`); done(); } }); return; }
  let target = r.dev;
  if (r.via === 'tunnel') target = r.hub; // ICMP ends on the edge that owns the name
  const ip = r.via === 'tunnel' ? NET.ip(r.hub) : (NET.addr.get(target)?.ip || NET.addr.get(target + '#lan')?.ip || host);
  for (let i = 0; i < count; i++) {
    stages.push([pdu('ICMP', from, target, `echo request seq=${i + 1} to ${ip}`)]);
    stages.push([pdu('ICMP', target, from, `echo reply seq=${i + 1} from ${ip}`)]);
  }
  out(`PING ${host} (${ip}) 56(84) bytes of data.`);
  let seq = 0;
  const f = stages;
  startFlow(f, { done: () => { for (let i = 1; i <= count; i++) out(`64 bytes from ${ip}: icmp_seq=${i} ttl=${r.via === 'lan' ? 64 : 52} time=${(r.via === 'lan' ? 0.4 + Math.random() : 14 + Math.random() * 6).toFixed(1)} ms`); out(`--- ${host} ping statistics ---\n${count} packets transmitted, ${count} received, 0% packet loss`); done(); } });
}

// ── The lab clock ────────────────────────────────────────────────────────
// Realtime mode runs on a clock that can go faster than the wall: 1× to an
// hour a second. LosOS's own timers fire on it (updates.nix: the 00:07
// reboot, the 03:00 upgrade, the 04:30 gc; the compute window; lososd's edge
// rescan and the registrar heartbeat), so a night passes in a minute. The
// qemu-wasm guests keep wall-clock time: an emulated CPU cannot be sped up.
const CLOCK = { t: Date.now(), speed: 1, last: performance.now(), nextScan: 0, nextBeat: 0 };
const SPEEDS = [1, 10, 60, 600, 3600];
const DAILY = [
  { at: '00:07', key: 'reboot', title: 'midnight-reboot.timer' },
  { at: '03:00', key: 'upgrade', title: 'nixos-upgrade.timer' },
  { at: '04:30', key: 'gc', title: 'nix-gc.timer' },
  { at: '07:00', key: 'windowEnd', title: 'compute window closes' },
  { at: '23:00', key: 'windowStart', title: 'compute window opens' },
];
const minuteOf = (t) => { const d = new Date(t); return d.getHours() * 60 + d.getMinutes(); };
const hhmm = (m) => String(Math.floor(m / 60)).padStart(2, '0') + ':' + String(m % 60).padStart(2, '0');
function clockText() {
  const d = new Date(CLOCK.t);
  return d.toLocaleDateString('en-GB', { weekday: 'short', day: 'numeric', month: 'short' }) + ' ' + d.toLocaleTimeString('en-GB');
}
function nextTimer() {
  const m = minuteOf(CLOCK.t);
  const ats = DAILY.map(e => { const [h, mm] = e.at.split(':').map(Number); return { e, m: h * 60 + mm }; });
  const later = ats.filter(a => a.m > m).sort((a, b) => a.m - b.m)[0] || ats.sort((a, b) => a.m - b.m)[0];
  return later;
}
function clockAdvance(ms) {
  const from = CLOCK.t, to = from + ms;
  CLOCK.t = to;
  // daily timers crossed in (from, to]
  for (let day = Math.floor(from / 864e5) - 1; day <= Math.floor(to / 864e5) + 1; day++) {
    for (const e of DAILY) {
      const base = new Date(day * 864e5); const [h, mm] = e.at.split(':').map(Number);
      const at = new Date(base.getFullYear(), base.getMonth(), base.getDate(), h, mm).getTime();
      if (at > from && at <= to) fireTimer(e);
    }
  }
  // lososd rescans its edges every five minutes; boxes on a tunnel announce every minute
  if (to >= CLOCK.nextScan) {
    if (CLOCK.nextScan && CLOCK.speed <= 60) for (const b of world.devices.filter(d => d.type === 'box' && d.power)) startFlow(flowScan(b.id), { title: 'rescan ' + b.name });
    CLOCK.nextScan = to + 300e3;
  }
  if (to >= CLOCK.nextBeat) {
    if (CLOCK.nextBeat && CLOCK.speed <= 10) for (const [bid, st] of NET.losos.box) if (st.path && (st.tunnel === 'registered' || st.tunnel === 'enrolled')) startFlow([[pdu('HTTP', bid, st.path.id, 'POST /heartbeat (registrar TTL)')], [pdu('HTTP', st.path.id, bid, '204')]], { title: 'heartbeat ' + nm(bid) });
    CLOCK.nextBeat = to + 60e3;
  }
}
function losDevices() { return world.devices.filter(d => ['box', 'edge-local', 'edge-official'].includes(d.type) && d.power); }
function fireTimer(e) {
  const L = NET.losos;
  if (e.key === 'reboot') {
    for (const d of losDevices()) {
      d.rebooting = true;
      startFlow([[pdu('lososd', d.id, d.id, '00:07 midnight-reboot.timer: systemctl reboot (tmpfs root, /persist kept)', { local: true })]], { title: 'reboot ' + d.name, done: () => {
        d.rebooting = false; renderCanvas();
        startFlow(flowAddress(d.id), { title: 'address ' + d.name });
        if (d.type === 'box') startFlow(flowScan(d.id), { title: 'edge scan ' + d.name });
        if (d.type === 'edge-local') startFlow(flowUplink(d.id), { title: 'uplink ' + d.name });
      } });
    }
    renderCanvas();
  } else if (e.key === 'upgrade') {
    const cloud = world.devices.find(x => x.type === 'internet' && x.power);
    for (const d of losDevices()) {
      d.gen = (d.gen || 41) + 1;
      const st = [[pdu('lososd', d.id, d.id, `03:00 nixos-upgrade: nixos-rebuild switch --impure --flake ${d.type === 'box' ? 'git+file:///etc/nixos#install' : '/etc/nixos#edge'}`, { local: true })]];
      if (cloud && NET.internet.get(d.id)) st.push([pdu('HTTP', d.id, cloud.id, 'GET https://cache.nixos.org/*.narinfo (substitute)')], [pdu('HTTP', cloud.id, d.id, '200 nar.xz')]);
      st.push([pdu('lososd', d.id, d.id, `switched to generation ${d.gen}; lososd restarted and re-attached to the rebuild`, { local: true })]);
      startFlow(st, { title: 'upgrade ' + d.name });
    }
  } else if (e.key === 'gc') {
    for (const d of losDevices()) startFlow([[pdu('lososd', d.id, d.id, '04:30 nix-collect-garbage --delete-older-than 14d; five boot entries kept', { local: true })]], { title: 'gc ' + d.name });
  } else {
    const open = e.key === 'windowStart';
    for (const [bid, st] of L.box) {
      const b = dev(bid);
      if (!b.cfg.shareCompute || st.mesh !== 'joined') continue;
      b.computeOpen = open;
      startFlow([[pdu('rke2', st.meshEdge, bid, open ? `${e.at} in ${b.name}'s zone: remove taint losos/window:NoSchedule, mesh pods may schedule` : `${e.at}: taint losos/window=closed:NoSchedule, owner's day starts`)]], { title: e.title });
    }
  }
  toast(`${e.at} ${e.title}`);
}
let clockTimer = null;
function startClock() {
  if (clockTimer) return;
  CLOCK.last = performance.now();
  clockTimer = setInterval(() => {
    const now = performance.now(), dt = now - CLOCK.last; CLOCK.last = now;
    if (SIM.mode === 'realtime') clockAdvance(dt * CLOCK.speed);
    const c = document.getElementById('simClock'); if (c) c.textContent = clockText();
  }, 200);
}
function skipToNextTimer() {
  const n = nextTimer();
  const d = new Date(CLOCK.t); const target = new Date(d.getFullYear(), d.getMonth(), d.getDate(), Math.floor(n.m / 60), n.m % 60).getTime();
  const at = target > CLOCK.t ? target : target + 864e5;
  clockAdvance(Math.max(0, at - CLOCK.t - 15e3));
  CLOCK.last = performance.now();
}
