#!/usr/bin/env python3
"""Make the flake's GitHub inputs resolvable without GitHub's tarball endpoint.

    flake-inputs.py <repository root>

Nix fetches a `github:` input as a tarball from codeload. Behind a proxy that
allows git but not that endpoint, every evaluation that reaches disko,
impermanence or home-manager fails with a 403, and so does every check in
the eval lane. For each `github` node in flake.lock whose tree is not in the
store and whose tarball does not answer, this fetches the locked rev over
git, adds the tree to the store and checks that its NAR hash is the one
flake.lock pins, so Nix then finds it without a download. Nothing is written
into the repository.
"""

import base64
import hashlib
import json
import subprocess
import sys
import tempfile
from pathlib import Path


def nix(*args, check=True):
    return subprocess.run(["nix", *args], capture_output=True, text=True, check=check)


def b32(data):
    """Nix's base-32, least significant bits first."""
    chars = "0123456789abcdfghijklmnpqrsvwxyz"
    n = (len(data) * 8 + 4) // 5
    out = []
    for i in range(n - 1, -1, -1):
        j, k = divmod(i * 5, 8)
        c = data[j] >> k
        if j + 1 < len(data):
            c |= data[j + 1] << (8 - k)
        out.append(chars[c & 0x1F])
    return "".join(out)


def store_path(nar_hash, name="source"):
    """Where a tree with this NAR hash lives: Nix's fixed-output path."""
    digest = base64.b64decode(nar_hash.split("-", 1)[1]).hex()
    inner = hashlib.sha256(f"source:sha256:{digest}:/nix/store:{name}".encode()).digest()
    folded = bytearray(20)
    for i, byte in enumerate(inner):
        folded[i % 20] ^= byte
    return f"/nix/store/{b32(bytes(folded))}-{name}"


def resolvable(locked):
    """In the store already, or GitHub's tarball endpoint answers."""
    if Path(store_path(locked["narHash"])).exists():
        return True
    url = f"https://github.com/{locked['owner']}/{locked['repo']}/archive/{locked['rev']}.tar.gz"
    p = subprocess.run(["curl", "-sfIL", "-o", "/dev/null", "--max-time", "20", url], check=False)
    return p.returncode == 0


def seed(locked):
    url = f"https://github.com/{locked['owner']}/{locked['repo']}"
    with tempfile.TemporaryDirectory() as tmp:
        repo = Path(tmp) / "git"
        tree = Path(tmp) / "source"
        tree.mkdir()
        subprocess.run(["git", "init", "-q", str(repo)], check=True)
        subprocess.run(["git", "-C", str(repo), "fetch", "-q", "--depth", "1", url, locked["rev"]], check=True)
        archive = subprocess.run(
            ["git", "-C", str(repo), "archive", "FETCH_HEAD"], capture_output=True, check=True
        ).stdout
        subprocess.run(["tar", "-x", "-C", str(tree)], input=archive, check=True)
        path = nix("store", "add", "--name", "source", str(tree)).stdout.strip()
    got = nix("path-info", "--json", "--json-format", "1", path, check=False)
    if got.returncode != 0:
        got = nix("path-info", "--json", path)
    info = json.loads(got.stdout)
    entry = info[path] if isinstance(info, dict) else info[0]
    if entry["narHash"] != locked["narHash"]:
        raise SystemExit(f"{url} at {locked['rev']}: NAR hash {entry['narHash']}, flake.lock pins {locked['narHash']}")
    return path


def main():
    root = Path(sys.argv[1] if len(sys.argv) > 1 else ".")
    nodes = json.loads((root / "flake.lock").read_text())["nodes"]
    for name, node in sorted(nodes.items()):
        locked = node.get("locked", {})
        if locked.get("type") != "github" or "narHash" not in locked:
            continue
        if resolvable(locked):
            print(f"{name}: in the store or fetchable")
            continue
        print(f"{name}: tarball unreachable, fetching {locked['owner']}/{locked['repo']} over git")
        print(f"{name}: {seed(locked)}")


if __name__ == "__main__":
    main()
