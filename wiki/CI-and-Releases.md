# CI and releases

CI is `.github/workflows/ci.yml`. It runs the same commands as the devenv
scripts, written out directly, because running jobs through `devenv shell`
was too slow. Keep the two in step by hand.

Nix is installed by `.github/actions/setup-nix`, which writes substituters to
`/etc/nix/nix.conf` before the daemon starts. (`NIX_CONFIG` is ignored for
untrusted clients, and the build silently compiles from source.)

## Jobs

- clippy and rustfmt (rustfmt checked on pull requests only)
- Rust tests for both crates
- flake evaluation
- package builds
- installer ISO: built on every push, then booted under OVMF and SeaBIOS by
  `tests/iso-boot.py`. A boot passes when the installer sends a DNS query for
  `github.com`. On timeout, the job uploads a screenshot.
- the Nextcloud image: weekly and on request only

## Releases

Push a `v*` tag, or draft a release with a new `v*` tag in the web UI. The
release job:

1. builds the installer ISO and the demo QCOW2,
2. attaches the ISO and its `.sha256` to the GitHub release,
3. pushes both to GHCR (the QCOW2 is over GitHub's 2 GiB asset limit).

Your release notes are kept. The download section goes between two marker
comments, and a re-run replaces only that section.

## Binary cache

CI publishes signed Nix store paths as OCI artifacts at
`ghcr.io/dasmatus/losos/nix-cache`. They are served by a separate deployment
of the [LosOS Desktop proxy](https://github.com/dasmatus/losos-desktop/tree/main/proxy)
with `GHCR_REPOSITORY=dasmatus/losos`.

Setup:

1. `nix key generate-secret --key-name losos-1`
2. Store the secret key as repository secret `NIX_CACHE_SIGNING_KEY`.
3. Store the public key (`nix key convert-secret-to-public`) as repository
   variable `NIX_CACHE_PUBLIC_KEY`.
4. Store the proxy's HTTPS URL as repository variable `LOSOS_PROXY_URL`.
5. Make the `losos/nix-cache` package public.
6. Keep `losos.cache.substituters` and `losos.cache.trustedPublicKeys` in
   `modules/options.nix` at the same URL and key. Their defaults are
   `https://losos-proxy.dasmat.us` and the `losos-1` public key, so a stock
   appliance and installer medium already pull from the cache;
   `tests/invariants.nix` fails if either default is dropped. Rotating the
   signing key means changing the variable and the default together.

Check `<proxy-url>/nix-cache-info` and `nix copy --from <proxy-url>
<store-path>` before relying on it. An unreachable cache is not fatal: the
appliance waits 5 s and falls back to `cache.nixos.org` and building. Never
put the secret key in the flake or on an appliance.
