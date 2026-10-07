#!/usr/bin/env bash
# demo/edge-lan/run.sh — a losos box and an edge proxy on one virtual network.
#
# What it shows: the box finds the edge on the LAN by itself (mDNS/DNS-SD),
# the Mesh pane says so and lets storage be shared; the edge is switched off,
# the box notices within a minute and refuses to turn sharing on, in its own
# words; the edge comes back and the gate reopens. See wiki/Mesh.md, "Finding
# the edge", and backend/src/edge.rs.
#
# Host requirements (everything else comes through `nix shell`):
#   * nix with flakes (https://nixos.org/download), 20 GB of free disk,
#     8 GB of RAM to give the box;
#   * /dev/kvm for a demo that takes minutes rather than hours — without it
#     QEMU falls back to software emulation and the install alone takes
#     about 45 minutes;
#   * no root: the two VMs talk over a VDE switch (a unix socket), and the
#     host reaches them through QEMU port forwards on 127.0.0.1.
#
# Usage:
#   demo/edge-lan/run.sh build [--iso PATH]   build the edge VM and the ISO
#   demo/edge-lan/run.sh up                   start the LAN, the edge, install
#                                             the box from the ISO, boot it,
#                                             claim it (prints the admin key)
#   demo/edge-lan/run.sh walk                 the walkthrough, over the API
#   demo/edge-lan/run.sh edge off|on          power the edge off, or back on
#   demo/edge-lan/run.sh status               what is running
#   demo/edge-lan/run.sh down                 stop everything (keeps disks)
#   demo/edge-lan/run.sh clean                stop and delete the state dir
#
# Ports on the host (override with the environment variables named):
#   LOSOS_BOX_PORT      8080  the box's web UI    http://127.0.0.1:8080/
#   LOSOS_EDGE_API_PORT 8443  the edge's registrar http://127.0.0.1:8443/health
#
# State lives in $LOSOS_DEMO_STATE (default demo/edge-lan/state, gitignored):
# the VDE socket, both disks, QMP sockets, logs, and the admin key.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo="$(cd "$here/../.." && pwd)"
state="${LOSOS_DEMO_STATE:-$here/state}"
box_port="${LOSOS_BOX_PORT:-8080}"
edge_api_port="${LOSOS_EDGE_API_PORT:-8443}"
box_mem="${LOSOS_BOX_MEM:-8192}"
box_cpus="${LOSOS_BOX_CPUS:-4}"
box_disk_gb="${LOSOS_BOX_DISK_GB:-40}"
# The password the claim sets on the box. Demo only; change it after.
box_password="${LOSOS_BOX_PASSWORD:-Losos-demo-2026!}"
flake="${LOSOS_FLAKE:-$repo}"
# Extra `nix` arguments, e.g. input overrides on a machine that cannot reach
# GitHub. Word-split on purpose.
nix_args=${LOSOS_NIX_ARGS:-}

mkdir -p "$state"
log() { printf '\033[1;36m[%s]\033[0m %s\n' "$(date +%H:%M:%S)" "$*" >&2; }
die() { printf '\033[1;31merror:\033[0m %s\n' "$*" >&2; exit 1; }

# ── Tools through nix shell ────────────────────────────────────────────────
# One `nix shell` per tool set rather than requiring installs. The closure
# is fetched once from cache.nixos.org and then cached locally.
tools="nixpkgs#qemu nixpkgs#vde2 nixpkgs#swtpm nixpkgs#curl nixpkgs#jq"
in_tools() { nix shell $nix_args --inputs-from "$flake" $tools -c "$@"; }

have_kvm() { [ -w /dev/kvm ]; }
if have_kvm; then accel_args=(-enable-kvm -cpu host); else accel_args=(-cpu max); fi

