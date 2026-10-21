#!/bin/sh
# LosOS box, edge, box demo: stage one.
#
#   curl -fsSL https://raw.githubusercontent.com/dasmatus/losos/main/nix/demo/box-edge-box/bootstrap.sh | sh
#
# Looks at this machine (Linux, x86_64, KVM, processors, memory, free disk,
# Nix), picks how big a demo it can carry, writes stage two to
# $LOSOS_DEMO_DIR/stage2.sh with those choices at its top, and runs it.
# Stage two builds or fetches everything through Nix and starts three VMs on
# one private network: box A, an edge between them, box B. Nothing is
# installed on this machine outside the Nix store and the demo folder, and
# nothing needs root except --host-mdns's one network device.
#
# Two sizes:
#   full   two boxes installed from the LosOS installer ISO and an edge VM.
#          Needs KVM, about 14 GiB of memory and 60 GB of disk.
#   light  two boxes with LosOS's control plane and admin pages and an edge
#          gateway, as small test VMs. Runs anywhere Nix runs, KVM or not,
#          with about 6 GiB of memory and 15 GB of disk.
#
# Options (after `sh -s --` when piped):
#   --full | --light   pick the size instead of letting the checks pick
#   --pause            stop at each step of the light demo until Enter, so
#                      the admin pages can be opened and looked at
#   --host-mdns        put this machine on the VMs' network too, so its own
#                      mDNS responder finds boxa.local, boxb.local and
#                      edge.local and a browser here opens them by name.
#                      Makes one network device, the one step that needs
#                      root: through sudo, doas or run0, whichever is here
#                      (LOSOS_DEMO_SU names one).
#   --write-only       write stage two and stop
#   --dir DIR          where stage two and the VMs live
#                      (default ~/.local/share/losos-demo)
#   --ref REF          the LosOS flake to use (default github:dasmatus/losos)
#
# The same settings as environment variables: LOSOS_DEMO_MODE,
# LOSOS_DEMO_DIR, LOSOS_FLAKE, LOSOS_DEMO_HOST_MDNS=1, and LOSOS_NIX_ARGS for
# extra `nix` arguments.
set -eu

mode="${LOSOS_DEMO_MODE:-auto}"
dir="${LOSOS_DEMO_DIR:-${XDG_DATA_HOME:-$HOME/.local/share}/losos-demo}"
flake="${LOSOS_FLAKE:-github:dasmatus/losos}"
nix_args="${LOSOS_NIX_ARGS:-}"
pause=0
write_only=0
host_mdns="${LOSOS_DEMO_HOST_MDNS:-0}"

say() { printf '%s\n' "$*"; }
die() { printf 'losos-demo: %s\n' "$*" >&2; exit 1; }

while [ $# -gt 0 ]; do
  case $1 in
    --full) mode=full ;;
    --light) mode=light ;;
    --pause) pause=1 ;;
    --host-mdns) host_mdns=1 ;;
    --write-only) write_only=1 ;;
    --dir) [ $# -ge 2 ] || die "--dir needs a folder"; dir=$2; shift ;;
    --ref) [ $# -ge 2 ] || die "--ref needs a flake reference"; flake=$2; shift ;;
    -h|--help) sed -n '2,40p' "$0" 2>/dev/null | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) die "unknown option $1 (try --help)" ;;
  esac
  shift
done

# ── What this machine has ──────────────────────────────────────────────────
row() { printf '  %-10s %-34s %s\n' "$1" "$2" "$3"; }
say "LosOS box, edge, box demo"
say ""
say "This machine:"

os=$(uname -s)
arch=$(uname -m)
[ "$os" = Linux ] || die "the demo runs its VMs on Linux; this is $os"
[ "$arch" = x86_64 ] || die "LosOS is built for x86_64; this machine is $arch"
row system "$os $arch" ok

if [ -r /dev/kvm ] && [ -w /dev/kvm ]; then
  kvm=1; row KVM "/dev/kvm" ok
elif [ -e /dev/kvm ]; then
  kvm=0; row KVM "/dev/kvm is there, not usable" "add yourself to the kvm group for a fast demo"
else
  kvm=0; row KVM "none" "the VMs run in software emulation"
fi

cpus=$(getconf _NPROCESSORS_ONLN 2>/dev/null || nproc 2>/dev/null || echo 1)
row processors "$cpus" ""

mem_kb=$(awk '/^MemTotal:/ {print $2}' /proc/meminfo)
avail_kb=$(awk '/^MemAvailable:/ {print $2}' /proc/meminfo)
mem_gib=$((mem_kb / 1048576))
avail_gib=$((avail_kb / 1048576))
row memory "$mem_gib GiB, $avail_gib GiB free" ""

