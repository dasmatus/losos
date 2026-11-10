#!/usr/bin/env node
// LosOS: trust the certificate of the box "@HOST@" on this computer.
//
// What this script does, and all it does:
//
//   1. writes the certificate at the bottom of this file to a temporary file;
//   2. checks that its SHA-256 fingerprint is
//        @FINGERPRINT@
//      which is the fingerprint the setup page shows you;
//   3. adds it to the certificate stores your browsers read, for your user:
//        macOS    your login keychain (Safari, Chrome, Firefox read it)
//        Linux    the NSS store Chrome and Chromium use, and every Firefox
//                 profile on this account; the system store too if run as root
//   4. deletes the temporary file.
//
// It installs no software, changes no other setting, sends nothing anywhere
// and needs no password unless your operating system asks for one itself
// (macOS does, to change what your keychain trusts).
//
// The box served this script from its own address on your network; nothing
// in it came from the internet. To undo it later, delete the certificate
// named "LosOS @HOST@" from the same store.
//
// ── The shape of this file, and why plain node runs it ────────────────────
//
// modules/setup.nix (renderInstaller) turns this template into the single
// document served at GET /setup/trust.ts: a plain `sed` in the box's publish
// unit. That rules out a build step on the box — the rendered artifact must
// be this file, statically — and it rules out TypeScript's own syntax,
// because the owner runs the rendered document straight out of the pipe:
//
//     curl -fsSL http://<address>/setup/trust.ts | node
//
// Node detects module syntax on stdin, so plain-ESM JavaScript arrives and
// executes; a type annotation is the one thing its parser stops on. So the
// template is written in TypeScript style with every annotation in JSDoc —
// typed source for Bun and the editors, JavaScript for the pipe. It
// deliberately uses only node's own stdlib: it runs on an end-user machine,
// not where Bun's modules are.
//
// The .ps1 twin of this file (modules/setup/trust.ps1) keeps its PowerShell
// contract; the two are named in state.json's certificate.install
// (`node` and `ps1`), and every hand-off — wizard link, test, handbook line —
// goes through that document, so a URL rename never disagrees with what the
// wizard shows.

import {
  mkdtempSync,
  rmSync,
  writeFileSync,
  mkdirSync,
  readdirSync,
  statSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join, basename } from "node:path";
import { spawnSync } from "node:child_process";

const HOST = "@HOST@";
const FINGERPRINT = "@FINGERPRINT@";
const NAME = `LosOS ${HOST}`;

/** `$HOME`, and whatever the shell's empty expansion gave the tools it ran,
 *  which is where the stores below are looked up from. */
const HOME = process.env.HOME || "";

const say = (...lines) => console.log(...lines);
const warn = (...lines) => console.error(...lines);

/** `command -v cmd` — each availability check the shell version made, made
 *  the same way. Arguments never go through the shell otherwise. */
function have(command) {
  return spawnSync("sh", ["-c", `command -v ${command}`], {
    stdio: "ignore",
  }).status === 0;
}

const tmp = mkdtempSync(join(tmpdir(), "losos-trust-"));
for (const sig of ["exit", "SIGINT", "SIGTERM"]) {
  // The shell's `trap 'rm -rf "$tmp"' EXIT INT TERM`.
  process.on(sig, () => rmSync(tmp, { recursive: true, force: true }));
}
const pem = join(tmp, "losos-ca.crt");

// The certificate is the raw PEM block below; modules/setup.nix's sed
// replaces the single `@PEM@` line with the certificate file's bytes. The
// leading newline the template's formatting adds is trimmed off, so what
// this script installs is byte for byte what GET /setup/losos-ca.crt serves
// (nix/tests/setup.nix extracts the block back out and cmp's it).
const pemBlock = deleteLeadingEmptyLine(`
@PEM@
`);

/** The newline between the opening backtick and the marker line is the one
 *  byte the template adds that the shell heredoc did not have. */
function deleteLeadingEmptyLine(text) {
  return text.replace(/^\n/, "");
}

writeFileSync(pem, pemBlock, { encoding: "utf8" });

say("Trusting the certificate of " + HOST + " on this computer.");
say("Fingerprint (SHA-256): " + FINGERPRINT);
say("Compare it with the one the setup page shows. If they differ, press Ctrl-C.");

// With openssl around, the PEM below is checked against the fingerprint
// above before anything is trusted. A download cut short, or a box that is
// not the one the setup page described, stops here.
if (have("openssl")) {
  const got = spawnSync("openssl", ["x509", "-in", pem, "-noout", "-fingerprint", "-sha256"], {
    // The shell kept the failure silent (`2>/dev/null`) and read only
    // stdout, which is the `sha256 Fingerprint=…` display string.
    stdio: ["ignore", "pipe", "ignore"],
    encoding: "utf8",
  });
  const gotDisplay = (got.stdout || "").trim().split("=", 2)[1] || "";
  if (gotDisplay !== FINGERPRINT) {
    warn(`The certificate in this script does not match its own fingerprint (got '${gotDisplay}').`);
    warn("Nothing was changed. Reload the setup page and try again.");
    process.exit(1);
  }
}

let done_any = 0;
const mark = (label) => {
  done_any = 1;
  say("  added to " + label);
};

