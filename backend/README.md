# losos control plane

The appliance's privileged core, as one Rust crate shipping two binaries.

| Binary | Role |
|---|---|
| `lososd` | Root systemd daemon. Owns `/var/lib/losos/state.json`, exports the `org.losos1` D-Bus service, and serves the loopback admin HTTP API. |
| `losos-ctl` | Facade CLI. Relays each subcommand to `lososd` over the system bus — plus `install`, the auto-installer, which runs standalone on the installer ISO. |

`schema.json` specifies every JSON document, D-Bus method and HTTP route. If
it and this README disagree, `schema.json` and the code are right.

## Layout

```
src/
  lib.rs          crate docs and module wiring
  model.rs        Mode, RebuildState, Rebuild, State, Settings
  losos.rs        the Losos effect trait + generic command handlers
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

The `cmd_*` handlers in `losos.rs` (state, change, status, settings, apply,
claim, grow, password, recovery, app search) are generic over the `Losos`
trait and know nothing about paths, systemd, D-Bus or HTTP. `io_backend`
implements the trait for real; `fake` implements it in memory for tests. The
installer follows the same pattern with `Install`, `plan_install` and
`execute`: the plan is data, so tests check the order of destructive steps
without touching a disk.

## Build and test

```sh
cargo test   --manifest-path backend/Cargo.toml
cargo clippy --manifest-path backend/Cargo.toml --all-targets -- -D warnings
nix build .#losos-ctl               # doCheck = false: builds only, no tests
```

The VM tests are the acceptance gate. CI cannot run them, so run them locally
before changing the daemon or the installer:

```sh
nix build .#checks.x86_64-linux.losos-admin-daemon   # D-Bus, HTTP, token
nix build .#checks.x86_64-linux.losos-install        # detect, disko, LVM
```

## Environment

Every path can be overridden, which lets tests run against a temporary
directory.

| Variable | Default |
|---|---|
| `LOSOS_STATE_DIR` | `/var/lib/losos` |
| `LOSOS_OVERRIDES` | `/etc/nixos/modules/overrides.nix` |
| `LOSOS_FLAKE` | `/etc/nixos#install` |
| `LOSOS_NO_DBUS` | unset; any value disables the bus listener |
| `LOSOS_ADMIN_PORT` | `8082` |
| `LOSOS_ADMIN_TOKEN_FILE` | `/var/secrets/losos-admin-token` |
| `LOSOS_FLAKE_URL` | `https://github.com/dasmatus/losos.git` |
| `LOSOS_FLAKE_WORK` | `/tmp/losos-flake` |
| `LOSOS_KEYFILE` | `/etc/keys/persist-keyfile` |

## Things that look wrong but are intended

- **`change --mode`, `apply` and `factory-reset` all write
  `modules/overrides.nix`.** `change` line-patches the one `sharingMyStorage`
  assignment; the other two replace the file. It is the only Nix file the
  control plane writes and the only one the flake evaluates for a runtime
  setting. (`change` used to patch `/etc/nixos/defaults.nix`, a path no
  installed box has, and reported success when it was missing.)
- **A corrupt `state.json` becomes the default state.** A blank admin UI would
  be worse, and the next write repairs the file.
- **`disko` runs with `--yes-wipe-all-disks`.** Otherwise it prompts on stdin,
  reads EOF and aborts.
- **The installer deletes `.git` from its flake clone.** Nix's git fetcher only
  sees tracked files, and `install-target.nix` is written into the clone
  afterwards. With `.git` present it would be ignored and the install would
  target the default disk.
- **The keyfile is copied to `/mnt/etc/keys` before `nixos-install`.** The
  chrooted bootloader install looks up `boot.initrd.secrets` inside `/mnt`.
- **`ensure_token` accepts any token file containing exactly 64 lowercase hex
  characters.** To rotate the token, write a new value in that format and
  restart `lososd`, or delete the file and restart it to mint a random one. The
  first-owner claim returns the token only once; after claiming, a newly minted
  token cannot be fetched through the UI, so rotation requires local access to
  the token file.
- **The daemon shares one `IoLosos`.** `bin/lososd.rs` builds it once and
  passes clones to `dbus::serve`, `http::serve` and every rebuild watcher. Its
  lock serialises read-modify-write of `state.json` across the bus and HTTP,
  and stops a finishing rebuild from overwriting a newer one. Two separately
  built backends would not lock each other out.
- **`lososd` re-attaches a watcher at startup** to any rebuild recorded as
  `building`. `nixos-rebuild switch` restarts the daemon mid-rebuild; without
  this the state would stay `building` forever.
