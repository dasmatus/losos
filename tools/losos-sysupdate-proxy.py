#!/usr/bin/env python3
"""Loopback systemd-sysupdate source backed by a GHCR OCI artifact.

Only the manifest and the single, checksum-verified OS bundle are served.
OCI downloads, temporary files, and cached payloads all live below
/persist/var/cache/losos-sysupdate-proxy; do not relocate this to /tmp.
"""
from __future__ import annotations

import hashlib
import json
import os
import re
import shutil
import subprocess
import tempfile
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.parse import urlsplit

HOST = os.environ.get("LOSOS_SYSUPDATE_HOST", "127.0.0.1")
PORT = int(os.environ.get("LOSOS_SYSUPDATE_PORT", "8786"))
OCI_REF = os.environ.get(
    "LOSOS_SYSUPDATE_OCI_REF", "ghcr.io/dasmatus/losos/os-updates:main"
)
ROOT = Path(os.environ.get(
    "LOSOS_SYSUPDATE_CACHE", "/persist/var/cache/losos-sysupdate-proxy"
))
VERSIONS = ROOT / "versions"
TEMP = ROOT / "tmp"
CURRENT = ROOT / "current"
LAST_CHECK = ROOT / "last-check"
BUNDLE = re.compile(r"^losos-os_[A-Za-z0-9.+_-]+\.tar\.xz$")
SUM = re.compile(r"^([0-9a-f]{64})  (losos-os_[A-Za-z0-9.+_-]+\.tar\.xz)$")
DIGEST = re.compile(r"^[0-9a-f]{64}$")
REFRESH_SECONDS = 30
DOWNLOAD_TIMEOUT = 3 * 60 * 60
CHUNK = 1024 * 1024
refresh_lock = threading.Lock()


class SourceUnavailable(Exception):
    pass


def run_oras(args: list[str], timeout: int) -> subprocess.CompletedProcess[str]:
    executable = shutil.which("oras")
    if not executable:
        raise SourceUnavailable("oras is not on PATH")
    env = os.environ.copy()
    # oras' own temporary writes and extracted OCI files must stay on the
    # encrypted persistent filesystem, never the tmpfs root or /tmp.
    env["TMPDIR"] = str(TEMP)
    env["TMP"] = str(TEMP)
    env["TEMP"] = str(TEMP)
    env["HOME"] = str(ROOT / "home")
    try:
        return subprocess.run(
            [executable, *args],
            cwd=ROOT,
            env=env,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            timeout=timeout,
            check=False,
        )
    except (OSError, subprocess.TimeoutExpired) as exc:
        raise SourceUnavailable(f"oras command failed: {exc}") from exc


def current_version_dir() -> Path | None:
    try:
        name = CURRENT.read_text(encoding="ascii").strip()
    except OSError:
        return None
    if not DIGEST.fullmatch(name):
        return None
    directory = VERSIONS / name
    if not directory.is_dir():
        return None
    try:
        manifest = (directory / "SHA256SUMS").read_text(encoding="ascii")
        rows = manifest.splitlines()
        if len(rows) != 1:
            return None
        match = SUM.fullmatch(rows[0])
        if not match:
            return None
        payload = directory / match.group(2)
        if not payload.is_file():
            return None
    except OSError:
        return None
    return directory


def remote_manifest_digest() -> str:
    result = run_oras(["manifest", "fetch", "--descriptor", OCI_REF], timeout=30)
    if result.returncode:
        raise SourceUnavailable(
            "could not read the update OCI descriptor: " + result.stderr[-1000:]
        )
    try:
        descriptor = json.loads(result.stdout)
        digest = descriptor["digest"]
        if not isinstance(digest, str) or not digest.startswith("sha256:"):
            raise ValueError("descriptor has no sha256 digest")
        value = digest.removeprefix("sha256:")
        if not DIGEST.fullmatch(value):
            raise ValueError("malformed OCI digest")
        return value
    except (ValueError, KeyError, TypeError) as exc:
        raise SourceUnavailable(f"invalid OCI descriptor: {exc}") from exc