qmp() { # qmp SOCKET COMMAND — one QMP command, JSON reply on stdout
  local sock=$1 cmd=$2
  printf '{"execute":"qmp_capabilities"}\n{"execute":"%s"}\n' "$cmd" \
    | in_tools python3 -c '
import socket,sys,json
s=socket.socket(socket.AF_UNIX); s.connect(sys.argv[1]); f=s.makefile("rw")
f.readline()  # greeting
for line in sys.stdin:
    f.write(line); f.flush(); print(f.readline().strip())
' "$sock" 2>/dev/null || true
}

alive() { [ -S "$1" ] && qmp "$1" query-status | grep -q running; }

# ── build ─────────────────────────────────────────────────────────────────
cmd_build() {
  local iso=""
  while [ $# -gt 0 ]; do
    case $1 in
      --iso) iso=$2; shift 2 ;;
      *) die "build: unknown argument $1" ;;
    esac
  done
  log "building the edge VM (nixosConfigurations.edge-demo)"
  nix build $nix_args -o "$state/edge-vm" "$flake#nixosConfigurations.edge-demo.config.system.build.vm"
  if [ -n "$iso" ]; then
    [ -r "$iso" ] || die "no ISO at $iso"
    ln -sfn "$(realpath "$iso")" "$state/losos.iso"
    log "using the ISO at $iso"
  else
    log "building the installer ISO (this compiles the control plane; minutes to tens of minutes)"
    nix build $nix_args -o "$state/iso" "$flake#nixosConfigurations.iso.config.system.build.isoImage"
    ln -sfn "$(ls "$state"/iso/iso/*.iso | head -1)" "$state/losos.iso"
  fi
  log "built: $state/edge-vm/bin/run-edge-vm and $state/losos.iso"
}

# ── the LAN ────────────────────────────────────────────────────────────────
lan_up() {
  if [ -S "$state/lan.sock/ctl" ] && kill -0 "$(cat "$state/vde.pid" 2>/dev/null)" 2>/dev/null; then
    return
  fi
  rm -rf "$state/lan.sock" "$state/vde.pid"
  log "starting the virtual LAN (vde_switch)"
  in_tools vde_switch -s "$state/lan.sock" -d -p "$state/vde.pid" -M "$state/vde.mgmt"
  sleep 1
}

# ── the edge ───────────────────────────────────────────────────────────────
edge_up() {
  if alive "$state/edge.qmp"; then log "edge already running"; return; fi
  [ -x "$state/edge-vm/bin/run-edge-vm" ] || die "no edge VM; run: $0 build"
  lan_up
  log "starting the edge VM (serial console in $state/edge.log)"
  rm -f "$state/edge.qmp"
  # The run script honours NIX_DISK_IMAGE for the root disk and QEMU_OPTS for
  # extras; the LAN socket and the API port reach it by the two variables
  # demo/edge-lan/edge-vm.nix names. It wants to be the terminal's child, so
  # it gets a pty-less stdin and a log for a stdout.
  (
    cd "$state"
    export NIX_DISK_IMAGE="$state/edge.qcow2"
    export LOSOS_LAN_SOCK="$state/lan.sock"
    export LOSOS_EDGE_API_PORT="$edge_api_port"
    local extra=""
    have_kvm || extra="-accel tcg,thread=multi"
    export QEMU_OPTS="-qmp unix:$state/edge.qmp,server,nowait $extra"
    PATH="$(in_tools sh -c 'echo $PATH')" \
      nohup "$state/edge-vm/bin/run-edge-vm" </dev/null >"$state/edge.log" 2>&1 &
    echo $! >"$state/edge.pid"
  )
  log "waiting for the edge's registrar (http://127.0.0.1:$edge_api_port/health)"
  for _ in $(seq 1 180); do
    if in_tools curl -fsS --max-time 2 "http://127.0.0.1:$edge_api_port/health" >/dev/null 2>&1; then
      log "edge is up and advertising _losos-edge._tcp on the LAN"
      return
    fi
    sleep 2
  done
  die "the edge did not answer in 6 minutes; see $state/edge.log"
}

edge_off() {
  alive "$state/edge.qmp" || { log "edge is not running"; return; }
  log "powering the edge off"
  qmp "$state/edge.qmp" quit >/dev/null
  sleep 1
}

