'use strict';

/* losos dashboard — live service probes + rebuild status banner.
 *
 * Talks to the same-origin Nginx routes:
 *   /nextcloud, /forgejo/            (service probes)
 *   /api/health, /api/state, /api/status  (lososd admin API, Bearer-authed)
 * plus the Tahoe WUI on :3456 (cross-origin, probed with mode:no-cors).
 *
 * The admin token lives in sessionStorage ('losos-token'); it is entered on
 * the settings page. Without a token the dashboard still probes the public
 * services and shows a sign-in hint instead of rebuild progress.
 */

const PROBE_MS = 15000;
const STATUS_MS = 2000;
const DONE_HIDE_MS = 20000;

// ── Tiny helpers ──────────────────────────────────────────────────
// ($, getToken, dropToken, authHeaders, tahoeUrl live in common.js)

const errBanner = $('error-banner');
const errText = $('error-text');

function showError(msg) {
  errText.textContent = msg;
  errBanner.hidden = false;
}
$('error-close').addEventListener('click', function () { errBanner.hidden = true; });

// ── Status dots ───────────────────────────────────────────────────

const DOT_LABEL = { up: 'Up', down: 'Down', checking: 'Checking…' };

function setDot(name, state) {
  const dot = $('dot-' + name);
  const text = $('st-' + name);
  if (!dot || !text) { return; }
  dot.dataset.state = state;
  dot.setAttribute('aria-label', name + ' status: ' + state);
  text.textContent = DOT_LABEL[state] || state;
}

// ── Service probes ────────────────────────────────────────────────

// Same-origin probe: any HTTP response < 500 counts as up; network errors
// and 5xx count as down.
async function probeSameOrigin(url) {
  try {
    const res = await fetch(url, { cache: 'no-store' });
    return res.status < 500 ? 'up' : 'down';
  } catch (e) {
    return 'down';
  }
}

// lososd health must answer the JSON document {"ok": true}.
async function probeLososd() {
  try {
    const res = await fetch('/api/health', { cache: 'no-store' });
    if (!res.ok) { return 'down'; }
    const data = await res.json();
    return data && data.ok === true ? 'up' : 'down';
  } catch (e) {
    return 'down';
  }
}

// Tahoe WUI is cross-origin on :3456: a resolved (opaque) no-cors fetch
// means something answered; a rejected one means nothing did.
async function probeTahoe() {
  try {
    await fetch(tahoeUrl(), { mode: 'no-cors' });
    return 'up';
  } catch (e) {
    return 'down';
  }
}

async function runProbes() {
  try {
    const results = await Promise.all([
      probeSameOrigin('/nextcloud'),
      probeSameOrigin('/forgejo/'),
      probeLososd(),
      probeTahoe(),
    ]);
    setDot('nextcloud', results[0]);
    setDot('forgejo', results[1]);
    setDot('lososd', results[2]);
    setDot('tahoe', results[3]);
  } catch (e) {
    showError('Service probe failed unexpectedly: ' + e.message);
  }
}

// ── System card ───────────────────────────────────────────────────

function setSharingMode(label) { $('sys-mode').textContent = label; }

async function loadSystemInfo() {
  $('sys-hostname').textContent = location.hostname;
  if (!getToken()) { setSharingMode('—'); return; }
  try {
    const res = await fetch('/api/state', { headers: authHeaders(), cache: 'no-store' });
    if (res.status === 401) { dropToken(); setSharingMode('—'); showHint(); return; }
    if (!res.ok) { setSharingMode('—'); return; }
    const data = await res.json();
    if (data && data.mode === 'mesh') { setSharingMode('Mesh — contributing storage'); }
    else if (data && data.mode === 'local') { setSharingMode('Local — private'); }
    else { setSharingMode('—'); }
  } catch (e) {
    setSharingMode('—');
  }
}

// ── Rebuild status banner ─────────────────────────────────────────

const banner = $('rebuild-banner');
const bannerTitle = $('banner-title');
const bannerMsg = $('banner-msg');
const hint = $('signin-hint');

let statusTimer = null;
let hideTimer = null;
let lastBannerState = null;

function showHint() { hint.hidden = false; }
function hideHint() { hint.hidden = true; }

function showBanner(kind, title, msg) {
  banner.hidden = false;
  banner.dataset.kind = kind;
  bannerTitle.textContent = title;
  bannerMsg.textContent = msg || '';
}
function hideBanner() {
  banner.hidden = true;
  bannerMsg.textContent = '';
}

function renderStatus(data) {
  const state = (data && data.state) || 'idle';
  const message = (data && data.message) || '';

  if (state === 'building') {
    clearTimeout(hideTimer);
    showBanner('building', 'Rebuilding system…', message || 'Rebuild in progress.');
  } else if (state === 'done') {
    if (lastBannerState !== 'done') {
      showBanner('done', 'Rebuild complete.', message);
      clearTimeout(hideTimer);
      hideTimer = setTimeout(hideBanner, DONE_HIDE_MS);
    }
  } else if (state === 'failed') {
    clearTimeout(hideTimer);
    showBanner('failed', 'Rebuild failed.', message || 'See the lososd logs for details.');
  } else {
    clearTimeout(hideTimer);
    hideBanner();
  }
  lastBannerState = state;
}

async function pollStatusOnce() {
  if (!getToken()) { stopStatusPolling(); showHint(); return; }
  try {
    const res = await fetch('/api/status', { headers: authHeaders(), cache: 'no-store' });
    if (res.status === 401) {
      dropToken();
      stopStatusPolling();
      hideBanner();
      showHint();
      setSharingMode('—');
      return;
    }
    if (!res.ok) { return; } // transient: keep polling
    const data = await res.json();
    renderStatus(data);
  } catch (e) {
    // transient network error: keep polling silently, dots reflect outages
  }
}

function startStatusPolling() {
  stopStatusPolling();
  statusTimer = setInterval(pollStatusOnce, STATUS_MS);
  pollStatusOnce();
}
function stopStatusPolling() {
  if (statusTimer !== null) { clearInterval(statusTimer); statusTimer = null; }
}

// ── Wiring ────────────────────────────────────────────────────────

(function init() {
  const tahoe = tahoeUrl();
  const navTahoe = $('nav-tahoe');
  const linkTahoe = $('link-tahoe');
  if (navTahoe) { navTahoe.href = tahoe; }
  if (linkTahoe) { linkTahoe.href = tahoe; }

  if (getToken()) {
    hideHint();
    startStatusPolling();
  } else {
    showHint();
  }

  runProbes();
  setInterval(function () {
    runProbes().catch(function (e) { showError('Service probe failed unexpectedly: ' + e.message); });
  }, PROBE_MS);

  loadSystemInfo().catch(function () { setSharingMode('—'); });
})();
