#!/usr/bin/env bash
# nix/demo/record.sh: run a VM test and photograph it through the real box.
#
# Builds the driver of a flake check whose script carries the hooks in
# nix/demo/recorder.py, starts nix/demo/record.mjs beside it, and runs the
# test with LOSOS_RECORD_DIR set, so the VM script pauses at each step while
# the recorder photographs the pages that step names.
#
#   nix/demo/record.sh CHECK [OUTDIR]     e.g. nix/demo/record.sh losos-podman-apps
#
# OUTDIR defaults to nix/demo/recording/CHECK. Needs: nix with flakes; bun
# with the app's dependencies installed (`bun install` in admin-ui/app, for
# playwright); a chromium Playwright can run (its own, or
# LOSOS_CHROMIUM=/path/to/chromium). LOSOS_NIX=<command> replaces `nix`
# (a wrapper that adds input overrides, say); LOSOS_NIX_ARGS passes extra
# arguments to it.
set -euo pipefail

check="${1:?usage: record.sh CHECK [OUTDIR]}"
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo="$(cd "$here/../.." && pwd)"
out="$(mkdir -p "${2:-$here/recording/$check}" && cd "${2:-$here/recording/$check}" && pwd)"
nix="${LOSOS_NIX:-nix}"
# shellcheck disable=SC2206 # word-split on purpose
nix_args=(${LOSOS_NIX_ARGS:-})

rm -f "$out"/ack-* "$out"/timeline.jsonl "$out"/shots.jsonl "$out"/vm-finished "$out"/*.token "$out"/*.png
log() { printf '\033[1;36m[%s]\033[0m %s\n' "$(date +%H:%M:%S)" "$*" >&2; }

log "building the driver of $check"
$nix build "${nix_args[@]}" -o "$out/driver" "$repo#checks.x86_64-linux.$check.driver"

log "starting the recorder"
(cd "$repo/admin-ui/app" && LOSOS_RECORD_DIR="$out" bun "$here/record.mjs" >"$out/recorder.log" 2>&1) &
recorder=$!

log "running $check (log in $out/driver.log)"
set +e
(cd "$out" && LOSOS_RECORD_DIR="$out" "$out/driver/bin/nixos-test-driver" --no-interactive >"$out/driver.log" 2>&1)
status=$?
set -e
touch "$out/vm-finished"
wait "$recorder" || true
if [ "$status" -ne 0 ]; then
  log "$check FAILED (exit $status); see $out/driver.log"
  exit "$status"
fi
log "done: $(find "$out" -maxdepth 1 -name '*.png' | wc -l) pictures, captions in $out/shots.jsonl"
