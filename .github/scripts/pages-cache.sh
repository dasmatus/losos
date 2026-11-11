#!/usr/bin/env bash
# Shim: implementation is pages-cache.ts (Bun).
set -euo pipefail
exec bun "$(dirname "${BASH_SOURCE[0]}")/pages-cache.ts" "$@"
