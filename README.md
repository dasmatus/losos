# losos

A NixOS appliance that turns a mini-PC into a private cloud and a community
storage node: Nextcloud for your own files, Tahoe-LAFS contributing spare disk
to a distributed grid, the two users mutually unreadable. No SSH and no login
shell — administration happens over the web, and upgrades run unattended.

The root filesystem is tmpfs, rebuilt on every boot. Only what is explicitly
listed as persistent survives, on an encrypted partition, so a powered-off box
gives up nothing to someone holding it.

## What runs on it

| | |
|---|---|
| **Nextcloud** | Files, calendar and contacts for the `notshared` user. Reached at `<host>.local/nextcloud`. |
| **Tahoe-LAFS** | Contributes spare disk to a storage grid as the `shared` user. Web UI on `:3456`. |
| **Forgejo** | Optional git hosting at `<host>.local/forgejo/`. |
| **Admin UI** | A dependency-free static page at `<host>.local`, LAN-only, for the handful of settings the box exposes. |
| **Master proxy** | Optional. Reaches the appliance from the internet through a rathole tunnel to a VPS running Traefik, without opening a port at home. |

The two data users have mode-`700` homes, each with its own primary group, no
passwords and no shell. Neither can read the other's files — asserted in a
booted VM by `losos-impermanence`, not just claimed.

## Install

```sh
nix build .#nixosConfigurations.iso.config.system.build.isoImage
```

Write the image to a USB stick and boot the target machine. The installer
finds every fixed disk, merges them into one LVM volume group, encrypts it,
and installs — unattended. It needs a network connection, because it clones
the flake at run time rather than baking a copy into the ISO.

Encryption unlocks from the TPM where there is one. Without a TPM it falls
back to a keyfile in the initrd, which does **not** protect against someone
taking the machine — see [docs/security-model.md](docs/security-model.md).

## Development

```sh
devenv shell        # or: direnv allow
```

Everything is a named script, so CI and your shell run identical commands:

| | |
|---|---|
| `fmt` · `lint` · `test-rust` | Format, lint and test both Rust crates |
| `check-flake` · `check-eval` | Evaluate the flake; force the full module merge |
| `build-pkgs` · `build-iso` | Build the three packages; build the installer ISO |
| `vm-tests` | The four `nixos-test` VMs. Needs `/dev/kvm` |
| `devenv test` | Everything except the VM tests and the ISO |

`nix develop` still works and gives a toolchain-only shell for anyone without
the devenv CLI.

Two lock files exist: `flake.lock` pins the nixpkgs that *builds* the
appliance, `devenv.lock` the one that *lints and tests* it. They must be
bumped together; `check-pins` fails if they disagree.

## Testing

The VM tests are the real gate, and CI cannot run them — Codeberg's runners
cap at 10 minutes and 8 GB, and the tests need KVM. Run them locally:

| | |
|---|---|
| `losos-install` | Boots three empty disks and runs the installer through disko, LVM and LUKS |
| `losos-admin-daemon` | `lososd` over D-Bus, its HTTP API, and the Bearer token |
| `losos-edge-proxy` | Two VMs: the master-proxy register/reconcile/forward path, and that enrollment is closed |
| `losos-ds-render` | Renders every design-system component in headless Chromium |

The installer ISO and the `install` system closure are also local-only gates.
Build both before merging anything that touches the installer, the `iso` block
or `isoImage.*`.

## Layout

```
modules/           NixOS modules; all options live under losos.* in options.nix
backend/           lososd (root daemon) + losos-ctl (CLI facade), Rust
backend-registrar/ the master-proxy edge: registration API + config reconciler
admin-ui/          the static admin SPA and its stylesheets
tests/             nixos-test VMs
docs/              security model, baseline, design specs and plans
```

`CLAUDE.md` covers the cross-file architecture and the failure modes that bite
silently.

## Licence

AGPL-3.0-or-later. The repository is [REUSE](https://reuse.software)-compliant;
see `REUSE.toml` and `LICENSES/`.
