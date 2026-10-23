#!/usr/bin/env python3
"""Regression tests for the persistent systemd-sysupdate proxy/importer."""

from __future__ import annotations

import importlib.util
import io
import json
import os
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch


ROOT = Path(__file__).resolve().parent


def load_tool(name: str, filename: str):
    spec = importlib.util.spec_from_file_location(name, ROOT / filename)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot load {filename}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


proxy = load_tool("losos_sysupdate_proxy", "losos-sysupdate-proxy.py")
apply = load_tool("losos_sysupdate_apply", "losos-sysupdate-apply.py")


class FakeResponse(io.BytesIO):
    def __init__(self, body: bytes):
        super().__init__(body)
        self.status = 200

    def __enter__(self):
        return self

    def __exit__(self, *_args):
        self.close()


class ProxyTests(unittest.TestCase):
    def test_proxy_allowlist_rejects_arbitrary_paths(self):
        self.assertEqual(proxy.safe_name("/SHA256SUMS"), "SHA256SUMS")
        self.assertEqual(
            proxy.safe_name("/losos-components_42.tar.xz"),
            "losos-components_42.tar.xz",
        )
        for path in (
            "/",
            "/SHA256SUMS.gpg",
            "/../etc/passwd",
            "/losos-components_latest.tar.xz",
            "/losos-components_1.tar.xz/../secret",
            "/losos-components_1.tar.gz",
        ):
            with self.subTest(path=path):
                self.assertIsNone(proxy.safe_name(path))

    def test_archive_is_cached_atomically_under_configured_persistent_dir(self):
        with tempfile.TemporaryDirectory() as temporary:
            cache_dir = Path(temporary)
            with (
                patch.object(proxy, "CACHE_DIR", cache_dir),
                patch.object(proxy, "UPSTREAM", "https://updates.example.invalid/sysupdate"),
                patch.object(proxy.urllib.request, "urlopen", return_value=FakeResponse(b"bundle-data")) as urlopen,
            ):
                result = proxy.download(
                    "losos-components_42.tar.xz",
                    cache_dir / "losos-components_42.tar.xz",
                )
                self.assertEqual(result.read_bytes(), b"bundle-data")
                self.assertTrue(result.is_file())
                self.assertEqual(
                    sorted(p.name for p in cache_dir.iterdir()),
                    ["losos-components_42.tar.xz"],
                )
                urlopen.assert_called_once()

    def test_manifest_cache_expires_but_versioned_bundle_does_not(self):
        with tempfile.TemporaryDirectory() as temporary:
            manifest = Path(temporary) / "SHA256SUMS"
            archive = Path(temporary) / "losos-components_42.tar.xz"
            manifest.write_text("checksum  losos-components_42.tar.xz\n")
            archive.write_bytes(b"x")
            old = 1000
            os.utime(manifest, (old, old))
            os.utime(archive, (old, old))
            with patch.object(proxy, "MANIFEST_TTL_SECONDS", 10):
                with patch.object(proxy.time, "time", return_value=old + 11):
                    self.assertTrue(proxy.stale("SHA256SUMS", manifest))
                    self.assertFalse(proxy.stale(archive.name, archive))


class ApplyTests(unittest.TestCase):
    def test_candidates_are_numeric_directories_and_ignore_external_symlinks(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            slots = root / "slots"
            slots.mkdir()
            (slots / "losos-components_7").mkdir()
            (slots / "losos-components_19").mkdir()
            (slots / "losos-components_latest").mkdir()
            outside = root / "outside"
            outside.mkdir()
            (slots / "losos-components_99").symlink_to(outside, target_is_directory=True)
            with patch.object(apply, "SLOTS_DIR", slots):
                self.assertEqual(
                    [(v, p.name) for v, p in apply.candidates()],
                    [(7, "losos-components_7"), (19, "losos-components_19")],
                )

    def test_import_requires_valid_store_paths_and_nix_signature_checking(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary) / "losos-components_23"
            cache = directory / "cache"
            cache.mkdir(parents=True)
            (cache / "nix-cache-info").write_text("StoreDir: /nix/store\n")
            (cache / ("a" * 32 + ".narinfo")).write_text("StorePath: /nix/store/example\n")
            store_path = "/nix/store/" + ("a" * 32) + "-losos-ctl"
            manifest = {
                "format": 1,
                "version": 23,
                "sourceVersion": "v-test",
                "commit": "0123456789abcdef",
                "storePaths": [store_path],
            }
            (directory / "manifest.json").write_text(json.dumps(manifest))
            with patch.object(apply.subprocess, "run") as run:
                result = apply.import_bundle(23, directory)
            self.assertEqual(result["version"], 23)
            args = run.call_args.args[0]
            self.assertIn("--recursive", args)
            self.assertNotIn("--no-check-sigs", args)
            self.assertIn(store_path, args)

    def test_malformed_store_path_is_refused_before_nix_is_called(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary) / "losos-components_9"
            cache = directory / "cache"
            cache.mkdir(parents=True)
            (cache / "nix-cache-info").write_text("StoreDir: /nix/store\n")
            (directory / "manifest.json").write_text(json.dumps({
                "format": 1,
                "version": 9,
                "storePaths": ["/etc/shadow"],
            }))
            with patch.object(apply.subprocess, "run") as run:
                with self.assertRaisesRegex(ValueError, "malformed Nix store path"):
                    apply.import_bundle(9, directory)
                run.assert_not_called()

    def test_active_marker_is_atomically_written_to_supplied_disk_path(self):
        with tempfile.TemporaryDirectory() as temporary:
            marker = Path(temporary) / "active.json"
            apply.atomic_json_write(marker, {"version": 5})
            self.assertEqual(json.loads(marker.read_text()), {"version": 5})
            self.assertEqual([p.name for p in marker.parent.iterdir()], ["active.json"])


if __name__ == "__main__":
    unittest.main(verbosity=2)
