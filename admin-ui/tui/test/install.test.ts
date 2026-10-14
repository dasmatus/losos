/* install.sh under dash, with a fake curl that serves files from a folder
 * and a fake uname, so every branch runs without the network. */

import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { chmodSync, existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { after, before, describe, it } from "node:test";
import { fileURLToPath } from "node:url";

const SCRIPT = fileURLToPath(new URL("../install.sh", import.meta.url));
const shell = spawnSync("dash", ["-c", "true"]).status === 0 ? "dash" : "sh";

describe("install.sh", () => {
  let dir: string;
  let served: string;
  let bin: string;

  const program = "#!/bin/sh\necho 'losos 9.9.9'\n";
  const sum = (text: string): string => createHash("sha256").update(text).digest("hex");

  const run = (env: Record<string, string>) =>
    spawnSync(shell, [SCRIPT], {
      encoding: "utf8",
      env: { PATH: `${bin}:/usr/bin:/bin`, HOME: join(dir, "home"), FAKE_SERVED: served, ...env },
    });

  before(() => {
    dir = mkdtempSync(join(tmpdir(), "losos-install-"));
    served = join(dir, "served");
    bin = join(dir, "bin");
    mkdirSync(join(served, "main", "x86_64"), { recursive: true });
    mkdirSync(bin);
    writeFileSync(join(served, "main", "x86_64", "losos"), program);
    writeFileSync(join(served, "main", "x86_64", "SHA256SUMS"), `${sum(program)}  losos\n${"0".repeat(64)}  other\n`);
    // curl -fsSL --proto '=https' --tlsv1.2 -o <file> <url>
    writeFileSync(
      join(bin, "curl"),
      [
        "#!/bin/sh",
        'for a; do last=$a; done',
        'while [ $# -gt 0 ]; do [ "$1" = -o ] && out=$2; shift; done',
        'path=${last#https://proxy.losos.dasmat.us/updates/}',
        'echo "$last" >> "$FAKE_SERVED/requests"',
        '[ -f "$FAKE_SERVED/$path" ] || exit 22',
        'cp "$FAKE_SERVED/$path" "$out"',
      ].join("\n"),
    );
    writeFileSync(join(bin, "uname"), '#!/bin/sh\ncase "$1" in -s) echo "${FAKE_OS:-Linux}";; -m) echo "${FAKE_ARCH:-x86_64}";; esac\n');
    // The suite may run as root; a fake id keeps it out of /usr/local/bin.
    writeFileSync(join(bin, "id"), '#!/bin/sh\necho "${FAKE_UID:-1000}"\n');
    writeFileSync(join(bin, "sudo"), "#!/bin/sh\necho sudo was called >&2\nexit 99\n");
    for (const f of ["curl", "uname", "id", "sudo"]) chmodSync(join(bin, f), 0o755);
  });
  after(() => rmSync(dir, { recursive: true, force: true }));

  it("installs into ~/.local/bin after checking the checksum", () => {
    const result = run({});
    assert.equal(result.status, 0, result.stderr);
    const installed = join(dir, "home", ".local", "bin", "losos");
    assert.equal(readFileSync(installed, "utf8"), program);
    assert.match(result.stdout, /checksum [0-9a-f]{64} matches/);
    assert.match(result.stdout, /losos 9\.9\.9/);
    assert.match(result.stdout, /not on your PATH/);
    assert.ok(!result.stderr.includes("sudo"));
    const requests = readFileSync(join(served, "requests"), "utf8");
    assert.match(requests, /^https:\/\/proxy\.losos\.dasmat\.us\/updates\/main\/x86_64\/SHA256SUMS$/m);
    assert.match(requests, /^https:\/\/proxy\.losos\.dasmat\.us\/updates\/main\/x86_64\/losos$/m);
  });

  it("warns when secret-tool is missing", () => {
    const result = run({});
    assert.match(result.stdout, /secret-tool is not installed/);
    assert.match(result.stdout, /libsecret-tools/);
    assert.match(result.stdout, /dnf install libsecret/);
    assert.match(result.stdout, /pacman -S libsecret/);
  });

  it("refuses another architecture or system with a sentence", () => {
    const arm = run({ FAKE_ARCH: "aarch64" });
    assert.equal(arm.status, 1);
    assert.match(arm.stderr, /built for x86_64 only, and this computer is aarch64/);
    const mac = run({ FAKE_OS: "Darwin" });
    assert.equal(mac.status, 1);
    assert.match(mac.stderr, /Linux only/);
  });

  it("stops on a checksum that does not match", () => {
    const home = join(dir, "home2");
    writeFileSync(join(served, "main", "x86_64", "losos"), `${program}# tampered\n`);
    try {
      const result = run({ HOME: home });
      assert.equal(result.status, 1);
      assert.match(result.stderr, /does not match its checksum/);
      assert.equal(existsSync(join(home, ".local", "bin", "losos")), false);
    } finally {
      writeFileSync(join(served, "main", "x86_64", "losos"), program);
    }
  });

  it("takes the channel from LOSOS_CHANNEL", () => {
    const result = run({ LOSOS_CHANNEL: "nightly" });
    assert.equal(result.status, 1);
    assert.match(result.stderr, /updates\/nightly\/x86_64\/SHA256SUMS/);
  });
});
