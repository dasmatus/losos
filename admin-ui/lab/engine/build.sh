#!/usr/bin/env bash
# Shim: implementation is build.ts (Bun).
set -euo pipefail
exec bun "$(dirname "${BASH_SOURCE[0]}")/build.ts" "$@"
