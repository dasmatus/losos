'use strict';

/* losos settings — macOS System Settings-flavored SPA.
 *
 * Flow:
 *   1. On load, fetch GET /api/settings with the Bearer token stored in
 *      sessionStorage ('losos-token'). On 401 (or no token) show the unlock
 *      card; a successful retry stores the token and continues.
 *   2. The form is populated from the settingsResponse keys
 *      {sharingMyStorage, nextcloudMode, forgejoMode, hostName, https,
 *      gpuEnable, apachePort, cfdEnable}. "Apply changes" stays disabled until dirty.
 *   3. Apply builds the overrides.nix body in JS (one `losos.<key> = <value>;`
 *      per line — booleans/ints bare, strings quoted) and POSTs it raw
 *      (text/plain) to /api/apply. This is the ONLY write path; /api/change
 *      is never called from the UI.
 *   4. The {"job": ...} ack starts a 2s poll of /api/status driving the
 *      progress block ("Applying…" spinner + live log line, done / failed).
 *   5. Factory reset: confirm() then POST /api/factory-reset, same poll flow.
 */

const POLL_MS = 2000;
const DEFAULT_HOSTNAME = 'mattbox';
const DEFAULT_APACHE_PORT = 11000;

function $(id) { return document.getElementById(id); }

const els = {
  applyBtn: $('apply-btn'),
  sectionTitle: $('section-title'),
  error: $('error-banner'),
  errorText: $('error-text'),

  progress: $('progress'),
  progressTitle: $('progress-title'),
  progressLog: $('progress-log'),

  sharing: $('in-sharing'),
  nextcloudMode: $('in-nextcloud-mode'),
  forgejoMode: $('in-forgejo-mode'),
  hostName: $('in-hostname'),
  https: $('in-https'),
  cfd: $('in-cfd'),
  gpu: $('in-gpu'),
  apachePort: $('in-apache-port'),
  factoryResetBtn: $('factory-reset-btn'),

  authOverlay: $('auth-overlay'),
  authForm: $('auth-form'),
  authToken: $('auth-token'),
  authError: $('auth-error'),
};

// ── Token ─────────────────────────────────────────────────────────

function getToken() { return sessionStorage.getItem('losos-token'); }
function saveToken(t) { sessionStorage.setItem('losos-token', t); }
function dropToken() { sessionStorage.removeItem('losos-token'); }

function authHeaders() {
  const t = getToken();
  return t ? { 'Authorization': 'Bearer ' + t } : {};
}

// ── Banners ───────────────────────────────────────────────────────

function showError(msg) {
  els.errorText.textContent = msg;
  els.error.hidden = false;
}
function clearError() {
  els.error.hidden = true;
  els.errorText.textContent = '';
}
$('error-close').addEventListener('click', clearError);

// ── Form state ────────────────────────────────────────────────────

let original = null;   // last settings known to the daemon
let applying = false;  // a rebuild cycle is in flight
let pollTimer = null;

function readForm() {
  const port = parseInt(els.apachePort.value, 10);
  return {
    sharingMyStorage: els.sharing.checked,
    nextcloudMode: els.nextcloudMode.value === 'native' ? 'native' : 'container',
    forgejoMode: els.forgejoMode.value === 'native' ? 'native' : 'container',
    hostName: els.hostName.value.trim(),
    https: els.https.checked,
    gpuEnable: els.gpu.checked,
    apachePort: Number.isInteger(port) ? port : NaN,
    cfdEnable: els.cfd.checked,
  };
}

function populate(s) {
  els.sharing.checked = !!s.sharingMyStorage;
  els.nextcloudMode.value = s.nextcloudMode === 'native' ? 'native' : 'container';
  els.forgejoMode.value = s.forgejoMode === 'native' ? 'native' : 'container';
  els.hostName.value = typeof s.hostName === 'string' ? s.hostName : DEFAULT_HOSTNAME;
  els.https.checked = !!s.https;
  els.gpu.checked = !!s.gpuEnable;
  els.apachePort.value = Number.isInteger(s.apachePort) ? s.apachePort : DEFAULT_APACHE_PORT;
  els.cfd.checked = !!s.cfdEnable;
  original = readForm();
  updateDirty();
}

function validPort(p) { return Number.isInteger(p) && p >= 1024 && p <= 65535; }

