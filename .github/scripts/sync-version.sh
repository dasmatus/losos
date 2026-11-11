#!/usr/bin/env bash
# Shim: implementation is sync-version.ts (Bun).
set -euo pipefail
exec bun "$(dirname "${BASH_SOURCE[0]}")/sync-version.ts" "$@"
