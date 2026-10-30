#!/usr/bin/env bash
# The Nix lints over the tree, or over the files named on the command line:
# nixfmt --check, deadnix --fail and statix with nix/statix.toml.
#
#   bash nix/lint-nix.sh                  every tracked .nix file
#   bash nix/lint-nix.sh a.nix b.nix      these files only
#
# CI's eval job, devenv's `lint-nix` and the git hooks (nix/hooks) all run
# this, so a file that one of them refuses is refused by all three.
set -uo pipefail
root=$(git rev-parse --show-toplevel)
cd "$root" || exit 1

if [ $# -gt 0 ]; then
  files=("$@")
else
  mapfile -t files < <(git ls-files -- '*.nix')
fi
[ ${#files[@]} -gt 0 ] || exit 0

# Ensure nix (portable if needed) before looking for tools.
if [ -x "$root/tools/ensure-nix-portable" ]; then
  # shellcheck source=/dev/null
  . "$root/tools/ensure-nix-portable"
fi

# The three tools, from the flake's nixpkgs when any is missing.
if ! command -v nixfmt >/dev/null || ! command -v deadnix >/dev/null || ! command -v statix >/dev/null; then
  flake="git+file://$root"
  [ "$(git rev-parse --is-shallow-repository)" = true ] && flake="$flake?shallow=1"
  exec nix shell --inputs-from "$flake" nixpkgs#nixfmt-rfc-style nixpkgs#deadnix nixpkgs#statix \
    -c bash "$root/nix/lint-nix.sh" "${files[@]}"
fi

rc=0
echo "── nixfmt (${#files[@]} files)"
nixfmt --check "${files[@]}" || rc=1
echo "── deadnix"
deadnix --fail "${files[@]}" || rc=1
echo "── statix"
# statix checks one path per call.
for f in "${files[@]}"; do
  statix check --config nix/statix.toml "$f" || rc=1
done
[ "$rc" -eq 0 ] && echo "Nix lints clean"
exit "$rc"
