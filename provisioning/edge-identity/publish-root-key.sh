#!/usr/bin/env bash
# Put the root PUBLIC key into keys/official-edge-root.pub of the LosOS
# repository on GitHub, as a pull request, so nobody pastes it by hand.
#
#   publish-root-key.sh --public-key HEX [--repo dasmatus/losos] [--base main]
#
# GITHUB_TOKEN (env): a fine-grained token with contents and pull-requests
# write on that repository. Keeps the file's comment block, replaces any key
# line, opens one PR on branch official-edge-root-key (or updates it).
# Needs: curl, jq, base64.
set -euo pipefail

public="" repo=dasmatus/losos base=main branch=official-edge-root-key path=keys/official-edge-root.pub
while [ $# -gt 0 ]; do
  case "$1" in
    --public-key) public=$2; shift 2 ;;
    --repo) repo=$2; shift 2 ;;
    --base) base=$2; shift 2 ;;
    --branch) branch=$2; shift 2 ;;
    -h|--help) sed -n '2,11p' "$0"; exit 0 ;;
    *) echo "unknown argument: $1" >&2; exit 64 ;;
  esac
done
[ "${#public}" = 64 ] && [[ "$public" =~ ^[0-9a-fA-F]+$ ]] || { echo "--public-key must be 64 hex characters" >&2; exit 64; }
: "${GITHUB_TOKEN:?GITHUB_TOKEN is not set}"

api="https://api.github.com/repos/$repo"
hdr=(-H "Authorization: Bearer $GITHUB_TOKEN" -H "Accept: application/vnd.github+json" -H "X-GitHub-Api-Version: 2022-11-28")
gh() { curl -fsS "${hdr[@]}" "$@"; }

base_sha=$(gh "$api/git/ref/heads/$base" | jq -r .object.sha)
if ! gh "$api/git/ref/heads/$branch" >/dev/null 2>&1; then
  jq -n --arg r "refs/heads/$branch" --arg s "$base_sha" '{ref:$r, sha:$s}' | gh -X POST "$api/git/refs" --data-binary @- >/dev/null
fi

current=$(gh "$api/contents/$path?ref=$branch")
file_sha=$(jq -r .sha <<<"$current")
new_content=$( { jq -r .content <<<"$current" | base64 -d | grep -v -E '^[0-9a-fA-F]{64}[[:space:]]*$'; printf '%s\n' "${public,,}"; } | base64 -w0)
jq -n --arg m "Official edge root key: publish the public half" --arg c "$new_content" --arg s "$file_sha" --arg b "$branch" \
  '{message:$m, content:$c, sha:$s, branch:$b}' | gh -X PUT "$api/contents/$path" --data-binary @- >/dev/null

existing=$(gh "$api/pulls?head=${repo%%/*}:$branch&base=$base&state=open" | jq -r '.[0].html_url // empty')
if [ -n "$existing" ]; then
  echo "$existing"
else
  jq -n --arg t "Official edge root key: publish the public half" --arg h "$branch" --arg b "$base" \
    --arg body "The LosOS root public key, published from the owner's box by the edge-identity provisioning workflow. Every box built from this tree after it merges treats edges certified by this key as official (trading allowed); nothing else changes. The private half never left the box." \
    '{title:$t, head:$h, base:$b, body:$body}' | gh -X POST "$api/pulls" --data-binary @- | jq -r .html_url
fi
