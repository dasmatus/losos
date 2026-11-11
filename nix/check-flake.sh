#!/usr/bin/env bash
# Shim: implementation is check-flake.ts (Bun).
set -euo pipefail
exec bun "$(dirname "${BASH_SOURCE[0]}")/check-flake.ts" "$@"
