#!/usr/bin/env python3
"""Persistent, signature-verifying source proxy for systemd-sysupdate.

The installed machine only trusts a manifest after OpenSSL verifies it against
the repository's pinned release certificate.  systemd-sysupdate subsequently
checks the archive's SHA-256 against that manifest.  Large release assets are
published as individually size-limited chunks; this service reassembles and
verifies them in its persistent cache before exposing the logical .tar.xz file.

All downloads, partial downloads, manifests and extracted-source cache entries
live below LOSOS_SYSUPDATE_CACHE_DIR (normally /persist/var/cache/losos-sysupdate).
Nothing is written to /tmp or any other tmpfs.
"""
from __future__ import annotations

import hashlib
import http.server
import os
from pathlib import Path
import re
import subprocess
import time
import urllib.error
import urllib.parse
import urllib.request

REPOSITORY = os.environ.get("LOSOS_SYSUPDATE_REPOSITORY", "dasmatus/losos")
CACHE = Path(os.environ.get(
    "LOSOS_SYSUPDATE_CACHE_DIR", "/persist/var/cache/losos-sysupdate-proxy"
))
CERT = Path(os.environ.get("LOSOS_SYSUPDATE_CERT", ""))
HOST = os.environ.get("LOSOS_SYSUPDATE_LISTEN", "127.0.0.1")
PORT = int(os.environ.get("LOSOS_SYSUPDATE_PORT", "8788"))
MANIFEST_TTL = int(os.environ.get("LOSOS_SYSUPDATE_MANIFEST_TTL", "300"))
NAME_RE = re.compile(r"^[A-Za-z0-9][A-Za-z0-9_.+-]{0,240}$")
HASH_RE = re.compile(r"^[0-9a-f]{64}$")
CHUNK_RE = re.compile(r"^(losos-update_[A-Za-z0-9_.+-]+\.tar\.xz)\.part([0-9]{3,})$")
REDIRECT_CODES = {301, 302, 303, 307, 308}


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        return None


def atomic_write(path: Path, data: bytes) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    # The temporary sibling is deliberately on the same persistent filesystem.
    temp = path.with_name(path.name + ".download")
    with temp.open("wb") as out:
        out.write(data)
        out.flush()
        os.fsync(out.fileno())
    os.replace(temp, path)


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def download(url: str, destination: Path, expected_hash: str | None = None) -> Path:
    destination.parent.mkdir(parents=True, exist_ok=True)
    if destination.is_file() and (expected_hash is None or sha256_file(destination) == expected_hash):
        return destination
    partial = destination.with_name(destination.name + ".download")
    partial.unlink(missing_ok=True)
    digest = hashlib.sha256()
    try:
        with urllib.request.urlopen(url, timeout=45) as response, partial.open("wb") as out:
            while True:
                block = response.read(1024 * 1024)
                if not block:
                    break
                out.write(block)
                digest.update(block)
            out.flush()
            os.fsync(out.fileno())
        actual = digest.hexdigest()
        if expected_hash is not None and actual != expected_hash:
            raise ValueError(f"SHA-256 mismatch for {destination.name}: {actual}")
        os.replace(partial, destination)
        return destination
    except Exception:
        partial.unlink(missing_ok=True)
        raise


def fetch_release_base() -> str:
    latest = f"https://github.com/{REPOSITORY}/releases/latest/download/SHA256SUMS.sysupdate"
    opener = urllib.request.build_opener(NoRedirect)
    request = urllib.request.Request(latest, method="GET", headers={"User-Agent": "LosOS-sysupdate-proxy/1"})
    try:
        response = opener.open(request, timeout=30)
    except urllib.error.HTTPError as error:
        if error.code not in REDIRECT_CODES:
            raise
        location = error.headers.get("Location", "")
    else:
        # An unredirected manifest is not a release asset URL and cannot be
        # safely paired with a signature from a different version.
        response.close()
        raise RuntimeError("GitHub latest release did not redirect to a versioned asset")
    expected = re.compile(
        r"^https://github\.com/" + re.escape(REPOSITORY)
        + r"/releases/download/[A-Za-z0-9._-]+/SHA256SUMS\.sysupdate$"
    )
    if not expected.fullmatch(location):
        raise RuntimeError("latest-release redirect was not a versioned LosOS update manifest")
    return location.rsplit("/", 1)[0]


def parse_manifest(data: bytes) -> dict[str, str]:
    entries: dict[str, str] = {}
    for raw in data.decode("ascii").splitlines():
        if not raw:
            continue
        match = re.fullmatch(r"([0-9a-f]{64}) [ *](\S+)", raw)
        if not match:
            raise ValueError("update manifest contains a malformed checksum line")
        checksum, name = match.groups()
        if not NAME_RE.fullmatch(name) or name in {".", ".."} or "/" in name or "\\" in name:
            raise ValueError("update manifest contains an unsafe asset name")
        if name in entries:
            raise ValueError("update manifest contains duplicate asset names")
        entries[name] = checksum
    if not entries or not any(name.startswith("losos-update_") and name.endswith(".tar.xz") for name in entries):
        raise ValueError("update manifest contains no versioned LosOS update bundle")
    for name in entries:
        if name.endswith(".tar.xz.part") or ".tar.xz.part" in name:
            if not CHUNK_RE.fullmatch(name):
                raise ValueError("update manifest contains an unsafe chunk name")
    return entries


