#!/usr/bin/env bash
# Tie every LosOS version to the newest release tag.
#
#   sync-version.sh              commit the tag + manifests to main via the GitHub API
#   sync-version.sh --dry-run    show what would change, change nothing
#   sync-version.sh --write       update files in the working tree, no commit
#
# Run from a checkout of main (.github/workflows/version.yml). The newest tag
# is the highest `vX.Y.Z` (any number of parts) on the remote by version
# order, so a backfill run or a re-pushed old tag never moves anything
# backwards, and files already ahead of every tag (bumped by hand before
# tagging) are left alone.
#
# The source of truth for the release is `nix/version.nix` (the tag, `vX.Y.Z`).
# Cargo.toml and package.json carry the bare semver (`X.Y.Z`) derived from it.
# This updates: `nix/version.nix`, every first-party Cargo.toml manifest
# version (`[package]` / `[workspace.package]` `version = "..."` lines, never
# dependency versions or `version.workspace`), and every first-party
# package.json `version` field (excluding vendor/node_modules).
#
# The commit is made with the GraphQL createCommitOnBranch mutation rather
# than `git push`. GitHub signs a commit it creates for GITHUB_TOKEN, which
# the repository's ruleset requires on every branch, and the mutation takes
# the head it expects, so it only ever fast-forwards main. When main moves
# between the checkout and the commit, the script reads the new head and
# tries again.
#
# Needs git, jq, python3, sort -V and, without --dry-run/--write, gh with
# GH_TOKEN and REPO (owner/name) set.
set -euo pipefail

mode=commit
case "${1:-}" in
  --dry-run) mode=dry ;;
  --write) mode=write ;;
  "") ;;
  *)
    echo "usage: $0 [--dry-run|--write]" >&2
    exit 2
    ;;
esac

cd "$(git rev-parse --show-toplevel)"
file=nix/version.nix
tag_shape='^v[0-9]+(\.[0-9]+)*$'

newest=$(git ls-remote --tags --refs origin 'v*' |
  sed 's|.*refs/tags/||' | grep -E "$tag_shape" | sort -V | tail -n 1 || true)
if [ -z "$newest" ]; then
  echo "no release tag on the remote; nothing to do"
  exit 0
fi

# The nix/version.nix value is the one line that is a quoted string.
read_value() { sed -n 's/^"\(.*\)"$/\1/p' "$1"; }

# Rewrite manifests to `newest` (tag) / its bare semver. Prints one line per
# changed file (relative path). Idempotent.
rewrite() {
  python3 - "$1" <<'PY'
import json, re, sys
from pathlib import Path

tag = sys.argv[1]
semver = tag[1:] if tag.startswith('v') else tag
root = Path('.')
changed = []

# nix/version.nix: replace the quoted version line.
p = root / 'nix/version.nix'
s = p.read_text()
n = re.sub(r'^".*"$', f'"{tag}"', s, flags=re.M)
if n != s:
    p.write_text(n); changed.append(str(p))

# Cargo.toml: only line-start `version = "..."` (package / workspace.package),
# never dependency tables (`serde = { version = "1" }`) or `version.workspace`.
cargo_ver = re.compile(r'^version\s*=\s*"[^"]*"', re.M)
for p in sorted(root.rglob('Cargo.toml')):
    if 'vendor' in p.parts or 'node_modules' in p.parts or 'target' in p.parts:
        continue
    s = p.read_text()
    n = cargo_ver.sub(f'version = "{semver}"', s)
    if n != s:
        p.write_text(n); changed.append(str(p))

# package.json: first-party only (no vendor/node_modules), replace "version".
pkg_ver = re.compile(r'^(\s*"version"\s*:\s*)"[^"]*"', re.M)
for p in sorted(root.rglob('package.json')):
    if 'vendor' in p.parts or 'node_modules' in p.parts or 'target' in p.parts:
        continue
    s = p.read_text()
    if not pkg_ver.search(s):
        continue
    n = pkg_ver.sub(rf'\g<1>"{semver}"', s)
    if n != s:
        p.write_text(n); changed.append(str(p))

print('\n'.join(changed))
PY
}

for attempt in 1 2 3; do
  current=$(read_value "$file")
  sem="${newest#v}"
  if [ "$current" = "$newest" ]; then
    # Still ensure manifests match (idempotent); a local --write may need it.
    changed=$(rewrite "$newest")
    if [ -z "$changed" ]; then
      echo "$file and manifests already at $newest ($sem)"
      exit 0
    fi
    echo "manifests were out of sync with $newest; updated:"
    printf '%s\n' "$changed"
  else
    if [ -n "$current" ] && [ "$(printf '%s\n%s\n' "$current" "$newest" | sort -V | tail -n 1)" = "$current" ]; then
      echo "$file says $current, ahead of the newest tag $newest; leaving it"
      exit 0
    fi
    changed=$(rewrite "$newest")
    echo "$file: ${current:-nothing} -> $newest (manifests -> $sem)"
    printf '%s\n' "$changed"
  fi

  if [ "$mode" = dry ]; then
    git --no-pager diff -- .
    git checkout -q -- .  # dry-run: restore
    exit 0
  fi
  if [ "$mode" = write ]; then
    echo "wrote version $newest / $sem to the working tree"
    exit 0
  fi

  # Commit every changed file via the GitHub API (signed, fast-forward only).
  mapfile -t files < <(git diff --name-only)
  if [ "${#files[@]}" -eq 0 ]; then
    echo "nothing to commit"
    exit 0
  fi
  head=$(git rev-parse HEAD)
  additions=$(for f in "${files[@]}"; do
    jq -n --arg path "$f" --arg contents "$(base64 -w 0 "$f")" '{path:$path, contents:$contents}'
  done | jq -s .)
  body=$(jq -n \
    --arg repo "$REPO" \
    --arg head "$head" \
    --argjson additions "$additions" \
    --arg headline "Set the LosOS version to $newest" \
    --arg text "$newest is the newest release tag. Manifests and nix/version.nix show it (boot menus, boot screen, os-release, console banners, crates, packages)." \
    '{
      query: "mutation($input: CreateCommitOnBranchInput!) { createCommitOnBranch(input: $input) { commit { oid url } } }",
      variables: { input: {
        branch: { repositoryNameWithOwner: $repo, branchName: "main" },
        expectedHeadOid: $head,
        message: { headline: $headline, body: $text },
        fileChanges: { additions: $additions }
      } }
    }')
  if out=$(gh api graphql --input - <<<"$body" 2>&1); then
    echo "committed: $(jq -r '.data.createCommitOnBranch.commit.url' <<<"$out")"
    if [ -n "${GITHUB_OUTPUT:-}" ]; then
      echo "committed=$newest" >>"$GITHUB_OUTPUT"
    fi
    exit 0
  fi
  echo "attempt $attempt refused: $out" >&2
  git fetch -q --depth 1 origin main
  git checkout -q --detach FETCH_HEAD
done

echo "::error::could not commit $newest to $file/manifests on main. If the refusal above names a rule, the repository's ruleset has to let GitHub Actions commit to main (signed, fast-forward only)." >&2
exit 1
