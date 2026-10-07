#!/usr/bin/env bash
# demo/edge-lan/run.sh — two losos boxes and an edge proxy on one virtual
# network, without the edge first, then with it.
#
# What it shows (the order is the verification checklist's,
# demo/edge-lan/CHECKLIST.md): scenario A, no edge anywhere — both boxes
# search their network, find nothing, refuse to turn sharing on in their own
# words, keep serving their owners, and one of them is rebooted to show the
# state holds; scenario B, the edge is switched on — both boxes find it by
# mDNS within a scan, the Mesh pane says so and the sharing switches go live;
# the edge is switched off and both notice; it returns and the gate reopens.
# See wiki/Mesh.md, "Finding the edge", and backend/src/edge.rs.
#
# Host requirements (everything else comes through `nix shell`):
#   * nix with flakes (https://nixos.org/download), 20 GB of free disk per
#     box, RAM to give each box (LOSOS_BOX_MEM, 8 GB default; 4 GB works for
#     the walkthrough);
#   * /dev/kvm for a demo that takes minutes rather than hours — without it
#     QEMU falls back to software emulation and each install takes about 45
#     minutes;
#   * no root: the VMs talk over a VDE switch (a unix socket), and the host
#     reaches them through QEMU port forwards on 127.0.0.1.
#
# Usage:
#   demo/edge-lan/run.sh build [--iso PATH]   build the edge VM and the ISO
#   demo/edge-lan/run.sh up                   start the LAN and the edge,
#                                             install each box from the ISO,
#                                             boot and claim them (prints the
#                                             admin keys)
#   demo/edge-lan/run.sh walk                 the walkthrough, over the API:
#                                             scenario A (edge off) then B
#   demo/edge-lan/run.sh join [N]             turn a box's mesh join on for
#                                             real (a rebuild on the box)
#   demo/edge-lan/run.sh edge off|on          power the edge off, or back on
#   demo/edge-lan/run.sh api [N] METHOD PATH  one request to box N (default 1)
#   demo/edge-lan/run.sh status               what is running
#   demo/edge-lan/run.sh down                 stop everything (keeps disks)
#   demo/edge-lan/run.sh clean                stop and delete the state dir
#
# Knobs (environment variables):
#   LOSOS_BOXES         2     how many boxes (1 shows the gate on one box)
#   LOSOS_BOX_PORT      8080  box 1's web UI on the host; box N is +N-1
#                             (http://127.0.0.1:8080/, http://127.0.0.1:8081/)
#   LOSOS_EDGE_API_PORT 8443  the edge's registrar http://127.0.0.1:8443/health
#   LOSOS_BOX_MEM/CPUS/DISK_GB, LOSOS_BOX_PASSWORD, LOSOS_FLAKE, LOSOS_NIX_ARGS
#
# State lives in $LOSOS_DEMO_STATE (default demo/edge-lan/state, gitignored):
# the VDE socket, the disks, QMP sockets, logs, and the admin keys.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo="$(cd "$here/../.." && pwd)"
state="${LOSOS_DEMO_STATE:-$here/state}"
boxes="${LOSOS_BOXES:-2}"
box_port="${LOSOS_BOX_PORT:-8080}"
edge_api_port="${LOSOS_EDGE_API_PORT:-8443}"
box_mem="${LOSOS_BOX_MEM:-8192}"
box_cpus="${LOSOS_BOX_CPUS:-4}"
box_disk_gb="${LOSOS_BOX_DISK_GB:-40}"
# The password the claim sets on the boxes. Demo only; change it after.
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

# ── the boxes ──────────────────────────────────────────────────────────────
# Everything about box N is keyed on N: disk, TPM state, QMP socket, MACs,
# the host port (box_port + N - 1) and the admin key. The two boxes are
# identical installs from the same ISO, which is the point: nothing on a box
# is demo-specific or knows about the other.
port_of() { echo $((box_port + $1 - 1)); }
key_of() { echo "$state/admin-key-$1"; }

