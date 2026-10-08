// Device icons for the logical view and the tray (viewBox 0 0 64 56), and the
// hardware drawings for the physical view. Network gear uses the generic
// shapes every topology diagram uses (cylinder router, flat switch, cloud);
// the LosOS devices get their own: a mini-PC with the salmon stripe.
const ICON = {
  box: `<g>
    <rect x="9" y="16" width="46" height="28" rx="5" fill="var(--surface)" stroke="var(--ink)" stroke-width="2"/>
    <rect x="9" y="16" width="46" height="6" rx="3" fill="var(--salmon)"/>
    <circle cx="17" cy="35" r="2.6" fill="var(--ok)"/>
    <rect x="24" y="33" width="24" height="3" rx="1.5" fill="var(--faint)"/>
    <path d="M14 44v4M50 44v4" stroke="var(--ink)" stroke-width="2" stroke-linecap="round"/></g>`,
  'edge-local': `<g>
    <rect x="9" y="16" width="46" height="28" rx="5" fill="var(--surface)" stroke="var(--ink)" stroke-width="2"/>
    <rect x="9" y="16" width="46" height="6" rx="3" fill="var(--accent)"/>
    <circle cx="17" cy="35" r="2.6" fill="var(--ok)"/>
    <path d="M27 36h18m-5-5 5 5-5 5" fill="none" stroke="var(--accent)" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round"/>
    <path d="M32 8a12 12 0 0 1 0 8M28 6a16 16 0 0 1 0 12" fill="none" stroke="var(--accent)" stroke-width="1.8" stroke-linecap="round" transform="rotate(-90 32 12)"/></g>`,
  'edge-official': `<g>
    <rect x="6" y="18" width="52" height="11" rx="2.5" fill="var(--surface)" stroke="var(--ink)" stroke-width="2"/>
    <rect x="6" y="31" width="52" height="11" rx="2.5" fill="var(--surface)" stroke="var(--ink)" stroke-width="2"/>
    <circle cx="12" cy="23.5" r="1.8" fill="var(--ok)"/><circle cx="12" cy="36.5" r="1.8" fill="var(--ok)"/>
    <path d="M18 23.5h20M18 36.5h20" stroke="var(--faint)" stroke-width="2" stroke-linecap="round"/>
    <path d="M47 9l8 3v6c0 5-3.5 8-8 9.5-4.5-1.5-8-4.5-8-9.5v-6z" fill="var(--accent)" stroke="var(--surface)" stroke-width="1.5"/>
    <path d="M43.5 17.5l2.5 2.5 4.5-5" fill="none" stroke="var(--surface)" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/></g>`,
  laptop: `<g>
    <rect x="13" y="11" width="38" height="26" rx="3" fill="#0f1726" stroke="var(--ink)" stroke-width="2"/>
    <rect x="17" y="15" width="12" height="8" rx="1.5" fill="none" stroke="#a3e635" stroke-width="1.4"/>
    <rect x="31" y="15" width="16" height="18" rx="1.5" fill="#1e2a3b"/>
    <rect x="17" y="25" width="12" height="8" rx="1.5" fill="#1e2a3b"/>
    <path d="M6 41h52l-4 5H10z" fill="var(--surface)" stroke="var(--ink)" stroke-width="2" stroke-linejoin="round"/></g>`,
  router: `<g>
    <ellipse cx="32" cy="35" rx="23" ry="8" fill="var(--surface)" stroke="var(--ink)" stroke-width="2"/>
    <path d="M9 27v8M55 27v8" stroke="var(--ink)" stroke-width="2"/>
    <ellipse cx="32" cy="27" rx="23" ry="8" fill="var(--surface)" stroke="var(--ink)" stroke-width="2"/>
    <g stroke="var(--accent)" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round" fill="none">
      <path d="M24 24l-6 3 6 3M40 24l6 3-6 3M29 22l3-3 3 3M29 32l3 3 3-3"/></g></g>`,
  switch: `<g>
    <path d="M8 24l8-8h40l-8 8z" fill="var(--sunk)" stroke="var(--ink)" stroke-width="2" stroke-linejoin="round"/>
    <path d="M56 16v14l-8 8V24z" fill="var(--sunk)" stroke="var(--ink)" stroke-width="2" stroke-linejoin="round"/>
    <rect x="8" y="24" width="40" height="14" fill="var(--surface)" stroke="var(--ink)" stroke-width="2"/>
    <g stroke="var(--accent)" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" fill="none">
      <path d="M14 28h12l-3-2.5M26 34H14l3 2.5M30 28h12l-3-2.5M42 34H30l3 2.5"/></g></g>`,
  ap: `<g>
    <ellipse cx="32" cy="38" rx="20" ry="7" fill="var(--surface)" stroke="var(--ink)" stroke-width="2"/>
    <circle cx="32" cy="38" r="2.2" fill="var(--accent)"/>
    <g fill="none" stroke="var(--accent)" stroke-width="2.2" stroke-linecap="round">
      <path d="M24 24a11 11 0 0 1 16 0M19 18a18 18 0 0 1 26 0M28.5 29a5 5 0 0 1 7 0"/></g></g>`,
  bus: `<g>
    <path d="M6 30h52" stroke="var(--ink)" stroke-width="4" stroke-linecap="round"/>
    <path d="M6 30h52" stroke="var(--surface)" stroke-width="1.6" stroke-dasharray="3 3"/>
    <rect x="2" y="24" width="6" height="12" rx="1.5" fill="var(--ink)"/><rect x="56" y="24" width="6" height="12" rx="1.5" fill="var(--ink)"/>
    <g stroke="var(--accent)" stroke-width="2.4" stroke-linecap="round"><path d="M18 30v-12M32 30v12M46 30v-12"/></g>
    <g fill="var(--accent)"><circle cx="18" cy="16" r="3"/><circle cx="32" cy="44" r="3"/><circle cx="46" cy="16" r="3"/></g></g>`,
  internet: `<g>
    <path d="M17 44h31a10 10 0 0 0 1-20 14 14 0 0 0-27-3 10 10 0 0 0-5 23z" fill="var(--surface)" stroke="var(--ink)" stroke-width="2" stroke-linejoin="round"/>
    <g fill="none" stroke="var(--wan)" stroke-width="1.6"><ellipse cx="33" cy="33" rx="9" ry="9"/><path d="M24 33h18M33 24c-4 5-4 13 0 18M33 24c4 5 4 13 0 18"/></g></g>`,
};

