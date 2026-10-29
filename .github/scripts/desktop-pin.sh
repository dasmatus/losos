#!/usr/bin/env bash
# Point LosOS Desktop's losos client and its box test at a release of this
# repository: rewrite nixos/pkgs/losos-pin.nix in a checkout of the desktop
# repository with the release's commit, its version and the three hashes
# that follow from them.
#
#   desktop-pin.sh TAG DESKTOP_DIR
#
# The desktop builds the terminal client from a sparse fetchgit of this
# repository and boots a box from a second one (nixos/pkgs/losos.nix there),
# rather than taking it as a flake input, which would copy this whole tree,
# handbook pictures and screenshots included, on every evaluation. So the pin
# is four values and three hashes, and the hashes are what Nix reports for
# each fetch with a placeholder in its place: the client's sources, the box's
# sources, then the client's vendored crates, which need the sources first.
# Each is computed with the desktop's own nixpkgs, whose fetchers made the
# hashes it already has.
#
# Prints the new file. Exits 0 without touching it when it already names the
# tag's commit. Needs git and nix; run from a checkout of this repository
# with the tag fetched (.github/workflows/desktop-pin.yml).
set -euo pipefail

if [ $# -ne 2 ]; then
  echo "usage: $0 TAG DESKTOP_DIR" >&2
  exit 2
fi
tag=$1
desktop=$(cd "$2" && pwd)
pin=$desktop/nixos/pkgs/losos-pin.nix
system=x86_64-linux
placeholder=sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=

if ! [[ $tag =~ ^v[0-9]+(\.[0-9]+)*(-[0-9A-Za-z.-]+)?$ ]]; then
  echo "$tag is not a release tag (vX.Y.Z)" >&2
  exit 2
fi
rev=$(git rev-parse "$tag^{commit}")
if [ ! -f "$pin" ]; then
  echo "$pin is missing: is $desktop a checkout of LosOS Desktop?" >&2
  exit 1
fi

# One `key = "value";` line of the pin.
set_value() {
  local key=$1 value=$2
  if ! grep -q "^  $key = \"[^\"]*\";$" "$pin"; then
    echo "$pin has no $key line" >&2
    exit 1
  fi
  sed -i "s|^  $key = \"[^\"]*\";$|  $key = \"$value\";|" "$pin"
}

if grep -q "^  rev = \"$rev\";$" "$pin"; then
  echo "LosOS Desktop already pins $tag ($rev)."
  exit 0
fi

set_value rev "$rev"
set_value version "${tag#v}"
set_value release "$tag"
set_value hash "$placeholder"
set_value cargoHash "$placeholder"
set_value boxHash "$placeholder"

# The hash Nix reports for one fixed-output attribute of the desktop's
# losos package, built with the placeholder in its place.
reported() {
  local attr=$1 log
  # path: copies the checkout as it is on disk, the rewritten pin with it,
  # where a git+file flake would see only what is committed.
  log=$(nix build --no-link --print-build-logs \
    "path:$desktop#legacyPackages.$system.losos.$attr" 2>&1 || true)
  got=$(sed -n 's/^ *got: *\(sha256-[A-Za-z0-9+/=]*\)$/\1/p' <<< "$log" | tail -n 1)
  if [ -z "$got" ]; then
    echo "$attr did not report a hash:" >&2
    echo "$log" >&2
    exit 1
  fi
  echo "$got"
}

set_value hash "$(reported src)"
set_value boxHash "$(reported box)"
set_value cargoHash "$(reported cargoDeps)"

cat "$pin"
