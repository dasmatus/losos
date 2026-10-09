# Baseline — 2026-09-07

Gate results on commit `31dbd1e`, before the hardening pass. This is a
historical record: several things below have changed since (`doCheck` is now
off, tahoe-lafs is gone, linting runs in CI). Current commands are in the
[development notes](development.md).

Measured with Nix 2.35.2, single-user store, 16 threads, `/dev/kvm` available.

| Gate | Result | Notes |
|---|---|---|
| `nix flake check --no-build -L` | pass | |
| `nix build .#losos-ctl` | pass | 57 tests via `doCheck` |
| `nix build .#losos-registrar` | pass | 9 tests via `doCheck` |
| `nix build .#losos-admin-ui` | pass | |
| `cargo test` / `clippy` / `fmt` (backend) | pass | |
| `cargo test` (backend-registrar) | pass | 9 passed |
| `cargo clippy -D warnings` (backend-registrar) | **fail** | `needless_return` at `src/opts.rs:154` |
| `cargo fmt --check` (backend-registrar) | **fail** | 17 hunks |
| `nixosConfigurations.install` toplevel | **fail** | tahoe-lafs tests |
| VM `losos-admin-daemon` | pass | 24.7 s |
| VM `losos-install` | pass | |
| VM `losos-ds-render` | **fail** | npm fetch |
| VM `losos-edge-proxy` | **fail** | dependency failed |
| installer ISO | not run | |

There were two root build failures; everything else failed because a
dependency did.

## Failures

**The appliance did not build.** `tahoe-lafs` failed its own test suite
because twisted 26.4.0 removed `failUnlessRaises`. That broke `system-path`
and the whole system closure. CI did not notice because it never built a
system closure, and `nix flake check` only evaluates it.

**`losos-ds-render`** failed downloading
`@esbuild/aix-ppc64-0.25.12.tgz` from registry.npmjs.org. Probably transient,
but not confirmed by a re-run. The test depends on the npm registry at build
time.

**`losos-edge-proxy`** never ran its script; only "1 dependency failed".
Either a cascade from the npm failure or a concurrent change to the
registrar endpoints the test used. Not isolated.

**Lint had never run.** `backend-registrar` had drifted from rustfmt and
clippy while CI stayed green, because CI only ran `nix build`, and `doCheck`
runs tests, not linters.
