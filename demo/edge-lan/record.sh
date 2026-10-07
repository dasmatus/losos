#!/usr/bin/env bash
# demo/edge-lan/record.sh — run the two-box walkthrough and photograph it.
#
# Builds the driver of checks.losos-edge-lan-two-boxes (demo/edge-lan/
# two-boxes.nix), starts demo/edge-lan/record.mjs beside it, and runs the
# walkthrough with LOSOS_RECORD_DIR set, so the VM script pauses at each step
# while the recorder photographs the Mesh pane of both boxes through their
# real admin UI. Afterwards compose.sh turns the pictures and captions into a
# slideshow video.
#
#   demo/edge-lan/record.sh [OUTDIR]        default demo/edge-lan/recording
#
# Needs: nix with flakes; node with the app's dependencies installed
# (`npm ci` in admin-ui/app, for playwright); a chromium Playwright can run
# (its own, or LOSOS_CHROMIUM=/path/to/chromium). KVM makes the run minutes;
# without it the three VMs take about 20 minutes under TCG. LOSOS_NIX_ARGS
# passes extra arguments to nix (input overrides on a machine that cannot
# reach GitHub).
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo="$(cd "$here/../.." && pwd)"
out="${1:-$here/recording}"
# shellcheck disable=SC2206 # word-split on purpose, as run.sh does
nix_args=(${LOSOS_NIX_ARGS:-})

mkdir -p "$out"
rm -f "$out"/ack-* "$out"/timeline.jsonl "$out"/shots.jsonl "$out"/vm-finished "$out"/*.token "$out"/*.port
log() { printf '\033[1;36m[%s]\033[0m %s\n' "$(date +%H:%M:%S)" "$*" >&2; }

log "building the walkthrough driver"
nix build "${nix_args[@]}" -o "$out/driver" "$repo#checks.x86_64-linux.losos-edge-lan-two-boxes.driver"

log "starting the recorder"
(cd "$repo/admin-ui/app" && LOSOS_RECORD_DIR="$out" node "$here/record.mjs" >"$out/recorder.log" 2>&1) &
recorder=$!

log "running the walkthrough (log in $out/driver.log)"
set +e
LOSOS_RECORD_DIR="$out" "$out/driver/bin/nixos-test-driver" --no-interactive >"$out/driver.log" 2>&1
status=$?
set -e
touch "$out/vm-finished"
wait "$recorder" || true
if [ "$status" -ne 0 ]; then
  log "the walkthrough FAILED (exit $status); see $out/driver.log"
  exit "$status"
fi
log "done: $(ls "$out"/*.png 2>/dev/null | wc -l) pictures and the captions in $out/shots.jsonl"
log "next: $here/compose.sh $out"
