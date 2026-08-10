<?php

declare(strict_types=1);

/**
 * losos application bootstrap.
 *
 * Implements IBootstrap so Nextcloud auto-discovers it from the <namespace>
 * declared in appinfo/info.xml (no manual registration needed). register()
 * wires the backend service + controller into the AppFramework container;
 * boot() is empty (the backend lives outside PHP, rebuild progress is polled).
 */

namespace OCA\Losos\AppInfo;

use OCA\Losos\Controller\SettingsController;
use OCA\Losos\Service\BackendService;
use OCP\AppFramework\App;
use OCP\AppFramework\Bootstrap\IBootContext;
use OCP\AppFramework\Bootstrap\IBootstrap;
use OCP\AppFramework\Bootstrap\IRegistrationContext;
use OCP\IConfig;
use OCP\IGroupManager;
use OCP\IRequest;
use OCP\IUserSession;

class Application extends App implements IBootstrap
{
    public const APP_ID = 'losos';

    public function __construct(array $urlParams = [])
    {
        parent::__construct(self::APP_ID, $urlParams);
    }

    public function register(IRegistrationContext $context): void
    {
        // Bridge to the Haskell `losos-ctl` CLI (invoked through sudo). Rebuilt
        // per request, but the binary path is resolved once from app config.
        $context->registerService(BackendService::class, static function ($c): BackendService {
            return new BackendService($c->get(IConfig::class));
        });

        // Admin AJAX controller. AppFramework resolves the route short-name
        // `settings` to OCA\Losos\Controller\SettingsController by FQCN.
        $context->registerService(SettingsController::class, static function ($c): SettingsController {
            return new SettingsController(
                self::APP_ID,
                $c->get(IRequest::class),
                $c->get(IUserSession::class),
                $c->get(BackendService::class),
                $c->get(IGroupManager::class),
            );
        });
    }

    public function boot(IBootContext $context): void
    {
        // Nothing to boot: the backend runs as a separate root service, and
        // rebuild progress is polled on demand from the settings UI.
    }
}
