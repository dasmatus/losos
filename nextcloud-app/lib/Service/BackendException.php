<?php

declare(strict_types=1);

namespace OCA\Losos\Service;

/** Raised on any backend error (bad exit, malformed JSON, sudo failure). */
class BackendException extends \RuntimeException
{
}
