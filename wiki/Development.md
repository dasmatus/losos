# Development

## Shell

```sh
devenv shell        # or: direnv allow
devenv test         # pin checks, lint, rust tests, flake eval (no builds)
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

`devenv test` does not build packages or the installer ISO, or run VM tests.
Those are separate opt-in scripts.

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

Another job then rewrites the history so that no commit credits Claude or
links a claude.ai session, and force-pushes the branches and tags that
moved. If `main` changed under you, `git pull --rebase` picks the rewritten
commits up cleanly; a plain `git pull` would merge the two histories.

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

## Pull requests

The template in `.github/pull_request_template.md` fills the description:
Before / After prose, a screenshots table, How, Tested, notes for the
reviewer. Keep every section.

Screenshots are required for any change a person can see: the admin UI and
the wizard, the installer and the tty1 banner, the Nextcloud and Forgejo
themes, the wiki and docs pages. Take a Before and an After per screen at
the same window size and in the same state, so the only difference is the
change, and drag them into the table. A change with nothing visible says
"No visible change." under that heading, and why.

Under Tested, list what ran and what did not. The VM tests need KVM; if
they were not run, say so.

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
