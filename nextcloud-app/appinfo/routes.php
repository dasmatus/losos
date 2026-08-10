<?php
/**
 * AppFramework routes. Each entry maps a URL + verb to a controller method:
 *   name = "<resource>#<method>"  ->  OCA\Losos\Controller\<Resource>Controller::<method>
 * The controller is admin-guarded (see SettingsController) and CSRF is
 * enforced by AppFramework's @CSRFRequired annotation on the methods.
 */
declare(strict_types=1);

return [
	'routes' => [
		// Current mode + sharing flag + last rebuild state, for first paint.
		['name' => 'settings#getState', 'url' => '/state', 'verb' => 'GET'],
		// Apply a new mode (local|mesh) and trigger a rebuild. Returns the job id.
		['name' => 'settings#setMode', 'url' => '/mode', 'verb' => 'POST'],
		// Long-pollable rebuild progress for the notification/toast.
		['name' => 'settings#rebuildStatus', 'url' => '/status', 'verb' => 'GET'],
	],
];