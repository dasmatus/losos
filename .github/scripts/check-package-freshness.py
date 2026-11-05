#!/usr/bin/env python3
"""Check upstream release freshness against the nixpkgs pinned in flake.lock.

This is the "actively checks" half of keeping a NixOS appliance current: the
lockfile-update workflow moves the nixpkgs pin on a schedule, but nixos-unstable
itself can lag behind upstream on packages where a delay matters. This script
compares the version nixpkgs provides for a curated set of system packages with
the latest stable upstream release and reports anything that is behind.

It is intentionally a monitor, not an auto-bumper: a lagging nixpkgs version is
usually fixed by taking a newer nixpkgs (lockfile-updates), but sometimes by an
explicit override while nixpkgs catches up. The report is the input to that
decision.

    check-package-freshness.py            print a markdown report to stdout
    check-package-freshness.py --json     machine-readable report
    check-package-freshness.py --strict   exit 1 if any tracked package lags

Exit status: 0 on success (or no lag without --strict), 1 when --strict finds
lag, 2 on a usage/fetch/evaluation failure.
"""

import argparse
import json
import os
import re
import subprocess
import sys
import urllib.error
import urllib.parse
import urllib.request
from dataclasses import asdict, dataclass
from pathlib import Path
from typing import Callable

UA = "losos-package-freshness/1 (+https://github.com/)"

# nixpkgs attribute -> (human name, upstream checker). The checker returns the
# latest stable upstream version string, or None when it cannot be determined
# (reported as "unknown", never treated as lag). Keep this list to packages
# where a quicker update cycle has clear security or stability value on an
# appliance: the kernel, the init system, core crypto/TLS, and core system
# plumbing. Broader stacks (desktop, language runtimes) belong in their own
# review, not in this daily signal.
PACKAGES: list[tuple[str, str, str]] = [
    ("linuxPackages_latest.kernel", "Linux kernel", "kernel_org"),
    ("systemd", "systemd", "github_release:systemd/systemd"),
    ("openssl", "OpenSSL", "github_tag:openssl/openssl"),
    ("curl", "curl", "github_release:curl/curl"),
    ("openssh", "OpenSSH", "github_tag:openssh/openssh-portable"),
    ("glibc", "glibc", "sourceware_glibc"),
    ("dbus", "D-Bus", "gitlab_tag:dbus/dbus"),
    ("util-linux", "util-linux", "github_release:util-linux/util-linux"),
    ("cryptsetup", "cryptsetup", "gitlab_tag:cryptsetup/cryptsetup"),
    ("kmod", "kmod", "kernel_org_kmod"),
    ("linux-firmware", "linux-firmware", "kernel_org_firmware"),
]


@dataclass
class Row:
    attr: str
    name: str
    nixpkgs: str | None
    upstream: str | None
    status: str  # up-to-date | behind | ahead | unknown
    note: str = ""


class Failure(Exception):
    pass


def http_json(url: str) -> object:
    req = urllib.request.Request(url, headers={"User-Agent": UA, "Accept": "application/json"})
    token = os.environ.get("GITHUB_TOKEN") or os.environ.get("GH_TOKEN")
    host = urllib.parse.urlsplit(url).hostname
    if token and host == "api.github.com":
        req.add_header("Authorization", f"Bearer {token}")
    try:
        with urllib.request.urlopen(req, timeout=30) as r:
            return json.load(r)
    except (urllib.error.URLError, TimeoutError, json.JSONDecodeError) as e:
        raise Failure(f"fetch {url}: {e}") from e


def norm(ver: str) -> str:
    ver = ver.strip()
    ver = re.sub(r"^(release-|v\.?|V_)", "", ver)
    ver = ver.replace("_", ".")
    # openssh-portable tags like V_10_2_P1
    ver = re.sub(r"[.-]?P(\d+)$", r"p\1", ver, flags=re.IGNORECASE)
    return ver


def version_key(ver: str) -> tuple:
    # A tolerant key: split into integer/text runs so "6.19.1" and "3.0.14"
    # compare sensibly without a packaging dependency.
    s = norm(ver)
    s = re.sub(r"([a-zA-Z]+)", r".\1.", s)
    parts: list[object] = []
    for part in s.split("."):
        if not part:
            continue
        if part.isdigit():
            parts.append((0, int(part)))
        else:
            m = re.match(r"^([a-zA-Z]+)(\d*)$", part)
            if m:
                tag = m.group(1).lower()
                # stable releases sort after pre-release markers when mixed
                order = {"alpha": -3, "beta": -2, "rc": -1, "p": 1, "patch": 1}.get(tag, 0)
                parts.append((1, order, tag, int(m.group(2) or 0)))
            else:
                parts.append((2, part.lower()))
    return tuple(parts)


def compare(nixpkgs: str, upstream: str) -> str:
    try:
        a, b = version_key(nixpkgs), version_key(upstream)
    except Exception:
        return "unknown"
    if a == b:
        return "up-to-date"
    # Treat as up-to-date when nixpkgs carries the same upstream base with a
    # distro suffix we failed to normalize into equality.
    if norm(nixpkgs).startswith(norm(upstream)) or norm(upstream).startswith(norm(nixpkgs)):
        return "up-to-date"
    return "behind" if a < b else "ahead"


