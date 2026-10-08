// ── qemu-wasm engine ────────────────────────────────────────────────────
// One qemu-system-x86_64 (ktock/qemu-wasm, emscripten, pthreads) per powered
// box or edge. Each guest's virtio-net is a QEMU "socket" netdev; emscripten
// turns its TCP connect into a WebSocket, which this file intercepts and
// plugs into the simulated switches. Routers answer DHCP, ARP and ping in
// JavaScript; frames between two guests on one segment are passed through
// untouched.
const ENGINE = (() => {
  const E = { available: false, reason: '', vms: new Map(), stats: { sockets: 0, framesIn: 0, framesOut: 0, last: '' } };
  const BASE = 'qemu/';
  const FILES = { 'bzImage': 'guest/bzImage', 'rootfs.bin': 'guest/rootfs.bin', 'bios-256k.bin': 'qemu/pc-bios/bios-256k.bin', 'kvmvapic.bin': 'qemu/pc-bios/kvmvapic.bin', 'linuxboot_dma.bin': 'qemu/pc-bios/linuxboot_dma.bin', 'vgabios-stdvga.bin': 'qemu/pc-bios/vgabios-stdvga.bin', 'efi-virtio.rom': 'qemu/pc-bios/efi-virtio.rom', 'gear.bin': 'guest/gear.bin' };
  let blobs = null, factory = null;
  const loadScript = (src) => new Promise((ok, bad) => { const s = document.createElement('script'); s.src = src; s.onload = ok; s.onerror = () => bad(new Error('could not load ' + src)); document.head.appendChild(s); });
  E.init = async () => {
    if (!LAB.engine) { E.reason = LAB.box ? 'This copy of the lab ships without the qemu-wasm engine, so consoles are simulated. The hosted copy boots real guests.' : 'This viewer is not cross-origin isolated, so qemu-wasm cannot start its threads here.'; return; }
    if (!self.crossOriginIsolated) { E.reason = 'This page is served without COOP/COEP headers, so qemu-wasm cannot start its threads. Use serve.py.'; return; }
    try {
      const r = await fetch(BASE + 'out.js', { method: 'HEAD' });
      if (!r.ok) throw new Error('missing');
    } catch { E.reason = 'The qemu-wasm build is not next to this page.'; return; }
    try {
      splashSay('Loading qemu-wasm and the x86_64 guests');
      await loadScript('vendor/xterm.js');
      await loadScript('vendor/xterm-pty.js');
      const css = document.createElement('link'); css.rel = 'stylesheet'; css.href = 'vendor/xterm.css'; document.head.appendChild(css);
      factory = (await import(new URL(BASE + 'out.js', location.href).href)).default;
      blobs = {};
      await Promise.all(Object.entries(FILES).map(async ([name, url]) => { const r = await fetch(url); if (!r.ok) throw new Error(url); blobs[name] = new Uint8Array(await r.arrayBuffer()); }));
      E.available = true;
      installSocketShim();
      trackWorkers();
    } catch (e) { E.reason = 'qemu-wasm failed to load: ' + e.message; }
  };
  E.running = (id) => E.vms.has(id);
  E.stopAll = () => { for (const id of [...E.vms.keys()]) E.stop(id); };

  // Pthread workers carry the VM id in their URL, so a power-off can end them.
  const workersByVm = new Map();
  function trackWorkers() {
    const W = window.Worker;
    window.Worker = function (url, opts) {
      const w = new W(url, opts);
      const m = String(url).match(/[?&]vm=([\w-]+)/);
      if (m) { if (!workersByVm.has(m[1])) workersByVm.set(m[1], []); workersByVm.get(m[1]).push(w); }
      return w;
    };
    window.Worker.prototype = W.prototype;
  }

  // ── The socket shim and the fabric ─────────────────────────────────────
  const ports = new Map(); // devId → {sock, buf}
  function installSocketShim() {
    const Real = window.WebSocket;
    class FabricSocket extends EventTarget {
      constructor(url) {
        super();
        this.url = url; this.readyState = 0; this.binaryType = 'arraybuffer'; this.protocol = 'binary'; this.bufferedAmount = 0;
        this.devId = url.split('/').pop(); E.stats.sockets++;
        ports.set(this.devId, { sock: this, buf: new Uint8Array(0) });
        setTimeout(() => { this.readyState = 1; this._fire('open'); }, 0);
      }
      _fire(type, data) { const ev = type === 'message' ? new MessageEvent('message', { data }) : new Event(type); this['on' + type] && this['on' + type](ev); this.dispatchEvent(ev); }
      send(data) {
        const p = ports.get(this.devId); if (!p) return;
        const chunk = data instanceof ArrayBuffer ? new Uint8Array(data) : new Uint8Array(data.buffer ? data.buffer.slice(data.byteOffset, data.byteOffset + data.byteLength) : data);
        const nb = new Uint8Array(p.buf.length + chunk.length); nb.set(p.buf); nb.set(chunk, p.buf.length); p.buf = nb;
        while (p.buf.length >= 4) {
          const len = (p.buf[0] << 24 | p.buf[1] << 16 | p.buf[2] << 8 | p.buf[3]) >>> 0;
          if (p.buf.length < 4 + len) break;
          const frame = p.buf.slice(4, 4 + len); p.buf = p.buf.slice(4 + len);
          E.stats.framesIn++; E.stats.last = macStr(frame, 0) + ' t=' + ((frame[12] << 8) | frame[13]).toString(16);
          try { fabricIn(this.devId, frame); } catch (e) { console.warn('fabric', e); }
        }
      }
      close() { this.readyState = 3; ports.delete(this.devId); this._fire('close'); }
    }
    // emscripten's SOCKFS reads these off the instance (sock.readyState === sock.OPEN);
    // without them a poll never reports POLLOUT, and QEMU's socket netdev waits
    // for that before it starts reading, so the guest would never receive.
    for (const [k, v] of Object.entries({ CONNECTING: 0, OPEN: 1, CLOSING: 2, CLOSED: 3 })) { FabricSocket[k] = v; FabricSocket.prototype[k] = v; }
    window.WebSocket = function (url, protocols) {
      if (String(url).startsWith('ws://losos-lab/')) return new FabricSocket(String(url));
      return new Real(url, protocols);
    };
    Object.assign(window.WebSocket, { CONNECTING: 0, OPEN: 1, CLOSING: 2, CLOSED: 3 });
  }
  function deliver(devId, frame) {
    const p = ports.get(devId); if (!p || p.sock.readyState !== 1) return;
    const out = new Uint8Array(4 + frame.length);
    out[0] = frame.length >>> 24; out[1] = (frame.length >>> 16) & 255; out[2] = (frame.length >>> 8) & 255; out[3] = frame.length & 255;
    out.set(frame, 4); E.stats.framesOut++; E.stats.lastOut = Array.from(frame, x => x.toString(16).padStart(2, '0')).join('');
    p.sock._fire('message', out.buffer);
  }
  const macStr = (b, o) => Array.from(b.slice(o, o + 6), x => x.toString(16).padStart(2, '0')).join(':');
  const macBytes = (s) => s.split(':').map(h => parseInt(h, 16));
  const ipStr = (b, o) => Array.from(b.slice(o, o + 4)).join('.');
  const ipBytes = (s) => s.split('.').map(Number);
  const ROUTER_MAC = (rid) => '52:54:00:0a:' + rid.replace(/\D/g, '').padStart(2, '0').slice(-2) + ':01';
  function csum(b, o, n) { let s = 0; for (let i = 0; i < n; i += 2) s += (b[o + i] << 8) + (i + 1 < n ? b[o + i + 1] : 0); while (s >> 16) s = (s & 0xffff) + (s >> 16); return (~s) & 0xffff; }
  // a router's guest takes over DHCP, ARP and ping once its console says it is ready
  const guestUp = (id) => !!E.vms.get(id)?.ready;
  const guestSeg = (id) => NET.gearSeg.get(id) || NET.segOf(id);
  const lastLog = new Map();
  function fabricIn(src, f) {
    const dst = macStr(f, 0), type = (f[12] << 8) | f[13];
    const seg = guestSeg(src);
    const isDhcp = type === 0x0800 && f[23] === 17 && ((f[34] << 8 | f[35]) === 68 || (f[36] << 8 | f[37]) === 67);
    const proto = type === 0x0806 ? 'ARP' : type === 0x0800 ? (f[23] === 1 ? 'ICMP' : isDhcp ? 'DHCP' : f[23] === 6 && ((f[34] << 8 | f[35]) === 80 || (f[36] << 8 | f[37]) === 80) ? 'HTTP' : null) : null;
    const s = seg ? NET.segs.get(seg) : null;
    // every other running guest on the same segment (switches, buses and APs flood)
    let toDev = null;
    if (seg) for (const id of E.vms.keys()) {
      if (id === src || guestSeg(id) !== seg) continue;
      const m = E.vms.get(id).mac;
      if (dst === 'ff:ff:ff:ff:ff:ff' || (f[0] & 1) || dst === m) { deliver(id, f); if (dst === m) toDev = id; }
    }
    // the segment's router and the simulated hosts on it
    if (s) answer(src, s, f);
    if (!proto || (proto === 'DHCP' && !(s && s.router && guestUp(s.router)))) return;
    const k = src + proto + (toDev || '');
    if (performance.now() - (lastLog.get(k) || 0) < 25) return;
    lastLog.set(k, performance.now());
    const info = proto === 'ARP' ? `ARP ${f[21] === 1 ? 'who-has ' + ipStr(f, 38) : ipStr(f, 28) + ' is-at ' + macStr(f, 22)}`
      : proto === 'ICMP' ? `ICMP ${f[34] === 8 ? 'echo request' : f[34] === 0 ? 'echo reply' : 'type ' + f[34]} ${ipStr(f, 26)} → ${ipStr(f, 30)}`
      : proto === 'DHCP' ? `DHCP ${(f[36] << 8 | f[37]) === 67 ? 'from client ' + macStr(f, 6) : 'from udhcpd on ' + nm(src)}`
      : `TCP :80 ${ipStr(f, 26)} → ${ipStr(f, 30)}`;
    const target = toDev || (s && (s.router || null)) || null;
    if (target && target !== src) startFlow([[pdu(proto, src, target, info, { real: true })]]);
  }
  function answer(src, s, f) {
    const type = (f[12] << 8) | f[13];
    const hosts = [];
    if (s.router && dev(s.router).power && !guestUp(s.router)) hosts.push({ id: s.router, ip: NET.addr.get(s.router + '#lan')?.ip, mac: ROUTER_MAC(s.router), router: true });
    for (const id of s.members) { if (id === src || E.vms.has(id) || !TYPES[dev(id).type].endpoint) continue; const a = NET.addr.get(id); if (a) hosts.push({ id, ip: a.ip, mac: dev(id).mac }); }
    for (const [id, sg] of NET.gearSeg) { if (sg !== s.id || id === src || E.vms.has(id) || dev(id).type === 'router') continue; const m = NET.mgmt.get(id); if (m) hosts.push({ id, ip: m.ip, mac: dev(id).mac }); }
    if (type === 0x0806 && f[21] === 1) {
      const want = ipStr(f, 38); const h = hosts.find(x => x.ip === want); if (!h) return;
      const r = new Uint8Array(42);
      r.set(f.slice(6, 12), 0); r.set(macBytes(h.mac), 6); r[12] = 8; r[13] = 6;
      r.set([0, 1, 8, 0, 6, 4, 0, 2], 14); r.set(macBytes(h.mac), 22); r.set(ipBytes(h.ip), 28); r.set(f.slice(22, 28), 32); r.set(f.slice(28, 32), 38);
      setTimeout(() => { deliver(src, r); if (h.id !== s.router || true) startFlow([[pdu('ARP', h.id, src, `ARP ${h.ip} is-at ${h.mac}`, { real: true })]]); }, 2);
      return;
    }
    if (type !== 0x0800) return;
    const ihl = (f[14] & 15) * 4, p = f[23], dstIp = ipStr(f, 30);
    if (p === 1 && f[14 + ihl] === 8) {
      const h = hosts.find(x => x.ip === dstIp); if (!h) return;
      const r = f.slice();
      r.set(f.slice(6, 12), 0); r.set(macBytes(h.mac), 6);
      r.set(f.slice(30, 34), 26); r.set(f.slice(26, 30), 30); r[22] = 64; r[24] = 0; r[25] = 0;
      const ic = csum(r, 14, ihl); r[24] = ic >> 8; r[25] = ic & 255;
      const o = 14 + ihl; r[o] = 0; r[o + 2] = 0; r[o + 3] = 0;
      const c2 = csum(r, o, r.length - o); r[o + 2] = c2 >> 8; r[o + 3] = c2 & 255;
      setTimeout(() => { deliver(src, r); startFlow([[pdu('ICMP', h.id, src, `ICMP echo reply ${h.ip} → ${ipStr(f, 26)} (answered by the simulator)`, { real: true })]]); }, 3);
      return;
    }
    if (p === 17 && (f[14 + ihl + 2] << 8 | f[14 + ihl + 3]) === 67 && s.router && dev(s.router).power && !guestUp(s.router)) dhcp(src, s, f, 14 + ihl + 8, hosts[0]);
  }
  function dhcp(src, s, f, o, rt) {
    let mt = 0;
    for (let i = o + 240; i < f.length && f[i] !== 255;) { const c = f[i], l = f[i + 1]; if (c === 53) mt = f[i + 2]; i += c === 0 ? 1 : 2 + l; }
    if (mt !== 1 && mt !== 3) return;
    const lease = NET.addr.get(src); if (!lease || lease.src !== 'dhcp') return;
    const opts = [53, 1, mt === 1 ? 2 : 5, 54, 4, ...ipBytes(rt.ip), 51, 4, 0, 1, 81, 128, 1, 4, 255, 255, 255, 0, 3, 4, ...ipBytes(lease.gw), 6, 4, ...ipBytes(lease.gw), 255];
    const bootp = new Uint8Array(240 + opts.length);
    bootp[0] = 2; bootp[1] = 1; bootp[2] = 6; bootp.set(f.slice(o + 4, o + 8), 4);
    bootp.set(ipBytes(lease.ip), 16); bootp.set(ipBytes(rt.ip), 20); bootp.set(f.slice(o + 28, o + 44), 28);
    bootp.set([99, 130, 83, 99], 236); bootp.set(opts, 240);
    const udpLen = 8 + bootp.length, ipLen = 20 + udpLen;
    const r = new Uint8Array(14 + ipLen);
    r.set([255, 255, 255, 255, 255, 255], 0); r.set(macBytes(rt.mac), 6); r[12] = 8; r[13] = 0;
    r.set([0x45, 0, ipLen >> 8, ipLen & 255, 0, 0, 0, 0, 64, 17, 0, 0], 14); r.set(ipBytes(rt.ip), 26); r.set([255, 255, 255, 255], 30);
    const c = csum(r, 14, 20); r[24] = c >> 8; r[25] = c & 255;
    r.set([0, 67, 0, 68, udpLen >> 8, udpLen & 255, 0, 0], 34); r.set(bootp, 42);
    setTimeout(() => deliver(src, r), 5);
    startFlow([[pdu('DHCP', src, s.router, `DHCP${mt === 1 ? 'DISCOVER' : 'REQUEST'} from the guest's udhcpc`, { real: true })], [pdu('DHCP', s.router, src, `DHCP${mt === 1 ? 'OFFER' : 'ACK'} ${lease.ip} (router in the simulator)`, { real: true })]]);
  }

  // ── Starting and stopping guests ───────────────────────────────────────
  // Guests start one after another: two emscripten instances initialising
  // at the same moment raced each other and one of them never printed.
  // At most three at once: each guest is a TCG CPU plus QEMU's own threads,
  // and with a fourth the tab ran out of cores and every guest stalled
  // (a kernel panic in the timer check, or no output at all).
  E.MAX = 3;
  E.full = () => E.vms.size >= E.MAX;
  const FULL = `${E.MAX} guests are running, as many as one tab runs well. Power one off to boot this one.`;
  let startChain = Promise.resolve();
  E.start = (id) => { const p = startChain.then(() => startNow(id)).then(() => new Promise(r => setTimeout(r, 400))); startChain = p.catch(() => {}); return p; };
  const startNow = async (id) => {
    if (!E.available || E.vms.has(id)) return;
    if (E.full()) { toast(FULL, 6000); return; }
    const d = dev(id); evaluate();
    const a = NET.addr.get(id);
    const seg = NET.segOf(id) ? NET.segs.get(NET.segOf(id)) : null;
    const gear = isGear(d);
    const append = [`console=ttyS0`, 'root=/dev/vda', 'ro', 'rootwait', 'loglevel=4', 'no_timer_check', `losos.host=${d.name}`, `losos.role=${d.type === 'box' ? 'box' : d.type}`];
    if (gear) {
      const m = NET.mgmt.get(id);
      if (m) append.push(`losos.ip=${m.ip}/${m.mask}`);
      if (m && m.gw) append.push(`losos.gw=${m.gw}`);
      if (d.type === 'router' && m) {
        const sn = NET.subnet.get(id);
        append.push(`losos.dhcp=${sn}.100-${sn}.199`);
        const leases = [...NET.addr].filter(([k, v]) => v.router === id && v.src === 'dhcp' && dev(k)).map(([k, v]) => dev(k).mac + '@' + v.ip);
        if (leases.length) append.push('losos.leases=' + leases.join(','));
      }
    } else if (a && !(seg && seg.kind === 'lan')) append.push(`losos.ip=${a.ip}/${a.mask}`);
    const { master, slave } = openpty();
    const term = new Terminal({ fontSize: 12, fontFamily: 'IBM Plex Mono, Menlo, monospace', theme: { background: '#000000' }, convertEol: false, scrollback: 2000 });
    term.loadAddon(master);
    const vm = { term, slave, started: performance.now(), mac: d.type === 'router' ? ROUTER_MAC(id) : d.mac };
    E.vms.set(id, vm);
    const Module = {
      arguments: ['-nographic', '-M', 'pc', '-m', '96M', '-accel', 'tcg,tb-size=64', '-L', '/pack/', '-vga', 'none', '-nic', 'none',
        '-netdev', 'socket,id=n0,connect=localhost:8888', '-device', `virtio-net-pci,netdev=n0,mac=${vm.mac},romfile=`,
        '-drive', `if=virtio,format=raw,file=/pack/${gear ? 'gear.bin' : 'rootfs.bin'},readonly=on`,
        '-kernel', '/pack/bzImage', '-append', append.join(' ')],
      pty: slave,
      websocket: { url: 'ws://losos-lab/' + id },
      locateFile: (p) => BASE + p + (p.endsWith('.worker.js') ? '?vm=' + id : ''),
      mainScriptUrlOrBlob: new URL(BASE + 'out.js', location.href).href,
      preRun: [(mod) => { mod.FS.mkdir('/pack'); for (const [n, b] of Object.entries(blobs)) mod.FS.writeFile('/pack/' + n, b); }],
      print: (t) => console.log('[' + d.name + ']', t), printErr: (t) => console.warn('[' + d.name + ']', t),
    };
    vm.module = Module;
    try {
      await factory(Module);
      vm.readyPoll = setInterval(() => {
        const b = term.buffer.active; let txt = '';
        for (let i = Math.max(0, b.length - 40); i < b.length; i++) txt += b.getLine(i).translateToString(true) + '\n';
        if (/is ready/.test(txt) && /# *$/m.test(txt)) { vm.ready = true; clearInterval(vm.readyPoll); renderCanvas(); if (UI.sel?.id === id) renderSide(); }
      }, 1000);
      const pty = Module.pty, oldPoll = Module.TTY.stream_ops.poll;
      Module.TTY.stream_ops.poll = function (stream, timeout) { if (!pty.readable) return (pty.readable ? 1 : 0) | (pty.writable ? 4 : 0); return oldPoll.call(stream, timeout); };
    } catch (e) { term.write('\r\nqemu-wasm failed: ' + e.message + '\r\n'); }
    renderCanvas(); if (UI.sel?.id === id) renderSide();
  };
  E.stop = (id) => {
    const vm = E.vms.get(id); if (!vm) return;
    clearInterval(vm.readyPoll);
    for (const w of workersByVm.get(id) || []) w.terminate();
    workersByVm.delete(id);
    const p = ports.get(id); if (p) { p.sock.readyState = 3; ports.delete(id); }
    try { vm.term.dispose(); } catch {}
    E.vms.delete(id);
  };
  E.attach = (id, host) => {
    const vm = E.vms.get(id);
    const box = document.createElement('div'); box.className = 'xterm-host'; host.appendChild(box);
    if (!vm) {
      const d = dev(id);
      box.innerHTML = `<div style="color:#c9d1d9;padding:14px;font:12.5px/1.5 var(--f-mono)">${d.power ? 'The device is on in the simulation, but its guest is not running yet.' : 'Powered off.'}<br><br>${d.power && E.full() ? esc(FULL) : '<button class="btn" id="bootVm">Boot the x86_64 guest</button>'}</div>`;
      const boot = box.querySelector('#bootVm');
      if (boot) boot.onclick = () => { if (!d.power) { togglePower(id); } else { E.start(id).then(() => renderSide()); renderSide(); } };
      return;
    }
    if (!vm.opened) { vm.term.open(box); vm.opened = true; vm.el = vm.term.element; }
    else box.appendChild(vm.el);
    // fit the terminal to the inspector: the guest's output wraps instead of being cut
    requestAnimationFrame(() => {
      const cw = vm.term._core?._renderService?.dimensions?.css?.cell?.width || 7.3;
      const ch = vm.term._core?._renderService?.dimensions?.css?.cell?.height || 15;
      const cols = Math.max(40, Math.floor((box.clientWidth - 12) / cw)), rows = Math.max(10, Math.floor((box.clientHeight - 8) / ch));
      if (cols !== vm.term.cols || rows !== vm.term.rows) vm.term.resize(cols, rows);
    });
    setTimeout(() => vm.term.focus(), 30);
  };
  return E;
})();
