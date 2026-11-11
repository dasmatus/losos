#!/bin/sh
# LosOS: install the losos terminal client for this user.
#
# What this script does, and all it does:
#
#   1. checks that this is a 64-bit x86 Linux computer, the one kind the
#      client is built for;
#   2. takes the program, losos, and SHA256SUMS, the list of their
#      checksums, from beside this script when it came out of the build's
#      zip, and otherwise downloads that zip, the newest successful build of
#      the channel's branch, through nightly.link:
#        https://nightly.link/dasmatus/losos/workflows/ci/<channel>/losos-pages-updates.zip
#   3. checks the program's SHA-256 against that list, with sha256sum or
#      shasum, and stops if they differ;
#   4. puts it in ~/.local/bin, or in /usr/local/bin when run as root;
#   5. tells you if secret-tool is missing. The client keeps your keys and
#      tokens in the desktop's keychain through it, and runs without it only
#      in CI mode.
#
# It never asks for a password and never runs sudo. Run it as root yourself
# if you want the program in /usr/local/bin.
#
#   curl -fsSLO https://nightly.link/dasmatus/losos/workflows/ci/main/losos-pages-updates.zip
#   unzip -o losos-pages-updates.zip -d losos-pages-updates
#   sh losos-pages-updates/install.sh
#
# LOSOS_CHANNEL picks the branch a download comes from (main unless set). To
# undo it, delete the file it names at the end.

set -eu

CHANNEL=${LOSOS_CHANNEL:-main}
ARTIFACT="https://nightly.link/dasmatus/losos/workflows/ci/$CHANNEL/losos-pages-updates.zip"

say() { printf '%s\n' "$*"; }
warn() { printf '%s\n' "$*" >&2; }
die() {
  warn "$*"
  exit 1
}

# ── What this computer is ────────────────────────────────────────────────
os=$(uname -s)
arch=$(uname -m)
case "$os" in
  Linux) ;;
  *) die "The losos client is built for Linux only, and this computer runs $os. Nothing was installed." ;;
esac
case "$arch" in
  x86_64 | amd64) ;;
  *) die "The losos client is built for x86_64 only, and this computer is $arch. Nothing was installed." ;;
esac

if command -v sha256sum >/dev/null 2>&1; then
  sha256() { sha256sum "$1" | cut -d' ' -f1; }
elif command -v shasum >/dev/null 2>&1; then
  sha256() { shasum -a 256 "$1" | cut -d' ' -f1; }
else
  die "Neither sha256sum nor shasum is installed, so the download cannot be checked. Nothing was installed."
fi

# ── Where it goes ────────────────────────────────────────────────────────
# Root gets the directory every user's PATH has. Anyone else gets their own,
# which needs no rights they do not already have.
if [ "$(id -u)" -eq 0 ]; then
  dest=/usr/local/bin
else
  dest="$HOME/.local/bin"
fi

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT INT TERM

# ── Where the program comes from ─────────────────────────────────────────
# Run as `sh <folder>/install.sh` out of the unzipped build, the program and
# its sums are beside the script. Piped into sh, $0 is the shell, and the
# build's zip is downloaded instead.
here=
case "$0" in
  install.sh | */install.sh) here=$(dirname -- "$0") ;;
esac
if [ -n "$here" ] && [ -f "$here/losos" ] && [ -f "$here/SHA256SUMS" ]; then
  src=$here
  say "Installing losos from $src/."
else
  if command -v curl >/dev/null 2>&1; then
    fetch() { curl -fsSL --proto '=https' --tlsv1.2 -o "$2" "$1"; }
  elif command -v wget >/dev/null 2>&1; then
    fetch() { wget -q --https-only -O "$2" "$1"; }
  else
    die "Neither curl nor wget is installed, so nothing can be downloaded. Install one and run this again."
  fi
  if command -v unzip >/dev/null 2>&1; then
    unpack() { unzip -q -o "$1" -d "$2"; }
  elif command -v bsdtar >/dev/null 2>&1; then
    unpack() { mkdir -p "$2" && bsdtar -xf "$1" -C "$2"; }
  elif command -v python3 >/dev/null 2>&1; then
    unpack() { python3 -m zipfile -e "$1" "$2"; }
  else
    die "Neither unzip, bsdtar nor python3 is installed, so the download cannot be opened. Install unzip and run this again."
  fi
  say "Installing losos from the $CHANNEL branch."
  say "  from $ARTIFACT"
  fetch "$ARTIFACT" "$tmp/build.zip" || die "Could not download $ARTIFACT. Check the channel name and your connection. Nothing was installed."
  unpack "$tmp/build.zip" "$tmp/build" || die "Could not open the downloaded zip. Nothing was installed."
  src=$tmp/build
fi
[ -f "$src/SHA256SUMS" ] || die "The build has no SHA256SUMS. Nothing was installed."
[ -f "$src/losos" ] || die "The build has no losos. Nothing was installed."

# The list names each file once: "<sha256>  <name>". A download cut short,
# or a file that is not the one published, stops here.
want=$(awk '$2 == "losos" || $2 == "*losos" { print $1; exit }' "$src/SHA256SUMS")
[ -n "$want" ] || die "SHA256SUMS does not list losos. Nothing was installed."
got=$(sha256 "$src/losos")
if [ "$got" != "$want" ]; then
  warn "The download does not match its checksum."
  warn "  expected $want"
  warn "  got      $got"
  die "Nothing was installed. Try again later. If it keeps happening, the channel is broken."
fi
say "  checksum $got matches"

# Copied, not moved: the unzipped folder is the owner's, and stays whole.
mkdir -p "$dest"
cp "$src/losos" "$tmp/losos.new"
chmod 0755 "$tmp/losos.new"
mv -f "$tmp/losos.new" "$dest/losos"
say "Installed $dest/losos. It says: $("$dest/losos" --version 2>/dev/null || say "nothing, it did not start")"

case ":$PATH:" in
  *":$dest:"*) ;;
  *)
    say ""
    say "$dest is not on your PATH. Add this line to your shell's startup file:"
    say "  export PATH=\"$dest:\$PATH\""
    ;;
esac

# ── The keychain ─────────────────────────────────────────────────────────
# Without secret-tool the client keeps nothing and says so; it never falls
# back to a plain file.
if ! command -v secret-tool >/dev/null 2>&1; then
  say ""
  say "secret-tool is not installed. losos keeps your keys and tokens in the desktop's keychain through it."
  say "  Debian, Ubuntu:  apt install libsecret-tools"
  say "  Fedora:          dnf install libsecret"
  say "  Arch:            pacman -S libsecret"
fi

say ""
say "Next: losos key import --gpg <your key>, then losos login <address of your box>."
say "The handbook walks through it: https://losos.dasmat.us/manual/terminal/"

# ── AI tools ─────────────────────────────────────────────────────────────
# One line when AI tools are on this computer and the companion is in none
# of them. It only looks; `losos agents install` asks before it changes any.
hint=$("$dest/losos" agents hint 2>/dev/null || true)
if [ -n "$hint" ]; then
  say ""
  say "$hint"
  say "The handbook explains it: https://losos.dasmat.us/manual/ai-agents/"
fi
