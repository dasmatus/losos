#!/usr/bin/env python3
"""Recompute the Nix hashes that pin the dependency lock files.

Each lock file below is vendored by a fixed-output derivation whose hash is
written in a .nix file (`cargoHash`, `pluginsCargoHash`, and the `hash` of each
Bun workspace's node_modules). When a lock file changes, through a Dependabot
bump, `cargo update` or `bun install`,
that hash goes stale: the package fails to build with the right hash in its
error, or a cache that still holds the old vendor directory hands it back and
the build fails later, offline, on a missing crate.

This script builds each affected vendor derivation with a placeholder hash, so
nothing can be substituted for it, reads the real hash out of Nix's mismatch
error and writes it over the old value in every tracked .nix file that carries
it. Two packages that vendor the same lock file (losos-registrar and its static
build) share one value and move together.

    fix-hashes.py                 entries whose lock files differ from origin/main
    fix-hashes.py --base REF      ... from REF (merge base), plus uncommitted edits
    fix-hashes.py --all           every entry
    fix-hashes.py --check         change nothing; exit 1 when a hash is stale

Exit status: 0 when every hash checked is current or was rewritten, 1 when
--check found a stale one, 2 on any other failure. The names of the entries it
changed go to stdout, one per line.
"""

import argparse
import re
import subprocess
import sys
from pathlib import Path

SYSTEM = "x86_64-linux"

# (name, the vendor derivation, derivations that must carry the same hash,
#  the lock files it vendors)
CARGO = "cargoDeps.vendorStaging"
# nix/bun-deps.nix, a passthru of each Bun-built package.
BUN = "nodeModules"
ENTRIES = [
    ("losos-ctl", f"packages.{SYSTEM}.losos-ctl.{CARGO}", [], ["backend/Cargo.lock"]),
    (
        "losos-registrar",
        f"packages.{SYSTEM}.losos-registrar.{CARGO}",
        [f"packages.{SYSTEM}.losos-registrar-static.{CARGO}"],
        ["edge/registrar/Cargo.lock"],
    ),
    (
        "losos",
        f"packages.{SYSTEM}.losos.{CARGO}",
        [f"packages.{SYSTEM}.losos-static.{CARGO}"],
        ["admin-ui/tui/Cargo.lock"],
    ),
    (
        "losos-plugins",
        f"packages.{SYSTEM}.losos-plugins.{CARGO}",
        [f"packages.{SYSTEM}.losos-ci-pm-plugin.{CARGO}"],
        ["backend/plugins/Cargo.lock"],
    ),
    ("losos-lab-core", f"packages.{SYSTEM}.losos-lab-core.{CARGO}", [], ["admin-ui/lab/core/Cargo.lock"]),
    ("losos-lab-virt", f"packages.{SYSTEM}.losos-lab-virt.{CARGO}", [], ["admin-ui/lab/virt-rpc/Cargo.lock"]),
    ("losos-lab-render", f"packages.{SYSTEM}.losos-lab-render.{CARGO}", [], ["admin-ui/lab/render/Cargo.lock"]),
    ("losos-mdx-wasm", f"packages.{SYSTEM}.losos-mdx-wasm.{CARGO}", [], ["backend/mdx/Cargo.lock"]),
    ("losos-admin-ui", f"packages.{SYSTEM}.losos-admin-ui.{BUN}", [], ["admin-ui/app/bun.lock"]),
    (
        "losos-handbook",
        f"packages.{SYSTEM}.losos-handbook.{BUN}",
        [],
        ["handbook/bun.lock", "handbook/vendor/braces/package.json"],
    ),
    ("losos-ds-render", f"checks.{SYSTEM}.losos-ds-render.{BUN}", [], ["admin-ui/design-system/react/bun.lock"]),
]

# The fetchers run Python requests and curl inside the sandbox. Where outgoing
# TLS is re-signed by a proxy (a development container), the CA only reaches
# them through these variables, and Nix passes an impure-env value only to a
# derivation that names it. On a plain runner they are unset and do nothing.
IMPURE_ENV = ["REQUESTS_CA_BUNDLE", "CURL_CA_BUNDLE", "SSL_CERT_FILE", "GIT_SSL_CAINFO", "NIX_SSL_CERT_FILE"]

