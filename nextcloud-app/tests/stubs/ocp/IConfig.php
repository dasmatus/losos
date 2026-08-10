<?php

declare(strict_types=1);

/**
 * Minimal OCP\IConfig stub so the unit suite runs without a full Nextcloud
 * install. Only declares the methods BackendService actually calls.
 * (The real interface lives in Nextcloud's lib/private/OCP/IConfig.php.)
 */

namespace OCP;

interface IConfig
{
    public function getAppValue(string $appId, string $key, string $default = ''): string;

    public function setAppValue(string $appId, string $key, string $value): void;

    public function getUserValue(string $userId, string $appId, string $key, string $default = ''): string;
}
