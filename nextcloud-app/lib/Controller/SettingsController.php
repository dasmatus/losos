<?php

declare(strict_types=1);

/**
 * Controller for the losos admin app — a *dedicated* settings page (its own
 * navigation entry, no longer a panel buried under Admin → Settings).
 *
 * Four endpoints (registered in appinfo/routes.php):
 *   GET  /         — render the macOS-style settings page (markup + assets)
 *   GET  /settings — current losos.* values parsed from overrides.nix (first paint)
 *   POST /apply    — apply the Nix code the page generated + trigger a rebuild
 *   GET  /status   — rebuild progress (polled by the page JS)
 *
 * The page JS renders the form from /settings, builds the overrides.nix body
 * from the form values, and POSTs it to /apply — "PHP sends the Nix code to
 * the Haskell server". All endpoints are admin-only: every call re-checks
 * admin membership (defense in depth on top of <types><site_admin/>). POST is
 * CSRF-protected by AppFramework's SecurityMiddleware (no @NoCSRFRequired on
 * apply, so the request token is required).
 */

namespace OCA\Losos\Controller;

use OCA\Losos\Service\BackendException;
use OCA\Losos\Service\BackendNotInstalledException;
use OCA\Losos\Service\BackendService;
use OCP\AppFramework\Controller;
use OCP\AppFramework\Http;
use OCP\AppFramework\Http\DataResponse;
use OCP\AppFramework\Http\TemplateResponse;
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

    /**
     * The dedicated losos settings page. Renders templates/main.php (the
     * macOS-style UI); the live values + actions are handled by the JS via the
     * other endpoints, so the template only carries markup + assets.
     *
     * @NoCSRFRequired
     */
    public function index(): TemplateResponse
    {
        if (!$this->isAdmin()) {
            // Non-admins shouldn't reach here (the nav is site_admin), but if
            // they do, refuse rather than render the privileged form.
            return new TemplateResponse('losos', 'denied', [], TemplateResponse::RENDER_AS_USER);
        }
        return new TemplateResponse('losos', 'main', [], TemplateResponse::RENDER_AS_USER);
    }

    /** GET /settings — current losos.* values, for the page's first paint. */
    public function settings(): DataResponse
    {
        if (!$this->isAdmin()) {
            return $this->forbidden();
        }
        if (!$this->backend->isInstalled()) {
            return new DataResponse(['installed' => false], Http::STATUS_OK);
        }
        try {
            return new DataResponse(
                array_merge(['installed' => true], $this->backend->getSettings()),
                Http::STATUS_OK,
            );
        } catch (BackendException $e) {
            return $this->backendError($e);
        }
    }

    /**
     * POST /apply — apply the generated overrides.nix body + trigger a rebuild.
     * The Nix code is the `nix` form field; the backend owns validation + the
     * atomic file rewrite + spawning nixos-rebuild. We re-check the obvious
     * "references losos.*" guard here too so a malformed request never reaches
     * sudo.
     */
    public function apply(string $nix = ''): DataResponse
    {
        if (!$this->isAdmin()) {
            return $this->forbidden();
        }
        if (trim($nix) === '' || stripos($nix, 'losos.') === false) {
            return new DataResponse(
                ['error' => 'invalid nix config: must reference losos.* options'],
                Http::STATUS_BAD_REQUEST,
            );
        }
        try {
            $result = $this->backend->apply($nix);
            return new DataResponse($result, Http::STATUS_ACCEPTED);
        } catch (BackendNotInstalledException $e) {
            return new DataResponse(
                ['error' => 'backend not installed', 'detail' => $e->getMessage()],
                Http::STATUS_CONFLICT,
            );
        } catch (BackendException $e) {
            return $this->backendError($e);
        }
    }

    /** GET /status — rebuild progress, polled every ~2s by the page JS. */
    public function status(): DataResponse
    {
        if (!$this->isAdmin()) {
            return $this->forbidden();
        }
        if (!$this->backend->isInstalled()) {
            return new DataResponse(['installed' => false], Http::STATUS_OK);
        }
        try {
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
        return new DataResponse(
            ['error' => 'backend', 'detail' => $e->getMessage()],
            Http::STATUS_INTERNAL_SERVER_ERROR,
        );
    }
}
