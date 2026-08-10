<?php

declare(strict_types=1);

/**
 * The admin-section entry in the left-hand settings navigation: an id, a
 * translatable name, an icon, and a sort priority. Declared in info.xml under
 * <settings><admin-section>…</admin-section></settings>.
 */

namespace OCA\Losos\Settings;

use OCP\IL10N;
use OCP\Settings\IIconizableSection;
use OCP\Settings\ISection;

class Section implements ISection, IIconizableSection
{
    public function __construct(
        private IL10N $l,
    ) {
    }

    public function getID(): string
    {
        return 'losos';
    }

    public function getName(): string
    {
        return $this->l->t('losos');
    }

    public function getPriority(): int
    {
        // Above most bundled admin sections (which sit in the 0–100 range) so
        // the appliance config is easy to find.
        return 80;
    }

    public function getIcon(): string
    {
        // Relative to the app's web root; served from css/img next to the app.
        return image_path('losos', 'app.svg');
    }
}
