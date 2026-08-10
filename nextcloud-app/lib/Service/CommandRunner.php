<?php

declare(strict_types=1);

namespace OCA\Losos\Service;

/**
 * Runs a command and returns [stdout, stderr, exitCode]. The default
 * implementation (ProcCommandRunner) uses proc_open; tests substitute a stub so
 * the suite never touches sudo.
 */
interface CommandRunner
{
    /** @return array{0:string,1:string,2:int} [stdout, stderr, exitCode] */
    public function run(array $argv): array;
}
