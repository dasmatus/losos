#!/usr/bin/env python3
"""Small on-demand Nix cache backed by the public GHCR OCI artifacts.

The publisher stores one signed narinfo/NAR pair at nix-cache:<store-hash>.
Nix talks to this process over HTTP; oras fetches an artifact only when Nix
asks for a path, and the signed narinfo is otherwise left intact. The URL
field is rewritten to this local server because it is not covered by the Nix
cache signature.
"""
import http.server
import os
import re
import shutil
import subprocess
import tempfile
import threading
from pathlib import Path
from urllib.parse import urlsplit

REPOSITORY = os.environ.get("LOSOS_GHCR_CACHE", "ghcr.io/dasmatus/losos/nix-cache")
CACHE = Path(os.environ.get("LOSOS_GHCR_CACHE_DIR", "/var/cache/losos-ghcr-cache"))
HOST = os.environ.get("LOSOS_GHCR_CACHE_HOST", "127.0.0.1")
PORT = int(os.environ.get("LOSOS_GHCR_CACHE_PORT", "8787"))
STORE_HASH = re.compile(r"^[0-9a-df-np-sv-z]{32}$")
NAR_NAME = re.compile(r"^[0-9a-df-np-sv-z]{1,64}\.nar(?:\.(?:zst|xz|bz2))?$")
locks = {}
locks_guard = threading.Lock()


def cached_nar(hash_value):
    directory = CACHE / "nar" / hash_value
    if not directory.is_dir():
        return None
    files = [p for p in directory.iterdir() if p.is_file() and NAR_NAME.fullmatch(p.name)]
    return files[0] if len(files) == 1 else None


def fetch_artifact(hash_value):
    info = CACHE / (hash_value + ".narinfo")
    if info.is_file() and cached_nar(hash_value):
        return True
    with locks_guard:
        path_lock = locks.setdefault(hash_value, threading.Lock())
    with path_lock:
        if info.is_file() and cached_nar(hash_value):
            return True
        CACHE.mkdir(parents=True, exist_ok=True)
        with tempfile.TemporaryDirectory(prefix="losos-oras-", dir=CACHE) as temporary:
            result = subprocess.run(
                ["oras", "pull", "--output", temporary, f"{REPOSITORY}:{hash_value}"],
                stdout=subprocess.DEVNULL,
                stderr=subprocess.PIPE,
                text=True,
                timeout=180,
                check=False,
            )
            if result.returncode:
                return False
            root = Path(temporary)
            narinfos = list(root.glob("*.narinfo"))
            if len(narinfos) != 1 or narinfos[0].name != info.name:
                return False
            lines = narinfos[0].read_text(encoding="utf-8").splitlines()
            urls = [line[5:].strip() for line in lines if line.startswith("URL: ")]
            if len(urls) != 1:
                return False
            nar_name = Path(urlsplit(urls[0]).path).name
            if not NAR_NAME.fullmatch(nar_name):
                return False
            nar_source = root / nar_name
            if not nar_source.is_file():
                return False
            nar_root = (CACHE / "nar").resolve()
            destination = (nar_root / hash_value).resolve()
            try:
                destination.relative_to(nar_root)
            except ValueError:
                return False
            destination.mkdir(parents=True, exist_ok=True)
            nar_target = destination / nar_name
            nar_target_resolved = nar_target.resolve()
            try:
                nar_target_resolved.relative_to(destination)
            except ValueError:
                return False
            os.replace(nar_source, nar_target_resolved)
            rewritten = []
            for line in lines:
                rewritten.append(f"URL: nar/{hash_value}/{nar_name}" if line.startswith("URL: ") else line)
            temporary_info = CACHE / (hash_value + ".narinfo.tmp")
            temporary_info.write_text("\n".join(rewritten) + "\n", encoding="utf-8")
            os.replace(temporary_info, info)
            return True


class Handler(http.server.BaseHTTPRequestHandler):
    server_version = "LosOS-GHCR-Cache/1"

    def log_message(self, fmt, *args):
        print("losos-ghcr-cache:", fmt % args, flush=True)

    def respond(self, status, body, content_type="text/plain; charset=utf-8"):
        self.send_response(status)
        self.send_header("Content-Type", content_type)
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        if self.command != "HEAD":
            self.wfile.write(body)

    def respond_file(self, path):
        size = path.stat().st_size
        self.send_response(200)
        self.send_header("Content-Type", "application/octet-stream")
        self.send_header("Content-Length", str(size))
        self.end_headers()
        if self.command != "HEAD":
            with path.open("rb") as source:
                shutil.copyfileobj(source, self.wfile, length=1024 * 1024)

    def do_HEAD(self):
        self.do_GET()

    def do_GET(self):
        path = urlsplit(self.path).path.lstrip("/")
        if path == "nix-cache-info":
            return self.respond(200, b"StoreDir: /nix/store\nWantMassQuery: 1\nPriority: 25\n",
                                "text/x-nix-cache-info")
        if path.endswith(".narinfo"):
            hash_value = path[:-8]
            if not STORE_HASH.fullmatch(hash_value) or "/" in path:
                return self.respond(404, b"not found\n")
            if not fetch_artifact(hash_value):
                return self.respond(404, b"cache miss\n")
            return self.respond(200, (CACHE / (hash_value + ".narinfo")).read_bytes(),
                                "text/x-nix-narinfo")
        parts = path.split("/")
        if len(parts) == 3 and parts[0] == "nar" and STORE_HASH.fullmatch(parts[1]) and NAR_NAME.fullmatch(parts[2]):
            if not fetch_artifact(parts[1]):
                return self.respond(404, b"cache miss\n")
            nar = CACHE / "nar" / parts[1] / parts[2]
            if not nar.is_file():
                return self.respond(404, b"not found\n")
            return self.respond_file(nar)
        return self.respond(404, b"not found\n")


if __name__ == "__main__":
    print(f"losos-ghcr-cache: serving {REPOSITORY} on http://{HOST}:{PORT}/", flush=True)
    http.server.ThreadingHTTPServer((HOST, PORT), Handler).serve_forever()
