#!/usr/bin/env bash
# Put the newest release tag into nix/version.nix on main.
#
#   sync-version.sh            commit the tag to main through the GitHub API
#   sync-version.sh --dry-run  say what it would commit, change nothing
#
# Run from a checkout of main (.github/workflows/version.yml). The newest tag
# is the highest `vX.Y.Z` (any number of parts) on the remote by version
# order, so a backfill run or a re-pushed old tag never moves the file
# backwards, and a file already ahead of every tag (bumped by hand before
# tagging) is left alone.
#
# The commit is made with the GraphQL createCommitOnBranch mutation rather
# than `git push`. GitHub signs a commit it creates for GITHUB_TOKEN, which
# the repository's ruleset requires on every branch, and the mutation takes
# the head it expects, so it only ever fast-forwards main. When main moves
# between the checkout and the commit, the script reads the new head and
# tries again.
#
# Needs git, jq, sort -V and, without --dry-run, gh with GH_TOKEN and REPO
# (owner/name) set.
set -euo pipefail

dry_run=false
case "${1:-}" in
  --dry-run) dry_run=true ;;
  "") ;;
  *)
    echo "usage: $0 [--dry-run]" >&2
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

# The value is the one line that is a quoted string; the rest is comments.
read_value() { sed -n 's/^"\(.*\)"$/\1/p' "$1"; }

# The file as it should read, written beside the checkout's copy, which is
# never touched.
next=$(mktemp)
trap 'rm -f "$next"' EXIT

for attempt in 1 2 3; do
  current=$(read_value "$file")
  if [ "$current" = "$newest" ]; then
    echo "$file already says $newest"
    exit 0
  fi
  if [ -n "$current" ] && [ "$(printf '%s\n%s\n' "$current" "$newest" | sort -V | tail -n 1)" = "$current" ]; then
    echo "$file says $current, ahead of the newest tag $newest; leaving it"
    exit 0
  fi

  sed "s/^\".*\"\$/\"${newest}\"/" "$file" >"$next"
  if [ "$(read_value "$next")" != "$newest" ]; then
    echo "::error::could not find the quoted version line in $file" >&2
    exit 1
  fi
  echo "$file: ${current:-nothing} -> $newest"
  if $dry_run; then
    diff -u --label "a/$file" --label "b/$file" "$file" "$next" || true
    exit 0
  fi

  head=$(git rev-parse HEAD)
  body=$(jq -n \
    --arg repo "$REPO" \
    --arg head "$head" \
    --arg path "$file" \
    --arg contents "$(base64 -w 0 "$next")" \
    --arg headline "Set the LosOS version to $newest" \
    --arg text "$newest is the newest release tag. Boxes built from main show it on the boot menus, the boot screen, os-release and the console banners." \
    '{
      query: "mutation($input: CreateCommitOnBranchInput!) { createCommitOnBranch(input: $input) { commit { oid url } } }",
      variables: { input: {
        branch: { repositoryNameWithOwner: $repo, branchName: "main" },
        expectedHeadOid: $head,
        message: { headline: $headline, body: $text },
        fileChanges: { additions: [ { path: $path, contents: $contents } ] }
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
  # Most often main moved since the checkout: start again from its new head.
  git fetch -q --depth 1 origin main
  git checkout -q --detach FETCH_HEAD
done

echo "::error::could not commit $newest to $file on main. If the refusal above names a rule, the repository's ruleset has to let GitHub Actions commit to main (signed, fast-forward only)." >&2
exit 1
