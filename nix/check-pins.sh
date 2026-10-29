#!/usr/bin/env bash
# check-pins: flake.lock, devenv.yaml and devenv.lock pin the same nixpkgs.
#
#   bash nix/check-pins.sh
#
# devenv runs standalone (devenv.nix says why), so two lock files pin
# nixpkgs: flake.lock the one that builds the appliance, devenv.lock the one
# whose rustc lints and tests it, and devenv.yaml names the rev devenv.lock
# was made from. Nothing in either tool keeps them in step, and when they
# drift a tree is clippy-clean in one and fails in the other. devenv's
# `check-pins`, CI's `pins` job and the pre-commit hook all run this.
set -euo pipefail
cd "$(dirname "$0")/.."

# jq where it is installed (devenv, the CI runner), else python3.
rev() {
  if command -v jq >/dev/null 2>&1; then
    jq -r '.nodes.nixpkgs.locked.rev' "$1"
  else
    python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["nodes"]["nixpkgs"]["locked"]["rev"])' "$1"
  fi
}
lock=$(rev flake.lock)
yaml=$(grep -oE '[0-9a-f]{40}' devenv.yaml | head -1)
dlock=$(rev devenv.lock)
if [ "$lock" != "$yaml" ] || [ "$lock" != "$dlock" ]; then
  echo "nixpkgs pins disagree:" >&2
  echo "  flake.lock  $lock" >&2
  echo "  devenv.yaml $yaml" >&2
  echo "  devenv.lock $dlock" >&2
  echo "Bump all three together: nix flake update nixpkgs, copy the rev into" >&2
  echo "devenv.yaml, then devenv update." >&2
  exit 1
fi
echo "nixpkgs pin agrees across flake.lock, devenv.yaml and devenv.lock ($lock)"
