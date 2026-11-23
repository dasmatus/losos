#!/usr/bin/env bash
# Attach assets to a tag's draft release, without touching its publish
# state. mobile-release.yml's jobs use this: ci.yml owns a release's life
# (the github-release action drafts it, attaches the installer ISO, then
# publishes it immutably), and immutable releases reject asset edits after
# the publish — so the phone builds must land while the release is still a
# draft. This script waits for ci.yml's draft to exist, folds its section
# into the release notes between its own marker comments (the same
# merge pattern the action's merge_body carries), then uploads.
#
# Usage: attach-release-asset.sh <section-id> <file> [<file> ...]
#   RELEASE_BODY  the markdown notes section to fold in (required)
#   TAG           the tag whose release to wait for (required)
#   GH_TOKEN      a token with contents:write (required)
#
# Written against curl and jq for the same reasons the github-release
# action is: explicit status checks, no third-party actions.
set -euo pipefail

[ "$#" -ge 2 ] || { echo "usage: $0 <section-id> <file> [<file> ...]" >&2; exit 2; }
SECTION_ID="$1"; shift

test -n "${GH_TOKEN:-}" || { echo "::error::no GH_TOKEN — the attach would silently do nothing" >&2; exit 1; }
test -n "${TAG:-}" || { echo "::error::no TAG — which release would this attach to?" >&2; }
test -n "${RELEASE_BODY:-}" || { echo "::error::no RELEASE_BODY section" >&2; exit 1; }

API="${GITHUB_API_URL}/repos/${GITHUB_REPOSITORY}"
# The token never reaches argv: `ps` would expose it for the whole upload.
umask 077
conf=$(mktemp)
trap 'rm -f "$conf"' EXIT
{
  printf 'header = "authorization: Bearer %s"\n' "$GH_TOKEN"
  printf 'header = "accept: application/vnd.github+json"\n'
  printf 'header = "x-github-api-version: 2022-11-28"\n'
} > "$conf"

api() {
  local m="$1" u="$2"; shift 2
  CODE=$(curl -sS -o /tmp/api.out -w '%{http_code}' -X "$m" -K "$conf" "$@" "$u")
  cat /tmp/api.out
}

# The release lane's ISO build takes hours; the phone jobs are usually
# done first. Wait (4-hour budget: 240 × 60s) for the tag's release to
# appear as a draft. A published immutable release is a hard error: its
# assets are write-once and the APK cannot be attached afterwards.
found=""
for attempt in $(seq 1 240); do
  CODE=404
  api GET "$API/releases/tags/$TAG" >/tmp/rel.json || true
  if [ "$CODE" = "200" ]; then
    if jq -e '(.draft == true)' /tmp/rel.json >/dev/null; then
      found=yes; break
    fi
    echo "::error::${TAG} is already published (immutable). Its assets are write-once; the phone builds attach to a release only while ci.yml has it as a draft. Re-run this workflow on a not-yet-published tag, or attach by hand from the release page."
    exit 1
  fi
  # A draft is invisible to the by-tag lookup; walk the list for it (the
  # github-release action carries the same two-step lookup).
  api GET "$API/releases?per_page=100" >/tmp/all.json || true
  if [ "$CODE" = "200" ] && jq -e --arg t "$TAG" \
      'map(select(.draft and .tag_name == $t)) | length > 0' /tmp/all.json >/dev/null; then
    jq --arg t "$TAG" 'map(select(.draft and .tag_name == $t)) | .[0]' /tmp/all.json >/tmp/rel.json
    found=yes; break
  fi
  echo "no release for $TAG yet (attempt ${attempt}/240); the release lane drafts it after its media builds"
  sleep 60
done
[ -n "$found" ] || { echo "::error::no draft release for $TAG appeared within the wait budget" >&2; exit 1; }

id=$(jq -r '.id // empty' /tmp/rel.json)
case "$id" in ''|*[!0-9]*) echo "::error::no usable release id" >&2; exit 1;; esac
upload=$(jq -r '.upload_url // empty' /tmp/rel.json | sed 's/{.*}$//')
echo "release id: $id"

# The notes section, between this section-id's own markers, so a re-run
# replaces only what sits between them and the ISO lane's block is
# untouched (its markers are losos-release-media's).
BEGIN="<!-- losos-mobile-${SECTION_ID}:begin -->"
END="<!-- losos-mobile-${SECTION_ID}:end -->"
jq --arg b "$BEGIN" --arg e "$END" --arg sec "$RELEASE_BODY" '
  ($b + "\n" + $sec + "\n" + $e) as $block
  | (.body // "") as $old
  | ($old | split($b)) as $p
  | if ($p | length) == 2 and ($p[1] | contains($e))
      then {body: ($p[0] + $block + ($p[1] | split($e) | .[1:] | join($e)))}
      else {body: ($old + "\n\n" + $block)}
    end
' /tmp/rel.json >/tmp/patch.json
api PATCH "$API/releases/$id" -H 'content-type: application/json' \
  --data-binary @/tmp/patch.json >/tmp/rel.json
if [ "$CODE" != "200" ]; then
  echo "::error::writing the release notes failed: HTTP $CODE" >&2
  cat /tmp/rel.json >&2; exit 1
fi
echo "release notes written"

drop_existing() {
  local want="$1" aid
  api GET "$API/releases/$id/assets?per_page=100" >/tmp/assets.json || true
  [ "$CODE" = "200" ] || return 0
  aid=$(jq -r --arg n "$want" '.[] | select(.name == $n) | .id' /tmp/assets.json | head -1)
  [ -n "$aid" ] || return 0
  api DELETE "$API/releases/assets/$aid" >/dev/null || true
  echo "replaced the existing $want (asset $aid)"
}

for f in "$@"; do
  test -f "$f" || { echo "::error::asset not found: $f" >&2; exit 1; }
  base="$(basename "$f")"
  echo "attaching $base ($(stat -c%s "$f") bytes)"
  ok=""
  for attempt in 1 2 3; do
    drop_existing "$base"
    CODE=$(curl -sS -o /tmp/api.out -w '%{http_code}' \
      -X POST -T "$f" -K "$conf" \
      -H 'content-type: application/octet-stream' -H 'Expect:' \
      --connect-timeout 30 \
      "${upload}?name=${base}")
    if [ "$CODE" = "201" ]; then ok=yes; break; fi
    echo "::warning::attaching $base attempt ${attempt}/3 returned HTTP $CODE"
    { cat /tmp/api.out; echo; } >&2 || true
    if [ "$attempt" -lt 3 ]; then sleep $((attempt * 15)); fi
  done
  test -n "$ok" || { echo "::error::attaching $base failed after 3 attempts" >&2; exit 1; }
  echo "attached $base"
done
