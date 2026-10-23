#!/usr/bin/env python3
"""Allowlist proxy and persistent disk cache for systemd-sysupdate bundles.

This deliberately is not a general-purpose URL proxy. It exposes only the
checksums manifest and immutable numeric-version bundles from the fixed
upstream. Both the cache and every partial download are placed in the
configured /var/lib path, which is persisted by LosOS impermanence.
"""

from __future__ import annotations

import http.client
import json
import os
import re
import shutil
import tarfile
import threading
import time
import urllib.error
import urllib.request
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

UPSTREAM = os.environ.get(
    "LOSOS_SYSUPDATE_UPSTREAM", "https://losos.dasmat.us/sysupdate"
).rstrip("/")
CACHE_DIR = Path(
    os.environ.get("LOSOS_SYSUPDATE_CACHE", "/var/lib/losos-sysupdate/proxy-cache")
)
BIND = os.environ.get("LOSOS_SYSUPDATE_BIND", "127.0.0.1")
PORT = int(os.environ.get("LOSOS_SYSUPDATE_PORT", "8789"))
MANIFEST_TTL_SECONDS = 60
ALLOWED_BUNDLE = re.compile(r"losos-components_[0-9]+\.tar\.xz\Z")
LOCKS: dict[str, threading.Lock] = {}
LOCKS_GUARD = threading.Lock()


def cache_lock(name: str) -> threading.Lock:
    with LOCKS_GUARD:
        return LOCKS.setdefault(name, threading.Lock())


def safe_name(request_path: str) -> str | None:
    if request_path == "/SHA256SUMS":
        return "SHA256SUMS"
    if request_path.startswith("/") and ALLOWED_BUNDLE.fullmatch(request_path[1:]):
        return request_path[1:]
    return None


def stale(name: str, path: Path) -> bool:
    if not path.is_file():
        return True
    if name == "SHA256SUMS":
        return time.time() - path.stat().st_mtime >= MANIFEST_TTL_SECONDS
    # Versioned bundles are immutable. Do not redownload a cached archive.
    return False


