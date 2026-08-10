<?php

declare(strict_types=1);

namespace OCA\Losos\Service;

/**
 * Default proc_open-based CommandRunner. Each argv element is shell-escaped.
 *
 * Reads stdout and stderr concurrently (non-blocking + stream_select) under a
 * deadline. This fixes two failure modes a naive proc_open has here:
 *   1. No timeout: a hung losos-ctl would block a PHP-FPM worker forever, and
 *      the 2s status-poll loop multiplies that into a full Nextcloud DoS.
 *   2. Pipe deadlock: reading stdout to EOF before stderr means a subprocess
 *      that writes >~64KB to stderr blocks on the full pipe while we block on
 *      stdout — a classic deadlock. Concurrent read avoids it.
 */
final class ProcCommandRunner implements CommandRunner
{
    private float $timeoutSec;

    public function __construct(float $timeoutSec = 30.0)
    {
        $this->timeoutSec = $timeoutSec;
    }

    public function run(array $argv): array
    {
        $cmd = implode(' ', array_map('escapeshellarg', $argv));
        $descriptors = [
            0 => ['pipe', 'r'],
            1 => ['pipe', 'w'],
            2 => ['pipe', 'w'],
        ];
        $proc = @proc_open($cmd, $descriptors, $pipes);
        if (!is_resource($proc)) {
            return ['', 'failed to start subprocess', 127];
        }
        fclose($pipes[0]);
        unset($pipes[0]);

        stream_set_blocking($pipes[1], false);
        stream_set_blocking($pipes[2], false);

        $stdout = '';
        $stderr = '';
        $deadline = microtime(true) + $this->timeoutSec;
        $eof = ['out' => false, 'err' => false];

        while (!$eof['out'] || !$eof['err']) {
            if (microtime(true) >= $deadline) {
                proc_terminate($proc, 9);
                return [$stdout, 'subprocess timed out after ' . $this->timeoutSec . 's', 124];
            }
            $read = [];
            if (!$eof['out']) {
                $read[] = $pipes[1];
            }
            if (!$eof['err']) {
                $read[] = $pipes[2];
            }
            $write = null;
            $except = null;
            // 1s poll so we re-check the deadline each iteration; losos-ctl is
            // fast in practice.
            $changed = @stream_select($read, $write, $except, 1, 0);
            if ($changed === false) {
                break;
            }
            foreach ($read as $pipe) {
                $chunk = (string) fread($pipe, 65536);
                $isOut = $pipe === $pipes[1];
                if ($isOut) {
                    $stdout .= $chunk;
                    if ($chunk === '' && feof($pipe)) {
                        $eof['out'] = true;
                    }
                } else {
                    $stderr .= $chunk;
                    if ($chunk === '' && feof($pipe)) {
                        $eof['err'] = true;
                    }
                }
            }
        }

        fclose($pipes[1]);
        fclose($pipes[2]);
        $exit = proc_close($proc);
        return [$stdout, $stderr, $exit];
    }
}
