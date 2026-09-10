'use strict';

/* losos homepage — launcher tiles + live service probes + rebuild banner.
 *
 * Talks to the same-origin Nginx routes, and to nothing else:
 *   /nextcloud, /forgejo/            (service probes)
 *   /api/health, /api/state, /api/settings, /api/status
 *                                    (lososd admin API, Bearer-authed
 *                                     except /api/health)
 *
 * Every fetch on this page is same-origin. It was not always: the Tahoe WUI
 * answered on :3456, a second origin, and the CSP in modules/containers.nix
 * named `$host:3456` in connect-src so this page could probe it. Tahoe-LAFS
 * is gone and that entry went with it — the header is plain
 * `connect-src 'self'` now, which covers this page whole (containers.nix
 * records why). Leave it that way: a probe of some other origin added later
 * should have to widen the header on purpose rather than find it already
 * open.
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

async function runProbes(signal) {
  try {
    const results = await Promise.all([
      probeSameOrigin('/nextcloud', signal),
      probeSameOrigin('/forgejo/', signal),
      probeLososd(signal),
    ]);
    setDot('nextcloud', results[0]);
    setDot('forgejo', results[1]);
    setDot('lososd', results[2]);
  } catch (e) {
    // Each probe already turns an outage into 'down', so reaching here means
    // something else broke — worth a banner, not a silent dot.
    if (isAbort(e)) { throw e; }
    showError('Service probe failed unexpectedly: ' + e.message);
  }
}

const probePoll = createPoller(runProbes, PROBE_MS);

// ── System card ───────────────────────────────────────────────────

const UNKNOWN = '—';

function setSharingMode(label) { $('sys-mode').textContent = label; }

function modeLabel(data) {
  if (data && data.mode === 'mesh') { return 'Mesh — contributing storage'; }
  if (data && data.mode === 'local') { return 'Local — private'; }
  return UNKNOWN;
}

// A window bound the daemon did not send, or sent in some other shape. The
// settings page will not write one — it validates HH:MM before Apply and
// lososd rejects the body again on the way in — so this is only ever an
// overrides.nix edited by hand on the box. Say so rather than printing
// "undefined".
function hhmm(v) { return typeof v === 'string' && v ? v : '??:??'; }

/* Paint the two mesh rows from a settingsResponse.
 *
 * This reports what the appliance is *configured* to do — the losos.cluster.*
 * keys as they stand in overrides.nix — not whether the rke2 agent reached
 * the edge and got itself a schedulable node. The admin API has no route
 * that knows the latter, and a green dot here would be claiming it. Hence
 * text rows and no dot.
 *
 * All three combinations have to read sensibly, because two of them are
 * ordinary states and not errors: not joined at all, joined but keeping the
 * CPU, and joined with a nightly window. A window whose end is before its
 * start wraps midnight — that is the default (23:00→07:00) and the common
 * case — so it is printed exactly as configured and never "corrected". */
function setClusterInfo(s) {
  const joined = !!(s && s.clusterEnable);
  const sharing = joined && !!s.shareCompute;
  $('sys-cluster').textContent = joined ? 'Joined' : 'Not joined';
  if (sharing) {
    $('sys-window').textContent = hhmm(s.computeWindowStart) + ' – ' + hhmm(s.computeWindowEnd);
  } else {
    $('sys-window').textContent = joined ? 'Compute not shared' : UNKNOWN;
  }
}

/* Both mesh rows back to "unknown".
 *
 * Note this is not setClusterInfo(null): "Not joined" is a fact about the
 * appliance, and a request that failed knows no facts. Painting the "off"
 * state on a failed fetch is how a page ends up quietly asserting the
 * opposite of what is true. */
function clearClusterInfo() {
  $('sys-cluster').textContent = UNKNOWN;
  $('sys-window').textContent = UNKNOWN;
}

// Everything on the card except the hostname needs the admin token. One
// helper puts the lot back to "unknown" so no failure path can leave half a
// stale card standing next to a fresh sign-in hint.
function clearAuthedInfo() {
  setSharingMode(UNKNOWN);
  clearClusterInfo();
}

/* Fill the System card.
 *
 * Two routes, because they answer different questions: /api/state is the
 * storage mode the daemon is actually in, /api/settings is the overrides.nix
 * body it last parsed — and the mesh keys exist only there. A 401 on either
 * abandons the whole card and raises the sign-in hint, rather than leaving
 * the half that already answered painted under a hint saying we are signed
 * out. */
async function loadSystemInfo() {
  $('sys-hostname').textContent = location.hostname;
  if (!getToken()) { clearAuthedInfo(); return; }
  try {
    const state = await apiFetch('/api/state');
    if (state.status === 401) { clearAuthedInfo(); showHint(); return; }
    setSharingMode(state.ok ? modeLabel(await state.json()) : UNKNOWN);

    const settings = await apiFetch('/api/settings');
    if (settings.status === 401) { clearAuthedInfo(); showHint(); return; }
    if (settings.ok) { setClusterInfo(await settings.json()); }
    else { clearClusterInfo(); }
  } catch (e) {
    clearAuthedInfo();
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
      // The card is painted once at load, and a finished rebuild is precisely
      // what invalidates it — Apply on the settings page in another tab, or
      // the 03:00 auto-upgrade, both land here. Inside the
      // once-per-completion guard: /api/status keeps answering `done` for as
      // long as nothing else runs, and re-fetching two routes every idle
      // period for a card nobody changed is the kind of background traffic
      // IDLE_STATUS_MS exists to avoid.
      loadSystemInfo().catch(clearAuthedInfo);
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
    clearAuthedInfo();
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
}

// ── Wiring ────────────────────────────────────────────────────────

(function init() {
  wireErrorBanner();

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
  loadSystemInfo().catch(clearAuthedInfo);
})();
