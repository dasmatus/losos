#!/usr/bin/env bash
# check-flake: evaluate every flake output, as `nix flake check --no-build`
# does, but one `nix eval` per derivation.
#
#   bash nix/check-flake.sh [nix flags…]
#
# `flake check` evaluates every output in one process and keeps all of it:
# fifty-odd VM tests, each with its own NixOS evaluations of one to three
# machines, plus every package and system. On a hosted runner that process
# grew past the runner's memory and the runner was shut down mid-check
# (around losos-keyring, losos-erase or losos-mesh-collab, wherever the
# total crossed the line), with no evaluation error printed. A process per
# derivation frees each evaluation when it ends, so the peak is a few of the
# heaviest checks (about 1.5 GB each) rather than the sum (past 14 GB).
# CHECK_FLAKE_JOBS evaluations run at once, the number of CPUs by default.
#
# Covered, matching what `flake check --no-build` checks for each kind:
#   checks, packages, devShells  the drvPath of every attribute, every system
#   nixosConfigurations          config.system.build.toplevel is a derivation
#                                (check-eval forces its drvPath, the merge)
#   nixosModules                 each one is a function or an attribute set
# Any other top-level output fails the script until it is handled here, so a
# new kind of output cannot go unchecked. Extra arguments go to every nix
# call (e.g. --override-input).
set -euo pipefail
cd "$(dirname "$0")/.."
root=$PWD
if [ -x "$root/tools/ensure-nix-portable" ]; then
  # shellcheck source=/dev/null
  . "$root/tools/ensure-nix-portable"
fi


flake="git+file://$PWD"
# A shallow clone (CI's checkout, a linked worktree of one) evaluates only
# with ?shallow=1: without it Nix looks for the history and fails on a
# missing object.
if [ "$(git rev-parse --is-shallow-repository)" = true ]; then
  flake="$flake?shallow=1"
fi
jobs=${CHECK_FLAKE_JOBS:-$(nproc)}
logs=$(mktemp -d)
trap 'rm -rf "$logs"' EXIT

# The attribute names under an output path, one per line.
names() {
  nix eval "$@" --raw --apply 'xs: builtins.concatStringsSep "\n" (builtins.attrNames xs)'
}

# One evaluation, in its own process, in the background once fewer than
# $jobs are running. A failure is recorded and the rest still run, so one
# run names every broken output.
check() {
  local label=$1
  shift
  while [ "$(jobs -rp | wc -l)" -ge "$jobs" ]; do wait -n || true; done
  echo "checking $label..."
  (nix eval "$@" >/dev/null 2>"$logs/$label.log" || touch "$logs/$label.failed") &
}

outputs=$(nix eval --impure --raw --expr \
  "builtins.concatStringsSep \"\n\" (builtins.attrNames (builtins.getFlake \"$flake\").outputs)")

for output in $outputs; do
  case $output in
    checks | packages | devShells)
      for system in $(names "${@}" "$flake#$output"); do
        for name in $(names "${@}" "$flake#$output.$system"); do
          check "$output.$system.$name" "${@}" --raw "$flake#$output.$system.\"$name\".drvPath"
        done
      done
      ;;
    nixosConfigurations)
      for name in $(names "${@}" "$flake#$output"); do
        check "$output.$name" "${@}" "$flake#$output.\"$name\".config.system.build.toplevel" \
          --apply 'd: if d.type or null == "derivation" then true else throw "toplevel is not a derivation"'
      done
      ;;
    nixosModules)
      for name in $(names "${@}" "$flake#$output"); do
        check "$output.$name" "${@}" "$flake#$output.\"$name\"" \
          --apply 'm: if builtins.isFunction m || builtins.isAttrs m then true else throw "not a module"'
      done
      ;;
    *)
      echo "no check for the output '$output'; add one to nix/check-flake.sh" >"$logs/$output.log"
      touch "$logs/$output.failed"
      ;;
  esac
done

wait

failed=0
for marker in "$logs"/*.failed; do
  [ -e "$marker" ] || continue
  label=$(basename "$marker" .failed)
  echo "check-flake: $label failed:" >&2
  cat "$logs/$label.log" >&2
  failed=$((failed + 1))
done
if [ "$failed" -gt 0 ]; then
  echo "check-flake: $failed output(s) failed to evaluate" >&2
  exit 1
fi
echo "check-flake: every output evaluated"
