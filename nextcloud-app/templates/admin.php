<?php /** @var array $_ */ ?>
<?php
/**
 * Admin settings panel for losos. The markup is intentionally static; the
 * interactive behaviour (fetch state, toggle mode, poll rebuild progress,
 * show notifications) lives in js/settings.js, which is loaded by the
 * <script> below via Nextcloud's asset pipeline.
 */
script('losos', 'settings');   // register/queue js/settings.js
style('losos', 'style');       // register/queue css/style.css
?>
<div id="losos-settings" class="section">
	<h2 data-anchor-name="losos"><?php p($l->t('losos')); ?></h2>

	<p class="losos-settings__intro">
		<?php p($l->t('Choose whether this box keeps its storage private (Local only) or contributes it to the Tahoe-LAFS mesh (On the mesh). Changing this triggers an automatic rebuild.')); ?>
	</p>

	<div class="losos-settings__status losos-status" data-state="unknown">
		<span class="losos-status__dot" aria-hidden="true"></span>
		<span class="losos-status__text"><?php p($l->t('Checking backend…')); ?></span>
	</div>

	<fieldset class="losos-settings__toggle" disabled>
		<legend><?php p($l->t('Storage mode')); ?></legend>
		<label class="losos-toggle">
			<input type="radio" name="losos-mode" value="local" id="losos-mode-local">
			<span class="losos-toggle__label">
				<span class="losos-toggle__title"><?php p($l->t('Local only')); ?></span>
				<span class="losos-toggle__desc"><?php p($l->t('Private Nextcloud; this box does not contribute storage to the mesh.')); ?></span>
			</span>
		</label>
		<label class="losos-toggle">
			<input type="radio" name="losos-mode" value="mesh" id="losos-mode-mesh">
			<span class="losos-toggle__label">
				<span class="losos-toggle__title"><?php p($l->t('On the mesh')); ?></span>
				<span class="losos-toggle__desc"><?php p($l->t('Contributes this box’s storage to the Tahoe-LAFS grid as a storage server.')); ?></span>
			</span>
		</label>
	</fieldset>

	<p class="losos-settings__backend-missing" hidden>
		<?php p($l->t('The losos-ctl backend is not installed. Rebuilds are unavailable until the Haskell backend is added to the NixOS configuration.')); ?>
	</p>

	<p class="losos-settings__error" role="alert" hidden></p>
</div>