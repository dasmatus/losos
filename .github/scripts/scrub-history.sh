#!/usr/bin/env bash
# Rewrite the history in a bare clone so that no commit message, author
# identity or tracked file credits Claude or links a claude.ai session, then
# report which refs moved.
#
#   scrub-history.sh <bare-clone> <report-file>
#
# The clone is rewritten in place with git-filter-repo (on PATH as
# `git filter-repo`). Afterwards <report-file> holds one line per ref whose
# hash changed, `<old> <new> <refname>`, which is exactly what a
# `--force-with-lease=<refname>:<old>` push wants. An empty report means the
# history was already clean and nothing needs pushing.
#
# What it touches:
#   * commit and tag messages, through scrub-message.py (the rules live
#     there; this file only wires them up);
#   * author and committer identities: commits recorded as
#     `Claude <noreply@anthropic.com>` become the repository owner's, since
#     the tool is not an author and the owner's name is the one already on
#     the rest of the history;
#   * tracked file contents: claude.ai/code session and project links are
#     cut out, nothing else — prose that happens to say "Claude" (CLAUDE.md,
#     comments about the dev tooling) is deliberate and stays.
#
# Commits that none of the rules fire on keep their hashes, so the second
# run over a cleaned history is a no-op and the history before the first
# offending commit never moves at all. Nothing is pruned: a commit whose
# only change was its message would otherwise be dropped as "empty".
#
# Run locally against a throwaway clone to preview the effect:
#   git clone --bare . /tmp/losos-scrub.git
#   .github/scripts/scrub-history.sh /tmp/losos-scrub.git /tmp/moved-refs
#   git -C /tmp/losos-scrub.git log --format='%h %an %s'
set -euo pipefail

repo=${1:?usage: scrub-history.sh <bare-clone> <report-file>}
report=${2:?usage: scrub-history.sh <bare-clone> <report-file>}
here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)

mailmap=$(mktemp)
replacements=$(mktemp)
trap 'rm -f "$mailmap" "$replacements"' EXIT

cat >"$mailmap" <<'EOF'
Matus Mastena <330471626+dasmatus@users.noreply.github.com> Claude <noreply@anthropic.com>
EOF

# git-filter-repo --replace-text syntax: `regex:<python regex>==><replacement>`,
# applied to every blob. An empty replacement deletes the match.
cat >"$replacements" <<'EOF'
regex:https://claude\.ai/code/[^\s)]+==>
EOF

# --force: filter-repo refuses to run anywhere that does not look like a
# fresh clone, and a bare clone made a moment ago does not pass its test.
# The caller owns this clone and nothing else points at it.
git -C "$repo" filter-repo \
  --force \
  --prune-empty never \
  --prune-degenerate never \
  --mailmap "$mailmap" \
  --replace-text "$replacements" \
  --message-callback "$(cat "$here/scrub-message.py")"

# filter-repo leaves `<old> <new> <refname>` for every ref it saw, under a
# header line; keep the refs that moved.
awk '$1 ~ /^[0-9a-f]{40}$/ && $1 != $2 { print $1, $2, $3 }' \
  "$repo/filter-repo/ref-map" >"$report"