def upstream_kernel_org() -> str | None:
    data = http_json("https://www.kernel.org/releases.json")
    for rel in data.get("releases", []):
        if rel.get("moniker") == "stable":
            return rel.get("version")
    return None


def upstream_github_release(repo: str) -> str | None:
    data = http_json(f"https://api.github.com/repos/{repo}/releases/latest")
    return norm(data.get("tag_name") or data.get("name") or "")


def upstream_github_tag(repo: str) -> str | None:
    data = http_json(f"https://api.github.com/repos/{repo}/tags?per_page=20")
    versions: list[str] = []
    for tag in data:
        name = norm(tag.get("name", ""))
        # Skip obvious pre-releases/drafts by marker in the normalized name.
        if re.search(r"(alpha|beta|rc\d)", name, re.IGNORECASE):
            continue
        if re.search(r"\d", name):
            versions.append(name)
    if not versions:
        return None
    versions.sort(key=version_key)
    return versions[-1]


def upstream_gitlab_tag(project: str) -> str | None:
    enc = project.replace("/", "%2F")
    data = http_json(f"https://gitlab.freedesktop.org/api/v4/projects/{enc}/repository/tags?per_page=20")
    versions = []
    for tag in data:
        name = norm(tag.get("name", ""))
        if re.search(r"(alpha|beta|rc\d)", name, re.IGNORECASE):
            continue
        if re.search(r"\d", name):
            versions.append(name)
    if not versions:
        return None
    versions.sort(key=version_key)
    return versions[-1]


def upstream_sourceware_glibc() -> str | None:
    # The glibc git tags are authoritative; the read-only cgit atom/feed is easy
    # to parse without HTML scraping. Fall back to the refs page text.
    data = http_json("https://sourceware.org/git/?p=glibc.git;a=tags")
    # Not JSON; handled below by text path.
    return None


def upstream_sourceware_glibc_text() -> str | None:
    req = urllib.request.Request(
        "https://sourceware.org/git/?p=glibc.git;a=tags",
        headers={"User-Agent": UA},
    )
    try:
        with urllib.request.urlopen(req, timeout=30) as r:
            text = r.read().decode("utf-8", "replace")
    except (urllib.error.URLError, TimeoutError) as e:
        raise Failure(f"fetch glibc tags: {e}") from e
    versions = re.findall(r">glibc-(\d+\.\d+(?:\.\d+)*)<", text)
    if not versions:
        return None
    versions = [v for v in versions if not re.search(r"(alpha|beta|rc)", v, re.IGNORECASE)]
    versions.sort(key=version_key)
    return versions[-1]


def upstream_kernel_org_kmod() -> str | None:
    # kmod releases are published under kernel.org /pub/linux/utils/kernel/kmod/;
    # the directory listing is plain text/HTML we can scan for tarballs.
    req = urllib.request.Request(
        "https://kernel.org/pub/linux/utils/kernel/kmod/",
        headers={"User-Agent": UA},
    )
    try:
        with urllib.request.urlopen(req, timeout=30) as r:
            text = r.read().decode("utf-8", "replace")
    except (urllib.error.URLError, TimeoutError) as e:
        raise Failure(f"fetch kmod releases: {e}") from e
    versions = re.findall(r"kmod-(\d+(?:\.\d+)+)\.tar", text)
    versions.sort(key=version_key)
    return versions[-1] if versions else None


def upstream_kernel_org_firmware() -> str | None:
    req = urllib.request.Request(
        "https://kernel.org/pub/linux/kernel/firmware/",
        headers={"User-Agent": UA},
    )
    try:
        with urllib.request.urlopen(req, timeout=30) as r:
            text = r.read().decode("utf-8", "replace")
    except (urllib.error.URLError, TimeoutError) as e:
        raise Failure(f"fetch linux-firmware releases: {e}") from e
    # linux-firmware is date-tagged (YYYYMMDD).
    versions = re.findall(r"linux-firmware-(\d{8})\.tar", text)
    return max(versions) if versions else None


CHECKERS: dict[str, Callable[[], str | None]] = {
    "kernel_org": upstream_kernel_org,
    "sourceware_glibc": upstream_sourceware_glibc_text,
    "kernel_org_kmod": upstream_kernel_org_kmod,
    "kernel_org_firmware": upstream_kernel_org_firmware,
}


def resolve_upstream(spec: str) -> str | None:
    if spec in CHECKERS:
        return CHECKERS[spec]()
    if spec.startswith("github_release:"):
        return upstream_github_release(spec.split(":", 1)[1])
    if spec.startswith("github_tag:"):
        return upstream_github_tag(spec.split(":", 1)[1])
    if spec.startswith("gitlab_tag:"):
        return upstream_gitlab_tag(spec.split(":", 1)[1])
    raise Failure(f"unknown upstream checker: {spec}")