def verify_manifest(data: bytes, signature: bytes) -> dict[str, str]:
    if not CERT.is_file():
        raise RuntimeError(f"pinned release certificate is missing: {CERT}")
    CACHE.mkdir(parents=True, exist_ok=True)
    public_key = CACHE / "release-public-key.pem"
    if not public_key.is_file() or public_key.stat().st_mtime < CERT.stat().st_mtime:
        result = subprocess.run(
            ["openssl", "x509", "-in", str(CERT), "-pubkey", "-noout"],
            check=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
        )
        atomic_write(public_key, result.stdout)
        public_key.chmod(0o644)
    manifest_file = CACHE / "verified-SHA256SUMS"
    signature_file = CACHE / "verified-SHA256SUMS.sig"
    atomic_write(manifest_file, data)
    atomic_write(signature_file, signature)
    result = subprocess.run(
        ["openssl", "dgst", "-sha256", "-verify", str(public_key),
         "-signature", str(signature_file), str(manifest_file)],
        stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True,
    )
    if result.returncode != 0 or "Verified OK" not in result.stdout:
        raise RuntimeError("update manifest signature verification failed")
    return parse_manifest(data)


def get_manifest(force: bool = False) -> tuple[bytes, dict[str, str], str]:
    manifest_path = CACHE / "verified-SHA256SUMS"
    signature_path = CACHE / "verified-SHA256SUMS.sig"
    base_path = CACHE / "release-base.txt"
    stamp_path = CACHE / "manifest-refreshed"
    fresh = (
        not force and manifest_path.is_file() and signature_path.is_file()
        and base_path.is_file() and stamp_path.is_file()
        and time.time() - float(stamp_path.read_text().strip()) < MANIFEST_TTL
    )
    if not fresh:
        try:
            base = fetch_release_base()
            manifest = download(base + "/SHA256SUMS.sysupdate", CACHE / "downloaded-SHA256SUMS").read_bytes()
            signature = download(base + "/SHA256SUMS.sysupdate.sig", CACHE / "downloaded-SHA256SUMS.sig").read_bytes()
            entries = verify_manifest(manifest, signature)
            atomic_write(base_path, (base + "\n").encode())
            atomic_write(stamp_path, (str(time.time()) + "\n").encode())
            # Preserve the exact bytes verified; systemd must see this manifest.
            atomic_write(manifest_path, manifest)
            atomic_write(signature_path, signature)
            return manifest, entries, base
        except Exception:
            # Offline boxes can continue to use the last valid signed manifest.
            # No cache is ever trusted merely because it exists: verify the
            # signature again before serving it.
            if not (manifest_path.is_file() and signature_path.is_file() and base_path.is_file()):
                raise
    manifest = manifest_path.read_bytes()
    signature = signature_path.read_bytes()
    base = base_path.read_text().strip()
    if not base.startswith(f"https://github.com/{REPOSITORY}/releases/download/"):
        raise RuntimeError("cached release base is not a trusted LosOS release URL")
    entries = verify_manifest(manifest, signature)
    return manifest, entries, base


def get_verified_asset(name: str, entries: dict[str, str], base: str) -> Path:
    expected = entries.get(name)
    if expected is None:
        raise FileNotFoundError(name)
    bundle_match = re.fullmatch(r"losos-update_[A-Za-z0-9_.+-]+\.tar\.xz", name)
    if bundle_match:
        bundle_dir = CACHE / "bundles"
        bundle_dir.mkdir(parents=True, exist_ok=True)
        output = bundle_dir / name
        if output.is_file() and sha256_file(output) == expected:
            return output
        chunks: list[tuple[int, str, str]] = []
        for part_name, checksum in entries.items():
            match = CHUNK_RE.fullmatch(part_name)
            if match and match.group(1) == name:
                chunks.append((int(match.group(2)), part_name, checksum))
        chunks.sort()
        if not chunks or [n for n, _, _ in chunks] != list(range(chunks[0][0], chunks[0][0] + len(chunks))):
            raise RuntimeError("update bundle chunk list is incomplete")
        if chunks[0][0] != 0:
            raise RuntimeError("update bundle chunks do not start at part 000")
        chunk_dir = CACHE / "parts"
        assembled = output.with_name(output.name + ".assembling")
        assembled.unlink(missing_ok=True)
        try:
            with assembled.open("wb") as out:
                for _, part_name, checksum in chunks:
                    part = download(base + "/" + urllib.parse.quote(part_name), chunk_dir / part_name, checksum)
                    with part.open("rb") as inp:
                        while True:
                            block = inp.read(1024 * 1024)
                            if not block:
                                break
                            out.write(block)
                out.flush()
                os.fsync(out.fileno())
            actual = sha256_file(assembled)
            if actual != expected:
                raise ValueError(f"assembled update bundle checksum mismatch: {actual}")
            os.replace(assembled, output)
        except Exception:
            assembled.unlink(missing_ok=True)
            raise
        return output
    return download(base + "/" + urllib.parse.quote(name), CACHE / "assets" / name, expected)


