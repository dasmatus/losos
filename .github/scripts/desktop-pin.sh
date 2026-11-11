#!/usr/bin/env bash
# Shim: implementation is desktop-pin.ts (Bun).
set -euo pipefail
exec bun "$(dirname "${BASH_SOURCE[0]}")/desktop-pin.ts" "$@"