# ── the box ────────────────────────────────────────────────────────────────
box_qemu() { # box_qemu BOOT(d|c) — start the box VM; stdout is QEMU's
  local boot=$1
  rm -f "$state/box.qmp"
  mkdir -p "$state/tpm"
  # swtpm: the installer enrols the disk key into it, so the installed box
  # unlocks with no passphrase. Same state dir for the install and every boot.
  if ! [ -S "$state/tpm/swtpm-sock" ] || ! kill -0 "$(cat "$state/tpm/swtpm.pid" 2>/dev/null)" 2>/dev/null; then
    in_tools swtpm socket --tpm2 --tpmstate dir="$state/tpm" \
      --ctrl type=unixio,path="$state/tpm/swtpm-sock" --pid file="$state/tpm/swtpm.pid" \
      --log file="$state/tpm/swtpm.log" -d
  fi
  # Two NICs: the LAN (DHCP from the edge, mDNS to find it) and a restricted
  # user network that carries ONLY the host's port forward to the web UI —
  # restrict=on means the box cannot reach anything through it, so its one
  # road to the internet is the edge's NAT, and the edge going away really
  # does leave it with no edge anywhere.
  in_tools qemu-system-x86_64 \
    -name losos-box -machine q35 "${accel_args[@]}" -smp "$box_cpus" -m "$box_mem" \
    -drive file="$state/box.qcow2",if=virtio,format=qcow2,discard=unmap \
    -cdrom "$state/losos.iso" -boot order="$boot" \
    -netdev vde,id=lan,sock="$state/lan.sock" -device virtio-net-pci,netdev=lan,mac=52:54:00:b0:11:01 \
    -netdev user,id=host,restrict=on,hostfwd=tcp:127.0.0.1:"$box_port"-:80 -device virtio-net-pci,netdev=host,mac=52:54:00:b0:11:02 \
    -chardev socket,id=chrtpm,path="$state/tpm/swtpm-sock" -tpmdev emulator,id=tpm0,chardev=chrtpm -device tpm-tis,tpmdev=tpm0 \
    -device virtio-rng-pci \
    -display none -vnc 127.0.0.1:"${LOSOS_BOX_VNC:-7}" \
    -qmp unix:"$state/box.qmp",server,nowait \
    -serial file:"$state/box-serial.log" \
    -daemonize -pidfile "$state/box.pid"
}

box_api() { # box_api METHOD PATH [JSON]
  local method=$1 path=$2 data=${3:-}
  local token=""
  [ -s "$state/admin-key" ] && token="$(cat "$state/admin-key")"
  in_tools curl -sS --max-time 15 -X "$method" \
    ${token:+-H "Authorization: Bearer $token"} \
    ${data:+-H "Content-Type: application/json" --data-binary "$data"} \
    -w '\n%{http_code}' "http://127.0.0.1:$box_port$path"
}

screen_digest() { # a digest of the box's console, for the "is it done" heuristic
  qmp "$state/box.qmp" '"screendump","arguments":{"filename":"'"$state"'/screen.ppm"}' >/dev/null
  [ -s "$state/screen.ppm" ] && in_tools sha256sum "$state/screen.ppm" | cut -c1-16 || echo none
}

