#!/usr/bin/env python3
"""Allowlist proxy and persistent disk cache for systemd-sysupdate bundles.

This deliberately is not a general-purpose URL proxy. It exposes only the
checksums manifest and immutable numeric-version bundles from the fixed
upstream. Both the cache and every partial download are placed in the
configured /var/lib path, which is persisted by LosOS impermanence.
"""

from __future__ import annotations

import os
import re
import shutil
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
                    with part.open("wb") as output:
                        shutil.copyfileobj(response, output, length=1024 * 1024)
                        output.flush()
                        os.fsync(output.fileno())
                if part.stat().st_size == 0:
                    raise OSError("upstream returned an empty update file")
                os.replace(part, destination)
                return destination
            except (OSError, urllib.error.URLError, TimeoutError) as error:
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
        except (OSError, urllib.error.URLError) as error:
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
