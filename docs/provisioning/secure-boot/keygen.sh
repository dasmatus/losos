#!/usr/bin/env bash
# Shim: implementation is keygen.ts (Bun).
set -euo pipefail
exec bun "$(dirname "${BASH_SOURCE[0]}")/keygen.ts" "$@"