// ── macOS: the login keychain ─────────────────────────────────────────────
// Safari, Chrome and Firefox all read trust settings from it. The keychain
// asks for the account password in a dialog: that is macOS, not this
// script.
function trust_macos() {
  const keychain = join(HOME, "Library/Keychains/login.keychain-db");
  const run = spawnSync(
    "security",
    ["add-trusted-cert", "-r", "trustRoot", "-k", keychain, pem],
    { stdio: "inherit" },
  );
  if (run.status === 0) {
    mark("your login keychain");
  } else {
    warn("Could not add it to the login keychain.");
    warn(`Open Keychain Access, drag ${pem} into 'login', then set its trust to Always Trust.`);
    process.exit(1);
  }
}

// ── Linux: NSS databases ──────────────────────────────────────────────────
// Chrome, Chromium, Brave and Edge read ~/.pki/nssdb. Firefox keeps one
// database per profile (cert9.db), including the snap and flatpak homes.
function nss_add(dir, label) {
  if (!isFile(join(dir, "cert9.db"))) {
    mkdirSync(dir, { recursive: true });
    const init = spawnSync("certutil", ["-d", `sql:${dir}`, "-N", "--empty-password"], {
      stdio: "inherit",
    });
    if (init.status !== 0) process.exit(init.status ?? 1);
  }
  // -D first so running the script twice (a new certificate after a
  // reinstall) replaces the entry instead of stacking a second one under
  // the same name. Its failure is silenced exactly as the shell's was.
  spawnSync("certutil", ["-d", `sql:${dir}`, "-D", "-n", NAME], {
    stdio: ["ignore", "inherit", "ignore"],
  });
  const add = spawnSync("certutil", ["-d", `sql:${dir}`, "-A", "-t", "C,,", "-n", NAME, "-i", pem], {
    stdio: "inherit",
  });
  if (add.status !== 0) process.exit(add.status ?? 1);
  mark(label);
}

/** `[ -d "$path" ]`. */
function isDir(path) {
  try {
    return statSync(path).isDirectory();
  } catch {
    return false;
  }
}

/** `[ -f "$path" ]`. */
function isFile(path) {
  try {
    return statSync(path).isFile();
  } catch {
    return false;
  }
}

/** The directories in `$base` that carry a cert9.db, in the order the shell
 *  walked them. */
function profiles_in(base) {
  if (!isDir(base)) return [];
  const found = [];
  for (const entry of readdirSync(base)) {
    const dir = join(base, entry);
    if (isFile(join(dir, "cert9.db"))) found.push(dir);
  }
  return found;
}

function firefox_profiles() {
  const found = [];
  for (const base of [
    join(HOME, ".mozilla/firefox"),
    join(HOME, "snap/firefox/common/.mozilla/firefox"),
    join(HOME, ".var/app/org.mozilla.firefox/.mozilla/firefox"),
    join(HOME, ".librewolf"),
  ]) {
    for (const p of profiles_in(base)) found.push(p);
  }
  return found;
}

// ── Linux: the system store, only when already root ───────────────────────
// curl, wget, package managers and anything else that is not a browser read
// it. Never escalates: a script that reaches for sudo is one you should not
// have piped into node.
function trust_system() {
  if (have("update-ca-certificates")) {
    // Debian, Ubuntu and their derivatives. install -m 0644 wrote the same
    // copy; the write carries the mode itself.
    writeFileSync(`/usr/local/share/ca-certificates/losos-${HOST}.crt`, pemBlock, {
      encoding: "utf8",
      mode: 0o644,
    });
    const run = spawnSync("update-ca-certificates", [], { stdio: ["ignore", "ignore", "inherit"] });
    if (run.status !== 0) process.exit(run.status ?? 1);
    mark("the system store (/usr/local/share/ca-certificates)");
  } else if (have("update-ca-trust")) {
    // Fedora, RHEL, openSUSE.
    writeFileSync(`/etc/pki/ca-trust/source/anchors/losos-${HOST}.crt`, pemBlock, {
      encoding: "utf8",
      mode: 0o644,
    });
    const run = spawnSync("update-ca-trust", [], { stdio: ["ignore", "ignore", "inherit"] });
    if (run.status !== 0) process.exit(run.status ?? 1);
    mark("the system store (/etc/pki/ca-trust/source/anchors)");
  } else if (have("trust")) {
    // Arch.
    const run = spawnSync("trust", ["anchor", "--store", pem], { stdio: "inherit" });
    if (run.status !== 0) process.exit(run.status ?? 1);
    mark("the system store (trust anchor)");
  } else {
    warn("  no system certificate tool found here; the system store was left alone.");
  }
}

function trust_linux() {
  if (!have("certutil")) {
    warn("This needs 'certutil' from NSS, which is not installed.");
    warn("  Debian/Ubuntu: sudo apt install libnss3-tools");
    warn("  Fedora:        sudo dnf install nss-tools");
    warn("  Arch:          sudo pacman -S nss");
    warn("  NixOS:         nix shell nixpkgs#nssTools");
    warn("Then run this again, or install the certificate from the setup page by hand.");
    process.exit(1);
  }
  nss_add(join(HOME, ".pki/nssdb"), "Chrome and Chromium (~/.pki/nssdb)");
  for (const p of firefox_profiles()) {
    nss_add(p, `Firefox profile ${basename(p)}`);
  }
  if (process.getuid() === 0) {
    trust_system();
  }
}

switch (process.platform) {
  case "darwin":
    trust_macos();
    break;
  case "linux":
    trust_linux();
    break;
  default:
    warn("This script knows macOS and Linux. On Windows, run the PowerShell line from the setup page instead.");
    process.exit(1);
}

if (done_any === 1) {
  say(`Done. Restart any browser that is open, then reopen the setup page at https://${HOST}.local`);
  say("(or at the address you are using now, over https).");
}
