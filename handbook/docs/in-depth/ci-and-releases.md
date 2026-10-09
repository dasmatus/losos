---
title: CI and releases
sidebar_position: 11
mdx:
  format: md
---

# CI and releases

CI is `.github/workflows/ci.yml`. It runs the same commands as the devenv
scripts, written out directly, because running jobs through `devenv shell`
was too slow. Keep the two in step by hand.

`.github/actions/setup-nix` installs Nix and writes the substituters to
`/etc/nix/nix.conf` before the daemon starts. Nix ignores `NIX_CONFIG` from
untrusted clients, and a build that misses the cache compiles from source
without saying so.

## Jobs

- clippy and rustfmt (rustfmt checked on pull requests only)
- Rust tests for both crates
- flake evaluation
- package builds
- the installer ISO, built on every push and then booted under OVMF and
  SeaBIOS by `tests/iso-boot.py`. A boot passes when the installer sends a
  DNS query for `github.com`. On timeout, the job uploads a screenshot.
- the Nextcloud image, weekly and on request only
- `losos-registrar` for the dev machine, a static musl build pushed to GHCR
  on every push to main and every tag and served by the proxy (below)

## Releases

Set `flake/version.nix` to the new tag on main first, then push a `v*`
tag, or draft a release with a new `v*` tag in the web UI. The boot menus,
the boot pictures, os-release and the console banners show that file's
value, and an installed box builds from the committed tree. The release
job:

1. writes the tag into `flake/version.nix` (with a warning if the committed
   value was another one), then builds the installer ISO and the demo QCOW2,
2. signs the ISO's UEFI loader in place with the `SECURE_BOOT_DB_KEY`
   secret (`losos-sign-iso`) and signs `SHA256SUMS` with the same key. If
   the secret is missing or signing fails, the release still publishes:
   the ISO is the unsigned build, the run carries a warning, and the
   release notes say *Not signed* and why,
3. attaches the ISO, its `.sha256`, `SHA256SUMS`, `SHA256SUMS.sig` and the
   certificate (`.pem` and `.cer`) to the GitHub release,
4. pushes both media to GHCR, since the QCOW2 is over GitHub's 2 GiB asset
   limit.

The `iso` job on main does the same signing before its boot legs. OVMF with
Microsoft's keys must refuse the medium, and the same OVMF store with the
committed certificate added beside Microsoft's must boot it (`tests/iso-boot.py --firmware
uefi-sb-ms|uefi-sb`). A fork's pull request has no key, so it boots unsigned
and skips the enrolled leg, and so does a run where signing failed. The key is made once on the owner's machine with
`provisioning/secure-boot/keygen.sh`.

Your release notes are kept. The download section goes between two marker
comments, and a re-run replaces only that section.

### Media for a tag that has none

A release whose tag run failed can get its media later. Open Actions, pick
the `ci` workflow, choose *Run workflow* on `main`, and fill in
`release_tags`: either `missing`, for every published release without an
installer ISO, or the tags themselves, separated by spaces (`v0.1.7
v0.1.8`). Each tag is built from its own tree, the same way a tag push
builds it, two at a time, and its media are attached to the existing
release. The release's title and your notes stay as they are. Tags from
before the edge gateway image (v0.1.6 and older) cannot build the media
and are skipped with a notice.

## Binary cache

CI publishes signed Nix store paths as OCI artifacts at
`ghcr.io/dasmatus/losos/nix-cache`. A separate deployment of the
[LosOS Desktop proxy](https://github.com/dasmatus/losos-desktop/tree/main/proxy)
serves them, with `GHCR_REPOSITORY=dasmatus/losos`.

Setup:

1. `nix key generate-secret --key-name losos-1`
2. Store the secret key as repository secret `NIX_CACHE_SIGNING_KEY`.
3. Store the public key (`nix key convert-secret-to-public`) as repository
   variable `NIX_CACHE_PUBLIC_KEY`.
4. Store the proxy's HTTPS URL as repository variable `LOSOS_PROXY_URL`.
   That is the domain attached to the `losos-cache-proxy` Vercel project,
   `https://proxy.losos.dasmat.us`, and nothing else. The earlier name,
   `losos-proxy.dasmat.us`, now 307-redirects there. A hostname that merely
   resolves to Vercel answers `DEPLOYMENT_NOT_FOUND`; nix then prints
   `warning: '<url>' does not appear to be a binary cache` in every job,
   builds from source, and the run stays green. `setup-nix` now checks
   `<url>/nix-cache-info` and annotates the run when that happens.
