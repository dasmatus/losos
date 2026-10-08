// ── "This box": the setup drawn from the running box's own API ──────────
// Only in the copy the appliance serves at /lab/. It reads the same three
// routes the admin pages do, with the key the admin page keeps for this tab,
// and draws the box, its router, the edges it found and the laptop the
// owner is looking from. Nothing here writes to the box.
async function readThisBox() {
  let token = null;
  try { token = sessionStorage.getItem('losos-token'); } catch { /* storage blocked */ }
  if (!token) return { error: 'Sign in on the admin page first, then open the lab from its sidebar: the lab reads this box with the same key.' };
  const get = async (path) => {
    const r = await fetch(path, { headers: { Authorization: 'Bearer ' + token }, cache: 'no-store' });
    if (!r.ok) throw new Error(`${path} answered ${r.status}`);
    return r.json();
  };
  try {
    const [settings, edge] = await Promise.all([get('/api/settings'), get('/api/edge').catch(() => null)]);
    return { settings, edge };
  } catch (e) {
    return { error: 'The box did not answer: ' + e.message };
  }
}

function edgeHost(url) {
  try { return new URL(url).hostname; } catch { return ''; }
}

// A device name on the canvas: lower case, letters, digits and dashes.
function deviceName(text, fallback) {
  const name = String(text || '').toLowerCase().replace(/\.local$/, '').replace(/[^a-z0-9-]+/g, '-').replace(/^-+|-+$/g, '');
  return name || fallback;
}

function thisBoxScenario({ settings: s, edge }) {
  const edges = (edge && edge.edges) || [];
  const lan = edges.filter(e => e.source === 'lan');
  const configured = edges.find(e => e.source === 'configured')
    || (edge && edge.configuredUrl ? { url: edge.configuredUrl, official: false, name: edgeHost(edge.configuredUrl) } : null);
  const found = edges.length ? edges.map(e => e.name || edgeHost(e.url)).join(', ') : 'no edge in reach';
  return {
    name: `This box: ${s.hostName}`,
    blurb: `Drawn from ${s.hostName}'s own settings: tunnel ${s.proxyEnable ? 'on' : 'off'}, compute mesh ${s.clusterEnable ? 'joined' : 'off'}, `
      + `storage ${s.sharingMyStorage ? 'shared' : 'local'}. Edges it found: ${found}. The router, the cabling and the laptop are assumed.`,
    build() {
      const inet = newDevice('internet', 640, 100, { px: 200, py: 110 });
      const hr = newDevice('router', 520, 270, { name: 'router', px: 40, py: 50 });
      const box = newDevice('box', 640, 430, {
        name: deviceName(s.hostName, 'losos'), px: 60, py: 250,
        cfg: { proxy: !!s.proxyEnable, tenantOnHub: !!s.proxyEnable, joinMesh: !!s.clusterEnable, shareCompute: !!s.shareCompute },
      });
      const lap = newDevice('laptop', 330, 470, { name: 'this-laptop', px: 260, py: 230 });
      connect(hr.id, inet.id, 'auto', 'wan');
      connect(hr.id, box.id, 'auto', 'lan1');
      connect(hr.id, lap.id, 'auto', 'lan2');
      lan.slice(0, 2).forEach((e, i) => {
        const name = deviceName(e.name || edgeHost(e.url).split('.')[0], 'lan-edge');
        const gw = newDevice('edge-local', 860 + i * 160, 430, { name, px: 200, py: 120 + i * 70 });
        connect(hr.id, gw.id, 'auto', 'lan' + (3 + i));
      });
      if (configured) {
        const host = edgeHost(configured.url);
        const domain = host.replace(/^(register|edge)\./, '') || 'losos.cfd';
        const hub = newDevice('edge-official', 960, 100, {
          name: deviceName(host.split('.')[0], 'edge'), px: 40, py: 70,
          cfg: { certified: !!configured.official, domain },
        });
        connect(hub.id, inet.id, 'fiber');
      }
      return box.id;
    },
  };
}