def validate_bundle_archive(name: str, archive_path: Path) -> None:
    """Validate the complete XZ/TAR stream before offering it to sysupdate.

    systemd-sysupdate's url-file source decompresses XZ to the target file.
    Validate first so an unsafe archive or compression bomb cannot fill the
    persistent target before the later extraction helper has a chance to reject it.
    """
    match = ALLOWED_BUNDLE.fullmatch(name)
    if match is None:
        raise InvalidBundleError("invalid update archive name")
    expected_version = int(match.group(0)[len("losos-components_") : -len(".tar.xz")])
    seen: set[str] = set()
    member_count = 0
    total_size = 0
    manifest_bytes: bytes | None = None
    has_cache_info = False
    has_narinfo = False

    try:
        with tarfile.open(archive_path, mode="r|xz") as bundle:
            for member in bundle:
                member_count += 1
                if member_count > MAX_ARCHIVE_MEMBERS:
                    raise InvalidBundleError("update archive contains too many entries")
                raw = member.name
                if not raw or raw.startswith("/") or "\\" in raw or "\x00" in raw:
                    raise InvalidBundleError(f"unsafe archive member path: {raw!r}")
                normalized = raw[:-1] if raw.endswith("/") else raw
                parts = normalized.split("/")
                if not normalized or any(part in {"", ".", ".."} for part in parts):
                    raise InvalidBundleError(f"unsafe archive member path: {raw!r}")
                if parts[0] not in {"manifest.json", "cache"}:
                    raise InvalidBundleError(f"unexpected archive entry: {parts[0]!r}")
                if parts[0] == "manifest.json" and len(parts) != 1:
                    raise InvalidBundleError("manifest.json must be a top-level file")
                if normalized in seen:
                    raise InvalidBundleError(f"duplicate archive member: {normalized!r}")
                seen.add(normalized)

                if member.isdir():
                    if normalized == "manifest.json":
                        raise InvalidBundleError("manifest.json must be a regular file")
                    continue
                if not member.isfile() or member.issym() or member.islnk() or member.sparse:
                    raise InvalidBundleError(f"non-regular archive member: {normalized!r}")
                if member.size < 0:
                    raise InvalidBundleError(f"negative archive member size: {normalized!r}")
                total_size += member.size
                if total_size > MAX_UNPACKED_BYTES:
                    raise InvalidBundleError("update archive expands beyond 100 GiB")

                if normalized == "manifest.json":
                    if member.size > MAX_MANIFEST_BYTES:
                        raise InvalidBundleError("update manifest exceeds 1 MiB")
                    source = bundle.extractfile(member)
                    if source is None:
                        raise InvalidBundleError("cannot read update manifest")
                    with source:
                        manifest_bytes = source.read(member.size + 1)
                    if len(manifest_bytes) != member.size:
                        raise InvalidBundleError("truncated update manifest")
                elif normalized == "cache":
                    raise InvalidBundleError("cache must be a directory")
                elif normalized == "cache/nix-cache-info":
                    has_cache_info = True
                elif len(parts) == 2 and parts[0] == "cache" and parts[1].endswith(".narinfo"):
                    has_narinfo = True

        if manifest_bytes is None:
            raise InvalidBundleError("update archive has no manifest.json")
        manifest = json.loads(manifest_bytes.decode("utf-8"))
        if not isinstance(manifest, dict) or manifest.get("format") != 1:
            raise InvalidBundleError("unsupported update manifest format")
        version = manifest.get("version")
        if not isinstance(version, int) or isinstance(version, bool) or version != expected_version:
            raise InvalidBundleError("manifest version does not match the archive filename")
        if not isinstance(manifest.get("sourceVersion"), str) or not isinstance(manifest.get("commit"), str):
            raise InvalidBundleError("update manifest must include sourceVersion and commit")
        paths = manifest.get("storePaths")
        if not isinstance(paths, list) or not paths or len(paths) > 10000:
            raise InvalidBundleError("update manifest must list 1 to 10000 store roots")
        if any(not isinstance(p, str) or not STORE_PATH_PATTERN.fullmatch(p) for p in paths):
            raise InvalidBundleError("update manifest has a malformed store path")
        signed_path = manifest.get("signedManifestStorePath")
        if not isinstance(signed_path, str) or not STORE_PATH_PATTERN.fullmatch(signed_path):
            raise InvalidBundleError("update manifest has no valid signed metadata store path")
        if not has_cache_info or not has_narinfo:
            raise InvalidBundleError("update archive does not contain a valid Nix binary cache")

        # sysupdate writes a decompressed TAR; LosOS then extracts the binary
        # cache, and Nix imports its NARs. Reserve enough disk for those copies
        # plus a safety margin before allowing systemd-sysupdate to download it.
        required = total_size * 3 + MIN_FREE_DISK_RESERVE
        free = shutil.disk_usage(CACHE_DIR).free
        if required > free:
            raise InvalidBundleError(
                f"insufficient persistent disk space: archive needs about {required} bytes, "
                f"only {free} bytes are free"
            )
    except InvalidBundleError:
        raise
    except (OSError, tarfile.TarError, ValueError, UnicodeDecodeError) as error:
        raise InvalidBundleError(f"invalid update archive: {error}") from error


def prune_bundle_cache(protect: Path | None = None) -> None:
    """Retain the two newest archives, never unlinking the response being served."""
    versions: list[tuple[int, Path]] = []
    for path in CACHE_DIR.iterdir():
        match = ALLOWED_BUNDLE.fullmatch(path.name)
        if match and path.is_file() and not path.is_symlink():
            versions.append((int(match.group(1)), path))
    versions.sort(key=lambda item: item[0], reverse=True)
    keep = {path.resolve() for _version, path in versions[:2]}
    if protect is not None:
        keep.add(protect.resolve())
    for _version, path in versions:
        if path.resolve() not in keep:
            try:
                path.unlink(missing_ok=True)
            except OSError:
                pass