class Handler(http.server.BaseHTTPRequestHandler):
    server_version = "LosOS-sysupdate-proxy/1"

    def log_message(self, fmt: str, *args: object) -> None:
        print("%s - %s" % (self.address_string(), fmt % args), flush=True)

    def do_GET(self) -> None:
        self._serve(False)

    def do_HEAD(self) -> None:
        self._serve(True)

    def _serve(self, head_only: bool) -> None:
        parsed = urllib.parse.urlsplit(self.path)
        if parsed.query or "%" in parsed.path:
            self.send_error(400, "invalid update URL")
            return
        if parsed.path == "/health":
            body = b"ok\n"
            self.send_response(200)
            self.send_header("Content-Type", "text/plain")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            if not head_only:
                self.wfile.write(body)
            return
        prefix = "/stable/"
        if not parsed.path.startswith(prefix):
            self.send_error(404)
            return
        name = parsed.path[len(prefix):]
        if name in {"SHA256SUMS", "SHA256SUMS.sysupdate"}:
            try:
                manifest, _, _ = get_manifest()
                self._send_bytes(manifest, "text/plain; charset=us-ascii", head_only)
            except Exception as error:
                self.log_error("cannot serve signed manifest: %s", error)
                self.send_error(503, "no verified update manifest available")
            return
        if name == "SHA256SUMS.gpg":
            # Verify=no is explicit in the transfer definition because the
            # project signs this manifest with its existing pinned RSA key,
            # rather than an OpenPGP key. The proxy verifies that signature.
            self.send_error(404)
            return
        if not NAME_RE.fullmatch(name) or "/" in name or "\\" in name:
            self.send_error(400, "invalid update filename")
            return
        try:
            _, entries, base = get_manifest()
            path = get_verified_asset(name, entries, base)
            self._send_file(path, head_only)
        except FileNotFoundError:
            self.send_error(404, "asset is not in the signed manifest")
        except Exception as error:
            self.log_error("cannot serve %s: %s", name, error)
            self.send_error(503, "update asset could not be verified or downloaded")

    def _send_bytes(self, data: bytes, content_type: str, head_only: bool) -> None:
        self.send_response(200)
        self.send_header("Content-Type", content_type)
        self.send_header("Content-Length", str(len(data)))
        self.send_header("Cache-Control", "no-store")
        self.end_headers()
        if not head_only:
            self.wfile.write(data)

    def _send_file(self, path: Path, head_only: bool) -> None:
        size = path.stat().st_size
        start, end, status = 0, max(0, size - 1), 200
        range_header = self.headers.get("Range")
        if range_header:
            match = re.fullmatch(r"bytes=([0-9]*)-([0-9]*)", range_header.strip())
            if not match or (not match.group(1) and not match.group(2)):
                self.send_error(416, "only one byte range is supported")
                return
            if match.group(1):
                start = int(match.group(1))
                end = min(size - 1, int(match.group(2)) if match.group(2) else size - 1)
            else:
                suffix = int(match.group(2))
                start = max(0, size - suffix)
            if start >= size or end < start:
                self.send_response(416)
                self.send_header("Content-Range", f"bytes */{size}")
                self.end_headers()
                return
            status = 206
        length = max(0, end - start + 1)
        self.send_response(status)
        self.send_header("Content-Type", "application/x-xz" if path.name.endswith(".xz") else "application/octet-stream")
        self.send_header("Accept-Ranges", "bytes")
        self.send_header("Content-Length", str(length))
        self.send_header("Cache-Control", "public, max-age=300")
        if status == 206:
            self.send_header("Content-Range", f"bytes {start}-{end}/{size}")
        self.end_headers()
        if head_only:
            return
        with path.open("rb") as stream:
            stream.seek(start)
            remaining = length
            while remaining:
                block = stream.read(min(1024 * 1024, remaining))
                if not block:
                    break
                self.wfile.write(block)
                remaining -= len(block)


def main() -> None:
    if not CERT.is_file():
        raise SystemExit(f"LosOS update signing certificate is missing: {CERT}")
    CACHE.mkdir(parents=True, exist_ok=True)
    for directory in ("assets", "parts", "bundles"):
        (CACHE / directory).mkdir(parents=True, exist_ok=True)
    server = http.server.ThreadingHTTPServer((HOST, PORT), Handler)
    server.daemon_threads = True
    print(f"serving signed LosOS updates on http://{HOST}:{PORT}/stable/; cache={CACHE}", flush=True)
    server.serve_forever()


if __name__ == "__main__":
    main()