def nixpkgs_rev(repo: Path) -> str:
    lock = json.loads((repo / "flake.lock").read_text())
    return lock["nodes"]["nixpkgs"]["locked"]["rev"]


def nixpkgs_versions(rev: str, attrs: list[str]) -> dict[str, str | None]:
    # Resolve every requested version in one evaluation so nixpkgs is fetched
    # once, not once per package.
    def nix_get(a: str) -> str:
        # Kernel packages expose the version on .kernel; most others on .version.
        # Quote attr components so names like `linux-firmware` are not parsed as
        # subtraction by Nix.
        path = ".".join(f'"{part}"' for part in a.split("."))
        return (
            f'"{a}" = '
            f'(let p = pkgs.{path}; in p.version or p.kernel.version or "");'
        )

    expr = (
        'let flake = builtins.getFlake "github:NixOS/nixpkgs/' + rev + '"; '
        "pkgs = flake.legacyPackages.x86_64-linux; in { "
        + " ".join(nix_get(a) for a in attrs)
        + " }"
    )
    p = subprocess.run(
        [
            "nix",
            "eval",
            "--json",
            "--extra-experimental-features",
            "nix-command flakes",
            "--expr",
            expr,
        ],
        capture_output=True,
        text=True,
    )
    if p.returncode != 0:
        raise Failure(f"nix eval failed: {p.stderr[-2000:].strip()}")
    try:
        data = json.loads(p.stdout)
    except json.JSONDecodeError as e:
        raise Failure(f"nix eval returned non-JSON: {e}") from e
    return {a: (data.get(a) or None) for a in attrs}


def render_markdown(rows: list[Row], rev: str) -> str:
    behind = [r for r in rows if r.status == "behind"]
    unknown = [r for r in rows if r.status == "unknown"]
    lines = [
        "## System package freshness vs nixos-unstable",
        "",
        f"Compared the nixpkgs pin in `flake.lock` (`{rev[:12]}`) with current "
        "upstream stable releases for packages where a lag matters on this appliance.",
        "",
        f"**Behind upstream:** {len(behind)}  |  **Unknown/unresolved:** {len(unknown)}  |  **Checked:** {len(rows)}",
        "",
        "| Package | nixpkgs | upstream (stable) | status |",
        "|---|---:|---:|---|",
    ]
    icons = {"up-to-date": "ok", "behind": "**behind**", "ahead": "ahead of check", "unknown": "unknown"}
    for r in rows:
        lines.append(
            f"| `{r.name}` (`{r.attr}`) | `{r.nixpkgs or '?'}` | `{r.upstream or '?'}` | {icons.get(r.status, r.status)}{(' — ' + r.note) if r.note else ''} |"
        )
    lines += [
        "",
        "A `behind` result means nixos-unstable has not packaged the newest stable "
        "release yet. The usual fix is a fresher nixpkgs pin (the weekly lockfile "
        "update); when an update cannot wait, carry a targeted override and drop it "
        "once nixpkgs catches up.",
        "",
        "_Generated by `.github/workflows/package-freshness.yml` / `.github/scripts/check-package-freshness.py`._",
    ]
    return "\n".join(lines) + "\n"


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--repo", default=".", help="repository root (default: .)")
    ap.add_argument("--json", action="store_true", help="emit JSON instead of markdown")
    ap.add_argument("--strict", action="store_true", help="exit 1 if any package is behind upstream")
    args = ap.parse_args()
    repo = Path(args.repo).resolve()

    try:
        rev = nixpkgs_rev(repo)
    except Exception as e:
        print(f"package-freshness: cannot read nixpkgs rev: {e}", file=sys.stderr)
        return 2

    attrs = [a for a, _, _ in PACKAGES]
    nix_note = ""
    try:
        nix_versions = nixpkgs_versions(rev, attrs)
    except Exception as e:
        nix_versions = {a: None for a in attrs}
        nix_note = f"nix eval failed: {e}"

    rows: list[Row] = []
    for attr, name, spec in PACKAGES:
        note = nix_note
        nix_ver = nix_versions.get(attr)
        try:
            up_ver = resolve_upstream(spec)
        except Failure as e:
            up_ver = None
            note = (note + "; " if note else "") + str(e)
        if nix_ver and up_ver:
            status = compare(nix_ver, up_ver)
        else:
            status = "unknown"
            if not nix_ver and not nix_note:
                note = (note + "; " if note else "") + f"no version from nixpkgs attr {attr}"
        rows.append(Row(attr=attr, name=name, nixpkgs=nix_ver, upstream=up_ver, status=status, note=note))
        print(f"{name}: nixpkgs={nix_ver} upstream={up_ver} -> {status}{(' ('+note+')') if note else ''}", file=sys.stderr)

    payload = {"nixpkgs_rev": rev, "rows": [asdict(r) for r in rows]}
    if args.json:
        print(json.dumps(payload, indent=2))
    else:
        print(render_markdown(rows, rev))

    if args.strict and any(r.status == "behind" for r in rows):
        return 1
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except Failure as e:
        print(f"package-freshness: {e}", file=sys.stderr)
        raise SystemExit(2)
