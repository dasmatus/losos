<?php

declare(strict_types=1);

namespace OCA\Losos\Tests;

use OCA\Losos\Service\BackendException;
use OCA\Losos\Service\BackendNotInstalledException;
use OCA\Losos\Service\BackendService;
use OCA\Losos\Service\CommandRunner;
use OCP\IConfig;
use PHPUnit\Framework\TestCase;

/**
 * In-memory IConfig that only stores app values (the only side BackendService
 * uses). Backed by an array so tests are hermetic and need no Nextcloud.
 */
final class InMemoryConfig implements IConfig
{
    private array $app = [];

    public function __construct(array $values = [])
    {
        $this->app = $values;
    }

    public function getAppValue(string $appId, string $key, string $default = ''): string
    {
        return $this->app[$appId][$key] ?? $default;
    }

    public function setAppValue(string $appId, string $key, string $value): void
    {
        $this->app[$appId][$key] = $value;
    }

    public function getUserValue(string $userId, string $appId, string $key, string $default = ''): string
    {
        return $default;
    }
}

/**
 * Stub CommandRunner: returns a canned [stdout, stderr, exit] and records the
 * last argv it was asked to run, so assertions can verify the sudo invocation.
 */
final class StubRunner implements CommandRunner
{
    public array $lastArgv = [];
    public ?string $lastStdin = null;
    public array $next = ['', '', 0];

    public function __construct(array $next = ['', '', 0])
    {
        $this->next = $next;
    }

    public function run(array $argv, ?string $stdin = null): array
    {
        $this->lastArgv = $argv;
        $this->lastStdin = $stdin;
        return $this->next;
    }
}

class BackendServiceTest extends TestCase
{
    private string $bin;

    protected function setUp(): void
    {
        // A real, executable file so BackendService's is_file() guard passes.
        $this->bin = tempnam(sys_get_temp_dir(), 'lososctl_');
        chmod($this->bin, 0755);
    }

    protected function tearDown(): void
    {
        @unlink($this->bin);
    }

    private function cfg(): InMemoryConfig
    {
        return new InMemoryConfig(['losos' => [
            'backend_path' => $this->bin,
            'sudo_path' => '/run/wrappers/bin/sudo',
        ]]);
    }

    public function testGetStateParsesJsonAndUsesSudo(): void
    {
        $runner = new StubRunner(['{"mode":"local","sharing":false}', '', 0]);
        $svc = new BackendService($this->cfg(), $runner);

        $state = $svc->getState();

        $this->assertSame(['mode' => 'local', 'sharing' => false], $state);
        $this->assertSame(
            ['/run/wrappers/bin/sudo', '-n', $this->bin, 'state', '--json'],
            $runner->lastArgv,
        );
    }

    public function testSetModeMeshSendsChangeCommand(): void
    {
        $runner = new StubRunner(['{"job":"abc-1"}', '', 0]);
        $svc = new BackendService($this->cfg(), $runner);

        $result = $svc->setMode('mesh');

        $this->assertSame(['job' => 'abc-1'], $result);
        $this->assertSame(
            ['/run/wrappers/bin/sudo', '-n', $this->bin, 'change', '--mode', 'mesh'],
            $runner->lastArgv,
        );
    }

    public function testSetModeRejectsInvalidMode(): void
    {
        $svc = new BackendService($this->cfg(), new StubRunner());
        $this->expectException(\InvalidArgumentException::class);
        $svc->setMode('bogus');
    }

    public function testGetRebuildStatusParsesProgress(): void
    {
        $runner = new StubRunner(['{"state":"building","progress":42,"message":"evaluating"}', '', 0]);
        $svc = new BackendService($this->cfg(), $runner);

        $this->assertSame(
            ['state' => 'building', 'progress' => 42, 'message' => 'evaluating'],
            $svc->getRebuildStatus(),
        );
    }

    public function testGetSettingsCallsSettingsSubcommand(): void
    {
        $payload = '{"sharingMyStorage":true,"nextcloudMode":"aio","forgejoMode":"container","hostName":"mattbox","https":false,"gpuEnable":true,"aioApachePort":11000,"aioInterfacePort":8000}';
        $runner = new StubRunner([$payload, '', 0]);
        $svc = new BackendService($this->cfg(), $runner);

        $settings = $svc->getSettings();

        $this->assertSame('aio', $settings['nextcloudMode']);
        $this->assertSame('mattbox', $settings['hostName']);
        $this->assertSame(
            ['/run/wrappers/bin/sudo', '-n', $this->bin, 'settings', '--json'],
            $runner->lastArgv,
        );
    }

    public function testApplyPipesNixCodeToApplySubcommand(): void
    {
        $nix = "{ ... }:\n{\n  losos.hostName = \"box2\";\n}\n";
        $runner = new StubRunner(['{"job":"abc-9"}', '', 0]);
        $svc = new BackendService($this->cfg(), $runner);

        $result = $svc->apply($nix);

        $this->assertSame(['job' => 'abc-9'], $result);
        $this->assertSame(
            ['/run/wrappers/bin/sudo', '-n', $this->bin, 'apply'],
            $runner->lastArgv,
        );
        $this->assertSame($nix, $runner->lastStdin);
    }

    public function testNonZeroExitRaisesBackendException(): void
    {
        $runner = new StubRunner(['', 'sudo: a password is required', 1]);
        $svc = new BackendService($this->cfg(), $runner);

        $this->expectException(BackendException::class);
        $svc->getState();
    }

    public function testNonJsonOutputRaisesBackendException(): void
    {
        $runner = new StubRunner(['not json at all', '', 0]);
        $svc = new BackendService($this->cfg(), $runner);

        $this->expectException(BackendException::class);
        $svc->getState();
    }

    public function testMissingBackendRaisesNotInstalled(): void
    {
        $cfg = new InMemoryConfig(['losos' => ['backend_path' => '/no/such/bin']]);
        $svc = new BackendService($cfg, new StubRunner());

        $this->assertFalse($svc->isInstalled());
        $this->expectException(BackendNotInstalledException::class);
        $svc->getState();
    }

    public function testIsInstalledTrueForExecutableFile(): void
    {
        $svc = new BackendService($this->cfg(), new StubRunner());
        $this->assertTrue($svc->isInstalled());
    }
}
