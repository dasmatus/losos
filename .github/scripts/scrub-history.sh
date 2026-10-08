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
# Private rules. When SCRUB_PRIVATE_RULES names a file, a first pass runs
# the rules in it before the ones above. The file has three sections, each
# in git-filter-repo's own syntax:
#
#   [paths]     --paths-from-file lines; each path is removed from every commit
#   [text]      --replace-text lines, applied to every blob
#   [messages]  --replace-message lines, applied to every commit and tag message
#
# They are private because a rule names what it removes, so the file lives
# outside the repository (the workflow reads it from the
# HISTORY_SCRUB_PRIVATE_RULES secret). That pass prunes the commits and
# merges its rules leave empty: a commit whose only files were removed has
# nothing left to say. The pass below still prunes nothing.
#
# Commits that none of the rules fire on keep their hashes, so the second
# run over a cleaned history is a no-op and the history before the first
# offending commit never moves at all. Nothing is pruned: a commit whose
# only change was its message would otherwise be dropped as "empty".
#
# Run locally against a throwaway clone to preview the effect:
#   git clone --bare . /tmp/losos-scrub.git
#   [SCRUB_PRIVATE_RULES=rules.conf] \
#     .github/scripts/scrub-history.sh /tmp/losos-scrub.git /tmp/moved-refs
#   git -C /tmp/losos-scrub.git log --format='%h %an %s'
set -euo pipefail

repo=${1:?usage: scrub-history.sh <bare-clone> <report-file>}
report=${2:?usage: scrub-history.sh <bare-clone> <report-file>}
here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
mailmap=$work/mailmap
replacements=$work/replacements

# Every ref and where it points before anything is rewritten: the report is
# made against this, because two passes leave two ref maps behind.
git -C "$repo" for-each-ref --format='%(objectname) %(refname)' >"$work/before"

private=${SCRUB_PRIVATE_RULES:-}
if [ -n "$private" ]; then
  # Split the file into its sections, dropping comments and blank lines.
  awk -v dir="$work" '
    /^[[:space:]]*(#|$)/ { next }
    /^\[(paths|text|messages)\]$/ { section = substr($0, 2, length($0) - 2); next }
    /^\[/ { print "unknown section " $0 > "/dev/stderr"; exit 1 }
    section == "" { print "rule outside a section: " $0 > "/dev/stderr"; exit 1 }
    { print > (dir "/private-" section) }
  ' "$private"
  args=()
  if [ -s "$work/private-paths" ]; then
    args+=(--invert-paths --paths-from-file "$work/private-paths")
  fi
  if [ -s "$work/private-text" ]; then
    args+=(--replace-text "$work/private-text")
  fi
  if [ -s "$work/private-messages" ]; then
    args+=(--replace-message "$work/private-messages")
  fi
  if [ "${#args[@]}" -gt 0 ]; then
    git -C "$repo" filter-repo --force --prune-empty auto --prune-degenerate auto "${args[@]}"
  fi
fi

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

# Keep the refs that moved: `<old> <new> <refname>`, with an all-zero <new>
# for a ref whose every commit was pruned.
git -C "$repo" for-each-ref --format='%(objectname) %(refname)' >"$work/after"
awk '
  NR == FNR { after[$2] = $1; next }
  {
    new = ($2 in after) ? after[$2] : "0000000000000000000000000000000000000000"
    if (new != $1) print $1, new, $2
  }
' "$work/after" "$work/before" >"$report"
