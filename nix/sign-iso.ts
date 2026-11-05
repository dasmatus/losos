#!/usr/bin/env bun
// losos-sign-iso: sign, verify or inspect the UEFI loader inside a LosOS
// installer ISO, in place. Shim target for sign-iso.sh (repo entrypoint);
// nix/packages.nix packages sign-iso-impl.sh for the Nix-wrapped app.
import { mkdtempSync, rmSync, statSync, readFileSync, copyFileSync, chmodSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { spawnSync } from "node:child_process";

function usage(): never {
  process.stderr.write(
    "usage: losos-sign-iso sign   --key KEY --cert CERT [--out OUT.iso] IN.iso\n" +
      "       losos-sign-iso verify --cert CERT ISO\n" +
      "       losos-sign-iso show   ISO\n",
  );
  process.exit(2);
}

function run(cmd: string[], opts: { allowFail?: boolean; capture?: boolean } = {}) {
  const p = spawnSync(cmd[0], cmd.slice(1), {
    stdio: opts.capture ? ["ignore", "pipe", "pipe"] : "inherit",
    encoding: "utf8",
    maxBuffer: 64 * 1024 * 1024,
  });
  if (p.status !== 0 && !opts.allowFail) {
    if (opts.capture) process.stderr.write(p.stderr || p.stdout || "");
    process.exit(p.status ?? 1);
  }
  return p;
}

function efiOffset(iso: string): number {
  const p = run(["xorriso", "-indev", iso, "-report_el_torito", "plain"], { capture: true, allowFail: true });
  const text = (p.stdout || "") + (p.stderr || "");
  for (const line of text.split("\n")) {
    const parts = line.trim().split(/\s+/);
    // awk: $1=="El" && $4=="img" && $7=="UEFI" { print $NF; exit }
    if (parts[0] === "El" && parts[3] === "img" && parts[6] === "UEFI") {
      const lba = Number(parts[parts.length - 1]);
      if (Number.isFinite(lba)) return lba * 2048;
    }
  }
  process.stderr.write(`losos-sign-iso: ${iso} has no UEFI El Torito boot image\n`);
  process.exit(1);
}

const loader = "::/EFI/BOOT/BOOTX64.EFI";
const cmd = Bun.argv[2];
if (!cmd) usage();
const rest = Bun.argv.slice(3);
let key = "", cert = "", out = "", iso = "";
for (let i = 0; i < rest.length; i++) {
  const a = rest[i];
  if (a === "--key") key = rest[++i] ?? "";
  else if (a === "--cert") cert = rest[++i] ?? "";
  else if (a === "--out") out = rest[++i] ?? "";
  else if (a.startsWith("-")) usage();
  else { if (iso) usage(); iso = a; }
}
if (!iso) usage();

const tmp = mkdtempSync(join(tmpdir(), "losos-sign-iso-"));
try {
  if (cmd === "sign") {
    if (!key || !cert) usage();
    if (out) {
      copyFileSync(iso, out);
      chmodSync(out, statSync(out).mode | 0o200);
      iso = out;
    }
    const off = efiOffset(iso);
    run(["mcopy", "-i", `${iso}@@${off}`, loader, `${tmp}/unsigned.efi`]);
    run(["sbsign", "--key", key, "--cert", cert, "--output", `${tmp}/signed.efi`, `${tmp}/unsigned.efi`]);
    run(["sbverify", "--cert", cert, `${tmp}/signed.efi`], { capture: true });
    run(["mcopy", "-o", "-i", `${iso}@@${off}`, `${tmp}/signed.efi`, loader]);
    run(["mcopy", "-i", `${iso}@@${off}`, loader, `${tmp}/back.efi`]);
    const a = readFileSync(`${tmp}/signed.efi`);
    const b = readFileSync(`${tmp}/back.efi`);
    if (!a.equals(b)) {
      process.stderr.write("losos-sign-iso: the loader read back differs from the one written\n");
      process.exit(1);
    }
    console.log(`losos-sign-iso: signed EFI/BOOT/BOOTX64.EFI in ${iso} with ${cert}`);
  } else if (cmd === "verify") {
    if (!cert) usage();
    const off = efiOffset(iso);
    run(["mcopy", "-i", `${iso}@@${off}`, loader, `${tmp}/loader.efi`]);
    const p = run(["sbverify", "--cert", cert, `${tmp}/loader.efi`], { capture: true, allowFail: true });
    process.stdout.write(p.stdout || "");
    process.stderr.write(p.stderr || "");
    process.exit(p.status ?? 1);
  } else if (cmd === "show") {
    const off = efiOffset(iso);
    console.log(`EFI system partition image at byte offset ${off}`);
    run(["mcopy", "-i", `${iso}@@${off}`, loader, `${tmp}/loader.efi`]);
    console.log(`EFI/BOOT/BOOTX64.EFI: ${statSync(`${tmp}/loader.efi`).size} bytes`);
    const dir = run(["mdir", "-i", `${iso}@@${off}`, "::/EFI/losos"], { capture: true, allowFail: true });
    for (const line of ((dir.stdout || "") + (dir.stderr || "")).split("\n")) {
      const m = line.match(/^\s*(losos[^ ]*).*$/);
      if (m) console.log(`certificate on the medium: EFI/losos/${m[1]}`);
    }
    const sv = run(["sbverify", "--list", `${tmp}/loader.efi`], { capture: true, allowFail: true });
    process.stdout.write(sv.stdout || "");
  } else usage();
} finally {
  rmSync(tmp, { recursive: true, force: true });
}
