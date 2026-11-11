#!/usr/bin/env bash
# Shim: implementation is record-driver.ts (Bun); Playwright recorder is record.ts.
set -euo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo="$(cd "$here/../../.." && pwd)"
out="${1:-$here/recording}"
exec bun "$here/record-driver.ts" losos-box-edge-box "$out"