def validate_bundle(directory: Path) -> tuple[str, Path]:
    manifest_path = directory / "SHA256SUMS"
    if not manifest_path.is_file() or manifest_path.is_symlink():
        raise SourceUnavailable("OCI artifact has no regular SHA256SUMS")
    try:
        rows = manifest_path.read_text(encoding="ascii").splitlines()
    except (OSError, UnicodeError) as exc:
        raise SourceUnavailable(f"cannot read SHA256SUMS: {exc}") from exc
    if len(rows) != 1:
        raise SourceUnavailable("SHA256SUMS must name exactly one OS bundle")
    match = SUM.fullmatch(rows[0])
    if not match:
        raise SourceUnavailable("SHA256SUMS has an invalid bundle name or digest")
    expected, name = match.groups()
    path = directory / name
    if path.is_symlink() or not path.is_file():
        raise SourceUnavailable("OS bundle is not a regular file")
    digest = hashlib.sha256()
    try:
        with path.open("rb") as source:
            for block in iter(lambda: source.read(CHUNK), b""):
                digest.update(block)
    except OSError as exc:
        raise SourceUnavailable(f"cannot read OS bundle: {exc}") from exc
    if digest.hexdigest() != expected:
        raise SourceUnavailable("OS bundle digest does not match SHA256SUMS")
    return name, path


def refresh(force: bool = False) -> Path:
    ROOT.mkdir(mode=0o700, parents=True, exist_ok=True)
    VERSIONS.mkdir(mode=0o700, parents=True, exist_ok=True)
    TEMP.mkdir(mode=0o700, parents=True, exist_ok=True)
    (ROOT / "home").mkdir(mode=0o700, exist_ok=True)

    with refresh_lock:
        cached = current_version_dir()
        try:
            last = float(LAST_CHECK.read_text(encoding="ascii").strip())
        except (OSError, ValueError):
            last = 0.0

        # Do not fetch the large bundle every time systemd asks for the
        # manifest and then the payload. Re-check the cheap OCI descriptor
        # regularly; only pull layers when its digest changed.
        if not force and cached and time.time() - last < REFRESH_SECONDS:
            return cached

        try:
            remote_digest = remote_manifest_digest()
        except SourceUnavailable:
            if cached:
                print("sysupdate proxy: using the last verified cached OCI artifact", flush=True)
                return cached
            raise

        stamp_tmp = ROOT / "last-check.tmp"
        stamp_tmp.write_text(f"{time.time():.6f}\n", encoding="ascii")
        os.replace(stamp_tmp, LAST_CHECK)

        if cached and cached.name == remote_digest:
            return cached

        staging = Path(tempfile.mkdtemp(prefix="pull-", dir=VERSIONS))
        try:
            result = run_oras(
                ["pull", "--output", str(staging), OCI_REF],
                timeout=DOWNLOAD_TIMEOUT,
            )
            if result.returncode:
                raise SourceUnavailable(
                    "could not pull the update OCI artifact: " + result.stderr[-1500:]
                )
            bundle_name, _payload = validate_bundle(staging)
            unexpected = {
                child.name for child in staging.iterdir()
            } - {"SHA256SUMS", bundle_name}
            if unexpected:
                raise SourceUnavailable(
                    "unexpected OCI artifact entries: " + ", ".join(sorted(unexpected))
                )

            destination = VERSIONS / remote_digest
            if destination.exists():
                shutil.rmtree(destination)
            os.replace(staging, destination)
            pointer_tmp = ROOT / "current.tmp"
            pointer_tmp.write_text(remote_digest + "\n", encoding="ascii")
            os.replace(pointer_tmp, CURRENT)

            # Keep one preceding generation as a recovery/cache fallback.
            keep = {remote_digest}
            if cached:
                keep.add(cached.name)
            old = sorted(
                (
                    p for p in VERSIONS.iterdir()
                    if p.is_dir() and DIGEST.fullmatch(p.name) and p.name not in keep
                ),
                key=lambda p: p.stat().st_mtime,
                reverse=True,
            )
            for path in old:
                shutil.rmtree(path, ignore_errors=True)
            return destination
        except Exception as exc:
            shutil.rmtree(staging, ignore_errors=True)
            if cached:
                print(
                    f"sysupdate proxy: new artifact failed validation ({exc}); "
                    "serving the previous verified generation",
                    flush=True,
                )
                return cached
            raise


def selected_file(name: str) -> tuple[Path, str] | None:
    directory = refresh().resolve()
    sums_path = directory / "SHA256SUMS"
    if name == "SHA256SUMS":
        return (sums_path, "text/plain; charset=utf-8") if sums_path.is_file() else None
    if not BUNDLE.fullmatch(name):
        return None
    try:
        row = sums_path.read_text(encoding="ascii").strip()
        match = SUM.fullmatch(row)
        if not match:
            return None
        verified_name = match.group(2)
        path = directory / verified_name
        if bundle_name != name:
            return None
        bundle_path = directory / bundle_name
        return (bundle_path, "application/octet-stream") if bundle_path.is_file() else None
    except OSError:
        return None


