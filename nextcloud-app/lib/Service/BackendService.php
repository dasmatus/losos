<?php

declare(strict_types=1);

/**
 * BackendService — the bridge from PHP to the Haskell `losos-ctl` backend.
 *
 * losos-ctl is invoked through a locked-down sudoers rule (user `nextcloud` ->
 * root, NOPASSWD, command pinned to the binary path), so the PHP-FPM process
 * (running as `nextcloud`) can trigger privileged NixOS rebuilds without ever
 * holding root or a shell. The contract:
 *
 *   losos-ctl state   --json   -> {"mode":"local|mesh","sharing":bool}
 *   losos-ctl change  --mode <local|mesh>  -> {"job":"<id>"}   (async rebuild)
 *   losos-ctl status  --json   -> {"state":"idle|building|done|failed",
 *                                 "progress":0-100,"message":"…"}
 *
 * All subcommands return JSON on stdout. Non-zero exit, missing binary, or
 * malformed JSON raise BackendException; a missing/unconfigured backend raises
 * BackendNotInstalledException so the UI can show a clear message instead.
 *
 * The command executor is injectable (CommandRunner interface) so the PHPUnit
 * suite can stub the subprocess without touching sudo.
 */

namespace OCA\Losos\Service;

use OCP\IConfig;

class BackendService
{
    private const CFG_BACKEND_PATH = 'backend_path';
    private const CFG_SUDO_PATH = 'sudo_path';
    private const DEFAULT_BACKEND_PATH = '/run/current-system/sw/bin/losos-ctl';
    private const DEFAULT_SUDO_PATH = '/run/wrappers/bin/sudo';

    private CommandRunner $runner;

    public function __construct(
        private IConfig $config,
        ?CommandRunner $runner = null,
    ) {
        $this->runner = $runner ?? new ProcCommandRunner();
    }

    /** Current config + sharing flag. */
    public function getState(): array
    {
        return $this->callJson(['state', '--json']);
    }

    /**
     * Apply a new mode and trigger a rebuild. $mode must be 'local' or 'mesh'.
     * @return array{job: string}
     */
    public function setMode(string $mode): array
    {
        if (!in_array($mode, ['local', 'mesh'], true)) {
            throw new \InvalidArgumentException('mode must be "local" or "mesh"');
        }
        return $this->callJson(['change', '--mode', $mode]);
    }

    /** Rebuild progress, for the notification poll. */
    public function getRebuildStatus(): array
    {
        return $this->callJson(['status', '--json']);
    }

    /** True when the backend binary exists on disk; drives the "not installed" UI. */
    public function isInstalled(): bool
    {
        $path = $this->backendPath();
        return $path !== '' && is_file($path) && is_executable($path);
    }

    private function backendPath(): string
    {
        return (string)$this->config->getAppValue('losos', self::CFG_BACKEND_PATH, self::DEFAULT_BACKEND_PATH);
    }

    private function sudoPath(): string
    {
        return (string)$this->config->getAppValue('losos', self::CFG_SUDO_PATH, self::DEFAULT_SUDO_PATH);
    }

    /**
     * Run `sudo -n <backend> <args>`, parse JSON stdout, raise on failure.
     * @param list<string> $args subcommand + flags
     */
    private function callJson(array $args): array
    {
        $backend = $this->backendPath();
        if ($backend === '' || !is_file($backend)) {
            throw new BackendNotInstalledException(
                'losos-ctl backend is not installed at ' . $backend,
            );
        }

        $argv = array_merge([$this->sudoPath(), '-n'], [$backend], $args);
        [$stdout, $stderr, $exit] = $this->runner->run($argv);

        if ($exit !== 0) {
            throw new BackendException(sprintf(
                'losos-ctl %s failed (exit %d): %s',
                implode(' ', $args),
                $exit,
                trim($stderr) !== '' ? trim($stderr) : trim($stdout),
            ));
        }

        $decoded = json_decode(trim($stdout), true);
        if (!is_array($decoded)) {
            throw new BackendException(sprintf(
                'losos-ctl %s returned non-JSON: %s',
                implode(' ', $args),
                $stdout,
            ));
        }
        return $decoded;
    }
}
