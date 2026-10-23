#!/usr/bin/env python3
"""Import a downloaded LosOS component bundle into the signed Nix store.

systemd-sysupdate only extracts into /var/lib/losos-sysupdate/slots. This
helper does not execute code from the bundle. It accepts only numeric slot
names and Nix store paths, asks Nix to verify all source signatures using the
box's existing trusted-public-keys policy, and writes the active marker only
after the complete import succeeds.
"""

from __future__ import annotations

import json
import os
import re
import subprocess
import sys
from pathlib import Path

STATE_DIR = Path("/var/lib/losos-sysupdate")
SLOTS_DIR = STATE_DIR / "slots"
ACTIVE_FILE = STATE_DIR / "active.json"
SLOT_PATTERN = re.compile(r"losos-components_([0-9]+)\Z")
STORE_PATH_PATTERN = re.compile(r"/nix/store/[0-9a-z]{32}-[A-Za-z0-9+._?=-]+\Z")
MAX_STORE_PATHS = 10000


def read_active_version() -> int:
    try:
        document = json.loads(ACTIVE_FILE.read_text(encoding="utf-8"))
        version = document.get("version")
        return version if isinstance(version, int) and version >= 0 else 0
    except (OSError, ValueError, TypeError):
        return 0


def atomic_json_write(path: Path, value: object) -> None:
    # Same-filesystem atomic replacement under /var/lib: no tmpfs staging.
    temporary = path.with_name(f".{path.name}.{os.getpid()}.new")
    descriptor = os.open(temporary, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    try:
        with os.fdopen(descriptor, "w", encoding="utf-8") as output:
            json.dump(value, output, sort_keys=True)
            output.write("\n")
            output.flush()
            os.fsync(output.fileno())
        os.replace(temporary, path)
        directory_fd = os.open(path.parent, os.O_RDONLY | getattr(os, "O_DIRECTORY", 0))
        try:
            os.fsync(directory_fd)
        finally:
            os.close(directory_fd)
    finally:
        temporary.unlink(missing_ok=True)


def candidates() -> list[tuple[int, Path]]:
    found: list[tuple[int, Path]] = []
    if not SLOTS_DIR.is_dir():
        return found
    slots_real = SLOTS_DIR.resolve()
    for child in SLOTS_DIR.iterdir():
        match = SLOT_PATTERN.fullmatch(child.name)
        if not match or not child.is_dir():
            continue
        # A malicious archive must not be able to redirect the helper to an
        # unrelated directory through a symlink.
        resolved = child.resolve()
        if resolved.parent != slots_real:
            continue
        found.append((int(match.group(1)), resolved))
    return sorted(found)


def import_bundle(version: int, directory: Path) -> dict[str, object]:
    manifest_path = directory / "manifest.json"
    cache = directory / "cache"
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))

    if not isinstance(manifest, dict) or manifest.get("format") != 1:
        raise ValueError("unsupported LosOS update manifest format")
    if manifest.get("version") != version:
        raise ValueError("slot version does not match manifest version")
    raw_paths = manifest.get("storePaths")
    if not isinstance(raw_paths, list) or not raw_paths or len(raw_paths) > MAX_STORE_PATHS:
        raise ValueError("manifest must list 1 to 10000 Nix store paths")

    paths: list[str] = []
    seen: set[str] = set()
    for value in raw_paths:
        if not isinstance(value, str) or not STORE_PATH_PATTERN.fullmatch(value):
            raise ValueError("manifest contains a malformed Nix store path")
        if value in seen:
            continue
        seen.add(value)
        paths.append(value)

    cache = cache.resolve()
    if cache.parent != directory.resolve() or not (cache / "nix-cache-info").is_file():
        raise ValueError("bundle does not contain a valid local Nix binary cache")
    if not list(cache.glob("*.narinfo")):
        raise ValueError("bundle's Nix cache has no narinfo records")

    # Do not pass --no-check-sigs: Nix must validate each imported store
    # object against the trusted keys configured by modules/cache.nix. CI signs
    # the entire closure before it creates this cache.
    subprocess.run(
        ["nix", "copy", "--from", cache.as_uri(), "--recursive", *paths],
        check=True,
        stdin=subprocess.DEVNULL,
    )

    return {
        "version": version,
        "sourceVersion": str(manifest.get("sourceVersion", "unknown")),
        "commit": str(manifest.get("commit", "unknown")),
        "storePaths": len(paths),
    }


def main() -> int:
    try:
        STATE_DIR.mkdir(mode=0o750, parents=True, exist_ok=True)
        active = read_active_version()
        newer = [(v, p) for v, p in candidates() if v > active]
        if not newer:
            print(f"losos-sysupdate-apply: no staged component bundle newer than {active}")
            return 0

        version, directory = newer[-1]
        result = import_bundle(version, directory)
        atomic_json_write(ACTIVE_FILE, result)
        print(
            f"losos-sysupdate-apply: imported signed components version {version}; "
            "the nightly machine-local nixos-rebuild remains the activation path"
        )
        return 0
    except (OSError, ValueError, json.JSONDecodeError, subprocess.CalledProcessError) as error:
        print(f"losos-sysupdate-apply: refusing update bundle: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