mkdir -p "$dir"
# Absolute, so stage two finds it from any folder, and from inside $dir/out.
dir=$(cd "$dir" && pwd -P)
disk_gb=$(df -Pk "$dir" | awk 'NR == 2 {print int($4 / 1000000)}')
# The Nix store fills up too, and it may be on another disk.
store_gb=$disk_gb
[ -d /nix/store ] && store_gb=$(df -Pk /nix/store | awk 'NR == 2 {print int($4 / 1000000)}')
row disk "$disk_gb GB free in $dir" ""
[ "$store_gb" = "$disk_gb" ] || row "" "$store_gb GB free for /nix/store" ""

nix=""
if command -v nix >/dev/null 2>&1; then
  nix=$(command -v nix)
elif [ -x /nix/var/nix/profiles/default/bin/nix ]; then
  nix=/nix/var/nix/profiles/default/bin/nix
fi
if [ -n "$nix" ]; then
  row Nix "$("$nix" --version 2>/dev/null | head -1)" ok
else
  row Nix "not installed" "needed"
fi
command -v bash >/dev/null 2>&1 || die "stage two is a bash script; install bash"

# mDNS here: a responder on the network (Avahi, or systemd-resolved with
# MulticastDNS on), and programs asking it for .local names (nss-mdns, or
# resolved's nss-resolve, in the hosts line of nsswitch.conf). The VMs answer
# for their own names, but only on their own network; --host-mdns joins
# this machine to it.
mdns_host=""
if ps -e -o comm= 2>/dev/null | grep -qx avahi-daemon; then
  mdns_host=avahi
elif command -v resolvectl >/dev/null 2>&1 && resolvectl status 2>/dev/null | grep -q '+mDNS'; then
  mdns_host=resolved
fi
mdns_nss=0
grep -Eq '^hosts:.*[[:space:]](mdns[46]?(_minimal)?|resolve)([[:space:]]|$)' /etc/nsswitch.conf 2>/dev/null && mdns_nss=1
# How --host-mdns's one step becomes root: it already is, or sudo, doas or
# run0 (systemd 256 and later), the first one here.
su_cmd="${LOSOS_DEMO_SU:-}"
if [ -z "$su_cmd" ]; then
  if [ "$(id -u)" = 0 ]; then
    su_cmd=none
  else
    for c in sudo doas run0; do
      if command -v "$c" >/dev/null 2>&1; then su_cmd=$c; break; fi
    done
  fi
fi
case $mdns_host:$mdns_nss in
  avahi:1) row mDNS "avahi-daemon, nss-mdns" ok ;;
  resolved:1) row mDNS "systemd-resolved" ok ;;
  avahi:0) row mDNS "avahi-daemon, no nss-mdns" "programs here do not ask it for .local names" ;;
  resolved:0) row mDNS "systemd-resolved, no nss-resolve" "programs here do not ask it for .local names" ;;
  *) row mDNS "none" "the boxes' .local names stay inside the VMs' network" ;;
esac
say ""

if [ -z "$nix" ]; then
  say "The demo needs Nix: it brings QEMU, swtpm and every image itself, with"
  say "nothing installed outside /nix. Install it with the official installer,"
  say "open a new terminal, and run this line again:"
  say ""
  say "  sh <(curl -L https://nixos.org/nix/install) --daemon"
  say ""
  exit 1
fi

# ── How big a demo ─────────────────────────────────────────────────────────
full_ok=1
why_not_full=""
[ $kvm = 1 ] || { full_ok=0; why_not_full="no KVM (installing two boxes in software takes hours)"; }
[ "$mem_gib" -ge 14 ] || { full_ok=0; why_not_full="${why_not_full:+$why_not_full; }$mem_gib GiB of memory, 14 needed"; }
[ "$disk_gb" -ge 60 ] && [ "$store_gb" -ge 30 ] || { full_ok=0; why_not_full="${why_not_full:+$why_not_full; }60 GB of free disk needed"; }

light_ok=1
[ "$mem_gib" -ge 6 ] || { light_ok=0; say "Only $mem_gib GiB of memory: the light demo needs 6."; }
[ "$store_gb" -ge 15 ] || { light_ok=0; say "Only $store_gb GB free for /nix/store: the light demo needs 15."; }

