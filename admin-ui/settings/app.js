'use strict';

/* losos settings — macOS System Settings-flavored SPA.
 *
 * Flow:
 *   1. On load, fetch GET /api/settings with the Bearer token stored in
 *      sessionStorage ('losos-token'). On 401 (or no token) show the unlock
 *      card; a successful retry stores the token and continues. Then check
 *      /api/status, so a reload during a rebuild resumes it rather than
 *      coming back as an idle form.
 *   2. The form is populated from the settingsResponse keys
 *      {sharingMyStorage, nextcloudMode, forgejoMode, hostName, https,
 *      gpuEnable, apachePort, proxyEnable, clusterEnable, shareCompute,
 *      computeWindowStart, computeWindowEnd}. "Apply changes" stays disabled
 *      until the form is both dirty and valid; "Factory reset" stays
 *      disabled until the settings have actually loaded.
 *   3. Apply builds the overrides.nix body in JS (one `losos.<key> = <value>;`
 *      per line — booleans/ints bare, strings quoted) and POSTs it raw
 *      (text/plain) to /api/apply. This is the only write path; /api/change
 *      is never called from the UI.
 *   4. The {"job": ...} ack starts a 2s poll of /api/status driving the
 *      progress block ("Applying…" spinner + live log line, done / failed).
 *   5. Factory reset: confirm() then POST /api/factory-reset, same poll flow.
 *
 * The authed fetch, error banner, poll loop and modal focus handling are
 * shared with the dashboard and live in common.js.
 */

const POLL_MS = 2000;
const DEFAULT_HOSTNAME = 'mattbox';
const DEFAULT_APACHE_PORT = 11000;
const DEFAULT_WINDOW_START = '23:00';
const DEFAULT_WINDOW_END = '07:00';

/* The compute window on a 24-hour clock, the one shape every layer agrees
 * on: <input type="time"> emits exactly this, lososd's valid_hhmm() accepts
 * exactly this, and the mesh-join unit hands the pair to the edge as
 * written. Anchored, so "23:00 " or "1:2:3" is a rejection here rather than
 * a 400 from /api/apply after the rebuild button was already armed. */
const HHMM_RE = /^([01][0-9]|2[0-3]):[0-5][0-9]$/;

/* How long a freshly started rebuild may go without reporting `building`
 * before a terminal state is believed anyway. See isStaleTerminal(). */
const SETTLE_MS = 20000;

const els = {
  applyBtn: $('apply-btn'),
  sectionTitle: $('section-title'),

  progress: $('progress'),
  progressTitle: $('progress-title'),
  progressLog: $('progress-log'),

  sharing: $('in-sharing'),
  nextcloudMode: $('in-nextcloud-mode'),
  forgejoMode: $('in-forgejo-mode'),
  hostName: $('in-hostname'),
  hostNameError: $('hostname-error'),
  https: $('in-https'),
  proxy: $('in-proxy'),
  gpu: $('in-gpu'),
  apachePort: $('in-apache-port'),
  cluster: $('in-cluster'),
  shareCompute: $('in-share-compute'),
  windowStart: $('in-window-start'),
  windowEnd: $('in-window-end'),
  windowError: $('window-error'),
  factoryResetBtn: $('factory-reset-btn'),

  authOverlay: $('auth-overlay'),
  authForm: $('auth-form'),
  authToken: $('auth-token'),
  authError: $('auth-error'),
};

// ── Form state ────────────────────────────────────────────────────

let original = null;         // last settings known to the daemon
let settingsLoaded = false;  // the form holds real values, not placeholders
let applying = false;        // a rebuild cycle is in flight

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
    proxyEnable: els.proxy.checked,
    clusterEnable: els.cluster.checked,
    shareCompute: els.shareCompute.checked,
    computeWindowStart: els.windowStart.value,
    computeWindowEnd: els.windowEnd.value,
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
  els.proxy.checked = !!s.proxyEnable;
  els.cluster.checked = !!s.clusterEnable;
  els.shareCompute.checked = !!s.shareCompute;
  /* An <input type="time"> silently drops any value that is not HH:MM, so a
   * malformed one cannot be shown back the way a malformed hostname is: the
   * field would just read empty. Substitute the default instead, which is
   * also what the daemon falls back to. Reachable only from an overrides.nix
   * edited by hand on the box — /api/apply validates both bounds — and
   * `original` is read off the form below, so the substituted value is not
   * counted as a pending edit and Apply stays disabled until something is
   * actually changed. */
  els.windowStart.value = HHMM_RE.test(s.computeWindowStart) ? s.computeWindowStart : DEFAULT_WINDOW_START;
  els.windowEnd.value = HHMM_RE.test(s.computeWindowEnd) ? s.computeWindowEnd : DEFAULT_WINDOW_END;
  original = readForm();
  settingsLoaded = true;
  // announce: if what the daemon has stored is itself invalid, say so now
  // rather than waiting for the user to touch the field.
  refreshControls(true);
}