box_qemu() { # box_qemu N BOOT(d|c) — start box N; stdout is QEMU's
  local n=$1 boot=$2
  rm -f "$state/box$n.qmp"
  mkdir -p "$state/tpm$n"
  # swtpm: the installer enrols the disk key into it, so the installed box
  # unlocks with no passphrase. Same state dir for the install and every boot.
  if ! [ -S "$state/tpm$n/swtpm-sock" ] || ! kill -0 "$(cat "$state/tpm$n/swtpm.pid" 2>/dev/null)" 2>/dev/null; then
    in_tools swtpm socket --tpm2 --tpmstate dir="$state/tpm$n" \
      --ctrl type=unixio,path="$state/tpm$n/swtpm-sock" --pid file="$state/tpm$n/swtpm.pid" \
      --log file="$state/tpm$n/swtpm.log" -d
  fi
  # Two NICs: the LAN (DHCP from the edge, mDNS to find it) and a restricted
  # user network that carries ONLY the host's port forward to the web UI —
  # restrict=on means the box cannot reach anything through it, so its one
  # road to the internet is the edge's NAT, and the edge going away really
  # does leave it with no edge anywhere.
  in_tools qemu-system-x86_64 \
    -name "losos-box$n" -machine q35 "${accel_args[@]}" -smp "$box_cpus" -m "$box_mem" \
    -drive file="$state/box$n.qcow2",if=virtio,format=qcow2,discard=unmap \
    -cdrom "$state/losos.iso" -boot order="$boot" \
    -netdev vde,id=lan,sock="$state/lan.sock" -device virtio-net-pci,netdev=lan,mac="52:54:00:b0:1$n:01" \
    -netdev user,id=host,restrict=on,hostfwd=tcp:127.0.0.1:"$(port_of "$n")"-:80 -device virtio-net-pci,netdev=host,mac="52:54:00:b0:1$n:02" \
    -chardev socket,id=chrtpm,path="$state/tpm$n/swtpm-sock" -tpmdev emulator,id=tpm0,chardev=chrtpm -device tpm-tis,tpmdev=tpm0 \
    -device virtio-rng-pci \
    -display none -vnc 127.0.0.1:"$((${LOSOS_BOX_VNC:-7} + n - 1))" \
    -qmp unix:"$state/box$n.qmp",server,nowait \
    -serial file:"$state/box$n-serial.log" \
    -daemonize -pidfile "$state/box$n.pid"
}

box_api() { # box_api N METHOD PATH [JSON]
  local n=$1 method=$2 path=$3 data=${4:-}
  local token=""
  [ -s "$(key_of "$n")" ] && token="$(cat "$(key_of "$n")")"
  in_tools curl -sS --max-time 15 -X "$method" \
    ${token:+-H "Authorization: Bearer $token"} \
    ${data:+-H "Content-Type: application/json" --data-binary "$data"} \
    -w '\n%{http_code}' "http://127.0.0.1:$(port_of "$n")$path"
}

screen_digest() { # screen_digest N — a digest of the console, for the "is it done" heuristic
  qmp "$state/box$1.qmp" '"screendump","arguments":{"filename":"'"$state"'/screen'"$1"'.ppm"}' >/dev/null
  [ -s "$state/screen$1.ppm" ] && in_tools sha256sum "$state/screen$1.ppm" | cut -c1-16 || echo none
}

box_install() { # box_install N
  local n=$1
  [ -r "$state/losos.iso" ] || die "no ISO; run: $0 build"
  if [ -s "$state/installed$n" ]; then log "box $n already installed ($state/box$n.qcow2)"; return; fi
  alive "$state/box$n.qmp" && die "box $n's VM is already running"
  lan_up
  rm -f "$state/box$n.qcow2"
  in_tools qemu-img create -q -f qcow2 "$state/box$n.qcow2" "${box_disk_gb}G"
  log "box $n: booting the installer ISO; it installs itself unattended ($(have_kvm && echo 'a few minutes' || echo 'about 45 minutes without KVM'))"
  log "watch it on VNC 127.0.0.1:$((5900 + ${LOSOS_BOX_VNC:-7} + n - 1)) if you like"
  box_qemu "$n" d
  # The installer ends on a shell that says "done. Remove the install medium
  # and reboot"; there is no serial copy of that line, so completion is read
  # off the machine: the console stops changing and the disk stops growing.
  local started=$SECONDS last="" same=0 size="" lastsize="" minutes
  while :; do
    sleep 30
    alive "$state/box$n.qmp" || die "box $n's VM exited during the install; see $state/box$n-serial.log"
    local d; d=$(screen_digest "$n"); size=$(stat -c %s "$state/box$n.qcow2")
    if [ "$d" = "$last" ] && [ "$size" = "$lastsize" ]; then same=$((same + 1)); else same=0; fi
    last=$d; lastsize=$size
    minutes=$(( (SECONDS - started) / 60 ))
    printf '\r  box %s installing… %d min, disk %d MiB   ' "$n" "$minutes" $((size / 1048576)) >&2
    # Six quiet samples (three minutes) after the first ten minutes.
    if [ $same -ge 6 ] && [ $minutes -ge 10 ]; then break; fi
    if [ $minutes -ge 150 ]; then die "box $n's install did not finish in 150 minutes"; fi
  done
  echo >&2
  log "box $n: install finished after $minutes min; shutting the installer down"
  qmp "$state/box$n.qmp" quit >/dev/null; sleep 2
  date -u +%FT%TZ >"$state/installed$n"
}