box_install() {
  [ -r "$state/losos.iso" ] || die "no ISO; run: $0 build"
  if [ -s "$state/installed" ]; then log "box already installed ($state/box.qcow2)"; return; fi
  alive "$state/box.qmp" && die "a box VM is already running"
  lan_up
  rm -f "$state/box.qcow2"
  in_tools qemu-img create -q -f qcow2 "$state/box.qcow2" "${box_disk_gb}G"
  log "booting the installer ISO; the box installs itself unattended ($(have_kvm && echo 'a few minutes' || echo 'about 45 minutes without KVM'))"
  log "watch it on VNC 127.0.0.1:$((5900 + ${LOSOS_BOX_VNC:-7})) if you like"
  box_qemu d
  # The installer ends on a shell that says "done. Remove the install medium
  # and reboot"; there is no serial copy of that line, so completion is read
  # off the machine: the console stops changing and the disk stops growing.
  local started=$SECONDS last="" same=0 size="" lastsize="" minutes
  while :; do
    sleep 30
    alive "$state/box.qmp" || die "the box VM exited during the install; see $state/box-serial.log"
    local d; d=$(screen_digest); size=$(stat -c %s "$state/box.qcow2")
    if [ "$d" = "$last" ] && [ "$size" = "$lastsize" ]; then same=$((same + 1)); else same=0; fi
    last=$d; lastsize=$size
    minutes=$(( (SECONDS - started) / 60 ))
    printf '\r  installing… %d min, disk %d MiB   ' "$minutes" $((size / 1048576)) >&2
    # Six quiet samples (three minutes) after the first ten minutes.
    if [ $same -ge 6 ] && [ $minutes -ge 10 ]; then break; fi
    if [ $minutes -ge 150 ]; then die "the install did not finish in 150 minutes"; fi
  done
  echo >&2
  log "install finished after $minutes min; shutting the installer down"
  qmp "$state/box.qmp" quit >/dev/null; sleep 2
  date -u +%FT%TZ >"$state/installed"
}

box_boot() {
  [ -s "$state/installed" ] || box_install
  if alive "$state/box.qmp"; then log "box already running"; return; fi
  lan_up
  log "booting the installed box from its disk"
  box_qemu c
  log "waiting for the box's web UI (http://127.0.0.1:$box_port/)"
  for _ in $(seq 1 300); do
    if box_api GET /api/health 2>/dev/null | tail -1 | grep -q 200; then break; fi
    sleep 2
  done
  box_api GET /api/health | tail -1 | grep -q 200 || die "the box did not answer in 10 minutes; see $state/box-serial.log"
  log "box is up"
}

box_claim() {
  if [ -s "$state/admin-key" ]; then log "box already claimed; admin key in $state/admin-key"; return; fi
  log "waiting for LosOS cloud to finish its first start so the box can be claimed (minutes; ~45 without KVM)"
  local out code
  while :; do
    out=$(box_api GET /api/setup/claim); code=$(tail -1 <<<"$out")
    if [ "$code" = 200 ] && grep -q '"ready": *true' <<<"$out"; then break; fi
    if grep -q '"claimed": *true' <<<"$out"; then
      die "the box is already claimed and this run has no admin key; run: $0 clean"
    fi
    sleep 10
  done
  # The claim is the first-run password. From here it is a plain request:
  # the box names an IP literal in Host (the forward's), which the claim
  # route accepts as "this box".
  out=$(box_api POST /api/setup/claim "{\"password\":$(in_tools jq -Rn --arg p "$box_password" '$p')}")
  code=$(tail -1 <<<"$out")
  [ "$code" = 200 ] || die "claim answered $code: $(head -n -1 <<<"$out")"
  head -n -1 <<<"$out" | in_tools jq -r .token >"$state/admin-key"
  chmod 600 "$state/admin-key"
  log "claimed. Password: $box_password   Admin key (spare): $(cat "$state/admin-key")"
}

cmd_up() {
  edge_up
  box_boot
  box_claim
  log "ready. Open http://127.0.0.1:$box_port/mesh and sign in with the password; then: $0 walk"
}

# ── the walkthrough ────────────────────────────────────────────────────────
edge_doc() { box_api GET /api/edge | head -n -1; }

wait_edge() { # wait_edge true|false
  local want=$1 doc
  for _ in $(seq 1 30); do
    doc=$(edge_doc)
    if [ "$(in_tools jq -r .reachable <<<"$doc")" = "$want" ]; then echo "$doc"; return; fi
    sleep 5
  done
  die "the box still says reachable != $want after 150 s: $doc"
}

