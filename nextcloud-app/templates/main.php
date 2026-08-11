<?php /** @var array $_ */ ?>
<?php
/**
 * The losos settings page — a dedicated top-level app page styled after the
 * macOS System Settings: a left sidebar of sections + a right pane of grouped
 * rows (cards). The markup is static; the live values are fetched from
 * /settings on load and "Apply" generates the overrides.nix body and POSTs it
 * to /apply (js/admin.js). Styles: css/style.css (loaded below).
 */
script('losos', 'admin');   // register/queue js/admin.js
style('losos', 'style');    // register/queue css/style.css
?>
<div id="losos-app" class="losos" data-installed="unknown">

	<!-- Left sidebar: section switcher (anchors into the right pane). -->
	<aside class="losos-sidebar" role="navigation" aria-label="<?php p($l->t('Settings sections')); ?>">
		<div class="losos-sidebar__brand">
			<span class="losos-sidebar__dot" aria-hidden="true"></span>
			<span class="losos-sidebar__title">losos</span>
		</div>
		<nav class="losos-sidebar__nav">
			<a class="losos-sidebar__item is-active" href="#losos-section-general"><?php p($l->t('General')); ?></a>
			<a class="losos-sidebar__item" href="#losos-section-nextcloud">Nextcloud</a>
			<a class="losos-sidebar__item" href="#losos-section-forgejo">Forgejo</a>
			<a class="losos-sidebar__item" href="#losos-section-storage"><?php p($l->t('Storage')); ?></a>
			<a class="losos-sidebar__item" href="#losos-section-gpu"><?php p($l->t('GPU')); ?></a>
		</nav>
	</aside>

	<!-- Right pane: the form. -->
	<main class="losos-main">

		<header class="losos-main__header">
			<h1><?php p($l->t('losos settings')); ?></h1>
			<div class="losos-status" data-state="unknown">
				<span class="losos-status__dot" aria-hidden="true"></span>
				<span class="losos-status__text"><?php p($l->t('Checking backend…')); ?></span>
			</div>
		</header>

		<!-- Backend-not-installed banner (shown when /settings reports installed:false). -->
		<p class="losos-banner losos-banner--missing" hidden>
			<?php p($l->t('The losos-ctl backend is not installed. Rebuilds are unavailable until the Haskell backend is added to the NixOS configuration.')); ?>
		</p>

		<form id="losos-form" class="losos-form" autocomplete="off">

			<!-- ── General ─────────────────────────────────────────────── -->
			<section id="losos-section-general" class="losos-section is-active">
				<h2 class="losos-section__title"><?php p($l->t('General')); ?></h2>

				<div class="losos-card">
					<div class="losos-row">
						<div class="losos-row__label">
							<span class="losos-row__title"><?php p($l->t('Hostname')); ?></span>
							<span class="losos-row__desc"><?php p($l->t('mDNS name this box advertises on the local network (Avahi).')); ?></span>
						</div>
						<div class="losos-row__control">
							<input id="losos-hostName" name="hostName" type="text" class="losos-input" value="mattbox" spellcheck="false">
						</div>
					</div>
					<div class="losos-row">
						<div class="losos-row__label">
							<span class="losos-row__title"><?php p($l->t('HTTPS')); ?></span>
							<span class="losos-row__desc"><?php p($l->t('Serve Nextcloud over HTTPS via the Nginx router. Disable for local-only setups without certificates.')); ?></span>
						</div>
						<div class="losos-row__control">
							<label class="losos-switch">
								<input id="losos-https" name="https" type="checkbox">
								<span class="losos-switch__track" aria-hidden="true"></span>
							</label>
						</div>
					</div>
				</div>
			</section>

			<!-- ── Nextcloud ───────────────────────────────────────────── -->
			<section id="losos-section-nextcloud" class="losos-section">
				<h2 class="losos-section__title">Nextcloud</h2>

				<div class="losos-card">
					<div class="losos-row">
						<div class="losos-row__label">
							<span class="losos-row__title"><?php p($l->t('Deployment mode')); ?></span>
							<span class="losos-row__desc"><?php p($l->t('Native = NixOS service (this admin app is available). AIO = rootless Podman Nextcloud All-in-One master container.')); ?></span>
						</div>
						<div class="losos-row__control">
							<div class="losos-segmented" data-name="nextcloudMode">
								<button type="button" data-value="native"><?php p($l->t('Native')); ?></button>
								<button type="button" data-value="aio" class="is-active">AIO</button>
							</div>
						</div>
					</div>
					<div class="losos-row">
						<div class="losos-row__label">
							<span class="losos-row__title"><?php p($l->t('Apache port')); ?></span>
							<span class="losos-row__desc"><?php p($l->t('Host port the AIO Apache container binds to (Nginx proxies mattbox.local here).')); ?></span>
						</div>
						<div class="losos-row__control">
							<input id="losos-aioApachePort" name="aioApachePort" type="number" class="losos-input losos-input--num" value="11000" min="1" max="65535">
						</div>
					</div>
					<div class="losos-row">
						<div class="losos-row__label">
							<span class="losos-row__title"><?php p($l->t('AIO interface port')); ?></span>
							<span class="losos-row__desc"><?php p($l->t('Host port the AIO management interface binds to.')); ?></span>
						</div>
						<div class="losos-row__control">
							<input id="losos-aioInterfacePort" name="aioInterfacePort" type="number" class="losos-input losos-input--num" value="8000" min="1" max="65535">
						</div>
					</div>
				</div>
			</section>

			<!-- ── Forgejo ────────────────────────────────────────────── -->
			<section id="losos-section-forgejo" class="losos-section">
				<h2 class="losos-section__title">Forgejo</h2>

				<div class="losos-card">
					<div class="losos-row">
						<div class="losos-row__label">
							<span class="losos-row__title"><?php p($l->t('Deployment mode')); ?></span>
							<span class="losos-row__desc"><?php p($l->t('Native = NixOS service. Container = rootless Podman (codeberg.org/forgejo/forgejo:10).')); ?></span>
						</div>
						<div class="losos-row__control">
							<div class="losos-segmented" data-name="forgejoMode">
								<button type="button" data-value="native"><?php p($l->t('Native')); ?></button>
								<button type="button" data-value="container" class="is-active">Container</button>
							</div>
						</div>
					</div>
				</div>
			</section>

			<!-- ── Storage ─────────────────────────────────────────────── -->
			<section id="losos-section-storage" class="losos-section">
				<h2 class="losos-section__title"><?php p($l->t('Storage')); ?></h2>

				<div class="losos-card">
					<div class="losos-row">
						<div class="losos-row__label">
							<span class="losos-row__title"><?php p($l->t('Share my storage')); ?></span>
							<span class="losos-row__desc"><?php p($l->t('Contribute this box’s spare storage to the Tahoe-LAFS grid. Off = keep everything private (Nextcloud only).')); ?></span>
						</div>
						<div class="losos-row__control">
							<label class="losos-switch">
								<input id="losos-sharingMyStorage" name="sharingMyStorage" type="checkbox">
								<span class="losos-switch__track" aria-hidden="true"></span>
							</label>
						</div>
					</div>
				</div>
			</section>

			<!-- ── GPU ────────────────────────────────────────────────── -->
			<section id="losos-section-gpu" class="losos-section">
				<h2 class="losos-section__title"><?php p($l->t('GPU')); ?></h2>

				<div class="losos-card">
					<div class="losos-row">
						<div class="losos-row__label">
							<span class="losos-row__title"><?php p($l->t('Hardware acceleration')); ?></span>
							<span class="losos-row__desc"><?php p($l->t('Pass /dev/dri (Intel VA-API) through to the Nextcloud container for transcoding.')); ?></span>
						</div>
						<div class="losos-row__control">
							<label class="losos-switch">
								<input id="losos-gpuEnable" name="gpuEnable" type="checkbox">
								<span class="losos-switch__track" aria-hidden="true"></span>
							</label>
						</div>
					</div>
				</div>
			</section>

			<!-- ── Footer / actions ────────────────────────────────────── -->
			<div class="losos-actions">
				<span class="losos-actions__error" role="alert" hidden></span>
				<button id="losos-apply" type="submit" class="losos-btn losos-btn--primary" disabled>
					<?php p($l->t('Apply')); ?>
				</button>
			</div>

		</form>
	</main>
</div>