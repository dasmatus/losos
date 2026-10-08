// ── Guests under libvirt, through losos-registrar lab ──────────────────
// The Lab's guests can run under the libvirt of the machine the browser is
// on (KVM when it has it) instead of qemu-wasm in the tab. The helper,
// `losos-registrar lab`, starts each guest as a transient libvirt domain
// and bridges its serial console and network card to two WebSockets; the
// Lab stays the switch fabric, so a libvirt guest's frames cross the drawn
// cables exactly like a qemu-wasm guest's.
//
// Where the helper is:
//   box copy     /api/lab/ on this origin (lososd relays it, with the admin
//                token this tab holds); sockets at /api/lab/ws/.
//   hosted copy  http://127.0.0.1:8095/lab/v1/ on the viewer's own machine.
//                Asked only when the page itself is on loopback, or the
//                viewer opted in with ?libvirt (remembered; ?libvirt=0
//                forgets): Chrome asks every visitor of a public page for
//                local network access the moment it touches 127.0.0.1.
//
// Interface, kept narrow so the React Lab's engine can take it over:
//   LIBVIRT.probe()                    → {available, label, reason, images, maxGuests}
//   LIBVIRT.start({id,name,role}, cmdline, macs)
//                                      → {label, console: {onData(cb), send(text|bytes)},
//                                         nics: [{onFrame(cb), send(frame)}], stop()}
//   LIBVIRT.stop()                     → stops every guest this page started
const LIBVIRT = (() => {
  const L = { hello: null };
  const HOSTED = 'http://127.0.0.1:8095/lab/v1/';
  const OPT_IN = 'losos-lab-libvirt';
  const handles = new Set();

  function token() {
    try { return sessionStorage.getItem('losos-token'); } catch { return null; }
  }
  function where() {
    if (LAB.box) {
      return { api: '/api/lab/', ws: (location.protocol === 'https:' ? 'wss://' : 'ws://') + location.host + '/api/lab/ws/', auth: token() };
    }
    return { api: HOSTED, ws: HOSTED.replace(/^http/, 'ws') + 'guests/', auth: null };
  }
  function optedIn() {
    if (LAB.box) return true;
    if (/^(localhost|127\.\d+\.\d+\.\d+|\[::1\])$/.test(location.hostname)) return true;
    const q = new URLSearchParams(location.search).get('libvirt');
    try {
      if (q === '0') localStorage.removeItem(OPT_IN);
      else if (q !== null) localStorage.setItem(OPT_IN, '1');
      return localStorage.getItem(OPT_IN) === '1';
    } catch { return q !== null && q !== '0'; }
  }
  async function call(path, opts = {}, ms = 1500) {
    const w = where();
    const ctl = new AbortController();
    const timer = setTimeout(() => ctl.abort(), ms);
    const headers = Object.assign({}, opts.headers || {});
    if (w.auth) headers.Authorization = 'Bearer ' + w.auth;
    try {
      return await fetch(w.api + path, Object.assign({}, opts, { headers, signal: ctl.signal, cache: 'no-store' }));
    } finally { clearTimeout(timer); }
  }

  L.probe = async () => {
    const no = (reason) => ({ available: false, label: '', reason });
    if (!optedIn()) return no('not asked: open the lab with ?libvirt to look for losos-registrar lab on this computer');
    if (LAB.box && !where().auth) return no('sign in on the admin page first');
    try {
      const r = await call('hello');
      if (!r.ok) return no('the helper answered ' + r.status);
      const h = await r.json();
      if (!h.available) return no(h.reason === 'off' ? 'this box runs no libvirt helper (losos.lab.libvirt.enable)' : h.reason === 'notRunning' ? 'the libvirt helper is not running' : (h.reason || 'not available'));
      L.hello = h;
      return { available: true, label: h.label || 'libvirt', reason: '', images: h.images || {}, maxGuests: h.maxGuests || 0 };
    } catch (e) {
      return no(LAB.box ? 'the box did not answer' : 'no losos-registrar lab on 127.0.0.1:8095');
    }
  };

  function socket(url, ticket) {
    const s = new WebSocket(url, ['losos-lab', 'ticket.' + ticket]);
    s.binaryType = 'arraybuffer';
    return s;
  }
  function stream(s) {
    const subs = [];
    const early = [];
    s.onmessage = (e) => { const b = new Uint8Array(e.data); if (subs.length) subs.forEach(f => f(b)); else early.push(b); };
    return {
      on(cb) { subs.push(cb); while (early.length) cb(early.shift()); },
      send(data) { if (s.readyState === 1) s.send(typeof data === 'string' ? new TextEncoder().encode(data) : data); },
    };
  }

  L.start = async (device, cmdline, macs) => {
    const r = await call('guests', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ id: device.id, name: device.name, role: device.role, cmdline, macs }),
    }, 45000);
    let body = null;
    try { body = await r.json(); } catch { /* not JSON */ }
    if (!r.ok) throw new Error((body && body.error) || 'the helper answered ' + r.status);
    const w = where();
    const con = socket(w.ws + encodeURIComponent(body.guest) + '/console', body.ticket);
    const nicSockets = macs.map((_, i) => socket(w.ws + encodeURIComponent(body.guest) + '/nic/' + i, body.ticket));
    const conStream = stream(con);
    const handle = {
      label: body.label || (L.hello && L.hello.label) || 'libvirt',
      key: body.guest,
      console: { onData: conStream.on, send: conStream.send },
      nics: nicSockets.map(s => { const st = stream(s); return { onFrame: st.on, send: st.send }; }),
      onClose: null,
      stop() {
        if (!handles.delete(handle)) return;
        for (const s of [con, ...nicSockets]) { s.onclose = null; try { s.close(); } catch { /* closed */ } }
        call('guests/' + encodeURIComponent(body.guest), { method: 'DELETE', keepalive: true }, 10000).catch(() => {});
      },
    };
    con.onclose = () => { if (handles.has(handle) && handle.onClose) handle.onClose(); };
    handles.add(handle);
    return handle;
  };

  L.stop = () => { for (const h of [...handles]) h.stop(); };
  window.addEventListener('pagehide', () => L.stop());
  return L;
})();
