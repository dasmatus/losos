#!/usr/bin/env node
// LosOS: install the losos terminal client for this user.
//
// What this script does, and all it does:
//
//   1. checks that this is a 64-bit x86 Linux computer, the one kind the
//      client is built for;
//   2. takes the program, losos, and SHA256SUMS, the list of their
//      checksums, from beside this script when it came out of the build's
//      zip, and otherwise downloads that zip, the newest successful build of
//      the channel's branch, through nightly.link:
//        https://nightly.link/dasmatus/losos/workflows/ci/<channel>/losos-pages-updates.zip
//   3. checks the program's SHA-256 against that list — node computes it
//      itself, through its crypto module — and stops if they differ;
//   4. puts it in ~/.local/bin, or in /usr/local/bin when run as root;
//   5. tells you if secret-tool is missing. The client keeps your keys and
//      tokens in the desktop's keychain through it, and runs without it only
//      in CI mode.
//
// It never asks for a password and never runs sudo. Run it as root yourself
// if you want the program in /usr/local/bin.
//
//   curl -fsSLO https://nightly.link/dasmatus/losos/workflows/ci/main/losos-pages-updates.zip
//   unzip -o losos-pages-updates.zip -d losos-pages-updates
//   node losos-pages-updates/install.mjs
//
// LOSOS_CHANNEL picks the branch a download comes from (main unless set). To
// undo it, delete the file it names at the end.
//
// ── Why this ships as an install.mjs and not as this .ts ──────────────────
//
// The repository writes TypeScript, but what end users read is the bundle:
// CI packs this file with `bun build --target=node --bundle` (the tui job of
// .github/workflows/ci.yml) and users run the result with plain `node` on
// machines without Nix, without Bun and without node_modules. The shell
// version asked no more of them; neither does this one, but the runtime
// checks it could not do itself (the hash, the sums) are now computed in
// node. The tools it still shells out to — uname, id, curl or wget, the
// unpackers, secret-tool, losos — are the shell's list, for the same reason:
// which of them is present decides which branch a stranger's machine takes,
// and the tests drive those branches by faking exactly those tools.

import {
  mkdtempSync,
  rmSync,
  mkdirSync,
  copyFileSync,
  chmodSync,
  renameSync,
  readFileSync,
  existsSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, basename } from "node:path";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";

const CHANNEL = process.env.LOSOS_CHANNEL || "main";
const ARTIFACT = `https://nightly.link/dasmatus/losos/workflows/ci/${CHANNEL}/losos-pages-updates.zip`;

const say = (...lines: string[]) => console.log(...lines);
const warn = (...lines: string[]) => console.error(...lines);
function die(message: string): never {
  warn(message);
  process.exit(1);
}

/** `command -v cmd` — the availability checks the shell version made, made
 *  the same way. Arguments the tools receive never go through a shell. */
function have(command: string) {
  return spawnSync("sh", ["-c", `command -v ${command}`], { stdio: "ignore" }).status === 0;
}

/** Run a tool with the terminal as its stdio, and hand back its exit code —
 *  the shape `cmd ... || die "..."` needs. */
function tryRun(cmd: string, args: string[]): number {
  return spawnSync(cmd, args, { stdio: "inherit" }).status ?? 1;
}

/** `$(cmd ...)` — the first line fields of a captured run, trimmed. Bun's
 *  spawnSync returns strings and node's Buffers; encoding pins them both
 *  (this runs under node as install.mjs, and under bun here at home). */
function captured(cmd: string, args: string[], options = {}): string {
  const r = spawnSync(cmd, args, { encoding: "utf8", stdio: ["ignore", "pipe", "ignore"], ...options });
  return (r.stdout || "").trim();
}

// ── What this computer is ────────────────────────────────────────────────
// `uname`, not process.platform/arch: the sentences below quote uname's
// spelling (`x86_64`) back, and the arch and os branches are driven by faking
// uname in the tests, the way the shell version's were driven.
const os = captured("uname", ["-s"]);
const arch = captured("uname", ["-m"]);
switch (os) {
  case "Linux":
    break;
  default:
    die(`The losos client is built for Linux only, and this computer runs ${os}. Nothing was installed.`);
}
switch (arch) {
  case "x86_64":
  case "amd64":
    break;
  default:
    die(`The losos client is built for x86_64 only, and this computer is ${arch}. Nothing was installed.`);
}

// ── Where it goes ────────────────────────────────────────────────────────
// Root gets the directory every user's PATH has. Anyone else gets their own,
// which needs no rights they do not already have. Through the real `id`, the
// way the shell asked it: the test that may run as root fakes it to stay out
// of /usr/local/bin, which process.getuid() would walk past.
let dest;
if (captured("id", ["-u"]) === "0") {
  dest = "/usr/local/bin";
} else {
  dest = join(process.env.HOME || "", ".local/bin");
}

const tmp = mkdtempSync(join(tmpdir(), "losos-install-"));
for (const sig of ["exit", "SIGINT", "SIGTERM"]) {
  // The shell's `trap 'rm -rf "$tmp"' EXIT INT TERM`.
  process.on(sig, () => rmSync(tmp, { recursive: true, force: true }));
}