5. Make the `losos/nix-cache` package public, and `losos/images` (below)
   once the first push to main has created it. GitHub creates a package
   private, and the proxy then answers `502 token: 403` for everything in it.
6. Keep `losos.cache.substituters` and `losos.cache.trustedPublicKeys` in
   `modules/options.nix` at the same URL and key. Their defaults are
   `https://proxy.losos.dasmat.us` and the `losos-1` public key, so a stock
   appliance and installer medium already pull from the cache, and
   `tests/invariants.nix` fails if either default is dropped. Rotating the
   signing key means changing the variable and the default together.

Every job reports what it got from the cache. `setup-nix` puts a `nix` shim
on `PATH` that counts nix's own `copying path '…' from '<url>'` and
`building '…'` lines per invocation. It writes a **Nix binary cache use**
table to the job's step summary, one row per nix command that copied or
built anything, with paths from the LosOS cache, from `cache.nixos.org`,
from elsewhere, and built on the runner. It also prints one `LosOS cache: …`
line in the job log right after the command. A row with the LosOS column at
0 and a non-zero "built here" means this project's own paths were compiled
although an earlier run should have pushed them. A job that only evaluates
adds no row. The shim changes neither nix's output nor its exit status.

Check `<proxy-url>/nix-cache-info` and `nix copy --from <proxy-url>
<store-path>` before relying on it. An unreachable cache is not fatal. The
appliance waits 5 s and falls back to `cache.nixos.org` and building. Never
put the secret key in the flake or on an appliance.

### The copy on GitHub Pages

The GitHub Pages site that serves the handbook also serves a copy of the
cache at `https://losos.dasmat.us/proxy`, for when the proxy on Vercel does
not answer. Pages runs no code, so the copy is a plain file cache. On each
push to main, the `publish-cache` job writes the paths it publishes with
`nix copy --to file://`, signs them with the same `losos-1` key and leaves
out every path cache.nixos.org already serves. The `handbook` workflow runs
again when `ci` finishes on main and deploys the newest copy under `/proxy`.

It has three limits:

- It holds the newest build of main only. A box on an older revision finds
  its paths on the proxy alone.
- A Pages site may be at most 1 GB, so the copy keeps under 800 MiB of NARs
  and drops the largest paths first. The job's step summary names what it
  left out. The Nextcloud image, built weekly, is never in it.
- Its `nix-cache-info` says `Priority: 45`, after the proxy (30) and
  cache.nixos.org (40), so nix tries it after both.

`losos.cache.substituters` lists both by default. The repository's Pages
source must be "GitHub Actions" (Settings, Pages), as for the handbook.

## The dev-machine tool

The key ceremony ([Master proxy, Official edges](master-proxy.md#official-edges))
runs `losos-registrar provision` on the operator's own computer, which has no
Nix store. `.#losos-registrar-static` is the same crate linked statically
against musl. The `registrar-and-ui` job builds it and the `publish-tool` job
pushes it to GHCR as the OCI artifact
`ghcr.io/dasmatus/losos/images:<channel>-x86_64`, one layer per file,
`losos-registrar` and `SHA256SUMS`. That is the shape the proxy's
`/updates/<channel>/<arch>/<file>` route serves. It picks the layer whose
title is the file name and redirects to GHCR's storage, except `SHA256SUMS`,
which it serves inline. LosOS Desktop's sysupdate reads the same route. The
channel is `main` for a push to main, the tag for a release (which also moves
`stable`), and a cleaned-up branch name for a manual run elsewhere. Pull
requests never publish.

```sh
curl -fsSLO "https://proxy.losos.dasmat.us/updates/main/x86_64/{losos-registrar,SHA256SUMS}" && sha256sum -c --ignore-missing SHA256SUMS && chmod +x losos-registrar
```

When the proxy does not answer, the same two files of the newest main build
are at `https://losos.dasmat.us/proxy/updates/main/x86_64/`.

The job's step summary carries the artifact reference, the URL and the sum.
`oras pull ghcr.io/dasmatus/losos/images:main-x86_64` fetches the same files
without the proxy.
