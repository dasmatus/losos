#!/usr/bin/env bash
# Make the LosOS root key, once, and keep it where only this Forgejo's
# workflows can read it: as an Actions secret on the provisioning repository.
#
#   root-keygen.sh --forgejo-url http://box.local/forgejo --repo owner/repo \
#                  --secret-name LOSOS_ROOT_KEY [--public-out root.pub]
#
# FORGEJO_TOKEN (env) must be an access token of the repository's owner with
# write access to it. The private key exists on disk only inside this
# script's private tempdir and is shredded before it exits; what remains is
# the secret in Forgejo (encrypted at rest) and the public key, printed and
# written to --public-out. Refuses to run if the secret already exists:
# a second root key would silently orphan every certificate the first one
# signed. Needs: losos-registrar, curl, jq.
set -euo pipefail

forgejo_url="" repo="" secret_name=LOSOS_ROOT_KEY public_out=root.pub
while [ $# -gt 0 ]; do
  case "$1" in
    --forgejo-url) forgejo_url=${2%/}; shift 2 ;;
    --repo) repo=$2; shift 2 ;;
    --secret-name) secret_name=$2; shift 2 ;;
    --public-out) public_out=$2; shift 2 ;;
    -h|--help) sed -n '2,15p' "$0"; exit 0 ;;
    *) echo "unknown argument: $1" >&2; exit 64 ;;
  esac
done
[ -n "$forgejo_url" ] && [ -n "$repo" ] || { echo "need --forgejo-url and --repo" >&2; exit 64; }
: "${FORGEJO_TOKEN:?FORGEJO_TOKEN is not set}"

api="$forgejo_url/api/v1/repos/$repo/actions/secrets"
auth=(-H "Authorization: token $FORGEJO_TOKEN")

existing=$(curl -fsS "${auth[@]}" "$api") \
  || { echo "cannot list the Actions secrets of $repo at $forgejo_url (is FORGEJO_TOKEN an owner token with repository write?)" >&2; exit 1; }
if jq -e --arg n "$secret_name" 'map(.name) | index($n) != null' <<<"$existing" >/dev/null; then
  echo "the secret $secret_name already exists on $repo; not making a second root key" >&2
  exit 3
fi

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
chmod 700 "$tmp"
public=$(losos-registrar identity keygen --out "$tmp/root.key")

jq -n --rawfile d "$tmp/root.key" '{data: ($d | rtrimstr("\n"))}' \
  | curl -fsS "${auth[@]}" -H 'Content-Type: application/json' -X PUT "$api/$secret_name" --data-binary @- >/dev/null
shred -u "$tmp/root.key" 2>/dev/null || rm -f "$tmp/root.key"

printf '%s\n' "$public" > "$public_out"
echo "$public"
