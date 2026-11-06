#!/usr/bin/env bash
# Shim: implementation is derisk-theme.ts (Bun).
set -euo pipefail
exec bun "$(dirname "${BASH_SOURCE[0]}")/derisk-theme.ts" "$@"
