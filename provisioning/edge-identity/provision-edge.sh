#!/usr/bin/env bash
# Give one edge an official LosOS identity, from the machine that holds the
# root key, over SSH.
#
#   provision-edge.sh --name "LosOS edge Berlin" --url https://register.losos.cfd \
#                     --ssh root@edge.example --root-key root.key [--days 365]
#
# What it does, in order:
#   1. makes a fresh Ed25519 keypair for the edge (here, in a private tempdir;
#      the private half is shipped once and shredded);
#   2. signs a certificate for --name/--url with the root key (--days bounds it);
#   3. ships the key (0600) and the certificate over SSH to the edge's
#      losos.edge.identity.{keyFile,certFile} paths and restarts the registrar;
#   4. fetches GET <url>/identity?nonce=<fresh> and runs the box's four checks
#      on the answer (`losos-registrar identity verify`).
#
# The SSH account must be root or have passwordless sudo for `install` and
# `systemctl`. Step 4 fails until the edge's configuration carries the two
# identity options (see README.md: provision first, then enable); rerun with
# --verify-only after the rebuild. Needs: losos-registrar, ssh, tar, curl, od.
set -euo pipefail

name="" url="" target="" root_key="${LOSOS_ROOT_KEY_FILE:-}" days=365
key_path=/var/secrets/losos-edge-identity.key
cert_path=/etc/losos/edge-identity.cert.json
ssh_key="" known_hosts="" verify_only=0
while [ $# -gt 0 ]; do
  case "$1" in
    --name) name=$2; shift 2 ;;
    --url) url=$2; shift 2 ;;
    --ssh) target=$2; shift 2 ;;
    --root-key) root_key=$2; shift 2 ;;
    --days) days=$2; shift 2 ;;
    --key-path) key_path=$2; shift 2 ;;
    --cert-path) cert_path=$2; shift 2 ;;
    --ssh-key) ssh_key=$2; shift 2 ;;
    --known-hosts) known_hosts=$2; shift 2 ;;
    --verify-only) verify_only=1; shift ;;
    -h|--help) sed -n '2,24p' "$0"; exit 0 ;;
    *) echo "unknown argument: $1" >&2; exit 64 ;;
  esac
done
[ -n "$url" ] && [ -n "$root_key" ] || { echo "need --url and --root-key (or LOSOS_ROOT_KEY_FILE)" >&2; exit 64; }
url=${url%/}
case "$url" in http://*|https://*) ;; *) echo "--url must be http(s)://..." >&2; exit 64 ;; esac

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
chmod 700 "$tmp"

root_public=$(losos-registrar identity show --key "$root_key")

verify() {
  local nonce
  nonce=$(od -An -N32 -tx1 /dev/urandom | tr -d ' \n')
  if ! curl -fsS --max-time 10 "$url/identity?nonce=$nonce" > "$tmp/answer.json"; then
    echo "the edge at $url does not answer /identity yet (is losos.edge.identity.* set and the registrar restarted?)" >&2
    return 2
  fi
  losos-registrar identity verify --root-public "$root_public" --url "$url" \
    --nonce "$nonce" --answer "$tmp/answer.json"
}

if [ "$verify_only" = 1 ]; then
  verify
  exit $?
fi

[ -n "$name" ] && [ -n "$target" ] || { echo "need --name and --ssh" >&2; exit 64; }

# 1. and 2.
losos-registrar identity keygen --out "$tmp/edge.key" > "$tmp/edge.pub"
losos-registrar identity sign --root-key "$root_key" --public-key "$(cat "$tmp/edge.pub")" \
  --name "$name" --url "$url" --days "$days" > "$tmp/edge.cert.json"
echo "edge public key: $(cat "$tmp/edge.pub")"

# 3. One SSH session, the files on its stdin, nothing in argv but paths.
ssh_opts=(-o BatchMode=yes)
[ -n "$ssh_key" ] && ssh_opts+=(-i "$ssh_key")
[ -n "$known_hosts" ] && ssh_opts+=(-o "UserKnownHostsFile=$known_hosts" -o StrictHostKeyChecking=yes)
remote=$(cat <<REMOTE
set -e
if [ "\$(id -u)" != 0 ]; then sudo=sudo; else sudo=; fi
d=\$(mktemp -d)
tar -C "\$d" -xf -
\$sudo install -D -m 0600 -o root -g root "\$d/edge.key" '$key_path'
\$sudo install -D -m 0644 -o root -g root "\$d/edge.cert.json" '$cert_path'
rm -rf "\$d"
\$sudo systemctl try-restart losos-registrar.service || true
echo "installed $key_path and $cert_path"
REMOTE
)
# shellcheck disable=SC2029 # the remote script is composed here on purpose; only the two paths are interpolated
tar -C "$tmp" -cf - edge.key edge.cert.json | ssh "${ssh_opts[@]}" "$target" "$remote"
shred -u "$tmp/edge.key" 2>/dev/null || rm -f "$tmp/edge.key"

# 4.
if verify; then
  echo "done: $name at $url is official"
else
  echo "files are in place; the edge is not official yet. Set losos.edge.identity.keyFile = \"$key_path\" and certFile = \"$cert_path\" on it, rebuild, then run: $0 --url $url --root-key $root_key --verify-only" >&2
  exit 2
fi
