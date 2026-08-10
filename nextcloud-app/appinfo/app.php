<?php
/**
 * Legacy app entrypoint. Modern Nextcloud auto-discovers the IBootstrap
 * implementation in lib/AppInfo/Application.php from the <namespace> declared
 * in info.xml, so this file only needs to make sure that class is loaded.
 *
 * Kept intentionally tiny: instantiate the Application so services registered
 * in register() are available even under the legacy `require app.php` path
 * used by some Nextcloud internals.
 */
declare(strict_types=1);

if (!class_exists(\OCP\AppFramework\App::class)) {
    // Source-loaded outside Nextcloud (phpunit/composer): do nothing.
    return;
}

require_once __DIR__ . '/../lib/AppInfo/Application.php';

/** @phan-suppress-next-line PhanUndeclaredClassInCallable */
\OC::$server->getAppContainer('losos');