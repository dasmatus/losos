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

Push a `v*` tag, or draft a release with a new `v*` tag in the web UI. The
release job:

1. builds the installer ISO and the demo QCOW2,
2. signs the ISO's UEFI loader in place with the `SECURE_BOOT_DB_KEY`
   secret (`losos-sign-iso`, which refuses to publish unsigned once
   `keys/secure-boot-db.pem` carries a certificate) and signs `SHA256SUMS`
   with the same key,
3. attaches the ISO, its `.sha256`, `SHA256SUMS`, `SHA256SUMS.sig` and the
   certificate (`.pem` and `.cer`) to the GitHub release,
4. pushes both media to GHCR, since the QCOW2 is over GitHub's 2 GiB asset
   limit.

The `iso` job on main does the same signing before its boot legs. OVMF with
Microsoft's keys must refuse the medium, and OVMF with the committed
certificate enrolled must boot it (`tests/iso-boot.py --firmware
uefi-sb-ms|uefi-sb`). A fork's pull request has no key, so it boots unsigned
and skips the enrolled leg. The key is made once on the owner's machine with
`provisioning/secure-boot/keygen.sh`.

Your release notes are kept. The download section goes between two marker
comments, and a re-run replaces only that section.

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

## The dev-machine tool

The key ceremony ([Master proxy, Official edges](Master-Proxy#official-edges))
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

The job's step summary carries the artifact reference, the URL and the sum.
`oras pull ghcr.io/dasmatus/losos/images:main-x86_64` fetches the same files
without the proxy.
