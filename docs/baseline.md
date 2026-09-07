# Baseline — 2026-09-07

The state of every gate **before** the hardening pass changed anything, taken
on commit `31dbd1e` (the tip of `main`). Recorded because nothing in this
repository had ever been observed to build or fail from a developer machine:
the `nix` command on the maintainer's host is a shell alias to a toolbox
wrapper whose first action is `mkdir -p "$NIX_TOOLBOX_STORE"`, and that
variable is unset, so every `nix` invocation died with `mkdir: cannot create
directory ''` before doing anything.

Measured with the host Nix directly (2.35.2, single-user store, 24,886 paths)
on 16 threads with a writable `/dev/kvm`.

## Results

| Gate | Result | Notes |
|---|---|---|
| `nix flake check --no-build -L` | **pass** | all outputs evaluate |
| `nix build .#losos-ctl` | **pass** | 57 tests via `doCheck` |
| `nix build .#losos-registrar` | **pass** | 9 tests via `doCheck` |
| `nix build .#losos-admin-ui` | **pass** | |
| `cargo test` (backend) | **pass** | 57 passed |
| `cargo clippy -D warnings` (backend) | **pass** | |
| `cargo fmt --check` (backend) | **pass** | |
| `cargo test` (backend-registrar) | **pass** | 9 passed |
| `cargo clippy -D warnings` (backend-registrar) | **FAIL** | `needless_return` at `src/opts.rs:154` |
| `cargo fmt --check` (backend-registrar) | **FAIL** | 17 diff hunks |
| `nixosConfigurations.install` toplevel | **FAIL** | see below |
| VM `losos-admin-daemon` | **pass** | 24.7 s; D-Bus + HTTP + token |
| VM `losos-install` | **pass** | full disko/LVM/LUKS format + mount |
| VM `losos-ds-render` | **FAIL** | npm registry fetch, see below |
| VM `losos-edge-proxy` | **FAIL** | not isolated, see below |
| installer ISO | not run | |

Across the whole run there were exactly **two root builder failures**
(everything else is `Reason: 1 dependency failed` cascading from them):
`tahoe-lafs` and `losos-ds-render-0.1.0-npm-deps`.

## The two real failures

### The appliance does not build

`nix build .#nixosConfigurations.install.config.system.build.toplevel` fails.
The cause is `tahoe-lafs`, which fails inside **its own upstream test suite**:

```
AttributeError: 'Server' object has no attribute 'failUnlessRaises'
Ran 1708 tests in 363.883s
FAILED (skips=2, failures=1, errors=11, successes=1694)
```

`failUnlessRaises` is a deprecated Twisted `TestCase` alias that twisted
26.4.0 removed; tahoe-lafs 1.20.0-unstable still calls it. The single
non-error failure is `test_encodingutil.Windows.test_listdir_unicode` — a
Windows-only test running on Linux. All 12 failures are test-harness
artifacts; nothing functional is broken.

The failure cascades: `tahoe-lafs` → `system-path` →
`unit-tahoe.introducer-local.service` → `nixos-system-mattbox`. So the
product itself could not be built, and had not been able to for as long as
the pinned nixpkgs has carried twisted 26.4.0.

Nothing caught this because CI evaluates the flake and builds the three small
packages, and never builds a system closure. `nix flake check` only forces
each `nixosConfiguration`'s *toplevel derivation*, not the build.

### `losos-ds-render`: npm fetch

```
Error: couldn't fetch node_modules/@esbuild/aix-ppc64 at
https://registry.npmjs.org/@esbuild/aix-ppc64/-/aix-ppc64-0.25.12.tgz
    [56] Failure when receiving data from the peer
```

The fixed-output `npm-deps` derivation failed mid-download. This reads as a
transient network fault rather than a defect, but it has **not** been
confirmed by a clean re-run, so it is recorded as a failure rather than
waved away. Worth noting either way: this test's dependency set reaches
`registry.npmjs.org` at build time, so it is only as reliable as that
registry — which is a fair criticism of the check itself.

### `losos-edge-proxy`: not isolated

Every error in this section is `Reason: 1 dependency failed`; no builder
failure of its own. The test script never ran. Two candidate causes were not
separated, so no root cause is claimed here:

1. a cascade from the `npm-deps` failure above, within the same `nix build`
   invocation, and
2. contamination — this test ran last, by which point a concurrent agent had
   already removed the `/config` and `/tahoe` registrar endpoints that the
   old step 6 of `tests/edge-vm.nix` asserted on.

This needs a clean re-run on a quiet tree before anything is concluded.

### Lint had never run anywhere

`backend-registrar` had drifted to 17 rustfmt hunks and a clippy error while
CI stayed green. The three CI jobs only ran `nix build`, and
`buildRustPackage`'s `doCheck` runs the test suite, not the linters — so the
two commands `CLAUDE.md` documents as gates were enforced by nothing.
`backend` happened to stay clean; the other crate did not.

## How to reproduce

Inside the dev shell (`nix develop`, or `direnv allow`):

```sh
lint          # rustfmt --check + clippy -D warnings, both crates
test-rust     # both unit-test suites
check-flake   # nix flake check --no-build
check-eval    # forces the full module merge for both nixosConfigurations
build-pkgs    # all three flake packages
vm-tests      # the four nixos-test VMs (needs /dev/kvm)
build-iso     # installer ISO (local-only gate)
devenv test   # everything except the VM tests and the ISO
```
