#!/usr/bin/env bash
# Shim: implementation is test-payment.ts (Bun).
set -euo pipefail
exec bun "$(dirname "${BASH_SOURCE[0]}")/test-payment.ts" "$@"
