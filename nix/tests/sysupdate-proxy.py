#!/usr/bin/env python3
"""Offline regression tests for tools/losos-sysupdate-proxy.py."""
from __future__ import annotations

import hashlib
import importlib.util
import tempfile
import threading
import unittest
from http.server import ThreadingHTTPServer
from pathlib import Path
from urllib.error import HTTPError
from urllib.request import Request, urlopen

ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT / "tools" / "losos-sysupdate-proxy.py"
SPEC = importlib.util.spec_from_file_location("losos_sysupdate_proxy", SOURCE)
assert SPEC is not None and SPEC.loader is not None
proxy = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(proxy)


class BundleValidationTests(unittest.TestCase):
    def setUp(self) -> None:
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name)
        self.bundle_name = "losos-os_0.6.1.0000000001.tar.xz"
        self.payload = b"fake-but-checksummed-xz-tar-payload"
        (self.root / self.bundle_name).write_bytes(self.payload)
        digest = hashlib.sha256(self.payload).hexdigest()
        (self.root / "SHA256SUMS").write_text(
            f"{digest}  {self.bundle_name}\n", encoding="ascii"
        )

    def tearDown(self) -> None:
        self.tmp.cleanup()

    def test_valid_manifest_names_one_regular_payload(self) -> None:
        name, path = proxy.validate_bundle(self.root)
        self.assertEqual(name, self.bundle_name)
        self.assertEqual(path.read_bytes(), self.payload)

    def test_rejects_checksum_mismatch(self) -> None:
        (self.root / "SHA256SUMS").write_text(
            f"{'0' * 64}  {self.bundle_name}\n", encoding="ascii"
        )
        with self.assertRaises(proxy.SourceUnavailable):
            proxy.validate_bundle(self.root)

    def test_rejects_multiple_files_in_manifest(self) -> None:
        digest = hashlib.sha256(self.payload).hexdigest()
        (self.root / "SHA256SUMS").write_text(
            f"{digest}  {self.bundle_name}\n{digest}  other.tar.xz\n",
            encoding="ascii",
        )
        with self.assertRaises(proxy.SourceUnavailable):
            proxy.validate_bundle(self.root)

    def test_rejects_path_traversal_manifest_name(self) -> None:
        digest = hashlib.sha256(self.payload).hexdigest()
        (self.root / "SHA256SUMS").write_text(
            f"{digest}  ../{self.bundle_name}\n", encoding="ascii"
        )
        with self.assertRaises(proxy.SourceUnavailable):
            proxy.validate_bundle(self.root)

    def test_rejects_unlisted_bundle_name(self) -> None:
        old_refresh = proxy.refresh
        proxy.refresh = lambda: self.root
        try:
            self.assertIsNone(proxy.selected_file("../secret"))
            self.assertIsNone(proxy.selected_file("losos-os_0.6.1.tar.xz"))
        finally:
            proxy.refresh = old_refresh

    def test_serves_exact_manifest_bundle_and_byte_ranges(self) -> None:
        old_refresh = proxy.refresh
        proxy.refresh = lambda: self.root
        server = ThreadingHTTPServer(("127.0.0.1", 0), proxy.Handler)
        server.daemon_threads = True
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        base = f"http://127.0.0.1:{server.server_port}"
        try:
            with urlopen(base + "/updates/SHA256SUMS", timeout=3) as response:
                self.assertEqual(response.status, 200)
                self.assertIn(self.bundle_name.encode(), response.read())
            request = Request(
                base + "/updates/" + self.bundle_name,
                headers={"Range": "bytes=3-8"},
            )
            with urlopen(request, timeout=3) as response:
                self.assertEqual(response.status, 206)
                self.assertEqual(response.headers["Content-Range"], f"bytes 3-8/{len(self.payload)}")
                self.assertEqual(response.read(), self.payload[3:9])
            with self.assertRaises(HTTPError) as refused:
                urlopen(base + "/updates/%2e%2e%2fsecret", timeout=3)
            self.assertEqual(refused.exception.code, 404)
        finally:
            server.shutdown()
            server.server_close()
            thread.join(timeout=3)
            proxy.refresh = old_refresh


if __name__ == "__main__":
    unittest.main(verbosity=2)
