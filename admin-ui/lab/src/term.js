// ── A small terminal for simulated consoles ───────────────────────────────
// Keeps its scrollback per device so switching tabs keeps the session.
const TERMS = new Map();
function ansiToHtml(s) {
  let out = '', open = 0;
  const parts = String(s).split(/\x1b\[([\d;]*)m/);
  for (let i = 0; i < parts.length; i++) {
    if (i % 2 === 0) { out += esc(parts[i]); continue; }
    const codes = parts[i].split(';').filter(Boolean);
    if (!codes.length || codes.includes('0')) { out += '</span>'.repeat(open); open = 0; if (!codes.length) continue; }
    const cls = codes.filter(c => c !== '0').map(c => c === '1' ? 'b-1' : c === '44' ? 'bg-44' : c === '97' ? 'c-37' : 'c-' + c).join(' ');
    if (cls) { out += `<span class="${cls}">`; open++; }
  }
  return out + '</span>'.repeat(open);
}
class SimTerm {
  constructor(devId) { this.devId = devId; this.lines = []; this.input = ''; this.hist = []; this.hi = 0; this.busy = false; this.el = null; this.prompt = () => isGear(dev(devId)) ? `${nm(devId)}# ` : `${nm(devId)}:~# `; }
  attach(host) {
    this.el = document.createElement('div');
    this.el.className = 'term'; this.el.tabIndex = 0;
    this.el.setAttribute('role', 'textbox'); this.el.setAttribute('aria-label', 'Console of ' + nm(this.devId));
    this.el.addEventListener('keydown', (e) => this.key(e));
    this.el.addEventListener('paste', (e) => { this.input += (e.clipboardData.getData('text') || '').replace(/\n/g, ' '); this.render(); e.preventDefault(); });
    host.appendChild(this.el);
    this.render();
    setTimeout(() => this.el && this.el.focus({ preventScroll: true }), 30);
  }
  write(s) { const parts = String(s).split('\n'); if (!this.lines.length) this.lines.push(''); this.lines[this.lines.length - 1] += parts[0]; for (const p of parts.slice(1)) this.lines.push(p); if (this.lines.length > 800) this.lines.splice(0, this.lines.length - 800); this.render(); }
  println(s = '') { this.write(s + '\n'); }
  render() {
    if (!this.el || !this.el.isConnected) return;
    const body = this.lines.map(ansiToHtml).join('\n');
    const tail = this.busy ? '' : esc(this.prompt()) + esc(this.input) + '<span class="cur"> </span>';
    this.el.innerHTML = body + tail;
    this.el.scrollTop = this.el.scrollHeight;
  }
  key(e) {
    if (e.ctrlKey && e.key === 'l') { this.lines = []; this.render(); e.preventDefault(); return; }
    if (e.ctrlKey && e.key === 'c') { this.write(this.prompt() + this.input + '^C\n'); this.input = ''; this.busy = false; this.render(); e.preventDefault(); return; }
    if (this.busy) return;
    if (e.key === 'Enter') {
      const line = this.input; this.input = '';
      this.write(this.prompt() + line + '\n');
      if (line.trim()) { this.hist.push(line); this.hi = this.hist.length; }
      this.busy = true; this.render();
      runCommand(this, line.trim(), () => { this.busy = false; this.render(); });
      e.preventDefault(); return;
    }
    if (e.key === 'Backspace') { this.input = this.input.slice(0, -1); }
    else if (e.key === 'ArrowUp') { if (this.hi > 0) this.input = this.hist[--this.hi] || ''; }
    else if (e.key === 'ArrowDown') { if (this.hi < this.hist.length) this.input = this.hist[++this.hi] || ''; }
    else if (e.key === 'Tab') { const c = completions(this.devId).find(x => x.startsWith(this.input) && x !== this.input); if (c) this.input = c; }
    else if (e.key.length === 1 && !e.ctrlKey && !e.metaKey) this.input += e.key;
    else return;
    e.preventDefault(); this.render();
  }
}
function termFor(id) { if (!TERMS.has(id)) { const t = new SimTerm(id); TERMS.set(id, t); bootText(t); } return TERMS.get(id); }
function completions(id) {
  const d = dev(id); const base = ['help', 'ip addr', 'ip route', 'ping ', 'curl ', 'clear', 'hostname'];
  if (isGear(d)) return ['help', 'show interfaces', 'show ip route', 'show arp', 'show dhcp', 'show version', 'ping ', 'clear'];
  if (d.type === 'laptop') return [...base, 'avahi-resolve -n ', 'avahi-browse -rt _losos-edge._tcp', 'resolvectl query '];
  if (d.type === 'box') return [...base, 'losos-ctl status', 'losos-ctl edge', 'journalctl -u losos-rathole-client', 'avahi-browse -rt _losos-edge._tcp'];
  if (d.type === 'edge-local') return [...base, 'losos-edge boxes', 'journalctl -u losos-registrar', 'journalctl -u losos-rathole-uplink'];
  return [...base, 'journalctl -u losos-registrar', 'losos-registrar tenants'];
}
function bootText(t) {
  const d = dev(t.devId);
  if (d.type === 'laptop') { t.println('\x1b[90mLosOS Desktop · derisk · Terminal\x1b[0m'); t.println('Type \x1b[1mhelp\x1b[0m for the commands this simulated shell knows.'); return; }
  t.println('\x1b[90m[sim] Simulated console. Commands answer from the simulator\'s model, not from a running guest.\x1b[0m');
  if (!d.power) { t.println('\x1b[90m(powered off)\x1b[0m'); return; }
  t.println(banner(d).join('\n'));
  t.println('');
}
function banner(d) {
  const ip = NET.ip(d.id);
  const row = (s) => `\x1b[44;97m ${s.padEnd(58)} \x1b[0m`;
  if (isGear(d)) {
    const m = NET.mgmt.get(d.id);
    return ['', row(''), row('Netzgeräte Betriebssystem 1.0'), row(''), row(`${{ router: 'Router', switch: 'Switch', ap: 'Wi-Fi access point' }[d.type]} ${d.name} is ready`),
      ...(m ? [row(`  Address:  ${m.ip}/${m.mask}`), row(`  Status:   http://${m.ip}`)] : [row('  No address configured (losos.ip).')]), row(''), '',
      'Maintenance console. Try: show interfaces | show ip route | show arp', '                          show dhcp | show version'];
  }
  const title = d.type === 'box' ? 'LosOS is ready' : d.type === 'edge-local' ? `LosOS edge gateway (${d.name}) is ready` : `LosOS official edge (${d.name}) is ready`;
  return ['', row(''), row(title), row(''), ...(ip ? [row('On any computer on this network, open a web browser at:'), row('  http://' + ip), row('or, on computers that find the box by name:'), row('  http://' + d.name + '.local')] : [row('No network address yet. Plug in an Ethernet cable.')]), row('')];
}
function ipAddr(id) {
  const d = dev(id), a = NET.addr.get(id), f = NET.iface.get(id);
  const out = ['1: lo: <LOOPBACK,UP,LOWER_UP> mtu 65536', '    inet 127.0.0.1/8 scope host lo'];
  TYPES[d.type].ports.forEach(([p], i) => {
    const up = f && f.port === p;
    out.push(`${i + 2}: ${p}: <BROADCAST,MULTICAST${up ? ',UP,LOWER_UP' : ',NO-CARRIER'}> mtu 1500 state ${up ? 'UP' : 'DOWN'}`);
    out.push(`    link/ether ${d.mac}`);
    if (up && a) out.push(`    inet ${a.ip}/${a.mask} ${a.src === 'dhcp' ? 'dynamic ' : ''}scope global ${p}`);
  });
  return out.join('\n');
}
function runCommand(t, line, done) {
  const d = dev(t.devId);
  const out = (s) => t.println(s);
  if (!d || !d.power) { out('(device is powered off)'); return done(); }
  const [cmd, ...args] = line.split(/\s+/);
  const a = isGear(d) ? NET.mgmt.get(d.id) : NET.addr.get(d.id);
  if (isGear(d) && cmd === 'show') { out(gearShow(d, args)); return done(); }
  switch (cmd) {
    case '': return done();
    case 'help': out(completions(d.id).map(c => '  ' + c.trim()).join('\n')); return done();
    case 'clear': t.lines = []; return done();
    case 'hostname': out(d.name); return done();
    case 'uname': out(`Linux ${d.name} 6.12.51 #1-NixOS SMP PREEMPT_DYNAMIC x86_64 GNU/Linux`); return done();
    case 'ip':
      if (args[0] === 'route' || args[0] === 'r') { out(a ? (a.gw ? `default via ${a.gw} dev ${NET.iface.get(d.id).port} proto dhcp\n` : '') + `${a.ip.split('.').slice(0, 3).join('.')}.0/${a.mask} dev ${NET.iface.get(d.id).port} proto kernel scope link src ${a.ip}` : 'no routes'); return done(); }
      out(ipAddr(d.id)); return done();
    case 'ping': {
      const host = args.filter(x => !x.startsWith('-') && !/^\d+$/.test(x))[0];
      if (!host) { out('usage: ping <host>'); return done(); }
      if (!a) { out('ping: connect: Network is unreachable'); return done(); }
      return pingFlow(d.id, host, 3, out, done);
    }
    case 'curl': {
      const url = args.filter(x => !x.startsWith('-'))[0];
      if (!url) { out('usage: curl <url>'); return done(); }
      if (!a) { out('curl: (7) Network is unreachable'); return done(); }
      return httpFlow(d.id, url, (r) => {
        if (r.error) out({ mdns: `curl: (6) Could not resolve host: ${r.host}`, nodns: `curl: (6) Could not resolve host: ${r.host}`, nxdomain: `curl: (6) Could not resolve host: ${r.host}`, timeout: 'curl: (28) Connection timed out', refused: 'curl: (7) Connection refused' }[r.error] || 'curl: error');
        else out(r.text ?? `${r.status} ${r.title}`);
        done();
      });
    }
    case 'avahi-resolve': case 'resolvectl': {
      const host = args[args.length - 1];
      const r = resolveName(d.id, host || '');
      out(r.error ? `Failed to resolve host name '${host}': ${r.error === 'mdns' ? 'Timeout reached' : 'Name not found'}` : `${host}\t${NET.ip(r.via === 'tunnel' ? r.hub : r.dev)}`);
      return done();
    }
    case 'avahi-browse': {
      const es = world.devices.filter(e => e.type === 'edge-local' && e.power && e.cfg.advertise && NET.sameSeg(d.id, e.id));
      startFlow([bcastTo(d.id, 'query PTR _losos-edge._tcp.local', 'mDNS'), es.map(e => pdu('mDNS', e.id, d.id, `answer ${e.name}._losos-edge._tcp`))], { done: () => {
        if (!es.length) out('(no _losos-edge._tcp services on this network)');
        for (const e of es) out(`= ${NET.iface.get(d.id).port} IPv4 ${e.name}  _losos-edge._tcp  local\n   hostname = [${e.name}.local]\n   address = [${NET.ip(e.id)}]\n   port = [8443]\n   txt = ["url=http://${e.name}.local:8443" "rathole=${e.name}.local:2333" "enrol=${e.cfg.openEnrolment ? 'open' : 'closed'}"]`);
        done();
      } });
      return;
    }
    case 'losos-ctl': {
      if (d.type !== 'box') break;
      const st = NET.losos.box.get(d.id);
      if (args[0] === 'edge') { out(JSON.stringify({ path: st.path ? { name: nm(st.path.id), url: st.path.url, source: st.path.source } : null, edges: st.edges.map(e => ({ name: nm(e.id), url: e.url, official: e.official })) }, null, 2)); return done(); }
      out(JSON.stringify({ hostName: d.name, mode: d.cfg.joinMesh ? 'mesh' : 'local', address: a?.ip || null, internet: !!NET.internet.get(d.id), edge: st.path ? nm(st.path.id) : null, tunnel: st.tunnel, publicName: st.publicName, mesh: st.mesh, market: st.market }, null, 2));
      return done();
    }
    case 'losos-edge': {
      if (d.type !== 'edge-local') break;
      const sp = NET.losos.spoke.get(d.id);
      out(sp.enrolled.length ? sp.enrolled.map(b => `${nm(b)}\t${NET.ip(b)}\t${NET.losos.box.get(b).publicName || 'LAN only'}`).join('\n') : '(no enrolled boxes)');
      return done();
    }
    case 'losos-registrar': {
      if (d.type !== 'edge-official') break;
      const h = NET.losos.hub.get(d.id);
      out(['tenants: ' + (h.tenants.map(nm).join(', ') || '—'), 'relayed: ' + (h.relays.map(b => NET.losos.box.get(b).publicName).join(', ') || '—')].join('\n'));
      return done();
    }
    case 'journalctl': {
      const unit = args[args.indexOf('-u') + 1] || '';
      out(journal(d, unit)); return done();
    }
  }
  out(`${cmd}: command not found (simulated shell; try help)`);
  done();
}
function journal(d, unit) {
  const ts = () => new Date().toTimeString().slice(0, 8);
  if (d.type === 'box') {
    const st = NET.losos.box.get(d.id);
    if (!st.path) return `${ts()} ${d.name} systemd[1]: losos-rathole-client.service: skipped, ConditionPathExists=!/run/losos/edge-none was not met`;
    return [`${ts()} ${d.name} losos-rathole-client[812]: pinned Noise key for ${nm(st.path.id)}`, `${ts()} ${d.name} losos-rathole-client[812]: control channel established to ${nm(st.path.id)}:2333`, st.tunnel === 'refused' ? `${ts()} ${d.name} losos-registrar-announce[815]: register refused: ${st.reason}` : `${ts()} ${d.name} losos-registrar-announce[815]: registered (${st.tunnel})${st.publicName ? ' as ' + st.publicName : ''}`].join('\n');
  }
  if (d.type === 'edge-local') {
    const sp = NET.losos.spoke.get(d.id);
    return [...sp.enrolled.map(b => `${ts()} ${d.name} losos-registrar[402]: enrolled ${nm(b)} (trust on first use)`), `${ts()} ${d.name} losos-registrar[402]: uplink ${sp.uplink}${sp.uplink === 'up' ? `, relayed ${sp.relayed.length} box(es) under ${d.cfg.zone}` : ''}`].join('\n');
  }
  const h = NET.losos.hub.get(d.id);
  return [`${ts()} ${d.name} losos-registrar[388]: ${h.tenants.length} tenant(s), ${h.relays.length} relayed box(es)`, ...h.relays.map(b => `${ts()} ${d.name} losos-registrar[388]: route ${NET.losos.box.get(b).publicName} via spoke`)].join('\n');
}

// Netzgeräte Betriebssystem's `show`, answered from the model
function gearShow(d, args) {
  const m = NET.mgmt.get(d.id);
  const what = args[0] || '';
  if (what.startsWith('int')) {
    const rows = [['Interface', 'State', 'MAC', 'Address']];
    rows.push(['eth0', m ? 'up' : 'down', d.mac, m ? `${m.ip}/${m.mask}` : '-']);
    for (const [p] of TYPES[d.type].ports) {
      const l = world.links.find(l => (l.a.dev === d.id && l.a.port === p) || (l.b.dev === d.id && l.b.port === p));
      rows.push(['  ' + p, l && linkUp(l) ? 'up' : 'down', '', l ? '→ ' + nm(otherEnd(l, d.id).dev) : '']);
    }
    return rows.map(r => r[0].padEnd(10) + ' ' + r[1].padEnd(8) + ' ' + r[2].padEnd(18) + ' ' + r[3]).join('\n');
  }
  if (what === 'ip') return m ? (m.gw ? `default via ${m.gw} dev eth0\n` : '') + `${m.ip.split('.').slice(0, 3).join('.')}.0/${m.mask} dev eth0 scope link  src ${m.ip}` : '';
  if (what === 'arp') {
    const seg = NET.gearSeg.get(d.id), s = seg && NET.segs.get(seg);
    if (!s) return '';
    return s.members.filter(id => NET.addr.get(id)).map(id => `? (${NET.ip(id)}) at ${dev(id).mac} [ether]  on eth0`).join('\n');
  }
  if (what === 'dhcp') {
    if (d.type !== 'router') return `No DHCP server on a ${d.type}.`;
    const leases = [...NET.addr].filter(([, v]) => v.router === d.id && v.src === 'dhcp');
    if (!leases.length) return `No leases yet (pool ${NET.subnet.get(d.id)}.100-${NET.subnet.get(d.id)}.199).`;
    return ['Mac Address       IP Address      Host Name           Expires in', ...leases.map(([id, v]) => `${dev(id).mac.padEnd(17)} ${v.ip.padEnd(15)} ${nm(id).padEnd(19)} 23:59:12`)].join('\n');
  }
  if (what.startsWith('ver')) return 'NAME="Netzgeräte Betriebssystem"\nPRETTY_NAME="Netzgeräte Betriebssystem 1.0"\nID=netzgeraete\nVERSION_ID=1.0\nLinux ' + d.name + ' 6.1.0 #1 SMP PREEMPT_DYNAMIC x86_64 GNU/Linux';
  return 'usage: show interfaces | ip route | arp | dhcp | version';
}

// ── A byte-stream terminal for real guests where xterm is not shipped ────
// The box's copy carries no xterm.js, but a guest under libvirt is a real
// serial console: bytes in, keystrokes out. This draws what a busybox shell
// sends (text, CR, backspace, colours, erasing and cursor left/right),
// which is all the Lab's guests use, and sends keys as a terminal would.
class ByteTerm {
  constructor(label) { this.label = label; this.lines = [[]]; this.row = 0; this.col = 0; this.sgr = ''; this.esc = null; this.dec = new TextDecoder(); this.el = null; this.send = () => {}; this.queued = false; }
  write(bytes) {
    for (const ch of this.dec.decode(bytes, { stream: true })) this.put(ch);
    if (this.lines.length > 2000) { const cut = this.lines.length - 2000; this.lines.splice(0, cut); this.row -= cut; }
    if (!this.queued) { this.queued = true; requestAnimationFrame(() => { this.queued = false; this.render(); }); }
  }
  put(ch) {
    if (this.esc !== null) {
      this.esc += ch;
      if (this.esc === '[' || this.esc === ']') return;
      if (this.esc[0] === ']') { if (ch === '\x07' || this.esc.endsWith('\x1b\\')) this.esc = null; return; }
      if (this.esc[0] !== '[') { this.esc = null; return; }
      if (!/[@-~]/.test(ch)) return;
      const params = this.esc.slice(1, -1), n = parseInt(params, 10) || 1, line = this.lines[this.row];
      if (ch === 'm') this.sgr = params === '0' || params === '' ? '' : params;
      else if (ch === 'K') line.length = Math.min(line.length, this.col);
      else if (ch === 'D') this.col = Math.max(0, this.col - n);
      else if (ch === 'C') this.col += n;
      else if (ch === 'J' && params === '2') { this.lines = [[]]; this.row = 0; this.col = 0; }
      else if (ch === 'J') { line.length = Math.min(line.length, this.col); this.lines.length = this.row + 1; }
      else if (ch === 'H') this.col = 0;
      this.esc = null; return;
    }
    if (ch === '\x1b') { this.esc = ''; return; }
    if (ch === '\r') { this.col = 0; return; }
    if (ch === '\n') { this.row++; if (!this.lines[this.row]) this.lines[this.row] = []; this.col = 0; return; }
    if (ch === '\b') { this.col = Math.max(0, this.col - 1); return; }
    if (ch < ' ' || ch === '\x7f') return;
    const line = this.lines[this.row];
    while (line.length < this.col) line.push({ ch: ' ', sgr: '' });
    line[this.col++] = { ch, sgr: this.sgr };
  }
  text(last = 40) { return this.lines.slice(-last).map(l => l.map(c => c.ch).join('')).join('\n'); }
  render() {
    if (!this.el || !this.el.isConnected) return;
    const html = this.lines.map((l, r) => {
      let s = '', cur = '';
      l.forEach((c, i) => {
        if (c.sgr !== cur) { s += '\x1b[0m' + (c.sgr ? '\x1b[' + c.sgr + 'm' : ''); cur = c.sgr; }
        s += r === this.row && i === this.col ? '\x00' + c.ch + '\x01' : c.ch;
      });
      if (r === this.row && this.col >= l.length) s += '\x1b[0m' + ' '.repeat(this.col - l.length) + '\x00 \x01';
      return ansiToHtml(s + '\x1b[0m');
    }).join('\n').replace(/\x00/g, '<span class="cur">').replace(/\x01/g, '</span>');
    this.el.innerHTML = html;
    this.el.scrollTop = this.el.scrollHeight;
  }
  attach(host) {
    this.el = document.createElement('div');
    this.el.className = 'term'; this.el.tabIndex = 0;
    this.el.setAttribute('role', 'textbox'); this.el.setAttribute('aria-label', this.label);
    this.el.addEventListener('keydown', (e) => this.key(e));
    this.el.addEventListener('paste', (e) => { this.send((e.clipboardData.getData('text') || '').replace(/\n/g, '\r')); e.preventDefault(); });
    host.appendChild(this.el);
    this.render();
    setTimeout(() => this.el && this.el.focus({ preventScroll: true }), 30);
  }
  key(e) {
    const keys = { Enter: '\r', Backspace: '\x7f', Tab: '\t', Escape: '\x1b', ArrowUp: '\x1b[A', ArrowDown: '\x1b[B', ArrowRight: '\x1b[C', ArrowLeft: '\x1b[D', Home: '\x1b[H', End: '\x1b[F', Delete: '\x1b[3~' };
    let out = keys[e.key];
    if (!out && e.ctrlKey && /^[a-z]$/i.test(e.key)) out = String.fromCharCode(e.key.toUpperCase().charCodeAt(0) - 64);
    if (!out && e.key.length === 1 && !e.ctrlKey && !e.metaKey) out = e.key;
    if (!out) return;
    this.send(out); e.preventDefault();
  }
}
