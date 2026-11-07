#!/usr/bin/env bash
# Shim: implementation is lint-nix.ts (Bun).
set -euo pipefail
exec bun "$(dirname "${BASH_SOURCE[0]}")/lint-nix.ts" "$@"
