# Start the GHCR-backed Nix cache proxy for local `nix build` / `devenv shell`.
# CI does the same in .github/actions/setup-nix; this is the dev-machine half.
#
# Source from devenv's enterShell or `nix develop` (see flake.nix). Idempotent:
# a cache already listening on the port is left alone.
set -euo pipefail

losos_dev_cache() {
  local root port cache_dir script
  root="${LOSOS_ROOT:-${DEVENV_ROOT:-$(git rev-parse --show-toplevel 2>/dev/null || pwd)}}"
  port="${LOSOS_GHCR_CACHE_PORT:-8787}"
  cache_dir="${LOSOS_GHCR_CACHE_DIR:-${XDG_CACHE_HOME:-$HOME/.cache}/losos-ghcr-cache}"
  script="$root/tools/losos-ghcr-cache.py"

  export LOSOS_GHCR_CACHE_DIR="$cache_dir"
  mkdir -p "$cache_dir"

  if curl -fsS "http://127.0.0.1:${port}/nix-cache-info" >/dev/null 2>&1; then
    :
  elif [ ! -f "$script" ]; then
    echo "losos dev cache: $script missing, skipping" >&2
    return 0
  elif ! command -v oras >/dev/null 2>&1; then
    echo "losos dev cache: oras not on PATH, skipping local GHCR proxy" >&2
    return 0
  else
    nohup env LOSOS_GHCR_CACHE_PORT="$port" \
      "${PYTHON:-python3}" "$script" \
      >"$cache_dir/server.log" 2>&1 &
    echo "$!" >"$cache_dir/server.pid"
    local i
    for i in $(seq 1 20); do
      if curl -fsS "http://127.0.0.1:${port}/nix-cache-info" >/dev/null 2>&1; then
        echo "losos dev cache: http://127.0.0.1:${port}/ ready"
        break
      fi
      sleep 1
    done
    if ! curl -fsS "http://127.0.0.1:${port}/nix-cache-info" >/dev/null 2>&1; then
      echo "losos dev cache: proxy did not start; see $cache_dir/server.log" >&2
      tail -n 20 "$cache_dir/server.log" >&2 || true
    fi
  fi

  local subst key existing
  subst="http://127.0.0.1:${port} https://losos.dasmat.us/proxy"
  key="losos-1:35OIuYFMKDrlC6yTzqivDPqNqZTMjM0l2MB89bRRreg="
  existing="${NIX_CONFIG:-}"

  if [ -z "$existing" ] || ! printf '%s' "$existing" | grep -q 'extra-substituters'; then
    export NIX_CONFIG="${existing:+$existing$'\n'}extra-substituters = $subst"
  fi
  if [ -z "$existing" ] || ! printf '%s' "$existing" | grep -q 'extra-trusted-public-keys'; then
    export NIX_CONFIG="${NIX_CONFIG:+$NIX_CONFIG$'\n'}extra-trusted-public-keys = $key"
  fi
  if [ -z "$existing" ] || ! printf '%s' "$existing" | grep -q 'connect-timeout'; then
    export NIX_CONFIG="${NIX_CONFIG:+$NIX_CONFIG$'\n'}connect-timeout = 5"
  fi
  if [ -z "$existing" ] || ! printf '%s' "$existing" | grep -q '^fallback'; then
    export NIX_CONFIG="${NIX_CONFIG:+$NIX_CONFIG$'\n'}fallback = false"
  fi
  if [ -z "$existing" ] || ! printf '%s' "$existing" | grep -q 'always-allow-substitutes'; then
    export NIX_CONFIG="${NIX_CONFIG:+$NIX_CONFIG$'\n'}always-allow-substitutes = true"
  fi
}

losos_dev_cache