function validPort(p) { return Number.isInteger(p) && p >= 1024 && p <= 65535; }

/* RFC 1123 label, the same shape as the `pattern` on the input.
 *
 * The field used to accept any 63 characters, and the value becomes
 * networking.hostName, the mDNS name, Nextcloud's trusted_domains and
 * overwrite.cli.url, and Forgejo's ROOT_URL. A space or a slash fails the
 * evaluation; a leading hyphen or a 64th character gives a box that boots
 * unreachable — and there is no SSH and no shell login to fix it from. */
const HOSTNAME_RE = /^[A-Za-z0-9]([A-Za-z0-9-]{0,61}[A-Za-z0-9])?$/;

function validHostname(h) { return HOSTNAME_RE.test(h); }

function hostnameProblem(h) {
  if (h.length === 0) { return 'Enter a hostname.'; }
  if (h.length > 63) { return 'A hostname is at most 63 characters.'; }
  return 'Use letters, digits and hyphens only, starting and ending with a letter or a digit.';
}

/* Both ends of the compute window, valid.
 *
 * Takes the whole form rather than one field: the two bounds are one
 * setting, and an Apply that went out with a good start and a blank end
 * would write half a window. Any order is legal — an end before the start
 * wraps midnight, which is the default 23:00→07:00 — so there is nothing to
 * compare between them, only a shape to check on each. */
function validWindow(v) {
  return HHMM_RE.test(v.computeWindowStart) && HHMM_RE.test(v.computeWindowEnd);
}

function sameSettings(a, b) {
  return a.sharingMyStorage === b.sharingMyStorage &&
    a.nextcloudMode === b.nextcloudMode &&
    a.forgejoMode === b.forgejoMode &&
    a.hostName === b.hostName &&
    a.https === b.https &&
    a.gpuEnable === b.gpuEnable &&
    ((Number.isNaN(a.apachePort) && Number.isNaN(b.apachePort)) || a.apachePort === b.apachePort) &&
    a.proxyEnable === b.proxyEnable &&
    a.clusterEnable === b.clusterEnable &&
    a.shareCompute === b.shareCompute &&
    a.computeWindowStart === b.computeWindowStart &&
    a.computeWindowEnd === b.computeWindowEnd;
}

/* Recompute both buttons, and optionally the field messages.
 *
 * `announce` is true only on `change` (which for a text input means the
 * value was committed) — showing the message on every keystroke would fire
 * the alert at a screen reader halfway through the word being typed. */
function refreshControls(announce) {
  const v = readForm();
  const hostOk = validHostname(v.hostName);
  const windowOk = validWindow(v);
  const dirty = original !== null && !sameSettings(original, v);

  els.applyBtn.disabled = !settingsLoaded || !dirty || !hostOk ||
    !validPort(v.apachePort) || !windowOk || applying;
  // Factory reset is destructive and irreversible from this box: it stays
  // dead until the settings have loaded, which is also what keeps it out of
  // reach while the token overlay is up.
  els.factoryResetBtn.disabled = !settingsLoaded || applying;

  els.hostName.setAttribute('aria-invalid', hostOk ? 'false' : 'true');
  if (hostOk) {
    els.hostNameError.hidden = true;
    els.hostNameError.textContent = '';
  } else if (announce || els.hostNameError.textContent) {
    els.hostNameError.textContent = hostnameProblem(v.hostName);
    els.hostNameError.hidden = false;
  }

  // One message for the pair: both bounds share it, so aria-invalid is what
  // says which of the two is at fault. In practice a browser with a real
  // time picker only ever produces the empty case — the field cannot hold a
  // half-typed value — but the fallback text input on a browser without one
  // can, and that is the browser that most needs to be told the format.
  els.windowStart.setAttribute('aria-invalid',
    HHMM_RE.test(v.computeWindowStart) ? 'false' : 'true');
  els.windowEnd.setAttribute('aria-invalid',
    HHMM_RE.test(v.computeWindowEnd) ? 'false' : 'true');
  if (windowOk) {
    els.windowError.hidden = true;
    els.windowError.textContent = '';
  } else if (announce || els.windowError.textContent) {
    els.windowError.textContent = 'Set both ends of the window as a 24-hour time, HH:MM.';
    els.windowError.hidden = false;
  }
}

/* Build the full modules/overrides.nix body the daemon writes. Format
 * matches the backend's line-based parser and defaultOverridesNix exactly:
 * `{ ... }:` header, one `losos.<key> = <value>;` per line, booleans/ints
 * bare, strings double-quoted with escaping. */

