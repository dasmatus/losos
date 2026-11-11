#!/usr/bin/env bash
# Shim: implementation is compose.ts (Bun).
set -euo pipefail
exec bun "$(dirname "${BASH_SOURCE[0]}")/compose.ts" "$@"
