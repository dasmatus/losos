#!/usr/bin/env bash
# Shim: implementation is sign-iso.ts (Bun). The Nix-packaged losos-sign-iso
# uses sign-iso-impl.sh (self-contained shell) so it remains usable without
# Bun in its runtime closure.
set -euo pipefail
exec bun "$(dirname "${BASH_SOURCE[0]}")/sign-iso.ts" "$@"