// ── Where the program comes from ─────────────────────────────────────────
// Run as `node <folder>/install.mjs` out of the unzipped build, the program
// and its sums are beside the script. Piped into node, argv[1] is empty, so
// the build's zip is downloaded instead — the same signal the shell took
// from `$0` being the interpreter.
let src: string;
const argv1 = process.argv[1] || "";
if (basename(argv1) === "install.mjs") {
  src = dirname(argv1);
} else {
  // ── The download path. The fetch and unpack chains are the shell's, branch
  // for branch: what a reader of this file counts as "downloaders" and
  // "unpackers" is the same list it was.
  let fetch: ((url: string, out: string) => number) | undefined;
  if (have("curl")) {
    // curl -fsSL --proto '=https' --tlsv1.2 -o <file> <url>
    fetch = (url, out) =>
      tryRun("curl", ["-fsSL", "--proto", "=https", "--tlsv1.2", "-o", out, url]);
  } else if (have("wget")) {
    // wget -q --https-only -O <file> <url>
    fetch = (url, out) => tryRun("wget", ["-q", "--https-only", "-O", out, url]);
  } else {
    die("Neither curl nor wget is installed, so nothing can be downloaded. Install one and run this again.");
  }
  say(`Installing losos from the ${CHANNEL} branch.`);
  say(`  from ${ARTIFACT}`);
  const buildZip = join(tmp, "build.zip");
  if (fetch && fetch(ARTIFACT, buildZip) !== 0) {
    die(`Could not download ${ARTIFACT}. Check the channel name and your connection. Nothing was installed.`);
  }

  let unpack: ((zip: string, out: string) => number) | undefined;
  if (have("unzip")) {
    // unzip -q -o <zip> -d <folder>
    unpack = (zip, out) => tryRun("unzip", ["-q", "-o", zip, "-d", out]);
  } else if (have("bsdtar")) {
    // bsdtar opens a zip directly.
    unpack = (zip, out) => {
      mkdirSync(out, { recursive: true });
      return tryRun("bsdtar", ["-xf", zip, "-C", out]);
    };
  } else if (have("python3")) {
    // python3 -m zipfile -e <zip> <folder>
    unpack = (zip, out) => tryRun("python3", ["-m", "zipfile", "-e", zip, out]);
  } else {
    die("Neither unzip, bsdtar nor python3 is installed, so the download cannot be opened. Install unzip and run this again.");
  }
  const build = join(tmp, "build");
  if (unpack && unpack(buildZip, build) !== 0) {
    die("Could not open the downloaded zip. Nothing was installed.");
  }
  src = build;
}
if (!existsSync(join(src, "SHA256SUMS"))) die("The build has no SHA256SUMS. Nothing was installed.");
if (!existsSync(join(src, "losos"))) die("The build has no losos. Nothing was installed.");

// The list names each file once: "<sha256>  <name>". A download cut short,
// or a file that is not the one published, stops here.
// awk: '$2 == "losos" || $2 == "*losos" { print $1; exit }'.
const sums = readFileSync(join(src, "SHA256SUMS"), "utf8")
  .split("\n")
  .map((line) => line.trim().split(/\s+/))
  .find((fields) => (fields[1] === "losos" || fields[1] === "*losos") && fields[0] !== "");
const want = sums ? sums[0] : "";
if (!want) die("SHA256SUMS does not list losos. Nothing was installed.");

// sha256sum and shasum stood in because POSIX sh had no hash of its own; the
// runtime this runs on does.
const got = createHash("sha256").update(readFileSync(join(src, "losos"))).digest("hex");
if (got !== want) {
  warn("The download does not match its checksum.");
  warn(`  expected ${want}`);
  warn(`  got      ${got}`);
  die("Nothing was installed. Try again later. If it keeps happening, the channel is broken.");
}
say(`  checksum ${got} matches`);

// Copied, not moved: the unzipped folder is the owner's, and stays whole.
// Through a temp name, like the old `cp → chmod → mv -f`, so a half-written
// file never carries the final name — and across filesystems where the tmp
// directory happens to live elsewhere, which `mv` managed and rename does
// not, hence the fallback.
mkdirSync(dest, { recursive: true });
const fresh = join(tmp, "losos.new");
copyFileSync(join(src, "losos"), fresh);
chmodSync(fresh, 0o755);
try {
  renameSync(fresh, join(dest, "losos"));
} catch (e) {
  if ((e as NodeJS.ErrnoException).code === "EXDEV") {
    copyFileSync(fresh, join(dest, "losos"));
    chmodSync(join(dest, "losos"), 0o755);
  } else {
    throw e;
  }
}
// --version failed: the words the shell's `say` put inside the substitution.
const itSays = captured(join(dest, "losos"), ["--version"]) || "nothing, it did not start";
say(`Installed ${dest}/losos. It says: ${itSays}`);

const pathEdges = `:${process.env.PATH || ""}:`;
if (!pathEdges.includes(`:${dest}:`)) {
  say("");
  say(`${dest} is not on your PATH. Add this line to your shell's startup file:`);
  say(`  export PATH="${dest}:$PATH"`);
}

// ── The keychain ─────────────────────────────────────────────────────────
// Without secret-tool the client keeps nothing and says so; it never falls
// back to a plain file.
if (!have("secret-tool")) {
  say("");
  say("secret-tool is not installed. losos keeps your keys and tokens in the desktop's keychain through it.");
  say("  Debian, Ubuntu:  apt install libsecret-tools");
  say("  Fedora:          dnf install libsecret");
  say("  Arch:            pacman -S libsecret");
}

say("");
say("Next: losos key import --gpg <your key>, then losos login <address of your box>.");
say("The handbook walks through it: https://losos.dasmat.us/manual/terminal/");

// ── AI tools ─────────────────────────────────────────────────────────────
// One line when AI tools are on this computer and the companion is in none
// of them. It only looks; `losos agents install` asks before it changes any.
const hint = captured(join(dest, "losos"), ["agents", "hint"], { stdio: ["ignore", "pipe", "ignore"] });
if (hint !== "") {
  say("");
  say(hint);
  say("The handbook explains it: https://losos.dasmat.us/manual/ai-agents/");
}
