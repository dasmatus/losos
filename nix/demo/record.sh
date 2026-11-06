#!/usr/bin/env bash
# Shim: implementation is record-driver.ts (Bun); the Playwright recorder is record.ts.
set -euo pipefail
exec bun "$(dirname "${BASH_SOURCE[0]}")/record-driver.ts" "$@"