box_boot() { # box_boot N
  local n=$1
  [ -s "$state/installed$n" ] || box_install "$n"
  if alive "$state/box$n.qmp"; then log "box $n already running"; return; fi
  lan_up
  log "box $n: booting from its disk"
  box_qemu "$n" c
}

wait_box() { # wait_box N — until the web UI answers
  local n=$1
  log "box $n: waiting for the web UI (http://127.0.0.1:$(port_of "$n")/)"
  for _ in $(seq 1 300); do
    if box_api "$n" GET /api/health 2>/dev/null | tail -1 | grep -q 200; then log "box $n is up"; return; fi
    sleep 2
  done
  die "box $n did not answer in 10 minutes; see $state/box$n-serial.log"
}

box_claim() { # box_claim N
  local n=$1
  if [ -s "$(key_of "$n")" ]; then log "box $n already claimed; admin key in $(key_of "$n")"; return; fi
  log "box $n: waiting for LosOS cloud to finish its first start so the box can be claimed (minutes; ~45 without KVM)"
  local out code
  while :; do
    out=$(box_api "$n" GET /api/setup/claim); code=$(tail -1 <<<"$out")
    if [ "$code" = 200 ] && grep -q '"ready": *true' <<<"$out"; then break; fi
    if grep -q '"claimed": *true' <<<"$out"; then
      die "box $n is already claimed and this run has no admin key; run: $0 clean"
    fi
    sleep 10
  done
  # The claim is the first-run password. From here it is a plain request:
  # the box names an IP literal in Host (the forward's), which the claim
  # route accepts as "this box".
  out=$(box_api "$n" POST /api/setup/claim "{\"password\":$(in_tools jq -Rn --arg p "$box_password" '$p')}")
  code=$(tail -1 <<<"$out")
  [ "$code" = 200 ] || die "box $n: claim answered $code: $(head -n -1 <<<"$out")"
  head -n -1 <<<"$out" | in_tools jq -r .token >"$(key_of "$n")"
  chmod 600 "$(key_of "$n")"
  log "box $n claimed. Password: $box_password   Admin key (spare): $(cat "$(key_of "$n")")"
}

each_box() { for n in $(seq 1 "$boxes"); do "$@" "$n"; done; }

cmd_up() {
  # The edge is the LAN's DHCP server and its road to the internet, and the
  # installer needs both; so it is up for the install and the first boot.
  # `walk` switches it off before scenario A.
  edge_up
  # Sequential: two software-emulated installs at once would halve each
  # other's speed, and the heuristic that reads completion off the console
  # is per box anyway.
  each_box box_install
  each_box box_boot
  each_box wait_box
  each_box box_claim
  log "ready. Open http://127.0.0.1:$box_port/mesh (box 1) and http://127.0.0.1:$((box_port + boxes - 1))/mesh (box $boxes), sign in with the password; then: $0 walk"
}

# ── the walkthrough ────────────────────────────────────────────────────────
edge_doc() { box_api "$1" GET /api/edge | head -n -1; }

wait_edge_box() { # wait_edge_box N true|false
  local n=$1 want=$2 doc
  for _ in $(seq 1 30); do
    doc=$(edge_doc "$n")
    if [ "$(in_tools jq -r .reachable <<<"$doc")" = "$want" ]; then break; fi
    sleep 5
    doc=""
  done
  [ -n "$doc" ] || die "box $n still says reachable != $want after 150 s: $(edge_doc "$n")"
  printf '   box %s: ' "$n" >&2; in_tools jq -c '{reachable, official, edges: [.edges[] | {url, source, official}]}' <<<"$doc" >&2
}

wait_edge() { # wait_edge true|false — on every box
  local n
  for n in $(seq 1 "$boxes"); do wait_edge_box "$n" "$1"; done
}

refuse_mesh() { # refuse_mesh N — the gate closed: 409 in the daemon's words
  local n=$1 out code
  out=$(box_api "$n" POST /api/change '{"mode":"mesh"}'); code=$(tail -1 <<<"$out")
  [ "$code" = 409 ] || die "box $n: expected a 409 with no edge, got $code: $(head -n -1 <<<"$out")"
  printf '   box %s: 409 — %s\n' "$n" "$(head -n -1 <<<"$out" | in_tools jq -r .error)" >&2
}

