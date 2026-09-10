'use strict';

/* losos admin UI — shared helpers for the dashboard (/) and the settings
 * page (/settings/). Loaded as a plain <script> before each page's own
 * app.js: no modules, no bundler, top-level functions in one shared global
 * scope like the rest of this dependency-free SPA.
 *
 * Everything both pages need identically lives here — the admin token in
 * sessionStorage, the authed fetch, the error banner, the /api/status poll
 * loop and the modal focus plumbing. The poll loop especially: it was
 * copied into both pages and then drifted, which is how one bug shipped
 * twice wearing two different symptoms.
 */

// ── DOM + token ───────────────────────────────────────────────────

function $(id) { return document.getElementById(id); }

function getToken() { return sessionStorage.getItem('losos-token'); }
function saveToken(t) { sessionStorage.setItem('losos-token', t); }
function dropToken() { sessionStorage.removeItem('losos-token'); }

function authHeaders() {
  const t = getToken();
  return t ? { 'Authorization': 'Bearer ' + t } : {};
}

// ── Error banner ──────────────────────────────────────────────────
// Both pages carry #error-banner / #error-text / #error-close.

function showError(msg) {
  const banner = $('error-banner');
  const text = $('error-text');
  if (!banner || !text) { return; }
  text.textContent = msg;
  banner.hidden = false;
}

function clearError() {
  const banner = $('error-banner');
  const text = $('error-text');
  if (!banner) { return; }
  banner.hidden = true;
  if (text) { text.textContent = ''; }
}

function wireErrorBanner() {
  const close = $('error-close');
  if (close) { close.addEventListener('click', clearError); }
}

// ── Authed fetch ──────────────────────────────────────────────────

/* Call one of lososd's /api/* routes with the Bearer token attached and
 * caching off, and hand back the Response.
 *
 * A 401 means the token lososd minted is no longer the one this tab holds,
 * so the stored copy is dropped here and no caller can forget to. What to
 * do next differs per page — the dashboard shows a sign-in hint, settings
 * re-locks behind the token overlay — so that half stays with the caller,
 * which branches on `res.status === 401`.
 *
 * Passing an explicit Authorization header opts out of the eviction: the
 * unlock overlay probes a candidate token that is not the stored one, and a
 * rejected candidate must not log the tab out. */
async function apiFetch(url, opts) {
  const o = Object.assign({ cache: 'no-store' }, opts || {});
  const given = o.headers || {};
  const explicitAuth = !!(given.Authorization || given.authorization);
  o.headers = Object.assign({}, authHeaders(), given);
  const res = await fetch(url, o);
  if (res.status === 401 && !explicitAuth) { dropToken(); }
  return res;
}

// ── Polling ───────────────────────────────────────────────────────

/* A self-scheduling timer, replacing `setInterval` with an async handler.
 *
 * setInterval fires on the clock whether or not the previous round
 * finished, so rounds stack and their responses can land out of order.
 * That is not theoretical here: `nixos-rebuild switch` restarts lososd
 * mid-rebuild, so polls routinely outlive their period, and a late
 * `building` landing after a fresh `done` would walk the UI backwards.
 *
 * So: one round at a time, the period counted from when a round *finishes*,
 * the in-flight request aborted on stop(), and the round skipped outright
 * while the tab is hidden — a backgrounded tab has nothing to render and no
 * business hammering the appliance.
 *
 * `task(signal)` gets an AbortSignal and may be async. Each round carries a
 * generation number that stop() and start() bump, so a response arriving
 * after either is dropped rather than applied on top of newer state. */
