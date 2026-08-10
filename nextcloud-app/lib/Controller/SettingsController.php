<?php

declare(strict_types=1);

/**
 * Admin AJAX controller for the losos settings panel.
 *
 * Three endpoints (registered in appinfo/routes.php):
 *   GET  /state   — current mode + sharing flag + rebuild state (first paint)
 *   POST /mode    — apply a new mode and trigger a rebuild (returns job id)
 *   GET  /status   — rebuild progress (polled by the settings JS)
 *
 * All methods are admin-only: every call re-checks that the current user is a
 * member of the admin group, regardless of the <types><site_admin/> tag in
 * info.xml (defense in depth). POST is CSRF-protected by AppFramework's
 * default SecurityMiddleware (state-changing verbs require a request token
 * unless a @NoCSRFRequired annotation opts out, which none here do).
 */

namespace OCA\Losos\Controller;

use OCA\Losos\Service\BackendException;
use OCA\Losos\Service\BackendNotInstalledException;
use OCA\Losos\Service\BackendService;
use OCP\AppFramework\Controller;
use OCP\AppFramework\Http;
use OCP\AppFramework\Http\DataResponse;
use OCP\IGroupManager;
use OCP\IRequest;
use OCP\IUser;
use OCP\IUserSession;

class SettingsController extends Controller
{
    public function __construct(
        string $appName,
        IRequest $request,
        private IUserSession $userSession,
        private BackendService $backend,
        private IGroupManager $groupManager,
    ) {
        parent::__construct($appName, $request);
    }

    /** GET /state — current mode + sharing flag, plus install status. */
    public function getState(): DataResponse
    {
        if (!$this->isAdmin()) {
            return $this->forbidden();
        }
        if (!$this->backend->isInstalled()) {
            return new DataResponse([
                'installed' => false,
                'mode' => null,
                'sharing' => null,
            ], Http::STATUS_OK);
        }
        try {
            $state = $this->backend->getState();
            return new DataResponse(array_merge(['installed' => true], $state), Http::STATUS_OK);
        } catch (BackendException $e) {
            return $this->backendError($e);
        }
    }

    /** POST /mode — apply mode (local|mesh) and trigger a rebuild. */
    public function setMode(string $mode): DataResponse
    {
        if (!$this->isAdmin()) {
            return $this->forbidden();
        }
        if (!in_array($mode, ['local', 'mesh'], true)) {
            return new DataResponse(['error' => 'invalid mode'], Http::STATUS_BAD_REQUEST);
        }
        try {
            $result = $this->backend->setMode($mode);
            return new DataResponse($result, Http::STATUS_ACCEPTED);
        } catch (BackendNotInstalledException $e) {
            return new DataResponse(['error' => 'backend not installed', 'detail' => $e->getMessage()], Http::STATUS_CONFLICT);
        } catch (BackendException $e) {
            return $this->backendError($e);
        } catch (\InvalidArgumentException $e) {
            return new DataResponse(['error' => $e->getMessage()], Http::STATUS_BAD_REQUEST);
        }
    }

    /** GET /status — rebuild progress, polled every ~2s by the settings JS. */
    public function rebuildStatus(): DataResponse
    {
        if (!$this->isAdmin()) {
            return $this->forbidden();
        }
        if (!$this->backend->isInstalled()) {
            return new DataResponse(['installed' => false], Http::STATUS_OK);
        }
        try {
            // Mirror getState(): merge `installed` so the JS poller's
            // `if (!data.installed)` guard doesn't abort polling on the happy
            // path (the raw backend object has no `installed` key).
            return new DataResponse(
                array_merge(['installed' => true], $this->backend->getRebuildStatus()),
                Http::STATUS_OK,
            );
        } catch (BackendException $e) {
            return $this->backendError($e);
        }
    }

    private function isAdmin(): bool
    {
        $user = $this->userSession->getUser();
        return $user instanceof IUser && $this->groupManager->isAdmin($user->getUID());
    }

    private function forbidden(): DataResponse
    {
        return new DataResponse(['error' => 'admin only'], Http::STATUS_FORBIDDEN);
    }

    private function backendError(BackendException $e): DataResponse
    {
        return new DataResponse(['error' => 'backend', 'detail' => $e->getMessage()], Http::STATUS_INTERNAL_SERVER_ERROR);
    }
}
