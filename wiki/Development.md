# Development

## Shell

```sh
devenv shell        # or: direnv allow
devenv test         # everything except VM tests, images and media
```

`nix develop` gives the toolchain only, for people without devenv.

| Script                         | Does |
| ------------------------------ | ---- |
| `fmt` · `lint` · `test-rust`   | Format, lint and test both Rust crates |
| `check-flake` · `check-eval`   | Evaluate the flake; force the full module merge |
| `check-pins`                   | Fail if `flake.lock` and `devenv.lock` pin different nixpkgs |
| `build-pkgs` · `build-iso`     | Build the packages; build the installer ISO |
| `build-images` · `build-media` | OCI images; demo QCOW2 and closure ISO (large, opt-in) |
| `vm-tests`                     | All NixOS VM tests (needs `/dev/kvm`) |

## Rust

```sh
cargo test   --manifest-path backend/Cargo.toml
cargo clippy --manifest-path backend/Cargo.toml --all-targets -- -D warnings
```

Same for `backend-registrar/`. Both crates must be clippy-clean and
rustfmt-clean. The Nix packages set `doCheck = false`, so `nix build` does
not run tests.

On pushes to `main`, a CI job runs `cargo fmt` and commits the result. Clippy
is not auto-fixed.

## Admin UI

From `admin-ui/app/`:

```sh
npm run dev           # set LOSOS_API_ORIGIN to a real box
npm run typecheck
npm run test:browser
```

## VM tests

CI cannot run these (no KVM on hosted runners). Run them locally before
merging changes to the modules, daemon or installer:

```sh
devenv shell vm-tests
nix build .#checks.x86_64-linux.losos-admin-daemon
nix build .#checks.x86_64-linux.losos-install
```

Also build the system closure locally:

```sh
nix build .#nixosConfigurations.install.config.system.build.toplevel
```

## Lock files and hashes

- `flake.lock` pins the nixpkgs that builds the appliance; `devenv.lock` the
  one that lints and tests it. Update them together.
- After changing `Cargo.lock` or `admin-ui/app/package-lock.json`, update
  `cargoHash`/`npmDepsHash` in `flake/packages.nix` (the admin UI hash is
  also in `tests/admin-ui.nix`).

## Wiki

This wiki is generated from `wiki/` in the repository. Edit pages there; the
`wiki` workflow publishes them on push to `main`. Edits made in the GitHub
wiki editor are overwritten.
