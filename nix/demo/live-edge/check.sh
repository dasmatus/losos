#!/usr/bin/env bash
# Probe an edge the way a box meets it: control plane over HTTPS, then the
# tunnel's two doors (QUIC on UDP, the TCP fallback), the ports only a
# machine with a plain internet connection can reach.
#
#   nix/demo/live-edge/check.sh [https://register.dasmat.us]
#
# Read-only apart from one POST /register with a made-up id and a bad token,
# which the edge answers 401 and stores nothing for. SKIP_TUNNEL=1 leaves out
# the two tunnel probes (a session whose only way out is an HTTPS proxy).
set -u

edge="${1:-https://register.dasmat.us}"
edge="${edge%/}"
host="${edge#*://}"
host="${host%%/*}"
host="${host%%:*}"
tunnel_port="${TUNNEL_PORT:-2333}"
fails=0

pass() { printf 'ok    %s\n' "$1"; }
fail() { printf 'FAIL  %s\n' "$1"; fails=$((fails + 1)); }

get() { curl -sS -m 15 -w '\n%{http_code}' "$edge$1" 2>&1; }

check_status() {
  local path=$1 want=$2 label=$3 out code
  out=$(get "$path")
  code=${out##*$'\n'}
  if [ "$code" = "$want" ]; then pass "$label ($path, $code)"; else fail "$label ($path): wanted $want, got $code"; fi
}

check_status /health 200 "control plane answers"
check_status /tunnel-pin 200 "tunnel certificate pin is published"
check_status /passport/edge-key 200 "gateway key is published"
check_status /manage/ 200 "manage page is served"

out=$(curl -sS -m 15 -w '\n%{http_code}' -H 'content-type: application/json' \
  -d '{"appliance_id":"live-edge-check","token":"not-a-real-token","hostname":"live-edge-check.invalid"}' \
  "$edge/register" 2>&1)
if [ "${out##*$'\n'}" = 401 ]; then pass "unknown box is refused (POST /register, 401)"; else fail "POST /register with a bad token: wanted 401, got ${out##*$'\n'}"; fi

if [ "${SKIP_TUNNEL:-}" = 1 ]; then
  printf 'skip  tunnel probes (SKIP_TUNNEL=1)\n'
else
  if timeout 8 bash -c "exec 3<>/dev/tcp/$host/$tunnel_port" 2>/dev/null; then
    pass "tunnel fallback door open (TCP $tunnel_port)"
  else
    fail "tunnel fallback door closed or filtered (TCP $tunnel_port)"
  fi
  # A QUIC long header with an unknown version makes a QUIC server answer
  # with a Version Negotiation packet, so a reply proves UDP gets through
  # and the tunnel is listening, with no handshake.
  reply=$(python3 -I - "$host" "$tunnel_port" <<'PY' 2>&1
import os, socket, sys
host, port = sys.argv[1], int(sys.argv[2])
pkt = bytes([0xC3]) + (0x1A2A3A4A).to_bytes(4, "big") + bytes([8]) + os.urandom(8) + bytes([8]) + os.urandom(8)
pkt += bytes(1200 - len(pkt))
s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
s.settimeout(5)
s.sendto(pkt, (host, port))
try:
    data, _ = s.recvfrom(2048)
    print("reply" if data[1:5] == bytes(4) else "other")
except OSError:
    print("silent")
PY
  )
  if [ "$reply" = reply ]; then
    pass "tunnel answers QUIC (UDP $tunnel_port)"
  else
    fail "no QUIC answer on UDP $tunnel_port ($reply)"
  fi
fi

if [ "$fails" -eq 0 ]; then printf '\nedge %s: all checks passed\n' "$edge"; else printf '\nedge %s: %s check(s) failed\n' "$edge" "$fails"; fi
exit $((fails > 0))