/* Escape a value into a Nix double-quoted string literal.
 *
 * Nix interpolates ${...} inside double quotes, so escaping \ and " alone
 * was not escaping: a value containing ${...} was written into
 * overrides.nix verbatim and evaluated as root on the next rebuild. Order
 * matters — backslashes first, or the escapes introduced below get escaped
 * in turn. */
function nixString(s) {
  return '"' + String(s)
    .replace(/\\/g, '\\\\')
    .replace(/"/g, '\\"')
    .replace(/\$\{/g, '\\${') + '"';
}

function generateNix() {
  const v = readForm();
  const port = validPort(v.apachePort) ? v.apachePort : DEFAULT_APACHE_PORT;
  const host = validHostname(v.hostName) ? v.hostName : DEFAULT_HOSTNAME;
  const winStart = HHMM_RE.test(v.computeWindowStart) ? v.computeWindowStart : DEFAULT_WINDOW_START;
  const winEnd = HHMM_RE.test(v.computeWindowEnd) ? v.computeWindowEnd : DEFAULT_WINDOW_END;
  return '{ ... }:\n' +
    '{\n' +
    '  losos.sharingMyStorage = ' + (v.sharingMyStorage ? 'true' : 'false') + ';\n' +
    '  losos.nextcloud.mode = ' + nixString(v.nextcloudMode) + ';\n' +
    '  losos.forgejo.mode = ' + nixString(v.forgejoMode) + ';\n' +
    '  losos.hostName = ' + nixString(host) + ';\n' +
    '  losos.nextcloud.https = ' + (v.https ? 'true' : 'false') + ';\n' +
    '  losos.gpu.enable = ' + (v.gpuEnable ? 'true' : 'false') + ';\n' +
    '  losos.nextcloud.apachePort = ' + port + ';\n' +
    '  losos.proxy.enable = ' + (v.proxyEnable ? 'true' : 'false') + ';\n' +
    '  losos.cluster.enable = ' + (v.clusterEnable ? 'true' : 'false') + ';\n' +
    '  losos.cluster.shareCompute = ' + (v.shareCompute ? 'true' : 'false') + ';\n' +
    // Through nixString() like every other string written here, even though
    // HHMM_RE has already reduced these two to five characters from a fixed
    // alphabet. The escaping is what makes this file safe to generate from a
    // browser at all; a value exempted from it because today's validator
    // happens to precede it is how that property gets lost.
    '  losos.cluster.computeWindow.start = ' + nixString(winStart) + ';\n' +
    '  losos.cluster.computeWindow.end = ' + nixString(winEnd) + ';\n' +
    '}\n';
}

// ── Network: settings ─────────────────────────────────────────────

/* Returns {settings} on success, {unauthorized:true} on 401; throws on any
 * other failure. A successful call with a candidate token saves it. The
 * candidate goes in as an explicit header so a rejected one does not evict
 * the token already in sessionStorage (see apiFetch). */
async function fetchSettings(candidate) {
  const opts = candidate ? { headers: { 'Authorization': 'Bearer ' + candidate } } : {};
  const res = await apiFetch('/api/settings', opts);
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

let releaseAuth = null;

function showAuth(msg) {
  els.authError.textContent = msg || '';
  els.authError.hidden = !msg;
  els.authToken.value = '';
  els.authOverlay.hidden = false;
  // Mark the page behind the scrim inert and keep Tab inside the card. The
  // scrim on its own was only paint: one Tab out of the token field landed
  // in the settings form behind it.
  if (!releaseAuth) {
    releaseAuth = openModal(els.authOverlay, [$('topbar'), $('layout')]);
  }
  els.authToken.focus();
}

function hideAuth() {
  els.authOverlay.hidden = true;
  if (releaseAuth) { releaseAuth(); releaseAuth = null; }
}

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
    await resumeIfBuilding();
  } catch (e) {
    els.authError.textContent = 'Could not reach the appliance: ' + e.message;
    els.authError.hidden = false;
  }
});