class Handler(BaseHTTPRequestHandler):
    server_version = "LosOS-sysupdate-proxy/1"
    protocol_version = "HTTP/1.1"

    def log_message(self, fmt: str, *args: object) -> None:
        print("losos-sysupdate-proxy:", fmt % args, flush=True)

    def respond_error(self, code: int, message: str) -> None:
        body = (message + "\n").encode("utf-8")
        self.send_response(code)
        self.send_header("Content-Type", "text/plain; charset=utf-8")
        self.send_header("Content-Length", str(len(body)))
        self.send_header("Cache-Control", "no-store")
        self.end_headers()
        if self.command != "HEAD":
            try:
                self.wfile.write(body)
            except (BrokenPipeError, ConnectionResetError):
                pass

    def do_GET(self) -> None:
        self.serve_file()

    def do_HEAD(self) -> None:
        self.serve_file()

    def serve_file(self) -> None:
        path = urlsplit(self.path).path
        prefix = "/updates/"
        if not path.startswith(prefix):
            self.respond_error(404, "not found")
            return
        name = path[len(prefix):]
        if "/" in name or "\\" in name or "%" in name or name in ("", ".", ".."):
            self.respond_error(404, "not found")
            return
        try:
            selected = selected_file(name)
        except SourceUnavailable as exc:
            self.respond_error(503, str(exc))
            return
        if selected is None:
            self.respond_error(404, "not found")
            return
        file_path, content_type = selected
        try:
            base_dir = refresh().resolve()
            resolved_file_path = file_path.resolve()
            if resolved_file_path != base_dir and base_dir not in resolved_file_path.parents:
                self.respond_error(404, "not found")
                return
            size = resolved_file_path.stat().st_size
            start, end, status = 0, size - 1, 200
            range_header = self.headers.get("Range")
            if range_header:
                match = re.fullmatch(r"bytes=(\d*)-(\d*)", range_header.strip())
                if not match or size == 0:
                    self.send_response(416)
                    self.send_header("Content-Range", f"bytes */{size}")
                    self.send_header("Content-Length", "0")
                    self.end_headers()
                    return
                left, right = match.groups()
                if left:
                    start = int(left)
                    end = int(right) if right else size - 1
                elif right:
                    length = int(right)
                    start = max(0, size - length)
                if start >= size or end < start:
                    self.send_response(416)
                    self.send_header("Content-Range", f"bytes */{size}")
                    self.send_header("Content-Length", "0")
                    self.end_headers()
                    return
                end = min(end, size - 1)
                status = 206
            length = max(0, end - start + 1)
            self.send_response(status)
            self.send_header("Content-Type", content_type)
            self.send_header("Content-Length", str(length))
            self.send_header("Accept-Ranges", "bytes")
            self.send_header("Cache-Control", "no-cache")
            if status == 206:
                self.send_header("Content-Range", f"bytes {start}-{end}/{size}")
            self.end_headers()
            if self.command == "HEAD":
                return
            with resolved_file_path.open("rb") as source:
                source.seek(start)
                remaining = length
                while remaining:
                    block = source.read(min(CHUNK, remaining))
                    if not block:
                        break
                    self.wfile.write(block)
                    remaining -= len(block)
        except (BrokenPipeError, ConnectionResetError):
            # A cancelled client may resume via Range without corrupting cache.
            return
        except OSError as exc:
            print(f"losos-sysupdate-proxy: file serving failed: {exc}", flush=True)


def main() -> None:
    # All cache/download paths are explicitly on /persist. Failure to create
    # them is fatal; silently falling back to the root tmpfs is not allowed.
    ROOT.mkdir(mode=0o700, parents=True, exist_ok=True)
    TEMP.mkdir(mode=0o700, parents=True, exist_ok=True)
    server = ThreadingHTTPServer((HOST, PORT), Handler)
    server.daemon_threads = True
    print(
        f"losos-sysupdate-proxy: listening on {HOST}:{PORT}; OCI source {OCI_REF}; "
        f"persistent cache {ROOT}",
        flush=True,
    )
    server.serve_forever()


if __name__ == "__main__":
    main()
