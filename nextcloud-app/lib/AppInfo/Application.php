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
use OCP\INavigationManager;
use OCP\IRequest;
use OCP\IURLGenerator;
use OCP\IUser;
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
        // The dedicated settings page is reached via a top-navigation entry.
        // We register it dynamically (not via <navigations> in info.xml) so it
        // is hidden from non-admins entirely — static <navigations> shows for
        // everyone, and <types><site_admin/> only guards route access, not nav
        // visibility. Gating on isAdmin here is what keeps the icon admin-only.
        $server = $context->getServerContainer();
        $userSession = $server->get(IUserSession::class);
        $user = $userSession->getUser();
        if (!$user instanceof IUser) {
            return;
        }
        $groupManager = $server->get(IGroupManager::class);
        if (!$groupManager->isAdmin($user->getUID())) {
            return;
        }
        $urlGen = $server->get(IURLGenerator::class);
        $server->get(INavigationManager::class)->add([
            'id' => self::APP_ID,
            'name' => 'losos',
            'href' => $urlGen->linkToRoute('losos.settings.index'),
            'icon' => $urlGen->imagePath(self::APP_ID, 'app.svg'),
            // order 0 places it at the top of the app list; the appliance config
            // is the primary thing this box is for.
            'order' => 0,
        ]);
    }
}
