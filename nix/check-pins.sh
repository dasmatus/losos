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
#
# Implementation lives in check-pins.ts (Bun) to support the TypeScript script
# migration; this shim preserves the shell entrypoint and contract.
set -euo pipefail
exec bun "$(dirname "$0")/check-pins.ts" "$@"