function sameSettings(a, b) {
  return a.sharingMyStorage === b.sharingMyStorage &&
    a.nextcloudMode === b.nextcloudMode &&
    a.forgejoMode === b.forgejoMode &&
    a.hostName === b.hostName &&
    a.https === b.https &&
    a.gpuEnable === b.gpuEnable &&
    ((Number.isNaN(a.apachePort) && Number.isNaN(b.apachePort)) || a.apachePort === b.apachePort) &&
    a.cfdEnable === b.cfdEnable;
}

function updateDirty() {
  const v = readForm();
  const dirty = original !== null && !sameSettings(original, v);
  const valid = validPort(v.apachePort) && v.hostName.length > 0;
  els.applyBtn.disabled = !dirty || !valid || applying;
}

/* Build the full modules/overrides.nix body the daemon writes. Format
 * matches the backend's line-based parser and defaultOverridesNix exactly:
 * `{ ... }:` header, one `losos.<key> = <value>;` per line, booleans/ints
 * bare, strings double-quoted with escaping. */
function nixString(s) {
  return '"' + String(s).replace(/\\/g, '\\\\').replace(/"/g, '\\"') + '"';
}

function generateNix() {
  const v = readForm();
  const port = validPort(v.apachePort) ? v.apachePort : DEFAULT_APACHE_PORT;
  return '{ ... }:\n' +
    '{\n' +
    '  losos.sharingMyStorage = ' + (v.sharingMyStorage ? 'true' : 'false') + ';\n' +
    '  losos.nextcloud.mode = ' + nixString(v.nextcloudMode) + ';\n' +
    '  losos.forgejo.mode = ' + nixString(v.forgejoMode) + ';\n' +
    '  losos.hostName = ' + nixString(v.hostName || DEFAULT_HOSTNAME) + ';\n' +
    '  losos.nextcloud.https = ' + (v.https ? 'true' : 'false') + ';\n' +
    '  losos.gpu.enable = ' + (v.gpuEnable ? 'true' : 'false') + ';\n' +
    '  losos.nextcloud.apachePort = ' + port + ';\n' +
    '  losos.cfd.enable = ' + (v.cfdEnable ? 'true' : 'false') + ';\n' +
    '}\n';
}

// ── Network: settings ─────────────────────────────────────────────

/* Returns {settings} on success, {unauthorized:true} on 401; throws on any
 * other failure. A successful call with a candidate token saves it. */
async function fetchSettings(candidate) {
  const headers = candidate ? { 'Authorization': 'Bearer ' + candidate } : authHeaders();
  const res = await fetch('/api/settings', { headers: headers, cache: 'no-store' });
  if (res.status === 401) { return { unauthorized: true }; }
  if (!res.ok) {
    const data = await res.json().catch(function () { return {}; });
    throw new Error(data.error || ('HTTP ' + res.status));
  }
  const data = await res.json();
  if (candidate) { saveToken(candidate); }
  return { settings: data };
}

// ── Auth overlay ──────────────────────────────────────────────────

function showAuth(msg) {
  els.authOverlay.hidden = false;
  els.authError.textContent = msg || '';
  els.authError.hidden = !msg;
  els.authToken.value = '';
  els.authToken.focus();
}
function hideAuth() { els.authOverlay.hidden = true; }

els.authForm.addEventListener('submit', async function (ev) {
  ev.preventDefault();
  const candidate = els.authToken.value.trim();
  els.authError.hidden = true;
  if (!candidate) {
    els.authError.textContent = 'Enter the admin token.';
    els.authError.hidden = false;
    return;
  }
  try {
    const r = await fetchSettings(candidate);
    if (r.unauthorized) {
      els.authError.textContent = 'Invalid token.';
      els.authError.hidden = false;
      return;
    }
    populate(r.settings);
    hideAuth();
    clearError();
  } catch (e) {
    els.authError.textContent = 'Could not reach the appliance: ' + e.message;
    els.authError.hidden = false;
  }
});

// Session expired mid-use: drop the token and re-lock the page.
function sessionExpired() {
  dropToken();
  stopPolling();
  applying = false;
  els.factoryResetBtn.disabled = false;
  updateDirty();
  hideProgress();
  showAuth('Session expired — paste the admin token again.');
}

// ── Progress block ────────────────────────────────────────────────

function showProgress(kind, title, msg) {
  els.progress.hidden = false;
  els.progress.dataset.kind = kind;
  els.progressTitle.textContent = title;
  els.progressLog.textContent = msg || '';
}
function hideProgress() { els.progress.hidden = true; }

// ── Status polling ────────────────────────────────────────────────

function startPolling() {
  stopPolling();
  pollTimer = setInterval(pollOnce, POLL_MS);
  pollOnce();
}
function stopPolling() {
  if (pollTimer !== null) { clearInterval(pollTimer); pollTimer = null; }
}

async function pollOnce() {
  try {
    const res = await fetch('/api/status', { headers: authHeaders(), cache: 'no-store' });
    if (res.status === 401) { sessionExpired(); return; }
    if (!res.ok) { return; } // transient: keep polling
    const data = await res.json();
    const state = (data && data.state) || 'idle';
    const message = (data && data.message) || '';

    if (state === 'building') {
      showProgress('building', 'Applying…', message || 'Rebuild in progress.');
    } else if (state === 'done') {
      stopPolling();
      showProgress('done', 'Rebuild complete.', message);
      applying = false;
      els.factoryResetBtn.disabled = false;
      await reloadSettings();
    } else if (state === 'failed') {
      stopPolling();
      showProgress('failed', 'Rebuild failed.', message || 'See the lososd logs for details.');
      applying = false;
      els.factoryResetBtn.disabled = false;
      updateDirty();
    }
  } catch (e) {
    // transient network error: keep polling
  }
}

async function reloadSettings() {
  try {
    const r = await fetchSettings();
    if (r.unauthorized) { sessionExpired(); return; }
    populate(r.settings);
  } catch (e) {
    updateDirty();
    showError('Rebuild finished, but reloading settings failed: ' + e.message);
  }
}

// ── Apply / factory reset ─────────────────────────────────────────

async function runRebuild(trigger) {
  clearError();
  applying = true;
  updateDirty();
  els.factoryResetBtn.disabled = true;
  showProgress('building', 'Applying…', 'Starting rebuild…');
  try {
    const res = await trigger();
    if (res.status === 401) { sessionExpired(); return; }
    if (!res.ok) {
      const data = await res.json().catch(function () { return {}; });
      throw new Error(data.error || ('HTTP ' + res.status));
    }
    await res.json().catch(function () { return {}; }); // {"job": ...}
    startPolling();
  } catch (e) {
    applying = false;
    updateDirty();
    els.factoryResetBtn.disabled = false;
    showProgress('failed', 'Rebuild failed to start.', e.message);
    showError(e.message);
  }
}

els.applyBtn.addEventListener('click', function () {
  runRebuild(function () {
    return fetch('/api/apply', {
      method: 'POST',
      headers: Object.assign(authHeaders(), { 'Content-Type': 'text/plain' }),
      body: generateNix(),
    });
  }).catch(function (e) { showError('Apply failed unexpectedly: ' + e.message); });
});

els.factoryResetBtn.addEventListener('click', function () {
  if (!window.confirm('Reset all appliance settings to factory defaults and rebuild?')) { return; }
  runRebuild(function () {
    return fetch('/api/factory-reset', {
      method: 'POST',
      headers: authHeaders(),
    });
  }).catch(function (e) { showError('Factory reset failed unexpectedly: ' + e.message); });
});

// ── Sidebar section switching ─────────────────────────────────────

document.querySelectorAll('.side-item').forEach(function (item) {
  item.addEventListener('click', function (ev) {
    ev.preventDefault();
    document.querySelectorAll('.side-item').forEach(function (i) { i.classList.remove('is-active'); });
    item.classList.add('is-active');
    const id = item.dataset.section;
    document.querySelectorAll('.section').forEach(function (s) {
      s.classList.toggle('is-active', s.id === 'section-' + id);
    });
    els.sectionTitle.textContent = item.textContent.trim();
  });
});

// ── Wiring ────────────────────────────────────────────────────────

(function init() {
  const navTahoe = $('nav-tahoe');
  if (navTahoe) {
    navTahoe.href = location.protocol + '//' + location.hostname + ':3456/';
  }

  document.querySelectorAll('.content input, .content select').forEach(function (el) {
    el.addEventListener('input', updateDirty);
    el.addEventListener('change', updateDirty);
  });

  (async function () {
    if (!getToken()) {
      showAuth('');
      return;
    }
    try {
      const r = await fetchSettings();
      if (r.unauthorized) {
        dropToken();
        showAuth('Saved token was rejected — paste the current admin token.');
        return;
      }
      populate(r.settings);
      hideAuth();
    } catch (e) {
      showError('Failed to load settings: ' + e.message);
    }
  })().catch(function (e) {
    showError('Failed to load settings: ' + e.message);
  });
})();
