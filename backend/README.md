# losos control plane

The appliance's privileged core, as one Rust crate shipping two binaries.

| Binary | Role |
|---|---|
| `lososd` | Root systemd daemon. Owns `/var/lib/losos/state.json`, exports the `org.losos1` D-Bus service, and serves the loopback admin HTTP API. |
| `losos-ctl` | Facade CLI. Relays each subcommand to `lososd` over the system bus — plus `install`, the auto-installer, which runs standalone on the installer ISO. |

`schema.json` in this directory is the normative wire specification for every
JSON document, D-Bus method and HTTP route. When behaviour and this README
disagree, believe `schema.json` and the code.

## Layout

```
src/
  lib.rs          crate docs and module wiring
  model.rs        Mode, RebuildState, Rebuild, State, Settings
  losos.rs        the Losos effect trait + the six commands
  overrides.rs    parsing and rendering the two Nix files (pure)
  fake.rs         in-memory Losos, for tests
  io_backend.rs   the real Losos: files, environment, paths
  supervisor.rs   rebuild spawning, polling and outcome recording
  dbus.rs         the zbus service
  http.rs         the actix-web admin API
  facade.rs       the zbus client the CLI uses
  installer.rs    the installer's pure planner
  installer_io.rs the installer's effects, and its entry point
  bin/            the two binaries
```

The shape worth understanding is the seam in `losos.rs`. The six commands
(`cmd_state`, `cmd_change`, `cmd_status`, `cmd_settings`, `cmd_apply`,
`cmd_factory_reset`) are written once, generic over the `Losos` trait, and know
nothing about paths, systemd, D-Bus or HTTP. `io_backend` implements that trait
against the real world; `fake` implements it in memory. So the state machine is
tested exactly as it ships, with no filesystem and no subprocesses. The
installer repeats the pattern with `Install`, `plan_install` and `execute` —
there, the plan is data, so tests assert on the *ordering* of destructive steps
without formatting anything.

## Build and test

```sh
nix develop .                       # cargo, rustc, clippy, rustfmt, rust-analyzer
cargo test  --manifest-path backend/Cargo.toml
cargo clippy --manifest-path backend/Cargo.toml --all-targets -- -D warnings

nix build .#losos-ctl               # doCheck runs the suite again at build time
```

The two acceptance tests are NixOS VM tests and are the real gate:

```sh
nix build .#checks.x86_64-linux.losos-admin-daemon   # daemon: D-Bus, HTTP, token
nix build .#checks.x86_64-linux.losos-install        # installer: detect, disko, LVM
```

Neither runs in CI today — CI evaluates the flake and builds the ISO — so run
them locally before changing anything on these paths.

## Environment

Every path is overridable, which is how the CLI is driven against a throwaway
directory in tests.

| Variable | Default |
|---|---|
| `LOSOS_STATE_DIR` | `/var/lib/losos` |
| `LOSOS_CONFIG` | `/etc/nixos/defaults.nix` |
| `LOSOS_OVERRIDES` | `/etc/nixos/modules/overrides.nix` |
| `LOSOS_FLAKE` | `/etc/nixos#install` |
| `LOSOS_NO_DBUS` | unset; any value disables the bus listener |
| `LOSOS_ADMIN_PORT` | `8082` |
| `LOSOS_ADMIN_TOKEN_FILE` | `/var/secrets/losos-admin-token` |
| `LOSOS_FLAKE_URL` | `https://codeberg.org/dasmatus/losos.git` |
| `LOSOS_FLAKE_WORK` | `/tmp/losos-flake` |
| `LOSOS_KEYFILE` | `/etc/keys/persist-keyfile` |

## Things that look wrong and are not

- **`change --mode` writes `defaults.nix`; `apply` and `factory-reset` write
  `overrides.nix`.** Two different files. Which one wins at Nix evaluation time
  is decided by module import order in the flake, not here.
- **A corrupt `state.json` silently becomes the default state.** A blank admin
  UI is a worse failure than a reset, and the next write repairs the file.
- **`disko` is invoked with `--yes-wipe-all-disks`.** Its combined destroy mode
  prompts on stdin otherwise, and an unattended installer reads EOF and aborts.
- **The installer deletes the `.git` of the flake clone it just made.** Nix's
  git fetcher exposes only tracked files to flake evaluation, and
  `install-target.nix` is written into that clone afterwards. With `.git`
  present it would be invisible and the install would target the default disk.
- **The keyfile is copied into `/mnt/etc/keys` before `nixos-install` runs.**
  The chrooted bootloader install resolves `boot.initrd.secrets` inside `/mnt`.
- **`lososd` re-attaches a watcher at startup** to any rebuild recorded as
  `building`. `nixos-rebuild switch` restarts the daemon mid-rebuild, so the
  original watcher thread dies partway through; without the re-attach the state
  file would say `building` forever.