function iconSvg(type, size = 48) {
  return `<svg viewBox="0 0 64 56" width="${size}" height="${Math.round(size * 56 / 64)}" aria-hidden="true">${ICON[type] || ''}</svg>`;
}

// ── Hardware drawings for the physical view. Each returns {svg, w, h, ports}
// where ports maps port id → [x, y] relative to the drawing's top-left.
function hwDrawing(dev, linkState) {
  const T = dev.type;
  const led = (id) => {
    const s = linkState(id);
    return s === 'up' ? '#2fa36b' : s === 'wait' ? '#e0a526' : '#3b4650';
  };
  const pwr = dev.power ? '#2fa36b' : '#3b4650';
  if (T === 'box' || T === 'edge-local') {
    const stripe = T === 'box' ? 'var(--salmon)' : 'var(--accent)';
    return { w: 92, h: 50, ports: { eth0: [80, 40] }, svg: `
      <rect x="0" y="6" width="92" height="40" rx="7" fill="#cfd6dc" stroke="#7d8a96"/>
      <rect x="0" y="6" width="92" height="7" rx="3.5" fill="${stripe}"/>
      <rect x="6" y="18" width="40" height="22" rx="3" fill="#b9c2ca"/>
      <circle class="pwr" data-power="${dev.id}" cx="14" cy="29" r="4.2" fill="${pwr}" stroke="#4b5661" style="cursor:pointer"/>
      <rect x="52" y="24" width="14" height="9" rx="1.5" fill="#39434d"/>
      <rect x="74" y="34" width="12" height="9" rx="1.5" fill="#39434d"/>
      <circle cx="77" cy="32" r="1.6" fill="${led('eth0')}"/><circle cx="83" cy="32" r="1.6" fill="${led('eth0')}"/>
      <rect x="8" y="46" width="10" height="3" fill="#7d8a96"/><rect x="74" y="46" width="10" height="3" fill="#7d8a96"/>` };
  }
  if (T === 'edge-official') {
    return { w: 132, h: 30, ports: { eth0: [118, 15] }, svg: `
      <rect x="0" y="2" width="132" height="26" rx="2" fill="#3a444e" stroke="#20272d"/>
      <rect x="6" y="7" width="70" height="16" rx="1.5" fill="#2a323a"/>
      ${[0,1,2,3,4,5].map(i => `<rect x="${9 + i * 11}" y="10" width="8" height="10" rx="1" fill="#4a5662"/>`).join('')}
      <circle class="pwr" data-power="${dev.id}" cx="88" cy="15" r="3.6" fill="${pwr}" style="cursor:pointer"/>
      <rect x="111" y="10" width="14" height="10" rx="1.5" fill="#15191d"/>
      <circle cx="114" cy="8" r="1.4" fill="${led('eth0')}"/>
      <path d="M98 9l5 1.8v3.6c0 3-2.2 5-5 6-2.8-1-5-3-5-6v-3.6z" fill="#48b3c0"/>` };
  }
  if (T === 'laptop') {
    return { w: 96, h: 66, ports: { eth0: [6, 52], wlan0: [90, 52] }, svg: `
      <rect x="10" y="0" width="76" height="48" rx="4" fill="#1d242b" stroke="#0b0f13"/>
      <rect x="14" y="4" width="68" height="40" rx="2" fill="${dev.power ? '#0f1726' : '#161b20'}"/>
      ${dev.power ? `<rect x="18" y="8" width="20" height="12" rx="2" fill="none" stroke="#a3e635"/><rect x="41" y="8" width="37" height="32" rx="2" fill="#1e2a3b"/><rect x="18" y="23" width="20" height="17" rx="2" fill="#1e2a3b"/>` : ''}
      <path d="M0 50h96l-6 10H6z" fill="#c4ccd3" stroke="#7d8a96"/>
      <rect x="2" y="50" width="9" height="6" rx="1" fill="#39434d"/>
      <circle cx="48" cy="55" r="3" class="pwr" data-power="${dev.id}" fill="${pwr}" style="cursor:pointer"/>
      <circle cx="90" cy="55" r="1.8" fill="${led('wlan0')}"/>` };
  }
  if (T === 'router') {
    const lanX = [44, 56, 68, 80];
    const ports = { wan: [28, 36] };
    lanX.forEach((x, i) => (ports['lan' + (i + 1)] = [x, 36]));
    return { w: 100, h: 46, ports, svg: `
      <path d="M14 0v10M86 0v10" stroke="#5a6672" stroke-width="3" stroke-linecap="round"/>
      <rect x="0" y="10" width="100" height="34" rx="6" fill="#e9edf0" stroke="#7d8a96"/>
      <circle class="pwr" data-power="${dev.id}" cx="10" cy="22" r="3.4" fill="${pwr}" style="cursor:pointer"/>
      <rect x="22" y="31" width="12" height="9" rx="1.5" fill="#6b4fa0"/>
      ${lanX.map(x => `<rect x="${x - 5}" y="31" width="10" height="9" rx="1.5" fill="#39434d"/>`).join('')}
      <circle cx="28" cy="20" r="1.8" fill="${led('wan')}"/>
      ${lanX.map((x, i) => `<circle cx="${x}" cy="20" r="1.8" fill="${led('lan' + (i + 1))}"/>`).join('')}` };
  }
  if (T === 'switch') {
    const ports = {};
    const xs = [];
    for (let i = 0; i < 8; i++) { const x = 34 + i * 12; xs.push(x); ports['p' + (i + 1)] = [x, 17]; }
    return { w: 140, h: 30, ports, svg: `
      <rect x="0" y="2" width="140" height="26" rx="2.5" fill="#2f3a44" stroke="#1b2228"/>
      <text x="8" y="19" font-size="8" fill="#9fb0bf" font-family="var(--f-mono)">SW</text>
      <circle class="pwr" data-power="${dev.id}" cx="24" cy="15" r="2.6" fill="${pwr}" style="cursor:pointer"/>
      ${xs.map((x, i) => `<rect x="${x - 4.5}" y="12" width="9" height="9" rx="1" fill="#11161a"/><circle cx="${x}" cy="8" r="1.4" fill="${led('p' + (i + 1))}"/>`).join('')}` };
  }
  if (T === 'bus') {
    // a length of thin coax along the wall: a T-tap per port, a terminator at each end
    const ports = {};
    const xs = [];
    for (let i = 0; i < 8; i++) { const x = 22 + i * 26; xs.push(x); ports['t' + (i + 1)] = [x, 10]; }
    return { w: 228, h: 26, ports, svg: `
      <rect x="0" y="12" width="10" height="10" rx="2" fill="#2f3a44" class="pwr" data-power="${dev.id}" style="cursor:pointer"/>
      <rect x="218" y="12" width="10" height="10" rx="2" fill="#2f3a44"/>
      <path d="M10 17h208" stroke="#1b2228" stroke-width="4"/>
      <path d="M10 17h208" stroke="${dev.power ? '#c58b3a' : '#5a6672'}" stroke-width="1.4"/>
      ${xs.map((x, i) => `<path d="M${x} 17v-6" stroke="#2f3a44" stroke-width="4"/><circle cx="${x}" cy="10" r="3" fill="#2f3a44"/><circle cx="${x}" cy="22.5" r="1.4" fill="${led('t' + (i + 1))}"/>`).join('')}` };
  }
  if (T === 'ap') {
    return { w: 60, h: 40, ports: { eth0: [30, 36] }, svg: `
      <ellipse cx="30" cy="22" rx="28" ry="13" fill="#eef1f3" stroke="#7d8a96"/>
      <circle cx="30" cy="22" r="3.4" fill="${dev.power ? '#48b3c0' : '#3b4650'}" class="pwr" data-power="${dev.id}" style="cursor:pointer"/>
      <rect x="25" y="33" width="10" height="6" rx="1" fill="#39434d"/>` };
  }
  if (T === 'internet') {
    const ports = {};
    for (let i = 0; i < 8; i++) ports['wan' + (i + 1)] = [14 + i * 12, 58];
    return { w: 120, h: 64, ports, svg: `
      <path d="M26 54h70a16 16 0 0 0 2-32 24 24 0 0 0-46-6 18 18 0 0 0-26 38z" fill="var(--surface)" stroke="var(--wan)" stroke-width="2"/>
      <text x="60" y="40" text-anchor="middle" font-size="11" fill="var(--muted)">ISP / Internet</text>` };
  }
  return { w: 60, h: 40, ports: {}, svg: '' };
}
