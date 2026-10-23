#!/usr/bin/env python3
"""Safely unpack versioned systemd-sysupdate archives and import signed Nix roots.

The upstream artifact is XZ-compressed *.tar.xz. systemd-sysupdate decodes
url-file sources into a regular-file target, so the persistent target carries
an uncompressed .tar archive before this helper extracts its contents.

systemd-sysupdate downloads each XZ archive as a regular file below
/var/lib/losos-sysupdate/archives. This helper extracts it under
/var/lib/losos-sysupdate/slots, never /tmp or /run, and refuses path traversal,
links, special files, duplicate members, and oversized archives. Extracted data
is not executed. Nix verifies the signatures for imported store paths before
the staged version marker is committed.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import shutil
import subprocess
import sys
import tarfile
import uuid
from pathlib import Path, PurePosixPath
from typing import Any

STATE_DIR = Path("/var/lib/losos-sysupdate")
ARCHIVES_DIR = STATE_DIR / "archives"
SLOTS_DIR = STATE_DIR / "slots"
ACTIVE_FILE = STATE_DIR / "active.json"
GCROOTS_DIR = Path("/nix/var/nix/gcroots/auto")
ARCHIVE_PATTERN = re.compile(r"losos-components_([0-9]+)\.tar\.xz\Z")
SLOT_PATTERN = re.compile(r"losos-components_([0-9]+)\Z")
# Nix's canonical base32 alphabet excludes e, o, t and u.
STORE_PATH_PATTERN = re.compile(r"/nix/store/[0-9a-df-np-sv-z]{32}-[A-Za-z0-9+._?=-]+\Z")
MAX_STORE_PATHS = 10000
MAX_ARCHIVE_MEMBERS = 500000
MAX_UNPACKED_BYTES = 100 * 1024**3


def read_active_version() -> int:
    try:
        document = json.loads(ACTIVE_FILE.read_text(encoding="utf-8"))
        if not isinstance(document, dict):
            return 0
        version = document.get("version")
        return (
            version
            if isinstance(version, int) and not isinstance(version, bool) and version >= 0
            else 0
        )
    except (OSError, ValueError, TypeError):
        return 0


def atomic_json_write(path: Path, value: object) -> None:
    # Same-filesystem atomic replacement under /var/lib: no tmpfs staging.
    temporary = path.with_name(f".{path.name}.{os.getpid()}.{uuid.uuid4().hex}.new")
    descriptor = os.open(
        temporary,
        os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_NOFOLLOW", 0),
        0o600,
    )
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
    """Return versioned extracted slots, ignoring invalid names and symlinks."""
    found: list[tuple[int, Path]] = []
    if not SLOTS_DIR.is_dir():
        return found
    slots_real = SLOTS_DIR.resolve()
    for child in SLOTS_DIR.iterdir():
        match = SLOT_PATTERN.fullmatch(child.name)
        if not match or child.is_symlink() or not child.is_dir():
            continue
        resolved = child.resolve()
        if resolved.parent != slots_real:
            continue
        found.append((int(match.group(1)), resolved))
    return sorted(found)


def archive_candidates() -> list[tuple[int, Path]]:
    found: list[tuple[int, Path]] = []
    if not ARCHIVES_DIR.is_dir():
        return found
    archives_real = ARCHIVES_DIR.resolve()
    for child in ARCHIVES_DIR.iterdir():
        match = ARCHIVE_PATTERN.fullmatch(child.name)
        if not match or child.is_symlink() or not child.is_file():
            continue
        resolved = child.resolve()
        if resolved.parent != archives_real:
            continue
        found.append((int(match.group(1)), resolved))
    return sorted(found)


def safe_member_path(name: str) -> tuple[str, ...]:
    # Require normalized POSIX archive paths. Backslashes are rejected rather
    # than treated as an ordinary filename so Windows-created archives cannot
    # smuggle path separators through a future cross-platform extractor.
    if not name or name.startswith("/") or "\\" in name or "\x00" in name:
        raise ValueError(f"unsafe archive member path: {name!r}")
    normalized = name[:-1] if name.endswith("/") else name
    if not normalized:
        raise ValueError("archive contains an empty path")
    parts = normalized.split("/")
    if any(part in {"", ".", ".."} for part in parts):
        raise ValueError(f"unsafe archive member path: {name!r}")
    pure = PurePosixPath(*parts)
    if pure.is_absolute() or str(pure) != normalized:
        raise ValueError(f"non-normalized archive member path: {name!r}")
    if parts[0] != "manifest.json" and parts[0] != "cache":
        raise ValueError(f"unexpected top-level archive entry: {parts[0]!r}")
    if parts[0] == "manifest.json" and len(parts) != 1:
        raise ValueError("manifest.json must be a top-level file")
    return tuple(parts)


def extract_archive(version: int, archive: Path) -> Path:
    """Safely extract one .tar.xz bundle into a versioned directory on /var."""
    if archive.is_symlink() or not archive.is_file():
        raise ValueError("update archive must be a regular file")
    if not ARCHIVE_PATTERN.fullmatch(archive.name):
        raise ValueError("update archive filename does not contain a numeric version")
    archive_version = int(ARCHIVE_PATTERN.fullmatch(archive.name).group(1))  # type: ignore[union-attr]
    if archive_version != version:
        raise ValueError("archive version and requested slot version differ")

    SLOTS_DIR.mkdir(mode=0o750, parents=True, exist_ok=True)
    root = SLOTS_DIR.resolve()
    final = root / f"losos-components_{version}"
    staging = root / f".losos-components_{version}.{os.getpid()}.{uuid.uuid4().hex}.staging"
    staging.mkdir(mode=0o700)
    seen: set[str] = set()
    total_size = 0
    count = 0

    try:
        # Stream in order: do not load an attacker-controlled TAR directory
        # into memory via getmembers(), and never use tarfile.extract/extractall.
        with tarfile.open(archive, mode="r|") as bundle:
            for member in bundle:
                count += 1
                if count > MAX_ARCHIVE_MEMBERS:
                    raise ValueError("update archive contains too many entries")
                parts = safe_member_path(member.name)
                normalized = "/".join(parts)
                if normalized in seen:
                    raise ValueError(f"update archive repeats member {normalized!r}")
                seen.add(normalized)

                if member.isdir():
                    destination = staging.joinpath(*parts)
                    destination.mkdir(mode=0o750, parents=True, exist_ok=True)
                    continue
                if normalized == "cache" or not member.isfile() or member.issym() or member.islnk() or member.sparse:
                    raise ValueError(f"unsupported non-regular archive entry: {normalized!r}")
                if member.size < 0:
                    raise ValueError(f"negative size for archive member {normalized!r}")
                total_size += member.size
                if total_size > MAX_UNPACKED_BYTES:
                    raise ValueError("update archive expands past the 100 GiB safety limit")

                destination = staging.joinpath(*parts)
                destination.parent.mkdir(mode=0o750, parents=True, exist_ok=True)
                flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_NOFOLLOW", 0)
                fd = os.open(destination, flags, 0o640)
                try:
                    source = bundle.extractfile(member)
                    if source is None:
                        raise ValueError(f"cannot read archive member {normalized!r}")
                    with source, os.fdopen(fd, "wb") as output:
                        remaining = member.size
                        while remaining:
                            block = source.read(min(1024 * 1024, remaining))
                            if not block:
                                raise ValueError(f"truncated archive member {normalized!r}")
                            output.write(block)
                            remaining -= len(block)
                        output.flush()
                        os.fsync(output.fileno())
                except BaseException:
                    # If fdopen was not reached, close the raw descriptor.
                    try:
                        os.close(fd)
                    except OSError:
                        pass
                    raise

        manifest_path = staging / "manifest.json"
        if manifest_path.is_symlink() or not manifest_path.is_file():
            raise ValueError("bundle must contain a regular manifest.json")
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
        if not isinstance(manifest, dict) or manifest.get("format") != 1:
            raise ValueError("unsupported LosOS update manifest format")
        if manifest.get("version") != version:
            raise ValueError("slot version does not match manifest version")
        cache = staging / "cache"
        if cache.is_symlink() or not cache.is_dir():
            raise ValueError("bundle must contain a cache directory")
        cache_info = cache / "nix-cache-info"
        if cache_info.is_symlink() or not cache_info.is_file():
            raise ValueError("bundle cache has no regular nix-cache-info")
        if not any(path.is_file() and not path.is_symlink() for path in cache.glob("*.narinfo")):
            raise ValueError("bundle's Nix cache has no narinfo records")

        # The final path is a sibling on the same persistent filesystem.
        # Remove only our own exact version directory, never a symlink.
        if final.is_symlink():
            final.unlink()
        elif final.exists():
            if final.resolve().parent != root or not final.is_dir():
                raise ValueError("refusing to replace unexpected slot path")
            shutil.rmtree(final)
        os.replace(staging, final)
        return final
    except BaseException:
        if staging.exists() and staging.resolve().parent == root:
            shutil.rmtree(staging, ignore_errors=True)
        raise


def validate_store_paths(manifest: dict[str, Any]) -> list[str]:
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
    return paths


def import_bundle(version: int, directory: Path) -> dict[str, object]:
    manifest_path = directory / "manifest.json"
    cache = directory / "cache"
    if manifest_path.is_symlink() or not manifest_path.is_file():
        raise ValueError("manifest.json must be a regular file in the versioned slot")
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    if not isinstance(manifest, dict) or manifest.get("format") != 1:
        raise ValueError("unsupported LosOS update manifest format")
    if manifest.get("version") != version:
        raise ValueError("slot version does not match manifest version")
    paths = validate_store_paths(manifest)

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


def prune_slots() -> None:
    """Keep the two newest unpacked slots; sysupdate separately retains two archives."""
    slots = candidates()
    for _version, path in slots[:-2]:
        if path.resolve().parent == SLOTS_DIR.resolve():
            shutil.rmtree(path)


def install_gc_roots(version: int, store_paths: list[str]) -> None:
    """Pin imported outputs until the machine-local rebuild adopts their closure."""
    GCROOTS_DIR.mkdir(mode=0o755, parents=True, exist_ok=True)
    for index, store_path in enumerate(store_paths):
        target = GCROOTS_DIR / f"losos-sysupdate-components-{version}-{index}"
        if target.exists() and not target.is_symlink():
            raise ValueError(f"refusing to replace non-symlink GC root {target}")
        temporary = GCROOTS_DIR / f".{target.name}.{uuid.uuid4().hex}.new"
        try:
            os.symlink(store_path, temporary)
            os.replace(temporary, target)
        finally:
            temporary.unlink(missing_ok=True)


def prune_gc_roots() -> None:
    """Retain roots for the two newest successfully imported component versions."""
    if not GCROOTS_DIR.is_dir():
        return
    roots: list[tuple[int, Path]] = []
    for child in GCROOTS_DIR.iterdir():
        match = re.fullmatch(r"losos-sysupdate-components-([0-9]+)-[0-9]+", child.name)
        if match and child.is_symlink():
            roots.append((int(match.group(1)), child))
    versions = sorted({version for version, _path in roots}, reverse=True)[:2]
    keep = set(versions)
    for version, path in roots:
        if version not in keep:
            path.unlink(missing_ok=True)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--extract-only",
        action="store_true",
        help="safely extract the newest bundle without importing store paths (for storage tests)",
    )
    args = parser.parse_args()
    try:
        STATE_DIR.mkdir(mode=0o750, parents=True, exist_ok=True)
        active = read_active_version()
        newer = [(v, p) for v, p in archive_candidates() if v > active]
        if not newer:
            print(f"losos-sysupdate-apply: no archive newer than active version {active}")
            return 0

        version, archive = newer[-1]
        directory = extract_archive(version, archive)
        if args.extract_only:
            print(f"losos-sysupdate-apply: safely extracted {version} under {directory}")
            return 0

        result = import_bundle(version, directory)
        install_gc_roots(version, validate_store_paths(json.loads((directory / "manifest.json").read_text(encoding="utf-8"))))
        atomic_json_write(ACTIVE_FILE, result)
        prune_slots()
        prune_gc_roots()
        print(
            f"losos-sysupdate-apply: imported signed components version {version}; "
            "the nightly machine-local nixos-rebuild remains the activation path"
        )
        return 0
    except (
        OSError,
        ValueError,
        json.JSONDecodeError,
        tarfile.TarError,
        subprocess.CalledProcessError,
    ) as error:
        print(f"losos-sysupdate-apply: refusing update bundle: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
