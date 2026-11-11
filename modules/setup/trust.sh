#!/bin/sh
# LosOS: trust the certificate of the box "@HOST@" on this computer.
#
# What this script does, and all it does:
#
#   1. writes the certificate at the bottom of this file to a temporary file;
#   2. checks that its SHA-256 fingerprint is
#        @FINGERPRINT@
#      which is the fingerprint the setup page shows you;
#   3. adds it to the certificate stores your browsers read, for your user:
#        macOS    your login keychain (Safari, Chrome, Firefox read it)
#        Linux    the NSS store Chrome and Chromium use, and every Firefox
#                 profile on this account; the system store too if run as root
#   4. deletes the temporary file.
#
# It installs no software, changes no other setting, sends nothing anywhere
# and needs no password unless your operating system asks for one itself
# (macOS does, to change what your keychain trusts).
#
# The box served this script from its own address on your network; nothing
# in it came from the internet. To undo it later, delete the certificate
# named "LosOS @HOST@" from the same store.

set -eu

HOST='@HOST@'
FINGERPRINT='@FINGERPRINT@'
NAME="LosOS $HOST"

say() { printf '%s\n' "$*"; }
warn() { printf '%s\n' "$*" >&2; }

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT INT TERM
pem="$tmp/losos-ca.crt"

cat > "$pem" <<'PEM'
@PEM@
PEM

say "Trusting the certificate of $HOST on this computer."
say "Fingerprint (SHA-256): $FINGERPRINT"
say "Compare it with the one the setup page shows. If they differ, press Ctrl-C."

# With openssl around, the certificate below is checked against the
# fingerprint above before anything is trusted. A download cut short, or a
# box that is not the one the setup page described, stops here.
if command -v openssl >/dev/null 2>&1; then
  got=$(openssl x509 -in "$pem" -noout -fingerprint -sha256 2>/dev/null | cut -d= -f2)
  if [ "$got" != "$FINGERPRINT" ]; then
    warn "The certificate in this script does not match its own fingerprint (got '$got')."
    warn "Nothing was changed. Reload the setup page and try again."
    exit 1
  fi
fi

done_any=0
mark() { done_any=1; say "  added to $*"; }

# ── macOS: the login keychain ─────────────────────────────────────────────
# Safari, Chrome and Firefox all read trust settings from it. The keychain
# asks for the account password in a dialog: that is macOS, not this script.
trust_macos() {
  keychain="$HOME/Library/Keychains/login.keychain-db"
  if security add-trusted-cert -r trustRoot -k "$keychain" "$pem"; then
    mark "your login keychain"
  else
    warn "Could not add it to the login keychain."
    warn "Open Keychain Access, drag $pem into 'login', then set its trust to Always Trust."
    return 1
  fi
}

# ── Linux: NSS databases ──────────────────────────────────────────────────
# Chrome, Chromium, Brave and Edge read ~/.pki/nssdb. Firefox keeps one
# database per profile (cert9.db), including the snap and flatpak homes.
nss_add() {
  dir=$1
  label=$2
  if [ ! -f "$dir/cert9.db" ]; then
    mkdir -p "$dir"
    certutil -d "sql:$dir" -N --empty-password
  fi
  # -D first so running the script twice (a new certificate after a reinstall)
  # replaces the entry instead of stacking a second one under the same name.
  certutil -d "sql:$dir" -D -n "$NAME" 2>/dev/null || true
  certutil -d "sql:$dir" -A -t "C,," -n "$NAME" -i "$pem"
  mark "$label"
}

firefox_profiles() {
  for base in \
    "$HOME/.mozilla/firefox" \
    "$HOME/snap/firefox/common/.mozilla/firefox" \
    "$HOME/.var/app/org.mozilla.firefox/.mozilla/firefox" \
    "$HOME/.librewolf"; do
    [ -d "$base" ] || continue
    for p in "$base"/*/; do
      [ -f "${p}cert9.db" ] && printf '%s\n' "${p%/}"
    done
  done
}

# ── Linux: the system store, only when already root ──────────────────────
# curl, wget, package managers and anything else that is not a browser read
# it. Never escalates: a script that reaches for sudo is one you should not
# have piped into sh.
trust_system() {
  if command -v update-ca-certificates >/dev/null 2>&1; then
    # Debian, Ubuntu and their derivatives.
    install -m 0644 "$pem" "/usr/local/share/ca-certificates/losos-$HOST.crt"
    update-ca-certificates >/dev/null
    mark "the system store (/usr/local/share/ca-certificates)"
  elif command -v update-ca-trust >/dev/null 2>&1; then
    # Fedora, RHEL, openSUSE.
    install -m 0644 "$pem" "/etc/pki/ca-trust/source/anchors/losos-$HOST.crt"
    update-ca-trust
    mark "the system store (/etc/pki/ca-trust/source/anchors)"
  elif command -v trust >/dev/null 2>&1; then
    # Arch.
    trust anchor --store "$pem"
    mark "the system store (trust anchor)"
  else
    warn "  no system certificate tool found here; the system store was left alone."
  fi
}

trust_linux() {
  if ! command -v certutil >/dev/null 2>&1; then
    warn "This needs 'certutil' from NSS, which is not installed."
    warn "  Debian/Ubuntu: sudo apt install libnss3-tools"
    warn "  Fedora:        sudo dnf install nss-tools"
    warn "  Arch:          sudo pacman -S nss"
    warn "  NixOS:         nix shell nixpkgs#nssTools"
    warn "Then run this again, or install the certificate from the setup page by hand."
    return 1
  fi
  nss_add "$HOME/.pki/nssdb" "Chrome and Chromium (~/.pki/nssdb)"
  for p in $(firefox_profiles); do
    nss_add "$p" "Firefox profile $(basename "$p")"
  done
  if [ "$(id -u)" = 0 ]; then
    trust_system
  fi
}

case "$(uname -s)" in
  Darwin) trust_macos ;;
  Linux) trust_linux ;;
  *)
    warn "This script knows macOS and Linux. On Windows, run the PowerShell line from the setup page instead."
    exit 1
    ;;
esac

if [ "$done_any" = 1 ]; then
  say "Done. Restart any browser that is open, then reopen the setup page at https://$HOST.local"
  say "(or at the address you are using now, over https)."
fi
