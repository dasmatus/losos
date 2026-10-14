#!/bin/sh
# LosOS: install the losos terminal client for this user.
#
# What this script does, and all it does:
#
#   1. checks that this is a 64-bit x86 Linux computer, the one kind the
#      client is built for;
#   2. downloads two files from
#        https://proxy.losos.dasmat.us/updates/<channel>/x86_64/
#      the program, losos, and SHA256SUMS, the list of their checksums;
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
#   curl -fsSL https://proxy.losos.dasmat.us/updates/main/x86_64/install.sh | sh
#
# LOSOS_CHANNEL picks the channel (main unless set). To undo it, delete the
# file it names at the end.

set -eu

CHANNEL=${LOSOS_CHANNEL:-main}
BASE="https://proxy.losos.dasmat.us/updates/$CHANNEL"

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
  x86_64 | amd64) arch=x86_64 ;;
  *) die "The losos client is built for x86_64 only, and this computer is $arch. Nothing was installed." ;;
esac
URL="$BASE/$arch"

if command -v curl >/dev/null 2>&1; then
  fetch() { curl -fsSL --proto '=https' --tlsv1.2 -o "$2" "$1"; }
elif command -v wget >/dev/null 2>&1; then
  fetch() { wget -q --https-only -O "$2" "$1"; }
else
  die "Neither curl nor wget is installed, so nothing can be downloaded. Install one and run this again."
fi

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

say "Installing losos from the $CHANNEL channel."
say "  from $URL/"

fetch "$URL/SHA256SUMS" "$tmp/SHA256SUMS" || die "Could not download $URL/SHA256SUMS. Check the channel name and your connection. Nothing was installed."
fetch "$URL/losos" "$tmp/losos" || die "Could not download $URL/losos. Nothing was installed."

# The list names each file once: "<sha256>  <name>". A download cut short,
# or a file that is not the one published, stops here.
want=$(awk '$2 == "losos" || $2 == "*losos" { print $1; exit }' "$tmp/SHA256SUMS")
[ -n "$want" ] || die "SHA256SUMS does not list losos. Nothing was installed."
got=$(sha256 "$tmp/losos")
if [ "$got" != "$want" ]; then
  warn "The download does not match its checksum."
  warn "  expected $want"
  warn "  got      $got"
  die "Nothing was installed. Try again later. If it keeps happening, the channel is broken."
fi
say "  checksum $got matches"

mkdir -p "$dest"
chmod 0755 "$tmp/losos"
mv -f "$tmp/losos" "$dest/losos"
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
