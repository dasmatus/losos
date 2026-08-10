<?php

declare(strict_types=1);

namespace OCA\Losos\Service;

/** Raised when the backend binary / sudoers rule is not in place yet. */
class BackendNotInstalledException extends BackendException
{
}