function createPoller(task, periodMs) {
  let period = periodMs;
  let timer = null;
  let controller = null;
  let gen = 0;
  let running = false;

  function stop() {
    running = false;
    gen += 1;
    if (timer !== null) { clearTimeout(timer); timer = null; }
    if (controller !== null) { controller.abort(); controller = null; }
  }

  function schedule(myGen) {
    if (!running || myGen !== gen) { return; }
    timer = setTimeout(function () { void round(myGen); }, period);
  }

  async function round(myGen) {
    timer = null;
    if (!running || myGen !== gen) { return; }
    if (document.visibilityState === 'hidden') { schedule(myGen); return; }
    controller = new AbortController();
    try {
      await task(controller.signal);
    } catch (e) {
      // The AbortError from stop(), or the connection dropping while lososd
      // restarts mid-rebuild. Either way the chain stays alive; a real
      // outage shows up in the status dots, not as a dead page.
    }
    // Only clear the controller we installed: a handler that stopped and
    // restarted the poller has already installed a newer one.
    if (myGen === gen) { controller = null; }
    schedule(myGen);
  }

  function start() {
    stop();
    running = true;
    void round(gen);
  }

  // Returning to a visible tab should refresh now, not at the end of the
  // period that was already counting down when it was backgrounded.
  document.addEventListener('visibilitychange', function () {
    if (!running || document.visibilityState !== 'visible' || timer === null) { return; }
    clearTimeout(timer);
    timer = null;
    void round(gen);
  });

  return {
    start: start,
    stop: stop,
    setPeriod: function (ms) { period = ms; },
    isRunning: function () { return running; },
  };
}

/* Poll GET /api/status and route each document to one handler. Both pages
 * drive their rebuild UI from this.
 *
 * handlers: { onBuilding, onDone, onFailed, onIdle, onUnauthorized,
 *             periodMs }. Each state handler is called with
 * (data, message). Returns the poller — call .start(), .stop() or
 * .setPeriod(ms) on it. */
function pollStatus(handlers) {
  function fire(fn, data) {
    if (fn) { fn(data, (data && data.message) || ''); }
  }

  const poller = createPoller(async function (signal) {
    if (!getToken()) {
      poller.stop();
      if (handlers.onUnauthorized) { handlers.onUnauthorized(); }
      return;
    }
    const res = await apiFetch('/api/status', { signal: signal });
    if (res.status === 401) {
      poller.stop();
      if (handlers.onUnauthorized) { handlers.onUnauthorized(); }
      return;
    }
    // A 502 while lososd is restarting is expected mid-rebuild: keep polling.
    if (!res.ok) { return; }
    const data = await res.json();
    const state = (data && data.state) || 'idle';
    if (state === 'building') { fire(handlers.onBuilding, data); }
    else if (state === 'done') { fire(handlers.onDone, data); }
    else if (state === 'failed') { fire(handlers.onFailed, data); }
    else { fire(handlers.onIdle, data); }
  }, handlers.periodMs || 2000);

  return poller;
}

// ── Modal focus handling ──────────────────────────────────────────

const LS_FOCUSABLE = 'a[href], button:not([disabled]), input:not([disabled]), ' +
  'select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';

function focusablesIn(root) {
  return Array.prototype.filter.call(
    root.querySelectorAll(LS_FOCUSABLE),
    function (el) { return el.getClientRects().length > 0; },
  );
}

/* Open `card` as a modal over `outside` (the page regions it covers).
 *
 * `inert` does the real work: one attribute takes the background out of the
 * tab order, out of the accessibility tree and out of pointer hit-testing,
 * which is what a bare visual scrim never did. The Tab handler is both the
 * fallback for browsers without `inert` and the part that wraps focus at
 * the card's own first and last control. Escape stays with the caller —
 * the two modals close differently.
 *
 * Returns the close function: it releases the trap, un-inerts the
 * background and hands focus back to whatever opened the modal. */
function openModal(card, outside) {
  const restore = document.activeElement;
  outside.forEach(function (el) { if (el) { el.inert = true; } });

  function onKeydown(e) {
    if (e.key !== 'Tab') { return; }
    const items = focusablesIn(card);
    if (items.length === 0) { e.preventDefault(); return; }
    const first = items[0];
    const last = items[items.length - 1];
    const inside = card.contains(document.activeElement);
    if (e.shiftKey && (!inside || document.activeElement === first)) {
      e.preventDefault();
      last.focus();
    } else if (!e.shiftKey && (!inside || document.activeElement === last)) {
      e.preventDefault();
      first.focus();
    }
  }
  // Capture on the document, not the card: if focus ever does escape, this
  // is what pulls it back.
  document.addEventListener('keydown', onKeydown, true);

  return function closeModal() {
    document.removeEventListener('keydown', onKeydown, true);
    outside.forEach(function (el) { if (el) { el.inert = false; } });
    if (restore && typeof restore.focus === 'function') { restore.focus(); }
  };
}
