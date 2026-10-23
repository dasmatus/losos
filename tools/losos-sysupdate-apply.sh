#!/usr/bin/env bash
set -euo pipefail

# Both the downloaded archive and extracted version directories live on the
# encrypted persistent /persist filesystem. Never use /tmp or /var/tmp.
root=/persist/var/lib/losos-sysupdate
versions="$root/versions"
state="$root/state"
mkdir -p "$versions" "$state"

# A failed refresh must not discard a previously downloaded candidate.
if ! systemd-sysupdate --definitions=/etc/sysupdate.d update; then
  if ! find "$versions" -mindepth 1 -maxdepth 1 -type d -name 'losos-update_*' -print -quit | grep -q .; then
    echo "LosOS sysupdate could not download an update and has no staged version to apply" >&2
    exit 1
  fi
  echo "sysupdate download failed; checking persistent staged versions" >&2
fi

candidate="$(find "$versions" -mindepth 1 -maxdepth 1 -type d -name 'losos-update_*' -printf '%f\n' | sort -V | tail -n 1)"
if [[ -z "$candidate" ]]; then
  echo "No LosOS sysupdate version is staged"
  exit 0
fi
version="$(printf '%s' "$candidate" | sed 's/^losos-update_//')"
bundle="$versions/$candidate"

if [[ -f "$state/applied-version" ]] && [[ "$(cat "$state/applied-version")" == "$version" ]]; then
  echo "LosOS update $version is already applied"
  exit 0
fi
if [[ ! -f "$bundle/nix-store.export.xz" || ! -f "$bundle/source/flake.nix" || ! -f "$bundle/source/flake.lock" ]]; then
  echo "Staged LosOS update $version is incomplete; leaving the previous system active" >&2
  exit 1
fi

# nix-store validates every imported store object; xz streams directly to the
# importer instead of writing the decompressed NAR archive into a tmpfs.
xz --decompress --stdout "$bundle/nix-store.export.xz" | nix-store --import

# Follow the box owner's config branch when reachable, just like the former
# nightly ExecStartPre. Failure is non-fatal: the release bundle also works
# offline, and --impure preserves the live /etc/nixos device overrides.
if [[ "${LOSOS_SYSUPDATE_CONFIG_SYNC:-0}" == "1" ]] && [[ -x /run/current-system/sw/bin/losos-ctl ]]; then
  /run/current-system/sw/bin/losos-ctl config --update || \
    echo "Could not sync optional configuration upstream; keeping local configuration" >&2
fi

# Never install the generic prebuilt toplevel: that could overwrite this
# machine's disk, firmware, TPM and runtime settings. Re-evaluate the signed
# source bundle against the machine's persisted /etc/nixos files instead.
nixos-rebuild switch --flake "$bundle/source#install" --impure

# Only record success after activation, atomically on /persist.
printf '%s\n' "$version" > "$state/applied-version.new"
sync -f "$state/applied-version.new"
mv -f "$state/applied-version.new" "$state/applied-version"
sync -f "$state"

# Reclaim only LosOS update bundles and chunks, after activation succeeded.
# Failed updates retain these persistent files so the next attempt can resume.
cache="${LOSOS_SYSUPDATE_CACHE_DIR:-/persist/var/cache/losos-sysupdate-proxy}"
shopt -s nullglob
cached_bundles=( "$cache"/bundles/losos-update_*.tar.xz* )
cached_parts=( "$cache"/parts/losos-update_*.tar.xz.part* )
if ((${#cached_bundles[@]} > 0)); then rm -f -- "${cached_bundles[@]}"; fi
if ((${#cached_parts[@]} > 0)); then rm -f -- "${cached_parts[@]}"; fi

echo "LosOS update $version activated successfully"