// Session expired mid-use: drop the token and re-lock the page.
function sessionExpired() {
  dropToken();
  statusPoll.stop();
  applying = false;
  settingsLoaded = false;
  refreshControls(false);
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

let jobId = null;         // job id from the /api/apply or /api/factory-reset ack
let sawBuilding = false;  // this cycle has been observed building at least once
let settleUntil = 0;

/* Does this status document belong to a different job than the one we
 * started?
 *
 * The ack from /api/apply carries a job id; statusResponse does not (see
 * backend/schema.json — required is ["state","progress","message"], and
 * additionalProperties is false), so this comparison is a no-op today. It
 * starts working on its own the moment lososd puts `job` in the status
 * document, which is why the ack's id is kept rather than discarded. */
function wrongJob(data) {
  return !!(jobId && data && data.job && data.job !== jobId);
}

/* Is this *terminal* status the previous job's rather than ours?
 *
 * Without a job id to compare, the observable difference is ordering: a
 * done/failed arriving before this cycle has ever been seen building is the
 * previous rebuild's outcome. Believing it would say "Rebuild complete" and
 * re-arm Apply and Factory reset while the rebuild was still starting.
 *
 * Only terminal states go through here — `building` is what we are waiting
 * for and must never be gated by it. SETTLE_MS bounds the wait, so a cycle
 * that somehow never reports building still resolves instead of leaving the
 * form stuck on "Applying…" forever. */
function isStaleTerminal(data) {
  if (wrongJob(data)) { return true; }
  return applying && !sawBuilding && Date.now() < settleUntil;
}

const statusPoll = pollStatus({
  periodMs: POLL_MS,

  onBuilding: function (data, message) {
    if (wrongJob(data)) { return; }
    sawBuilding = true;
    applying = true;
    showProgress('building', 'Applying…', message || 'Rebuild in progress.');
    refreshControls(false);
  },

  onDone: function (data, message) {
    if (isStaleTerminal(data)) { return; }
    statusPoll.stop();
    showProgress('done', 'Rebuild complete.', message);
    applying = false;
    jobId = null;
    reloadSettings().catch(function (e) {
      refreshControls(false);
      showError('Rebuild finished, but reloading settings failed: ' + e.message);
    });
  },

  onFailed: function (data, message) {
    if (isStaleTerminal(data)) { return; }
    statusPoll.stop();
    showProgress('failed', 'Rebuild failed.', message || 'See the lososd logs for details.');
    applying = false;
    jobId = null;
    refreshControls(false);
  },

  onIdle: function (data) {
    if (isStaleTerminal(data)) { return; }
    statusPoll.stop();
    applying = false;
    jobId = null;
    hideProgress();
    refreshControls(false);
  },

  onUnauthorized: sessionExpired,
});

/* Pick up a rebuild that was already running when this page loaded (or when
 * the token was pasted). Without it a reload mid-rebuild came back as an
 * idle form with Apply and Factory reset live, and a second /api/apply
 * could be fired on top of the one still running. */
async function resumeIfBuilding() {
  const res = await apiFetch('/api/status');
  if (res.status === 401) { sessionExpired(); return; }
  if (!res.ok) { return; }
  const data = await res.json();
  if (!data || data.state !== 'building') { return; }
  applying = true;
  sawBuilding = true;   // we are joining it mid-flight, not starting it
  jobId = null;         // no ack to match against; the daemon owns this one
  settleUntil = 0;
  showProgress('building', 'Applying…', data.message || 'Rebuild in progress.');
  refreshControls(false);
  statusPoll.start();
}

async function reloadSettings() {
  const r = await fetchSettings();
  if (r.unauthorized) { sessionExpired(); return; }
  populate(r.settings);
}

// ── Apply / factory reset ─────────────────────────────────────────

async function runRebuild(trigger) {
  clearError();
  applying = true;
  sawBuilding = false;
  jobId = null;
  settleUntil = Date.now() + SETTLE_MS;
  refreshControls(false);
  showProgress('building', 'Applying…', 'Starting rebuild…');
  try {
    const res = await trigger();
    if (res.status === 401) { sessionExpired(); return; }
    if (!res.ok) {
      const data = await res.json().catch(function () { return {}; });
      throw new Error(data.error || ('HTTP ' + res.status));
    }
    const ack = await res.json().catch(function () { return {}; });
    jobId = (ack && ack.job) || null;
    statusPoll.start();
  } catch (e) {
    applying = false;
    jobId = null;
    refreshControls(false);
    showProgress('failed', 'Rebuild failed to start.', e.message);
    showError(e.message);
  }
}

els.applyBtn.addEventListener('click', function () {
  runRebuild(function () {
    return apiFetch('/api/apply', {
      method: 'POST',
      headers: { 'Content-Type': 'text/plain' },
      body: generateNix(),
    });
  }).catch(function (e) { showError('Apply failed unexpectedly: ' + e.message); });
});

els.factoryResetBtn.addEventListener('click', function () {
  if (!window.confirm('Reset all appliance settings to factory defaults and rebuild?')) { return; }
  runRebuild(function () {
    return apiFetch('/api/factory-reset', { method: 'POST' });
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
  wireErrorBanner();

  document.querySelectorAll('.content input, .content select').forEach(function (el) {
    el.addEventListener('input', function () { refreshControls(false); });
    el.addEventListener('change', function () { refreshControls(true); });
  });

  (async function () {
    if (!getToken()) {
      showAuth('');
      return;
    }
    const r = await fetchSettings();
    if (r.unauthorized) {
      showAuth('Saved token was rejected — paste the current admin token.');
      return;
    }
    populate(r.settings);
    hideAuth();
    await resumeIfBuilding();
  })().catch(function (e) {
    showError('Failed to load settings: ' + e.message);
  });
})();
