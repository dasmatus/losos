// ── Ready-made setups, each one a LosOS deployment from the wiki ─────────
const SCENARIOS = {
  'two-sites': {
    name: 'Two sites, one official edge',
    blurb: 'A home box on the official edge directly, and an office behind its own gateway that relays to the same hub.',
    build() {
      const inet = newDevice('internet', 640, 90, { px: 200, py: 110 });
      const hub = newDevice('edge-official', 900, 90, { name: 'edge', px: 40, py: 70 });
      const hr = newDevice('router', 330, 250, { name: 'home-router', px: 40, py: 50 });
      const ap = newDevice('ap', 180, 360, { name: 'home-wifi', px: 190, py: 40 });
      const box = newDevice('box', 340, 430, { name: 'mattbox', px: 60, py: 250 });
      const lap = newDevice('laptop', 140, 500, { name: 'matus-laptop', px: 260, py: 230 });
      const or = newDevice('router', 950, 250, { name: 'office-router', site: 'office', px: 40, py: 60 });
      const sw = newDevice('switch', 950, 370, { name: 'office-switch', site: 'office', px: 40, py: 120 });
      const gw = newDevice('edge-local', 760, 470, { name: 'acme-gw', site: 'office', px: 40, py: 180 });
      const ob = newDevice('box', 950, 500, { name: 'teambox', site: 'office', px: 260, py: 250 });
      const ob2 = newDevice('box', 1130, 470, { name: 'annabox', site: 'office', px: 400, py: 250 });
      connect(hr.id, inet.id, 'auto', 'wan'); connect(or.id, inet.id, 'auto', 'wan'); connect(hub.id, inet.id, 'fiber');
      connect(hr.id, box.id, 'auto', 'lan1'); connect(hr.id, ap.id, 'auto', 'lan2'); connect(ap.id, lap.id, 'wifi');
      connect(or.id, sw.id, 'auto', 'lan1'); connect(sw.id, gw.id); connect(sw.id, ob.id); connect(sw.id, ob2.id);
      return lap.id;
    },
  },
  home: {
    name: 'Home: one box',
    blurb: 'One box behind a home router, reached from the laptop on Wi-Fi and from outside through the official edge.',
    build() {
      const inet = newDevice('internet', 640, 100, { px: 200, py: 110 });
      const hub = newDevice('edge-official', 960, 100, { name: 'edge', px: 40, py: 70 });
      const hr = newDevice('router', 520, 270, { name: 'home-router', px: 40, py: 50 });
      const ap = newDevice('ap', 330, 380, { name: 'home-wifi', px: 190, py: 40 });
      const box = newDevice('box', 640, 430, { name: 'mattbox', px: 60, py: 250 });
      const lap = newDevice('laptop', 300, 520, { name: 'matus-laptop', px: 260, py: 230 });
      connect(hr.id, inet.id, 'auto', 'wan'); connect(hub.id, inet.id, 'fiber');
      connect(hr.id, box.id, 'auto', 'lan1'); connect(hr.id, ap.id, 'auto', 'lan2'); connect(ap.id, lap.id, 'wifi');
      return box.id;
    },
  },
  office: {
    name: 'Company with its own gateway',
    blurb: 'An office LAN: a gateway with open enrolment and its own mesh, three boxes, one uplink to the official edge.',
    build() {
      const inet = newDevice('internet', 640, 90, { px: 200, py: 110 });
      const hub = newDevice('edge-official', 960, 90, { name: 'edge', px: 40, py: 70 });
      const or = newDevice('router', 640, 230, { name: 'office-router', site: 'office', px: 40, py: 60 });
      const sw = newDevice('switch', 640, 350, { name: 'office-switch', site: 'office', px: 40, py: 120 });
      const gw = newDevice('edge-local', 360, 470, { name: 'acme-gw', site: 'office', px: 40, py: 180 });
      const b1 = newDevice('box', 560, 520, { name: 'teambox', site: 'office', px: 230, py: 250, cfg: { joinMesh: true } });
      const b2 = newDevice('box', 740, 520, { name: 'annabox', site: 'office', px: 360, py: 250, cfg: { joinMesh: true } });
      const b3 = newDevice('box', 920, 470, { name: 'peterbox', site: 'office', px: 490, py: 250 });
      const lap = newDevice('laptop', 360, 300, { name: 'matus-laptop', site: 'office', px: 230, py: 140 });
      connect(or.id, inet.id, 'auto', 'wan'); connect(hub.id, inet.id, 'fiber');
      connect(or.id, sw.id, 'auto', 'lan1'); connect(sw.id, gw.id); connect(sw.id, b1.id); connect(sw.id, b2.id); connect(sw.id, b3.id); connect(sw.id, lap.id);
      return gw.id;
    },
  },
  star: {
    name: 'Star: everything on one switch',
    blurb: 'Every device has its own cable to one central switch. One cable down takes one device off; the switch down takes everyone off.',
    build() {
      const inet = newDevice('internet', 640, 50, { px: 200, py: 110 });
      const hub = newDevice('edge-official', 960, 50, { name: 'edge', px: 40, py: 70 });
      const or = newDevice('router', 640, 170, { name: 'office-router', site: 'office', px: 40, py: 45 });
      const sw = newDevice('switch', 640, 360, { name: 'core-switch', site: 'office', px: 40, py: 105 });
      const gw = newDevice('edge-local', 380, 300, { name: 'acme-gw', site: 'office', px: 40, py: 160 });
      const b1 = newDevice('box', 400, 470, { name: 'teambox', site: 'office', px: 250, py: 250, cfg: { joinMesh: true } });
      const b2 = newDevice('box', 560, 540, { name: 'annabox', site: 'office', px: 370, py: 250, cfg: { joinMesh: true } });
      const b3 = newDevice('box', 740, 540, { name: 'peterbox', site: 'office', px: 490, py: 250 });
      const b4 = newDevice('box', 900, 470, { name: 'evabox', site: 'office', px: 470, py: 120 });
      const lap = newDevice('laptop', 900, 300, { name: 'matus-laptop', site: 'office', px: 300, py: 110 });
      connect(or.id, inet.id, 'auto', 'wan'); connect(hub.id, inet.id, 'fiber');
      connect(or.id, sw.id, 'auto', 'lan1');
      for (const d of [gw, b1, b2, b3, b4, lap]) connect(sw.id, d.id);
      return sw.id;
    },
  },
  bus: {
    name: 'Bus: one shared coax cable',
    blurb: 'All devices tap into one cable with a terminator at each end. Every frame reaches every tap, so every device sees every other device’s traffic.',
    build() {
      const inet = newDevice('internet', 640, 50, { px: 200, py: 110 });
      const hub = newDevice('edge-official', 960, 50, { name: 'edge', px: 40, py: 70 });
      const or = newDevice('router', 300, 190, { name: 'office-router', site: 'office', px: 40, py: 45 });
      const bus = newDevice('bus', 640, 360, { name: 'coax-bus', site: 'office', px: 250, py: 52 });
      const gw = newDevice('edge-local', 480, 200, { name: 'acme-gw', site: 'office', px: 40, py: 120 });
      const b1 = newDevice('box', 420, 500, { name: 'teambox', site: 'office', px: 250, py: 250, cfg: { joinMesh: true } });
      const b2 = newDevice('box', 640, 520, { name: 'annabox', site: 'office', px: 370, py: 250, cfg: { joinMesh: true } });
      const b3 = newDevice('box', 860, 500, { name: 'peterbox', site: 'office', px: 490, py: 250 });
      const lap = newDevice('laptop', 860, 200, { name: 'matus-laptop', site: 'office', px: 330, py: 140 });
      connect(or.id, inet.id, 'auto', 'wan'); connect(hub.id, inet.id, 'fiber');
      connect(or.id, bus.id, 'auto', 'lan1', 't1');
      connect(gw.id, bus.id, 'auto', 'eth0', 't2');
      connect(b1.id, bus.id, 'auto', 'eth0', 't4'); connect(b2.id, bus.id, 'auto', 'eth0', 't5'); connect(b3.id, bus.id, 'auto', 'eth0', 't6');
      connect(lap.id, bus.id, 'auto', 'eth0', 't7');
      return bus.id;
    },
  },
  web: {
    name: 'Web: four switches cabled to each other',
    blurb: 'Every switch has a cable to every other switch, so traffic has another way round when a cable is pulled. Real switches need spanning tree for this; the canvas takes the shortest live path.',
    build() {
      const inet = newDevice('internet', 640, 40, { px: 200, py: 110 });
      const hub = newDevice('edge-official', 960, 40, { name: 'edge', px: 40, py: 70 });
      const or = newDevice('router', 380, 120, { name: 'office-router', site: 'office', px: 40, py: 45 });
      const s1 = newDevice('switch', 460, 250, { name: 'sw-north', site: 'office', px: 40, py: 105 });
      const s2 = newDevice('switch', 820, 250, { name: 'sw-east', site: 'office', px: 262, py: 50 });
      const s3 = newDevice('switch', 460, 460, { name: 'sw-west', site: 'office', px: 40, py: 150 });
      const s4 = newDevice('switch', 820, 460, { name: 'sw-south', site: 'office', px: 262, py: 110 });
      const gw = newDevice('edge-local', 230, 330, { name: 'acme-gw', site: 'office', px: 218, py: 250 });
      const b1 = newDevice('box', 1050, 250, { name: 'teambox', site: 'office', px: 468, py: 250, cfg: { joinMesh: true } });
      const b2 = newDevice('box', 640, 580, { name: 'annabox', site: 'office', px: 343, py: 250, cfg: { joinMesh: true } });
      const lap = newDevice('laptop', 1050, 470, { name: 'matus-laptop', site: 'office', px: 455, py: 150 });
      connect(or.id, inet.id, 'auto', 'wan'); connect(hub.id, inet.id, 'fiber');
      connect(or.id, s1.id, 'auto', 'lan1');
      const S = [s1, s2, s3, s4];
      for (let i = 0; i < 4; i++) for (let j = i + 1; j < 4; j++) connect(S[i].id, S[j].id);
      connect(s1.id, gw.id); connect(s2.id, b1.id); connect(s3.id, b2.id); connect(s4.id, lap.id);
      return s1.id;
    },
  },
  lan: {
    name: 'LAN only, no internet',
    blurb: 'A switch, a gateway and two boxes with no router: link-local addresses, mDNS discovery and a local mesh. No public names, no market.',
    build() {
      const sw = newDevice('switch', 640, 260, { name: 'lab-switch', site: 'office', px: 40, py: 100 });
      const gw = newDevice('edge-local', 420, 420, { name: 'lab-gw', site: 'office', px: 40, py: 180, cfg: { uplink: false } });
      const b1 = newDevice('box', 640, 460, { name: 'box-a', site: 'office', px: 230, py: 250, cfg: { joinMesh: true } });
      const b2 = newDevice('box', 860, 420, { name: 'box-b', site: 'office', px: 360, py: 250, cfg: { joinMesh: true } });
      const lap = newDevice('laptop', 860, 230, { name: 'matus-laptop', site: 'office', px: 230, py: 140 });
      connect(sw.id, gw.id); connect(sw.id, b1.id); connect(sw.id, b2.id); connect(sw.id, lap.id);
      return b1.id;
    },
  },
  empty: { name: 'Empty canvas', blurb: 'Start from nothing: drag devices up from the tray.', build() { return null; } },
};
function loadScenario(key) { startWorld(key, () => SCENARIOS[key].build()); }
// A fresh world from `build`, which places the devices and returns the one
// to select; a built-in setup and an opened file both come through here.
function startWorld(key, build) {
  world = { devices: [], links: [], seq: 1 };
  TERMS.clear(); DESK.clear();
  if (typeof ENGINE !== 'undefined') ENGINE.stopAll();
  simReset();
  const focus = build();
  UI.scenario = key;
  evaluate(); PREV = null; trafficForChange();
  // power-on traffic for the whole scene, in boot order
  PREV = snapshot();
  const ends = world.devices.filter(d => TYPES[d.type].endpoint);
  ends.sort((a, b) => ['edge-official', 'edge-local', 'box', 'laptop'].indexOf(a.type) - ['edge-official', 'edge-local', 'box', 'laptop'].indexOf(b.type));
  for (const d of ends) startFlow(flowAddress(d.id), { title: 'address ' + d.name });
  for (const d of ends) if (d.type === 'edge-local') startFlow(flowUplink(d.id));
  for (const d of ends) if (d.type === 'box') startFlow(flowScan(d.id));
  UI.sel = focus ? { kind: 'dev', id: focus } : null;
  UI.tab = focus && dev(focus).type === 'laptop' ? 'desktop' : 'status';
  fitView();
  renderAll();
}