def download(name: str, destination: Path) -> Path:
    CACHE_DIR.mkdir(mode=0o750, parents=True, exist_ok=True)
    with cache_lock(name):
        if not stale(name, destination):
            return destination

        # Partial files are siblings of the eventual cache entry, never in
        # tempfile's default /tmp (which is tmpfs on LosOS).
        part = CACHE_DIR / f".{name}.{os.getpid()}.{threading.get_ident()}.part"
        url = f"{UPSTREAM}/{name}"
        last_error: Exception | None = None
        for attempt in range(3):
            try:
                request = urllib.request.Request(
                    url,
                    headers={"User-Agent": "LosOS-systemd-sysupdate-proxy/1"},
                )
                with urllib.request.urlopen(request, timeout=90) as response:
                    if response.status != 200:
                        raise OSError(f"upstream returned HTTP {response.status}")
                    limit = MAX_MANIFEST_BYTES if name == "SHA256SUMS" else MAX_BUNDLE_BYTES
                    length = response.headers.get("Content-Length")
                    if length is not None:
                        try:
                            declared = int(length)
                        except ValueError as error:
                            raise OSError("upstream returned an invalid Content-Length") from error
                        if declared < 1 or declared > limit:
                            raise OSError("upstream file exceeds the configured download limit")
                    total = 0
                    with part.open("wb") as output:
                        while block := response.read(1024 * 1024):
                            total += len(block)
                            if total > limit:
                                raise OSError("upstream file exceeds the configured download limit")
                            output.write(block)
                        output.flush()
                        os.fsync(output.fileno())
                if part.stat().st_size == 0:
                    raise OSError("upstream returned an empty update file")
                if ALLOWED_BUNDLE.fullmatch(name):
                    validate_bundle_archive(name, part)
                os.replace(part, destination)
                if ALLOWED_BUNDLE.fullmatch(name):
                    prune_bundle_cache(protect=destination)
                return destination
            except InvalidBundleError:
                part.unlink(missing_ok=True)
                raise
            except (OSError, urllib.error.URLError, TimeoutError, http.client.HTTPException) as error:
                last_error = error
                try:
                    part.unlink(missing_ok=True)
                except OSError:
                    pass
                if attempt < 2:
                    time.sleep(0.5 * (attempt + 1))
        raise OSError(f"could not fetch {name} from configured update upstream: {last_error}")


class Handler(BaseHTTPRequestHandler):
    server_version = "LosOSUpdateProxy/1"
    sys_version = ""

    def do_GET(self) -> None:
        self._serve(send_body=True)

    def do_HEAD(self) -> None:
        self._serve(send_body=False)

    def _serve(self, *, send_body: bool) -> None:
        name = safe_name(self.path)
        if name is None:
            self.send_error(404, "update path not found")
            return
        destination = CACHE_DIR / name
        try:
            cached = download(name, destination)
            size = cached.stat().st_size
            with cached.open("rb") as source:
                self.send_response(200)
                self.send_header("Content-Type", "text/plain" if name == "SHA256SUMS" else "application/x-xz")
                self.send_header("Content-Length", str(size))
                self.send_header(
                    "Cache-Control",
                    "no-store" if name == "SHA256SUMS" else "public, max-age=31536000, immutable",
                )
                self.send_header("X-Content-Type-Options", "nosniff")
                self.end_headers()
                if send_body:
                    shutil.copyfileobj(source, self.wfile, length=1024 * 1024)
        except (OSError, urllib.error.URLError, http.client.HTTPException, InvalidBundleError) as error:
            # Keep implementation details in the journal, not in the HTTP body.
            self.log_error("update download failed for %s: %s", name, error)
            try:
                self.send_error(502, "update source temporarily unavailable")
            except (BrokenPipeError, ConnectionResetError):
                pass

    def log_message(self, fmt: str, *args: object) -> None:
        # Avoid logging URL query strings or remote content. The path has been
        # allowlisted before it is fetched, but keep logs intentionally small.
        print("losos-sysupdate-proxy: " + (fmt % args), flush=True)


def main() -> None:
    if not UPSTREAM.startswith("https://"):
        raise SystemExit("LOSOS_SYSUPDATE_UPSTREAM must use HTTPS")
    if BIND not in {"127.0.0.1", "::1"}:
        raise SystemExit("update proxy may bind only to loopback")
    if not 1 <= PORT <= 65535:
        raise SystemExit("invalid proxy port")
    CACHE_DIR.mkdir(mode=0o750, parents=True, exist_ok=True)
    # At startup there are no live downloads; clear partial files left by a
    # crash without racing requests in ThreadingHTTPServer.
    for partial in CACHE_DIR.glob(".*.part"):
        partial.unlink(missing_ok=True)
    prune_bundle_cache()
    server = ThreadingHTTPServer((BIND, PORT), Handler)
    server.daemon_threads = True
    print(
        f"LosOS systemd-sysupdate proxy listening on {BIND}:{PORT}; "
        f"persistent cache: {CACHE_DIR}",
        flush=True,
    )
    server.serve_forever()


if __name__ == "__main__":
    main()