cmd_walk() {
  [ -s "$state/admin-key" ] || die "the box is not claimed; run: $0 up"
  local doc
  log "1. What the box found when it last looked for an edge proxy (GET /api/edge):"
  doc=$(wait_edge true); in_tools jq . <<<"$doc"
  log "   -> reachable, found on the LAN: $(in_tools jq -r '.edges[0].name + " at " + .edges[0].url' <<<"$doc")"
  log "   The Mesh pane shows this at the top and its Join switch is live."

  log "2. Powering the edge off."
  edge_off
  log "   Waiting for the box to notice (it looks every 20 s)…"
  doc=$(wait_edge false); in_tools jq . <<<"$doc"
  log "   -> no edge proxy found; the pane says so and the switches are greyed."

  log "3. Asking the box to share its storage anyway (POST /api/change mesh):"
  local out code
  out=$(box_api POST /api/change '{"mode":"mesh"}'); code=$(tail -1 <<<"$out")
  head -n -1 <<<"$out" | in_tools jq .
  [ "$code" = 409 ] && log "   -> refused with 409, in the box's own words. Nothing was written." \
                    || die "expected a 409, got $code"

  log "4. Something that is not sharing still works: the box's settings read back fine."
  box_api GET /api/settings | head -n -1 | in_tools jq '{hostName, clusterEnable, sharingMyStorage}'

  log "5. Bringing the edge back."
  edge_up
  doc=$(wait_edge true)
  log "   -> found again: $(in_tools jq -r '.edges[0].url' <<<"$doc"). The gate is open; the Join switch is live."
  log "done. To see the box actually accept a mesh join now: $0 join (starts a real rebuild on the box)."
}

cmd_join() {
  [ -s "$state/admin-key" ] || die "the box is not claimed; run: $0 up"
  local out code
  out=$(box_api POST /api/change '{"mode":"mesh"}'); code=$(tail -1 <<<"$out")
  head -n -1 <<<"$out" | in_tools jq .
  [ "$code" = 200 ] || die "the box refused ($code); is the edge up?"
  log "accepted: the box is rebuilding into mesh mode. Follow it with: $0 api GET /api/status"
}

cmd_api() { box_api "$@"; echo; }

cmd_edge() {
  case ${1:-} in
    off) edge_off ;;
    on) edge_up ;;
    *) die "usage: $0 edge off|on" ;;
  esac
}

cmd_status() {
  printf 'LAN switch: %s\n' "$([ -S "$state/lan.sock/ctl" ] && echo up || echo down)"
  printf 'edge VM:    %s\n' "$(alive "$state/edge.qmp" && echo running || echo stopped)"
  printf 'box VM:     %s (%s)\n' "$(alive "$state/box.qmp" && echo running || echo stopped)" \
    "$([ -s "$state/installed" ] && echo installed || echo 'not installed')"
  [ -s "$state/admin-key" ] && printf 'box:        claimed; web UI http://127.0.0.1:%s/\n' "$box_port"
  if alive "$state/box.qmp" && [ -s "$state/admin-key" ]; then
    printf 'edge seen:  %s\n' "$(edge_doc | in_tools jq -c '{reachable, edges: [.edges[].url]}')"
  fi
}

cmd_down() {
  log "stopping the VMs"
  alive "$state/box.qmp" && qmp "$state/box.qmp" quit >/dev/null
  edge_off
  [ -f "$state/tpm/swtpm.pid" ] && kill "$(cat "$state/tpm/swtpm.pid")" 2>/dev/null || true
  [ -f "$state/vde.pid" ] && kill "$(cat "$state/vde.pid")" 2>/dev/null || true
  rm -f "$state/vde.pid"
  log "down (disks kept in $state)"
}

cmd_clean() {
  cmd_down
  rm -rf "$state"
  log "clean"
}

case ${1:-} in
  build|up|walk|join|api|edge|status|down|clean) cmd=$1; shift; "cmd_$cmd" "$@" ;;
  *) sed -n '2,40p' "$0" | sed 's/^# \{0,1\}//'; exit 1 ;;
esac
