#!/usr/bin/env bash
# Evaluates every flake output, as `nix flake check --no-build` does, but a
# few attributes per `nix eval` process. One process holds every module
# system it has evaluated until it exits, and with the VM tests that passed
# 13 GiB: GitHub's 16 GiB runners were killed partway through the checks.
#
#   nix/check-flake.sh [flake] [-- extra nix flags]
#
# CI's eval job and devenv.nix's `check-flake` run it. LOSOS_EVAL_BATCH sets
# how many attributes one process evaluates (default 6).
set -euo pipefail

flake=${1:-.}
shift || true
[ "${1:-}" = "--" ] && shift
extra=("$@")
system=x86_64-linux
batch=${LOSOS_EVAL_BATCH:-6}

names() {
  nix eval --raw "${extra[@]}" "$flake#$1" \
    --apply 'x: builtins.concatStringsSep "\n" (builtins.attrNames x)'
}

# Forces `<group>.<name><suffix>` for each name, $batch to a process.
force() {
  local group=$1 suffix=$2
  local -a all chunk
  mapfile -t all < <(names "$group")
  local i
  for ((i = 0; i < ${#all[@]}; i += batch)); do
    chunk=("${all[@]:i:batch}")
    local list=""
    for n in "${chunk[@]}"; do list+=" \"$n\""; done
    echo "evaluating $group:${list}" >&2
    nix eval --raw "${extra[@]}" "$flake#$group" \
      --apply "g: builtins.concatStringsSep \"\n\" (map (n: n + \" \" + g.\${n}$suffix) [$list ])"
    echo
  done
}

force "checks.$system" ".drvPath"
force "packages.$system" ".drvPath"
force "devShells.$system" ".drvPath"
force nixosConfigurations ".config.system.build.toplevel.drvPath"
nix eval --raw "${extra[@]}" "$flake#nixosModules" \
  --apply 'm: builtins.concatStringsSep " " (builtins.attrNames m)' >/dev/null
echo "every output evaluated" >&2
