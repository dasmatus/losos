/**
 * losos settings page behaviour (dedicated app page).
 *
 * Flow:
 *   1. On load: GET /settings -> populate the form (hostname, mode pickers,
 *      toggles, ports) and enable Apply. If installed:false, show the banner.
 *   2. Sidebar clicks switch the visible section (pure DOM, no routing).
 *   3. On Apply: build the overrides.nix body from the form values (mirroring
 *      the Haskell defaultOverridesNix format — one `losos.<key> = <value>;`
 *      per line so the backend's line parser can read it back), POST it to
 *      /apply with the request token, then poll /status every 2s.
 *   4. Poll results drive the header status dot + a Nextcloud toast; on
 *      done/failed polling stops and the form re-enables.
 *
 * Endpoints are served at <webroot>/index.php/apps/losos/<route>. CSRF: every
 * POST carries the request token header that AppFramework's SecurityMiddleware
 * checks; GETs are read-only.
 */
(function () {
	'use strict';

	const POLL_MS = 2000;

	// t() fallback so this file is lintable in isolation; the real `t` is
	// provided by the server-side l10n JS at runtime.
	if (typeof t !== 'function') {
		window.t = function (app, str, vars) {
			if (vars) {
				return str.replace(/\{(\w+)\}/g, function (_, k) {
					return (k in vars) ? String(vars[k]) : '{' + k + '}';
				});
			}
			return str;
		};
	}

	const root = document.getElementById('losos-app');
	if (!root) {
		return;
	}

	const url = function (path) { return OC.generateUrl('apps/losos/' + path); };
	const csrfHeaders = function () { return { requesttoken: OC.requestToken }; };

	const statusEl = root.querySelector('.losos-status');
	const statusText = statusEl.querySelector('.losos-status__text');
	const missingEl = root.querySelector('.losos-banner--missing');
	const form = document.getElementById('losos-form');
	const applyBtn = document.getElementById('losos-apply');
	const errorEl = root.querySelector('.losos-actions__error');
	let toastId = null;
	let pollTimer = null;

	function setState(state) { statusEl.dataset.state = state; }
	function showError(msg) { errorEl.textContent = msg; errorEl.hidden = false; }
	function clearError() { errorEl.hidden = true; errorEl.textContent = ''; }

	function toast(html, options) {
		if (typeof OC.Notification !== 'undefined' && OC.Notification.showHtml) {
			if (toastId !== null) { OC.Notification.hide(toastId); }
			toastId = OC.Notification.showHtml(html, options || {});
			return;
		}
		statusText.textContent = html.replace(/<[^>]*>/g, '');
	}

	function escapeHtml(s) {
		return String(s).replace(/[&<>"']/g, function (c) {
			return ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[c];
		});
	}

	// ── Form read/write ─────────────────────────────────────────────
	function setSegmented(name, value) {
		const group = root.querySelector('.losos-segmented[data-name="' + name + '"]');
		if (!group) { return; }
		group.querySelectorAll('button').forEach(function (btn) {
			btn.classList.toggle('is-active', btn.dataset.value === value);
		});
	}
	function getSegmented(name) {
		const group = root.querySelector('.losos-segmented[data-name="' + name + '"]');
		const active = group ? group.querySelector('button.is-active') : null;
		return active ? active.dataset.value : null;
	}

	function populate(data) {
		root.dataset.installed = data.installed ? 'true' : 'false';
		if (!data.installed) {
			missingEl.hidden = false;
			setState('unknown');
			statusText.textContent = t('losos', 'Backend not installed');
			applyBtn.disabled = true;
			return;
		}
		missingEl.hidden = true;
		document.getElementById('losos-hostName').value = data.hostName || 'mattbox';
		document.getElementById('losos-https').checked = !!data.https;
		document.getElementById('losos-sharingMyStorage').checked = !!data.sharingMyStorage;
		document.getElementById('losos-gpuEnable').checked = !!data.gpuEnable;
		document.getElementById('losos-aioApachePort').value = data.aioApachePort || 11000;
		document.getElementById('losos-aioInterfacePort').value = data.aioInterfacePort || 8000;
		setSegmented('nextcloudMode', data.nextcloudMode || 'aio');
		setSegmented('forgejoMode', data.forgejoMode || 'container');
		applyBtn.disabled = false;
		setState('idle');
		statusText.textContent = t('losos', 'Settings loaded');
	}

	/**
	 * Build the overrides.nix body from the form. Format must match the
	 * Haskell defaultOverridesNix + lookupNix line parser: one
	 * `losos.<key> = <value>;` per line. Booleans bare, strings quoted, ints
	 * bare. The `{ ... }:` header makes it a valid Nix module (the flake's
	 * install config merges it into the evaluation).
	 */
	function buildNix() {
		const hostName = document.getElementById('losos-hostName').value.trim();
		const https = document.getElementById('losos-https').checked;
		const sharing = document.getElementById('losos-sharingMyStorage').checked;
		const gpu = document.getElementById('losos-gpuEnable').checked;
		const apache = parseInt(document.getElementById('losos-aioApachePort').value, 10) || 11000;
		const iface = parseInt(document.getElementById('losos-aioInterfacePort').value, 10) || 8000;
		const nextcloudMode = getSegmented('nextcloudMode') || 'aio';
		const forgejoMode = getSegmented('forgejoMode') || 'container';

		// Nix string: only [A-Za-z0-9._-] are safe unquoted in a hostname; we
		// quote regardless and escape any embedded quotes.
		const q = function (s) { return '"' + String(s).replace(/"/g, '\\"') + '"'; };
		return '{ ... }:\n{\n' +
			'  losos.sharingMyStorage = ' + (sharing ? 'true' : 'false') + ';\n' +
			'  losos.nextcloud.mode = ' + q(nextcloudMode) + ';\n' +
			'  losos.forgejo.mode = ' + q(forgejoMode) + ';\n' +
			'  losos.hostName = ' + q(hostName || 'mattbox') + ';\n' +
			'  losos.nextcloud.https = ' + (https ? 'true' : 'false') + ';\n' +
			'  losos.gpu.enable = ' + (gpu ? 'true' : 'false') + ';\n' +
			'  losos.aio.apachePort = ' + apache + ';\n' +
			'  losos.aio.interfacePort = ' + iface + ';\n' +
			'}\n';
	}

	// ── Network ─────────────────────────────────────────────────────
	async function fetchSettings() {
		try {
			const res = await fetch(url('settings'), { headers: csrfHeaders() });
			const data = await res.json();
			populate(data);
		} catch (e) {
			setState('failed');
			statusText.textContent = t('losos', 'Failed to reach backend');
		}
	}

	async function applyConfig() {
		clearError();
		applyBtn.disabled = true;
		setState('building');
		statusText.textContent = t('losos', 'Triggering rebuild…');
		toast(escapeHtml(t('losos', 'Rebuild started…')));
		try {
			const res = await fetch(url('apply'), {
				method: 'POST',
				headers: Object.assign(csrfHeaders(), { 'Content-Type': 'application/x-www-form-urlencoded' }),
				body: 'nix=' + encodeURIComponent(buildNix()),
			});
			if (res.status === 409) {
				missingEl.hidden = false;
				setState('unknown');
				statusText.textContent = t('losos', 'Backend not installed');
				return;
			}
			if (!res.ok) {
				const data = await res.json().catch(function () { return {}; });
				throw new Error(data.error || data.detail || ('HTTP ' + res.status));
			}
			startPolling();
		} catch (e) {
			setState('failed');
			statusText.textContent = t('losos', 'Rebuild failed to start');
			showError(e.message);
			applyBtn.disabled = false;
		}
	}

	async function pollOnce() {
		try {
			const res = await fetch(url('status'), { headers: csrfHeaders() });
			const data = await res.json();
			if (!data.installed) { stopPolling(); return; }
			const state = data.state || 'idle';
			setState(state);
			if (state === 'building') {
				const pct = Math.max(0, Math.min(100, Number(data.progress) || 0));
				statusText.textContent = t('losos', 'Rebuilding… {pct}%', { pct: pct });
				toast(escapeHtml(t('losos', 'Rebuilding… {pct}%', { pct: pct })));
			} else if (state === 'done') {
				stopPolling();
				statusText.textContent = t('losos', 'Rebuild complete');
				toast(escapeHtml(t('losos', 'Rebuild complete')), { timeout: 6 });
				applyBtn.disabled = false;
				await fetchSettings();
			} else if (state === 'failed') {
				stopPolling();
				statusText.textContent = t('losos', 'Rebuild failed');
				if (data.message) { showError(data.message); }
				toast(escapeHtml(t('losos', 'Rebuild failed')), { type: 'error' });
				applyBtn.disabled = false;
			}
		} catch (e) {
			// transient fetch error: keep polling, don't thrash the UI
		}
	}

	function startPolling() {
		stopPolling();
		pollTimer = setInterval(pollOnce, POLL_MS);
		pollOnce();
	}
	function stopPolling() {
		if (pollTimer !== null) { clearInterval(pollTimer); pollTimer = null; }
	}

	// ── Wiring ──────────────────────────────────────────────────────
	// Sidebar section switching.
	root.querySelectorAll('.losos-sidebar__item').forEach(function (item) {
		item.addEventListener('click', function (ev) {
			ev.preventDefault();
			root.querySelectorAll('.losos-sidebar__item').forEach(function (i) { i.classList.remove('is-active'); });
			item.classList.add('is-active');
			const target = item.getAttribute('href');
			root.querySelectorAll('.losos-section').forEach(function (s) { s.classList.remove('is-active'); });
			const section = target ? root.querySelector(target) : null;
			if (section) { section.classList.add('is-active'); }
		});
	});

	// Segmented controls: clicking a button makes it active, deactivates siblings.
	root.querySelectorAll('.losos-segmented').forEach(function (group) {
		group.querySelectorAll('button').forEach(function (btn) {
			btn.addEventListener('click', function () {
				group.querySelectorAll('button').forEach(function (b) { b.classList.remove('is-active'); });
				btn.classList.add('is-active');
			});
		});
	});

	form.addEventListener('submit', function (ev) {
		ev.preventDefault();
		applyConfig();
	});

	fetchSettings();
})();