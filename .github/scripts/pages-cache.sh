#!/usr/bin/env bash
# Write the static copy of the LosOS binary cache that the GitHub Pages site
# serves at /proxy, next to the handbook.
#
#   SIGNING_KEY=<losos-1 secret key> pages-cache.sh <out-dir> <installable>...
#
# The cache at proxy.losos.dasmat.us is LosOS Desktop's GHCR proxy, a Vercel
# function. GitHub Pages runs no code, so this is not that proxy: it is the
# plain file layout `nix copy --to file://` writes (nix-cache-info,
# <hash>.narinfo, nar/<file>), which any HTTP host can serve as a cache.
#
# What it holds, and what it leaves out:
#   - the closures of the given installables, signed with the same losos-1
#     key, so a box trusts both copies alike;
#   - minus every path cache.nixos.org already serves, which a box fetches
#     from there anyway;
#   - within PAGES_CACHE_BUDGET_MIB (default 800), because a Pages site may
#     be no larger than 1 GB and the handbook shares it. Over budget, the
#     largest paths are dropped first and named in the step summary.
# Only the newest build is in it: each deploy replaces the site.
set -euo pipefail

out=${1:?usage: pages-cache.sh <out-dir> <installable>...}
shift
[ $# -gt 0 ] || { echo "pages-cache: no installables" >&2; exit 2; }
[ -n "${SIGNING_KEY:-}" ] || { echo "pages-cache: SIGNING_KEY is empty" >&2; exit 1; }
budget=$(( ${PAGES_CACHE_BUDGET_MIB:-800} * 1024 * 1024 ))
summary=${GITHUB_STEP_SUMMARY:-/dev/stderr}

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
(umask 077 && printf '%s' "$SIGNING_KEY" > "$work/cache.sec")

rm -rf "$out"
mkdir -p "$out"
out=$(realpath "$out")

# nix signs each narinfo as it writes it.
nix copy --no-check-sigs \
  --to "file://$out?compression=zstd&parallel-compression=true&secret-key=$work/cache.sec" \
  "$@"

# Ordered after the LosOS proxy (30) and cache.nixos.org (40): a box asks
# this copy only for what neither of those answered.
printf 'StoreDir: /nix/store\nWantMassQuery: 1\nPriority: 45\n' > "$out/nix-cache-info"

drop() {
  local narinfo=$out/$1.narinfo
  rm -f "$out/$(sed -n 's/^URL: //p' "$narinfo")" "$narinfo"
}

find "$out" -maxdepth 1 -name '*.narinfo' -printf '%f\n' | sed 's/\.narinfo$//' > "$work/all"
# shellcheck disable=SC2016 # expanded by the inner sh
xargs -r -P 16 -I{} sh -c \
  '[ "$(curl -s -o /dev/null -w "%{http_code}" -I "https://cache.nixos.org/$1.narinfo")" = 200 ] && echo "$1"' \
  _ {} < "$work/all" > "$work/upstream" || true
while read -r hash; do drop "$hash"; done < "$work/upstream"

# Largest first, so the budget costs as few paths as possible.
for narinfo in "$out"/*.narinfo; do
  [ -e "$narinfo" ] || continue
  printf '%s %s %s\n' "$(sed -n 's/^FileSize: //p' "$narinfo")" \
    "$(basename "$narinfo" .narinfo)" "$(sed -n 's|^StorePath: /nix/store/||p' "$narinfo")"
done | sort -rn > "$work/sizes"
total=$(awk '{ s += $1 } END { print s + 0 }' "$work/sizes")
dropped=()
while [ "$total" -gt "$budget" ] && read -r size hash name; do
  drop "$hash"
  total=$(( total - size ))
  dropped+=("$name ($(( size / 1024 / 1024 )) MiB)")
done < "$work/sizes"

kept=$(find "$out" -maxdepth 1 -name '*.narinfo' | wc -l)
{
  echo "## Pages copy of the binary cache"
  echo
  echo "$kept path(s), $(( total / 1024 / 1024 )) MiB of NARs;" \
    "$(wc -l < "$work/upstream") left to cache.nixos.org."
  if [ "${#dropped[@]}" -gt 0 ]; then
    echo
    echo "Over the $(( budget / 1024 / 1024 )) MiB budget, so these are only on the LosOS proxy:"
    printf -- '- %s\n' "${dropped[@]}"
  fi
} >> "$summary"
if [ "${#dropped[@]}" -gt 0 ]; then
  echo "::warning::the Pages copy of the cache left out ${#dropped[@]} path(s) over its size budget; see the step summary"
fi