cmd_walk() {
  local n
  for n in $(seq 1 "$boxes"); do [ -s "$(key_of "$n")" ] || die "box $n is not claimed; run: $0 up"; done
  each_box wait_box

  log "Scenario A — no edge proxy anywhere."
  edge_off
  log "A1. What each box found when it last looked (GET /api/edge); it looks every 20 s:"
  wait_edge false
  log "    -> No edge proxy found, on every box. The Mesh pane says so and its sharing switches are greyed."

  log "A2. Asking each box to share its storage anyway (POST /api/change mesh):"
  each_box refuse_mesh
  log "    -> refused, nothing written. The switches say \"Needs an edge proxy in reach.\""

  log "A3. Everything that is not sharing still works: settings read back on every box."
  for n in $(seq 1 "$boxes"); do
    printf '   box %s: ' "$n" >&2
    box_api "$n" GET /api/settings | head -n -1 | in_tools jq -c '{hostName, clusterEnable, sharingMyStorage}' >&2
  done

  if [ "$boxes" -ge 2 ]; then
    log "A4. Rebooting box 2; it must come back saying the same."
    qmp "$state/box2.qmp" system_reset >/dev/null
    sleep 20
    wait_box 2
    wait_edge_box 2 false
    refuse_mesh 2
    log "    -> same state after the restart: local services up, no edge, sharing refused."
  fi

  log "Scenario B — the edge is switched on."
  edge_up
  log "B1. Both boxes find it by themselves (mDNS, _losos-edge._tcp), within one scan:"
  wait_edge true
  log "    -> Edge proxy found, On this network. The Mesh pane shows one row per edge with its sign (check: official; warning: not signed by the LosOS root key — this demo edge carries no identity)."
  log "B2. The gate is open: the Join switch is live on every box. '$0 join N' performs the real join (a rebuild on the box); the pooled-storage steps B3/B4 need the mesh wiring listed in CHECKLIST.md."

  log "B5. Powering the edge off; both boxes notice within two scans and refuse again:"
  edge_off
  wait_edge false
  each_box refuse_mesh
  log "    -> and each box's own apps keep working; this is scenario A again."

  log "B6. Bringing the edge back; the gate reopens with no hand on the boxes:"
  edge_up
  wait_edge true
  log "done. The order above is demo/edge-lan/CHECKLIST.md; the recording of the same walk from the boxes' admin UI is made by demo/edge-lan/record.sh."
}

cmd_join() {
  local n=${1:-1} out code
  [ -s "$(key_of "$n")" ] || die "box $n is not claimed; run: $0 up"
  out=$(box_api "$n" POST /api/change '{"mode":"mesh"}'); code=$(tail -1 <<<"$out")
  head -n -1 <<<"$out" | in_tools jq .
  [ "$code" = 200 ] || die "box $n refused ($code); is the edge up?"
  log "accepted: box $n is rebuilding into mesh mode. Follow it with: $0 api $n GET /api/status"
}

cmd_api() {
  local n=1
  case ${1:-} in [0-9]*) n=$1; shift ;; esac
  box_api "$n" "$@"; echo
}

cmd_edge() {
  case ${1:-} in
    off) edge_off ;;
    on) edge_up ;;
    *) die "usage: $0 edge off|on" ;;
  esac
}

cmd_status() {
  local n
  printf 'LAN switch: %s\n' "$([ -S "$state/lan.sock/ctl" ] && echo up || echo down)"
  printf 'edge VM:    %s\n' "$(alive "$state/edge.qmp" && echo running || echo stopped)"
  for n in $(seq 1 "$boxes"); do
    printf 'box %s VM:   %s (%s)' "$n" "$(alive "$state/box$n.qmp" && echo running || echo stopped)" \
      "$([ -s "$state/installed$n" ] && echo installed || echo 'not installed')"
    [ -s "$(key_of "$n")" ] && printf '; claimed; web UI http://127.0.0.1:%s/' "$(port_of "$n")"
    echo
    if alive "$state/box$n.qmp" && [ -s "$(key_of "$n")" ]; then
      printf '  edge seen: %s\n' "$(edge_doc "$n" | in_tools jq -c '{reachable, official, edges: [.edges[].url]}')"
    fi
  done
}

cmd_down() {
  local n
  log "stopping the VMs"
  for n in $(seq 1 "$boxes"); do
    alive "$state/box$n.qmp" && qmp "$state/box$n.qmp" quit >/dev/null
    [ -f "$state/tpm$n/swtpm.pid" ] && kill "$(cat "$state/tpm$n/swtpm.pid")" 2>/dev/null || true
  done
  edge_off
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
  *) sed -n '2,48p' "$0" | sed 's/^# \{0,1\}//'; exit 1 ;;
esac
