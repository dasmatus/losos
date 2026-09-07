'use strict';

/* losos homepage — launcher tiles + live service probes + rebuild banner.
 *
 * Talks to the same-origin Nginx routes:
 *   /nextcloud, /forgejo/            (service probes)
 *   /api/health, /api/state, /api/status  (lososd admin API, Bearer-authed)
 * plus the Tahoe WUI on :3456 (cross-origin, probed with mode:no-cors).
 *
 * The admin token lives in sessionStorage ('losos-token'); it is entered on
 * the settings page. Without a token the homepage still probes the public
 * services and shows a sign-in hint instead of rebuild progress.
 *
 * The shared pieces — authed fetch, error banner, poll loop, modal focus
 * handling — live in common.js and are shared verbatim with the settings
 * page. Page chrome — the floating topbar that condenses on scroll and the
 * tile address popover — is styled in home.css.
 */

const PROBE_MS = 15000;
const STATUS_MS = 2000;        // a rebuild is running: keep up with the log
const IDLE_STATUS_MS = 30000;  // nothing running: only watch for one starting
const DONE_HIDE_MS = 20000;

// Approximate popover box, used to keep it inside the viewport.
const CTX_W = 190;
const CTX_H = 110;

// ── Tiny helpers ──────────────────────────────────────────────────
// ($, getToken, dropToken, apiFetch, showError, pollStatus, openModal and
// friends all live in common.js.)

function isAbort(e) { return !!e && e.name === 'AbortError'; }

// ── Status dots ───────────────────────────────────────────────────

const DOT_LABEL = { up: 'Up', down: 'Down', checking: 'Checking…' };

/* Paint one service's state.
 *
 * The text is the only channel a screen reader gets: every dot is
 * aria-hidden and carries no label, because a dot that both labels itself
 * and sits next to a text node announces the state twice on the system card
 * and, inside an aria-hidden icon, never announces a change at all. The
 * text nodes are aria-live regions instead, so an up→down transition is
 * spoken when it happens. The tile ones are visually hidden and prefixed
 * with the service name (from data-status-prefix), since "Down" alone tells
 * you nothing about which service went. */
function setDot(name, state) {
  const dot = $('dot-' + name);
  if (dot) { dot.dataset.state = state; }
  const text = $('st-' + name);
  if (!text) { return; }
  const label = DOT_LABEL[state] || state;
  const prefix = text.dataset.statusPrefix;
  text.textContent = prefix ? prefix + ': ' + label : label;
}

// ── Service probes ────────────────────────────────────────────────

// Same-origin probe: any HTTP response < 500 counts as up; network errors
// and 5xx count as down. An abort is neither — it means the page stopped
// asking, so it is re-thrown rather than painted as an outage.
async function probeSameOrigin(url, signal) {
  try {
    const res = await fetch(url, { cache: 'no-store', signal: signal });
    return res.status < 500 ? 'up' : 'down';
  } catch (e) {
    if (isAbort(e)) { throw e; }
    return 'down';
  }
}

// lososd health must answer the JSON document {"ok": true}.
async function probeLososd(signal) {
  try {
    const res = await fetch('/api/health', { cache: 'no-store', signal: signal });
    if (!res.ok) { return 'down'; }
    const data = await res.json();
    return data && data.ok === true ? 'up' : 'down';
  } catch (e) {
    if (isAbort(e)) { throw e; }
    return 'down';
  }
}

/* Tahoe WUI is cross-origin on :3456: a resolved (opaque) no-cors fetch
 * means something answered; a rejected one means nothing did.
 *
 * cache:'no-store' matters more here than on the same-origin probes — an
 * opaque response is still cacheable, so without it the browser can keep
 * answering this probe from cache and hold the dot green long after Tahoe
 * has died. */
async function probeTahoe(signal) {
  try {
    await fetch(tahoeUrl(), { mode: 'no-cors', cache: 'no-store', signal: signal });
    return 'up';
  } catch (e) {
    if (isAbort(e)) { throw e; }
    return 'down';
  }
}

async function runProbes(signal) {
  try {
    const results = await Promise.all([
      probeSameOrigin('/nextcloud', signal),
      probeSameOrigin('/forgejo/', signal),
      probeLososd(signal),
      probeTahoe(signal),
    ]);
    setDot('nextcloud', results[0]);
    setDot('forgejo', results[1]);
    setDot('lososd', results[2]);
    setDot('tahoe', results[3]);
  } catch (e) {
    // Each probe already turns an outage into 'down', so reaching here means
    // something else broke — worth a banner, not a silent dot.
    if (isAbort(e)) { throw e; }
    showError('Service probe failed unexpectedly: ' + e.message);
  }
}

const probePoll = createPoller(runProbes, PROBE_MS);

// ── System card ───────────────────────────────────────────────────

function setSharingMode(label) { $('sys-mode').textContent = label; }