case $mode in
  auto)
    if [ $full_ok = 1 ]; then mode=full
    elif [ $light_ok = 1 ]; then mode=light; say "Light demo: $why_not_full."
    else die "this machine is too small for either demo"
    fi ;;
  full)
    [ $full_ok = 1 ] || say "Full demo asked for, though: $why_not_full. Carrying on." ;;
  light)
    [ $light_ok = 1 ] || say "Light demo asked for on a machine below its needs. Carrying on." ;;
  *) die "LOSOS_DEMO_MODE is full, light or auto, not $mode" ;;
esac

if [ "$host_mdns" = 1 ] && [ -z "$mdns_host" ]; then
  say "--host-mdns needs an mDNS responder here: install Avahi and nss-mdns, or turn"
  say "on MulticastDNS in systemd-resolved. Carrying on without it."
  host_mdns=0
elif [ "$host_mdns" = 1 ] && [ -z "$su_cmd" ]; then
  say "--host-mdns makes one network device as root, through sudo, doas or run0,"
  say "and none of them is here. Carrying on without it."
  host_mdns=0
elif [ "$host_mdns" = 0 ] && [ -n "$mdns_host" ] && [ "$mdns_nss" = 1 ] && [ -n "$su_cmd" ]; then
  say "To open the boxes as http://boxa.local/ from this machine, add --host-mdns"
  [ "$su_cmd" = none ] || say "(it asks $su_cmd once, to make one network device)."
fi

# Memory per installed box: 8 GiB where there is room, else 4, which is
# enough for the walk through.
box_mem=4096
[ "$mem_gib" -ge 24 ] && box_mem=8192

# Pin the flake to the revision it points at now, so stage two builds the
# same thing every time it runs.
nixf() { "$nix" --extra-experimental-features 'nix-command flakes' $nix_args "$@"; }
say "Looking up $flake ..."
meta=$(nixf flake metadata --json "$flake") || die "could not read the flake $flake"
# The flake's own revision is the top-level locked.rev; every input has a
# locked.rev of its own further down, so read it as JSON, with Nix.
rev=$(LOSOS_META="$meta" nixf eval --raw --impure \
  --expr '(builtins.fromJSON (builtins.getEnv "LOSOS_META")).locked.rev or ""') || rev=""
case $flake in
  github:*)
    if [ -n "$rev" ]; then
      base=$(printf '%s' "$flake" | cut -d/ -f1-2)
      flake="$base/$rev"
    fi ;;
esac
say "Using $flake"
say ""

