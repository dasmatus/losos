#!/usr/bin/env bash
# nix/demo/box-edge-box/record.sh: run the light box, edge, box demo and
# photograph it.
#
# Builds the driver of checks.losos-box-edge-box (demo.nix), starts
# record.mjs beside it and runs the demo with LOSOS_RECORD_DIR set, so the VM
# script pauses at each step while the recorder photographs both boxes' Mesh
# pane and the edge's manage page through the VMs' real admin UI.
#
#   nix/demo/box-edge-box/record.sh [OUTDIR]     default nix/demo/box-edge-box/recording
#
# Needs: nix with flakes; bun with admin-ui/app's dependencies installed
# (`bun install` there, for playwright); a chromium Playwright can run (its own,
# or LOSOS_CHROMIUM=/path/to/chromium). THEME=dark for dark pictures.
# LOSOS_NIX_ARGS passes extra arguments to nix. Without KVM the three VMs
# take about 15 minutes.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo="$(cd "$here/../../.." && pwd)"
out="${1:-$here/recording}"
# shellcheck disable=SC2206 # word-split on purpose, as run.sh does
nix_args=(${LOSOS_NIX_ARGS:-})

mkdir -p "$out"
out="$(cd "$out" && pwd)" # the driver runs from inside it
rm -f "$out"/ack-* "$out"/timeline.jsonl "$out"/shots.jsonl "$out"/vm-finished "$out"/*.token "$out"/*.port "$out"/*.key
log() { printf '\033[1;36m[%s]\033[0m %s\n' "$(date +%H:%M:%S)" "$*" >&2; }

log "building the demo driver"
nix build "${nix_args[@]}" -o "$out/driver" "$repo#checks.x86_64-linux.losos-box-edge-box.driver"

log "starting the recorder"
LOSOS_RECORD_DIR="$out" bun "$here/record.mjs" >"$out/recorder.log" 2>&1 &
recorder=$!

log "running the demo (log in $out/driver.log)"
set +e
(cd "$out" && LOSOS_RECORD_DIR="$out" "$out/driver/bin/nixos-test-driver" --no-interactive -o "$out" >"$out/driver.log" 2>&1)
status=$?
set -e
touch "$out/vm-finished"
wait "$recorder" || true
if [ "$status" -ne 0 ]; then
  log "the demo FAILED (exit $status); see $out/driver.log"
  exit "$status"
fi
log "done: $(ls "$out"/*.png 2>/dev/null | wc -l) pictures, captions in $out/shots.jsonl"
