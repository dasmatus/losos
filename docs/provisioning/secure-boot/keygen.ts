#!/usr/bin/env bun
// LosOS Secure Boot key ceremony: make the db signing key on YOUR machine.
// Shim target for keygen.sh.
import { existsSync, mkdirSync, chmodSync } from "node:fs";
import { resolve } from "node:path";
import { spawnSync } from "node:child_process";

function run(cmd: string[]) {
  const p = spawnSync(cmd[0], cmd.slice(1), { stdio: ["ignore", "pipe", "pipe"], encoding: "utf8" });
  if (p.status !== 0) { process.stderr.write(p.stderr || ""); process.exit(p.status ?? 1); }
  return p.stdout;
}

const dir = Bun.argv[2] || "./losos-secure-boot";
const name = "losos-secure-boot-db";
const days = "3650";
const key = resolve(dir, `${name}.key`);
const pem = resolve(dir, `${name}.pem`);
const cer = resolve(dir, `${name}.cer`);

if (existsSync(key)) {
  process.stderr.write(`keygen: ${key} exists; refusing to make a second key (see the header)\n`);
  process.exit(1);
}
mkdirSync(dir, { recursive: true });

run(["openssl", "req", "-new", "-x509", "-newkey", "rsa:2048", "-sha256", "-nodes", "-days", days,
  "-subj", "/CN=LosOS Secure Boot db/O=LosOS/", "-keyout", key, "-out", pem]);
chmodSync(key, 0o600);
chmodSync(pem, 0o644);
run(["openssl", "x509", "-in", pem, "-outform", "DER", "-out", cer]);
chmodSync(cer, 0o644);

const fpLine = run(["openssl", "x509", "-in", pem, "-noout", "-fingerprint", "-sha256"]).trim();
const fp = fpLine.split("=")[1] ?? fpLine;
process.stdout.write(`keygen: made ${dir}/${name}.{key,pem,cer}
        SHA256 fingerprint ${fp}

Next, in this order (docs/provisioning/secure-boot/README.md has the long form):

  1. Put the private key where CI signs with it, and nowhere else:
       gh secret set SECURE_BOOT_DB_KEY --repo dasmatus/losos < ${dir}/${name}.key
  2. Publish the certificate: append ${dir}/${name}.pem to nix/keys/secure-boot-db.pem
     (below its comment) and open a pull request. From that merge on, every
     ISO CI builds on main and every release is signed with this key, the
     medium carries the certificate at EFI/losos/, and SHA256SUMS.sig on a
     release verifies against it.
  3. Keep ${dir}/${name}.key offline. Losing it means a new key and a new
     enrolment on every machine; leaking it means anyone can sign a medium
     those machines trust.
`);
