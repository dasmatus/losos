#!/usr/bin/env python3
"""Offline unit tests for the LosOS systemd-sysupdate proxy."""
import importlib.util
from pathlib import Path
import unittest

path = Path(__file__).with_name("losos-sysupdate-proxy.py")
spec = importlib.util.spec_from_file_location("losos_sysupdate_proxy", path)
assert spec is not None and spec.loader is not None
proxy = importlib.util.module_from_spec(spec)
spec.loader.exec_module(proxy)

BUNDLE = "losos-update_v0.6.1.tar.xz"
PART = BUNDLE + ".part000"


def manifest(*entries: tuple[str, str]) -> bytes:
    return "".join(f"{digest}  {name}\n" for digest, name in entries).encode("ascii")


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