async function loadSystemInfo() {
  $('sys-hostname').textContent = location.hostname;
  if (!getToken()) { setSharingMode('—'); return; }
  try {
    const res = await apiFetch('/api/state');
    if (res.status === 401) { setSharingMode('—'); showHint(); return; }
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

/* The dashboard keeps polling after a rebuild finishes rather than stopping:
 * rebuilds start without anyone clicking Apply here (the 03:00 auto-upgrade,
 * or the settings page in another tab), and a homepage that stopped watching
 * would never show them. It backs off to IDLE_STATUS_MS instead, and
 * createPoller skips the round outright while the tab is hidden — together
 * that is what stops an idle open tab spending ~46 requests a minute on the
 * appliance forever. */
const statusPoll = pollStatus({
  periodMs: STATUS_MS,

  onBuilding: function (data, message) {
    statusPoll.setPeriod(STATUS_MS);
    clearTimeout(hideTimer);
    showBanner('building', 'Rebuilding system…', message || 'Rebuild in progress.');
    lastBannerState = 'building';
  },

  onDone: function (data, message) {
    statusPoll.setPeriod(IDLE_STATUS_MS);
    if (lastBannerState !== 'done') {
      showBanner('done', 'Rebuild complete.', message);
      clearTimeout(hideTimer);
      hideTimer = setTimeout(hideBanner, DONE_HIDE_MS);
    }
    lastBannerState = 'done';
  },

  onFailed: function (data, message) {
    statusPoll.setPeriod(IDLE_STATUS_MS);
    clearTimeout(hideTimer);
    showBanner('failed', 'Rebuild failed.', message || 'See the lososd logs for details.');
    lastBannerState = 'failed';
  },

  onIdle: function () {
    statusPoll.setPeriod(IDLE_STATUS_MS);
    clearTimeout(hideTimer);
    hideBanner();
    lastBannerState = 'idle';
  },

  // No token, or lososd rejected the one we had (apiFetch has already
  // dropped it): fall back to the public view.
  onUnauthorized: function () {
    hideBanner();
    showHint();
    setSharingMode('—');
  },
});

// ── Floating topbar: condense to a corner pill on scroll ─────────

const floatBar = $('float-bar');

function onScroll() {
  if (floatBar) { floatBar.classList.toggle('cond', window.scrollY > 80); }
}

// ── Tile address popover: copy / open in new tab ──────────────────

const ctxOverlay = $('ctx-overlay');
const ctxMenu = $('ctx-menu');
const ctxAddrEl = $('ctx-addr');
let ctxUrl = '';
let releaseMenu = null;

/* Open the address popover at (x, y).
 *
 * It is a plain focus-managed popover, not an ARIA menu: role="menu" is a
 * contract for roving tabindex and arrow-key navigation, and claiming it
 * without implementing it leaves a screen reader announcing a menu that
 * does not respond like one. Two buttons in a labelled dialog behave
 * exactly as announced. */
function openMenu(addr, url, x, y) {
  ctxAddrEl.textContent = addr;
  ctxUrl = url;
  ctxOverlay.hidden = false;
  // Clamp both edges: on a viewport narrower than the popover the right-edge
  // clamp alone goes negative and pushes it off-screen to the left.
  ctxMenu.style.left = Math.max(8, Math.min(x, window.innerWidth - CTX_W)) + 'px';
  ctxMenu.style.top = Math.max(8, Math.min(y, window.innerHeight - CTX_H)) + 'px';
  releaseMenu = openModal(ctxMenu, [floatBar, $('main')]);
  $('ctx-copy').focus();
}

function closeMenu() {
  if (ctxOverlay.hidden) { return; }
  ctxOverlay.hidden = true;
  // Un-inerts the page and hands focus back to whatever opened the popover.
  if (releaseMenu) { releaseMenu(); releaseMenu = null; }
}

/* Every tile exposes its address two ways: a right-click, which is what a
 * mouse user reaches for, and a visible button, which is the only one of the
 * two that exists on a touch screen or a keyboard. */
function wireTile(id, menuId, addr, url) {
  const tile = $(id);
  if (tile) {
    tile.addEventListener('contextmenu', function (e) {
      e.preventDefault();
      openMenu(addr, url, e.clientX, e.clientY);
    });
  }
  const btn = $(menuId);
  if (btn) {
    btn.addEventListener('click', function () {
      const box = btn.getBoundingClientRect();
      openMenu(addr, url, box.left, box.bottom + 4);
    });
  }
}

function wireContextMenu() {
  if (!ctxOverlay) { return; }
  // The overlay eats the click that closes the popover; a second right-click
  // closes it too instead of stacking the browser menu on top.
  ctxOverlay.addEventListener('click', function (e) {
    if (e.target === ctxOverlay) { closeMenu(); }
  });
  ctxOverlay.addEventListener('contextmenu', function (e) {
    e.preventDefault();
    closeMenu();
  });
  document.addEventListener('keydown', function (e) {
    if (e.key === 'Escape' && !ctxOverlay.hidden) { closeMenu(); }
  });
  $('ctx-copy').addEventListener('click', function () {
    if (navigator.clipboard) { navigator.clipboard.writeText(ctxAddrEl.textContent); }
    closeMenu();
  });
  $('ctx-open').addEventListener('click', function () {
    window.open(ctxUrl, '_blank', 'noopener');
    closeMenu();
  });

  wireTile('tile-nextcloud', 'menu-nextcloud', location.host + '/nextcloud',
    location.origin + '/nextcloud');
  wireTile('tile-forgejo', 'menu-forgejo', location.host + '/forgejo/',
    location.origin + '/forgejo/');
  wireTile('tile-tahoe', 'menu-tahoe', location.hostname + ':3456/', tahoeUrl());
}

// ── Wiring ────────────────────────────────────────────────────────

(function init() {
  wireErrorBanner();

  const tileTahoe = $('tile-tahoe');
  if (tileTahoe) { tileTahoe.href = tahoeUrl(); }

  window.addEventListener('scroll', onScroll, { passive: true });
  onScroll();
  wireContextMenu();

  if (getToken()) {
    hideHint();
    statusPoll.start();
  } else {
    showHint();
  }

  probePoll.start();
  loadSystemInfo().catch(function () { setSharingMode('—'); });
})();
