<?php
/**
 * AppFramework routes for the losos admin app — a dedicated settings page (its
 * own top-navigation entry, not a panel under Admin → Settings).
 *
 *   name = "<resource>#<method>"  ->  OCA\Losos\Controller\<Resource>Controller::<method>
 *
 * Four endpoints: the page render, a first-paint GET of the current values,
 * the POST that applies the page-generated Nix code, and the rebuild-status
 * poll. The page is a GET (CSRF-exempt via @NoCSRFRequired on index); /apply is
 * a POST, so AppFramework's SecurityMiddleware enforces the request token.
 */
declare(strict_types=1);

return [
	'routes' => [
		// The macOS-style settings page itself (markup + assets).
		['name' => 'settings#index', 'url' => '/', 'verb' => 'GET'],
		// Current losos.* values parsed from overrides.nix — first paint.
		['name' => 'settings#settings', 'url' => '/settings', 'verb' => 'GET'],
		// Apply the generated overrides.nix body + trigger a rebuild.
		['name' => 'settings#apply', 'url' => '/apply', 'verb' => 'POST'],
		// Rebuild progress, polled every ~2s by the page JS.
		['name' => 'settings#status', 'url' => '/status', 'verb' => 'GET'],
	],
];