# ── Stage two ──────────────────────────────────────────────────────────────
stage2="$dir/stage2.sh"
{
  cat <<EOF
#!/usr/bin/env bash
# LosOS box, edge, box demo: stage two, written by bootstrap.sh on
# $(date -u +%Y-%m-%dT%H:%MZ) for this machine. Run it again any time:
#
#   bash $stage2            build what is missing, then run the demo
#   bash $stage2 build      only build
#   bash $stage2 demo       only run the demo
#   bash $stage2 down       stop the VMs (full demo; keeps the disks)
#   bash $stage2 host-mdns-off   remove the network device --host-mdns made
#   bash $stage2 clean      stop and delete everything in $dir
#
# The choices bootstrap.sh made; edit them here.
mode=$mode
flake='$flake'
dir='$dir'
nix='$nix'
nix_args='$nix_args'
kvm=$kvm
box_mem=$box_mem
pause=$pause
host_mdns=$host_mdns
mdns_host=$mdns_host
su_cmd=$su_cmd
EOF
  cat <<'EOF'

set -euo pipefail
nixf() { "$nix" --extra-experimental-features 'nix-command flakes' $nix_args "$@"; }
log() { printf '\033[1;36m[%s]\033[0m %s\n' "$(date +%H:%M:%S)" "$*" >&2; }
die() { printf '\033[1;31merror:\033[0m %s\n' "$*" >&2; exit 1; }
mkdir -p "$dir"

# ── This machine on the VMs' network (--host-mdns) ─────────────────────────
# One tap device, vde-tap77, with an address of its own on the VMs' network,
# so this machine's mDNS responder hears the VMs and answers boxa.local,
# boxb.local and edge.local for the browser here. Making it is the only step
# that needs root, and it belongs to you afterwards. The light demo's test
# driver plugs a tap of that name (vlan 77) into its network itself; the
# full demo plugs it into run.sh's switch.
tap=vde-tap77
as_root() { if [ "$su_cmd" = none ]; then "$@"; else "$su_cmd" "$@"; fi; }
ip_cmd() { command -v ip || echo "$(nixf build --no-link --print-out-paths 'nixpkgs#iproute2^out' 2>/dev/null)/bin/ip"; }
host_mdns_up() {
  [ "$host_mdns" = 1 ] || return 0
  local ip addr
  ip=$(ip_cmd)
  if [ "$mode" = light ]; then addr=192.168.77.254/24; else addr=10.77.0.254/24; fi
  if ! "$ip" -o -4 addr show dev "$tap" 2>/dev/null | grep -q "inet $addr "; then
    log "making $tap at ${addr%/*} for this machine's mDNS$([ "$su_cmd" = none ] || echo " ($su_cmd may ask for your password)")"
    as_root sh -c "'$ip' link show $tap >/dev/null 2>&1 || '$ip' tuntap add dev $tap mode tap user $(id -u)
      '$ip' addr flush dev $tap && '$ip' addr add $addr dev $tap && '$ip' link set $tap up"
  fi
}
host_mdns_link() { # resolved keeps mDNS per link: switch it on once the tap is plugged in
  [ "$host_mdns" = 1 ] && [ "$mdns_host" = resolved ] || return 0
  local _
  for _ in $(seq 120); do
    [ "$(cat "/sys/class/net/$tap/carrier" 2>/dev/null)" = 1 ] && break
    sleep 1
  done
  as_root resolvectl mdns "$tap" yes || log "could not switch mDNS on for $tap; run as root: resolvectl mdns $tap yes"
}
host_mdns_plug() { # full: plug the tap into run.sh's switch, and keep it there
  [ "$host_mdns" = 1 ] || return 0
  if [ -s "$dir/state/plug2tap.pid" ] && kill -0 "$(cat "$dir/state/plug2tap.pid")" 2>/dev/null; then return; fi
  setsid nohup "$nix" --extra-experimental-features 'nix-command flakes' $nix_args shell nixpkgs#vde2 \
    -c vde_plug2tap -s "$dir/state/lan.sock" "$tap" >"$dir/state/plug2tap.log" 2>&1 </dev/null &
  echo $! >"$dir/state/plug2tap.pid"
  host_mdns_link
}
host_mdns_report() { # what this machine's own resolver makes of the names
  [ "$host_mdns" = 1 ] || return 0
  local n a
  for n in "$@"; do
    a=$(getent ahostsv4 "$n.local" 2>/dev/null | awk '{print $1; exit}')
    if [ -n "$a" ]; then log "this machine finds $n.local at $a by mDNS: http://$n.local/"
    else log "this machine does not find $n.local: is nss-mdns or nss-resolve in nsswitch.conf, and UDP 5353 open on $tap?"
    fi
  done
}
host_mdns_off() {
  if [ -s "$dir/state/plug2tap.pid" ]; then kill "$(cat "$dir/state/plug2tap.pid")" 2>/dev/null || true; rm -f "$dir/state/plug2tap.pid"; fi
  if [ -e "/sys/class/net/$tap" ]; then as_root "$(ip_cmd)" link del "$tap" && log "removed $tap"; fi
}

# ── Light: three small VMs in one test run ─────────────────────────────────
# nix/demo/box-edge-box/demo.nix in the LosOS tree. The test driver brings
# its own QEMU and uses KVM when there is one.
# Every build goes through Nix, which hands back what it already has for this
# revision at once and builds anew when $flake has changed.
light_build() {
  log "building the three VMs (the first time compiles LosOS's control plane: tens of minutes)"
  nixf build -L -o "$dir/driver" "$flake#checks.x86_64-linux.losos-box-edge-box.driver"
}

light_pauser() { # reads the demo's steps as they happen and waits for Enter
  local rec=$1 seen=0 line n caption
  while :; do
    if [ -s "$rec/timeline.jsonl" ]; then
      n=$(wc -l <"$rec/timeline.jsonl")
      while [ "$seen" -lt "$n" ]; do
        seen=$((seen + 1))
        line=$(sed -n "${seen}p" "$rec/timeline.jsonl")
        caption=$(printf '%s' "$line" | sed -n 's/.*"caption": "\([^"]*\)".*/\1/p')
        printf '\n\033[1m%s\033[0m\n' "$caption" >/dev/tty
        if [ "$host_mdns" = 1 ]; then
          printf '  Box A: http://boxa.local/mesh   Box B: http://boxb.local/mesh\n' >/dev/tty
        else
          printf '  Box A: http://127.0.0.1:18081/mesh   Box B: http://127.0.0.1:18082/mesh\n' >/dev/tty
        fi
        printf '  Sign in on each with "Use the spare key" and the key in %s/rec/boxa.token or boxb.token.\n' "$dir" >/dev/tty
        [ -s "$rec/edge.key" ] && printf '  The edge: http://127.0.0.1:18443/manage/ with the key in %s/rec/edge.key\n' "$dir" >/dev/tty
        printf '  Press Enter for the next step. ' >/dev/tty
        read -r _ </dev/tty
        touch "$rec/ack-$seen"
      done
    fi
    kill -0 "$2" 2>/dev/null || return 0
    sleep 1
  done
}

light_demo() {
  light_build
  local out="$dir/out" rec="$dir/rec" logf="$dir/demo.log"
  rm -rf "$out" "$rec"; mkdir -p "$out" "$rec"
  host_mdns_up
  log "starting box A, box B and the edge ($([ "$kvm" = 1 ] && echo 'a few minutes' || echo 'about 15 minutes without KVM'))"
  log "the full log goes to $logf"
  local pausing=0
  [ "$pause" = 1 ] && [ -e /dev/tty ] && pausing=1
  (
    cd "$out"
    [ $pausing = 1 ] && export LOSOS_RECORD_DIR="$rec" LOSOS_RECORD_WAIT=0
    exec "$dir/driver/bin/nixos-test-driver" --no-interactive -o "$out" >"$logf" 2>&1
  ) &
  local pid=$!
  [ $pausing = 1 ] && light_pauser "$rec" "$pid" &
  # The narration: one line per step of the demo, as it passes.
  tail --pid="$pid" -n +1 -F "$logf" 2>/dev/null | grep --line-buffered -E '^=== |^box[ab] asked|found [a-z]+\.local' &
  if [ "$host_mdns" = 1 ]; then
    host_mdns_link &
    # Once the edge is up, ask this machine's resolver for all three names.
    (until grep -q '^=== 2:' "$logf" 2>/dev/null; do kill -0 "$pid" 2>/dev/null || exit 0; sleep 2; done
     host_mdns_report boxa boxb edge) &
  fi
  local rc=0
  wait "$pid" || rc=$?
  sleep 2
  if [ $rc = 0 ]; then
    log "the demo passed every step. Log: $logf"
  else
    tail -n 40 "$logf" >&2
    die "the demo stopped (exit $rc); the whole log is $logf"
  fi
}

# ── Full: two boxes installed from the ISO and an edge VM ──────────────────
# nix/demo/edge-lan/run.sh in the LosOS tree does the work; this fetches
# that tree at the pinned revision and runs it with the state kept here.
full_src() {
  log "fetching the LosOS tree"
  local path
  path=$(nixf flake prefetch --json "$flake" | sed -n 's/.*"storePath":"\([^"]*\)".*/\1/p')
  [ -n "$path" ] || die "could not fetch $flake"
  ln -sfn "$path" "$dir/src"
}
full_run() {
  [ -d "$dir/src/nix/demo/edge-lan" ] || full_src
  # run.sh calls plain `nix`: give it this one, with flakes on.
  PATH="$(dirname "$nix"):$PATH" \
    NIX_CONFIG="extra-experimental-features = nix-command flakes${NIX_CONFIG:+$'\n'$NIX_CONFIG}" \
    LOSOS_FLAKE="$flake" LOSOS_DEMO_STATE="$dir/state" LOSOS_BOX_MEM="$box_mem" \
    LOSOS_NIX_ARGS="$nix_args" bash "$dir/src/nix/demo/edge-lan/run.sh" "$@"
}
full_build() {
  full_src
  full_run build
}
full_demo() {
  full_build
  host_mdns_up
  full_run up
  host_mdns_plug
  full_run walk
  full_run publish
  host_mdns_report boxa boxb edge
  log "the boxes keep running: box A http://127.0.0.1:8080/  box B http://127.0.0.1:8081/ (the password is printed above)"
  log "stop them with: bash $0 down"
}

cmd=${1:-run}
case $mode:$cmd in
  light:build) light_build ;;
  light:demo|light:run) light_demo ;;
  light:down) log "the light demo stops its VMs when it ends" ;;
  full:build) full_build ;;
  full:demo|full:run) full_demo ;;
  full:down) host_mdns_off; full_run down ;;
  *:host-mdns-off) host_mdns_off ;;
  *:clean)
    host_mdns_off
    [ "$mode" = full ] && [ -d "$dir/src" ] && full_run clean || true
    rm -rf "$dir"; log "deleted $dir" ;;
  *) die "usage: bash $0 [build|demo|down|host-mdns-off|clean]" ;;
esac
EOF
} >"$stage2"
chmod +x "$stage2"

say "Wrote $stage2 ($mode demo)."
if [ $write_only = 1 ]; then
  say "Run it with: bash $stage2"
  exit 0
fi
say ""
exec bash "$stage2" </dev/null
