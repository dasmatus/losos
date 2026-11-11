#!/usr/bin/env bash
# Decide which lanes of ci.yml a change needs. Shim preserves the original
# entrypoint; implementation is ci-changes.ts (Bun).
set -euo pipefail
exec bun "$(dirname "${BASH_SOURCE[0]}")/ci-changes.ts" "$@"
