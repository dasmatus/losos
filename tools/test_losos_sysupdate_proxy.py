#!/usr/bin/env python3
"""Offline unit tests for the LosOS systemd-sysupdate proxy."""
import hashlib
import importlib.util
import io
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
import urllib.error

path = Path(__file__).with_name("losos-sysupdate-proxy.py")
spec = importlib.util.spec_from_file_location("losos_sysupdate_proxy", path)
assert spec is not None and spec.loader is not None
proxy = importlib.util.module_from_spec(spec)
spec.loader.exec_module(proxy)

BUNDLE = "losos-update_v0.6.1.tar.xz"
PART = BUNDLE + ".part000"


def manifest(*entries: tuple[str, str]) -> bytes:
    return "".join(f"{digest}  {name}\n" for digest, name in entries).encode("ascii")


class FakeResponse(io.BytesIO):
    def __init__(self, data: bytes, status: int, headers: dict[str, str]):
        super().__init__(data)
        self.status = status
        self.headers = headers

    def __enter__(self):
        return self

    def __exit__(self, *_args):
        self.close()


class DownloadTests(unittest.TestCase):
    def test_resumes_persistent_partial_with_range(self) -> None:
        payload = b"persistent update payload" * 20
        offset = 97
        digest = hashlib.sha256(payload).hexdigest()
        with tempfile.TemporaryDirectory() as temporary:
            destination = Path(temporary) / "asset.part"
            partial = destination.with_name(destination.name + ".download")
            partial.write_bytes(payload[:offset])
            response = FakeResponse(
                payload[offset:],
                206,
                {"Content-Range": f"bytes {offset}-{len(payload) - 1}/{len(payload)}"},
            )
            with patch.object(proxy.urllib.request, "urlopen", return_value=response) as opened:
                self.assertEqual(proxy.download("https://example.invalid/asset", destination, digest), destination)
            request = opened.call_args.args[0]
            self.assertEqual(request.get_header("Range"), f"bytes={offset}-")
            self.assertEqual(destination.read_bytes(), payload)
            self.assertFalse(partial.exists())

    def test_restarts_when_server_ignores_range(self) -> None:
        payload = b"fresh response from byte zero"
        digest = hashlib.sha256(payload).hexdigest()
        with tempfile.TemporaryDirectory() as temporary:
            destination = Path(temporary) / "asset.part"
            destination.with_name(destination.name + ".download").write_bytes(b"stale prefix")
            response = FakeResponse(payload, 200, {})
            with patch.object(proxy.urllib.request, "urlopen", return_value=response):
                proxy.download("https://example.invalid/asset", destination, digest)
            self.assertEqual(destination.read_bytes(), payload)

    def test_keeps_partial_after_transient_connection_failure(self) -> None:
        payload = b"already downloaded bytes"
        with tempfile.TemporaryDirectory() as temporary:
            destination = Path(temporary) / "asset.part"
            partial = destination.with_name(destination.name + ".download")
            partial.write_bytes(payload)
            with patch.object(proxy.urllib.request, "urlopen", side_effect=urllib.error.URLError("connection reset")):
                with self.assertRaises(urllib.error.URLError):
                    proxy.download("https://example.invalid/asset", destination, "a" * 64)
            self.assertEqual(partial.read_bytes(), payload)


class ManifestTests(unittest.TestCase):
    def test_parses_bundle_and_split_asset_hashes(self) -> None:
        parsed = proxy.parse_manifest(manifest(("a" * 64, BUNDLE), ("b" * 64, PART)))
        self.assertEqual(parsed[BUNDLE], "a" * 64)
        self.assertEqual(parsed[PART], "b" * 64)

    def test_accepts_binary_style_sha256sum_entries(self) -> None:
        data = f'{"a" * 64} *{BUNDLE}\n'.encode("ascii")
        self.assertEqual(proxy.parse_manifest(data)[BUNDLE], "a" * 64)

    def test_rejects_path_traversal(self) -> None:
        with self.assertRaisesRegex(ValueError, "unsafe asset name"):
            proxy.parse_manifest(manifest(("a" * 64, "../payload.tar.xz")))

    def test_rejects_duplicate_assets(self) -> None:
        with self.assertRaisesRegex(ValueError, "duplicate"):
            proxy.parse_manifest(manifest(("a" * 64, BUNDLE), ("b" * 64, BUNDLE)))

    def test_rejects_invalid_checksum_line(self) -> None:
        with self.assertRaisesRegex(ValueError, "malformed"):
            proxy.parse_manifest(b"not-a-checksum  " + BUNDLE.encode("ascii") + b"\n")

    def test_rejects_unrecognized_chunk_names(self) -> None:
        with self.assertRaisesRegex(ValueError, "unsafe chunk"):
            proxy.parse_manifest(manifest(("a" * 64, BUNDLE), ("b" * 64, BUNDLE + ".partXYZ")))


if __name__ == "__main__":
    unittest.main()
