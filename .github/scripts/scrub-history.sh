#!/usr/bin/env bash
# Shim: implementation is scrub-history.ts (Bun).
set -euo pipefail
exec bun "$(dirname "${BASH_SOURCE[0]}")/scrub-history.ts" "$@"
