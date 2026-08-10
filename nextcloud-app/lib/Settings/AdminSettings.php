<?php

declare(strict_types=1);

/**
 * The admin settings *panel* — renders the toggle UI under the losos section.
 *
 * Implements OCP\Settings\ISettings: getForm() returns a TemplateResponse
 * pointing at templates/admin.php; getSection() returns the section id from
 * Section.php; getPriority() orders it within the section. Declared in
 * info.xml under <settings><admin>…</admin></settings>.
 */

namespace OCA\Losos\Settings;

use OCP\AppFramework\Http\TemplateResponse;
use OCP\IInitialStateService;
use OCP\Settings\ISettings;

class AdminSettings implements ISettings
{
    public function __construct(
        private IInitialStateService $initialState,
    ) {
    }

    public function getForm(): TemplateResponse
    {
        // The JS reads the live state from the controller on load, so we only
        // seed an empty initial state here (kept for forward compatibility with
        // server-rendered state). settings.js does the real fetch.
        $this->initialState->provideInitialState('losos', 'state', [
            'installed' => null,
            'mode' => null,
        ]);

        return new TemplateResponse('losos', 'admin', [], TemplateResponse::RENDER_AS_ADMIN);
    }

    public function getSection(): string
    {
        return 'losos';
    }

    public function getPriority(): int
    {
        return 10;
    }
}