GOT = re.compile(r"got:\s+(sha256-[A-Za-z0-9+/]+=*)")


class Failure(Exception):
    pass


def run(args, repo, check=True):
    p = subprocess.run(args, cwd=repo, capture_output=True, text=True)
    if check and p.returncode != 0:
        raise Failure(f"{' '.join(args[:3])} … failed:\n{p.stderr[-4000:]}")
    return p


def flake_ref(repo):
    # git+file: only tracked files, so build output and node_modules in the
    # checkout are not copied into the store with the source.
    return f"git+file://{repo}"


def current_hash(repo, attr):
    out = run(["nix", "eval", "--raw", f"{flake_ref(repo)}#{attr}.outputHash"], repo).stdout.strip()
    if not out.startswith("sha256-"):
        out = run(["nix", "hash", "convert", "--hash-algo", "sha256", "--to", "sri", out], repo).stdout.strip()
    return out


def real_hash(repo, attr):
    env_list = " ".join(f'"{v}"' for v in IMPURE_ENV)
    expr = f"""
      let
        flake = builtins.getFlake "{flake_ref(repo)}";
        fod = flake.{attr};
      in fod.overrideAttrs (o: {{
        outputHash = flake.inputs.nixpkgs.lib.fakeHash;
        outputHashAlgo = null;
        impureEnvVars = (o.impureEnvVars or [ ]) ++ [ {env_list} ];
      }})
    """
    p = run(["nix", "build", "--no-link", "--impure", "--expr", expr], repo, check=False)
    got = GOT.findall(p.stderr)
    if p.returncode == 0 or not got:
        raise Failure(f"{attr}: no hash from Nix (exit {p.returncode}):\n{p.stderr[-4000:]}")
    return got[-1]


def changed_files(repo, base):
    merge_base = run(["git", "merge-base", base, "HEAD"], repo).stdout.strip()
    files = run(["git", "diff", "--name-only", merge_base], repo).stdout.split()
    files += run(["git", "ls-files", "--others", "--exclude-standard"], repo).stdout.split()
    return set(files)


def replace_everywhere(repo, old, new):
    files = run(["git", "grep", "-lF", old, "--", "*.nix"], repo, check=False).stdout.split()
    if not files:
        raise Failure(f"{old} is in no tracked .nix file; write the hash as an SRI string")
    for f in files:
        path = repo / f
        path.write_text(path.read_text().replace(old, new))
    return files


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--repo", default=".", help="checkout to work in (default: .)")
    ap.add_argument("--base", default="origin/main", help="ref the lock files are compared with")
    ap.add_argument("--all", action="store_true", help="check every entry, changed or not")
    ap.add_argument("--check", action="store_true", help="report stale hashes, change nothing")
    args = ap.parse_args()
    repo = Path(args.repo).resolve()

    if args.all:
        entries = ENTRIES
    else:
        changed = changed_files(repo, args.base)
        entries = [e for e in ENTRIES if changed.intersection(e[3])]
    if not entries:
        print("no lock file with a pinned hash changed", file=sys.stderr)
        return 0

    rewrites = {}  # old value -> (new value, entry name)
    stale = []
    for name, attr, siblings, _ in entries:
        print(f"{name}: vendoring {attr}", file=sys.stderr)
        new = real_hash(repo, attr)
        olds = {current_hash(repo, a) for a in [attr, *siblings]} - {new}
        for old in olds:
            if old in rewrites and rewrites[old][0] != new:
                raise Failure(
                    f"{old} stands for two different lock files ({rewrites[old][1]} and {name}); "
                    "give each package its own hash"
                )
            rewrites[old] = (new, name)
        if olds:
            stale.append(name)
            print(f"{name}: stale, the lock file now vendors to {new}", file=sys.stderr)
        else:
            print(f"{name}: current", file=sys.stderr)

    if args.check:
        for name in stale:
            print(name)
        return 1 if stale else 0

    for old, (new, name) in rewrites.items():
        files = replace_everywhere(repo, old, new)
        print(f"{name}: {old} -> {new} in {', '.join(files)}", file=sys.stderr)
    for name in stale:
        print(name)
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except Failure as e:
        print(f"fix-hashes: {e}", file=sys.stderr)
        sys.exit(2)
