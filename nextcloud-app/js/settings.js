/**
 * losos admin settings behaviour.
 *
 * Flow:
 *   1. On load, GET /state -> set the active radio + backend status dot.
 *   2. When the admin changes the radio, POST /mode (with request token) ->
 *      disable the toggle and start polling /status every 2s.
 *   3. Poll results drive the status dot and a Nextcloud toast notification
 *      ("Rebuilding… 42%"); on state === done|failed the toast is finalised
 *      and polling stops.
 *
 * Endpoints are served at <webroot>/index.php/apps/losos/<route> (AppFramework
 * non-OCS routes). CSRF: every POST carries the request token header that
 * AppFramework's SecurityMiddleware checks.
 */
(function () {
	'use strict';

	const POLL_MS = 2000;
	const root = document.getElementById('losos-settings');
	if (!root) {
		return;
	}

	const url = (path) => OC.generateUrl('apps/losos/' + path);
	const csrfHeaders = () => ({ requesttoken: OC.requestToken });

	const statusEl = root.querySelector('.losos-status');
	const statusText = statusEl.querySelector('.losos-status__text');
	const fieldset = root.querySelector('.losos-settings__toggle');
	const radios = Array.from(root.querySelectorAll('input[name="losos-mode"]'));
	const missingEl = root.querySelector('.losos-settings__backend-missing');
	const errorEl = root.querySelector('.losos-settings__error');
	let toastId = null;
	let pollTimer = null;

	function setState(state) {
		// state in {idle, building, done, failed, unknown}
		statusEl.dataset.state = state;
	}

	function showError(msg) {
		errorEl.textContent = msg;
		errorEl.hidden = false;
	}
	function clearError() {
		errorEl.hidden = true;
		errorEl.textContent = '';
	}

	function toast(html, options) {
		// OC.Notification is the admin-app notification toast (in-page), which
		// matches wtf.md's "progress shown in a notification".
		if (typeof OC.Notification !== 'undefined' && OC.Notification.showHtml) {
			if (toastId !== null) {
				OC.Notification.hide(toastId);
			}
			toastId = OC.Notification.showHtml(html, options || {});
			return;
		}
		// Fallback to the status line if the notification API is unavailable.
		statusText.textContent = html.replace(/<[^>]*>/g, '');
	}

	async function fetchState() {
		try {
			const res = await fetch(url('state'), { headers: csrfHeaders() });
			const data = await res.json();
			if (!data.installed) {
				missingEl.hidden = false;
				setState('unknown');
				statusText.textContent = t('losos', 'Backend not installed');
				return;
			}
			missingEl.hidden = true;
			const mode = data.mode === 'mesh' ? 'mesh' : 'local';
			const radio = root.querySelector('#losos-mode-' + mode);
			if (radio) {
				radio.checked = true;
			}
			fieldset.disabled = false;
			setState(data.sharing ? 'idle' : 'idle');
			statusText.textContent = data.sharing
				? t('losos', 'On the mesh')
				: t('losos', 'Local only');
		} catch (e) {
			setState('failed');
			statusText.textContent = t('losos', 'Failed to reach backend');
		}
	}

	async function applyMode(mode) {
		clearError();
		fieldset.disabled = true;
		setState('building');
		statusText.textContent = t('losos', 'Triggering rebuild…');
		toast(escapeHtml(t('losos', 'Rebuild started…')));
		try {
			const res = await fetch(url('mode'), {
				method: 'POST',
				headers: Object.assign(csrfHeaders(), { 'Content-Type': 'application/x-www-form-urlencoded' }),
				body: 'mode=' + encodeURIComponent(mode),
			});
			if (res.status === 409) {
				missingEl.hidden = false;
				setState('unknown');
				statusText.textContent = t('losos', 'Backend not installed');
				fieldset.disabled = true;
				return;
			}
			if (!res.ok) {
				const data = await res.json().catch(() => ({}));
				throw new Error(data.detail || ('HTTP ' + res.status));
			}
			startPolling();
		} catch (e) {
			setState('failed');
			statusText.textContent = t('losos', 'Rebuild failed to start');
			showError(e.message);
			fieldset.disabled = false;
		}
	}

	async function pollOnce() {
		try {
			const res = await fetch(url('status'), { headers: csrfHeaders() });
			const data = await res.json();
			if (!data.installed) {
				stopPolling();
				return;
			}
			const state = data.state || 'idle';
			const pct = Math.max(0, Math.min(100, Number(data.progress) || 0));
			setState(state);
			if (state === 'building') {
				statusText.textContent = t('losos', 'Rebuilding… {pct}%', { pct });
				toast(escapeHtml(t('losos', 'Rebuilding… {pct}%', { pct })));
			} else if (state === 'done') {
				stopPolling();
				statusText.textContent = t('losos', 'Rebuild complete');
				toast(escapeHtml(t('losos', 'Rebuild complete')), { timeout: 6 });
				fieldset.disabled = false;
				await fetchState();
			} else if (state === 'failed') {
				stopPolling();
				statusText.textContent = t('losos', 'Rebuild failed');
				if (data.message) showError(data.message);
				toast(escapeHtml(t('losos', 'Rebuild failed')), { type: 'error' });
				fieldset.disabled = false;
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
		if (pollTimer !== null) {
			clearInterval(pollTimer);
			pollTimer = null;
		}
	}

	function escapeHtml(s) {
		return String(s).replace(/[&<>"']/g, (c) => ({
			'&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;',
		}[c]));
	}

	radios.forEach((radio) => {
		radio.addEventListener('change', () => applyMode(radio.value));
	});

	// t() fallback so this file is readable outside Nextcloud if ever loaded
	// in isolation; the real `t` is provided by the server-side l10n JS.
	if (typeof t !== 'function') {
		window.t = function (app, str, vars) {
			if (vars) {
				return str.replace(/\{(\w+)\}/g, (_, k) => (k in vars ? String(vars[k]) : '{' + k + '}'));
			}
			return str;
		};
	}

	fetchState();
})();