# losos

A NixOS appliance that turns a mini-PC into a private cloud and a node in a
community compute-and-storage mesh: Nextcloud for your own files, spare disk and
spare CPU contributed to a cluster of other boxes, the two users mutually
unreadable. No SSH and no login shell — administration happens over the web, and
upgrades run unattended.

The root filesystem is tmpfs, rebuilt on every boot. Only what is explicitly
listed as persistent survives, on an encrypted partition, so a powered-off box
gives up nothing to someone holding it.

## What runs on it

| | |
|---|---|
| **Nextcloud** | Files, calendar and contacts for the `notshared` user. Reached at `<host>.local/nextcloud`. |
| **Mesh storage** | Contributes spare disk to the cluster as the `shared` user, replicated by Longhorn. The domain is fscrypt-locked whenever sharing is off. |
| **Mesh compute** | Optional. Contributes CPU to the cluster during a nightly window — "share my compute when I sleep". |
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

`/persist` is formatted ext4 with the `encrypt` feature. That is not a
preference: fscrypt needs a filesystem that implements it, and btrfs does not.
It costs transparent compression and data checksums, which is why mesh
replication matters more than it used to.

## The mesh

A box runs **two** Kubernetes instances, and they are deliberately different
clusters:

| | Local | Mesh |
|---|---|---|
| What | this box's own Nextcloud and Forgejo | Longhorn storage, shared compute |
| Server | this box | the edge VPS |
| Needs the network? | **no** | yes |

The split is the whole point. A Kubernetes agent cannot start its kubelet while
its server is unreachable, and this box reboots itself every night — so if your
own files lived in the edge's cluster, any outage spanning midnight would take
them offline until it ended, on a machine with no shell to fix it from. Your
data stays in the local cluster, which depends on nothing off-box.

Joining the mesh is off by default. Two switches in the settings page control
it: one to join at all, one to contribute CPU — and the second only lends the
machine out during a nightly window, so the box is yours while you are using it.

Storage you contribute is locked when you are not contributing it. The `shared`
domain sits under an fscrypt policy whose key is sealed to the TPM; with sharing
off the key is not in the kernel keyring and the directory is unreadable **even
to root on the running machine**. That is a layer on top of the disk encryption,
not a replacement for it: LUKS protects a box that is switched off, fscrypt
protects this domain from the box that is switched on.

Contributed files are namespaced per machine, so a pooled volume stays
attributable to the box that supplied it.

## Development

```sh
devenv shell        # or: direnv allow
```

Everything is a named script:

| | |
|---|---|
| `fmt` · `lint` · `test-rust` | Format, lint and test both Rust crates |
| `check-flake` · `check-eval` | Evaluate the flake; force the full module merge |
| `build-pkgs` · `build-iso` | Build the three packages; build the installer ISO |
| `vm-tests` | The four `nixos-test` VMs. Needs `/dev/kvm` |
| `devenv test` | Everything except the VM tests and the ISO |

`nix develop` still works and gives a toolchain-only shell for anyone without
the devenv CLI.

Formatting is not your problem: a CI job reformats and pushes back on every
push to `main`, and its commit carries `[skip ci]` so it does not retrigger
the workflow. Clippy is *not* auto-fixed — it rewrites code rather than
whitespace, so it stays a gate you have to satisfy yourself.

CI does **not** go through devenv, and spells the same commands out directly.
It was tried: every job first realises the whole dev toolchain, and both
devenv jobs hit Codeberg's 10-minute cap (9m31s and 10m05s) while the one
non-devenv job took 1m32s. The duplication between `devenv.nix` and
`.forgejo/workflows/ci.yml` is the price of that cap, and the two have to be
kept in step by hand.

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

The `install` system closure is a local-only gate; build it before merging
anything that touches the module set.

The installer ISO **is** built in CI, and published as a downloadable
artifact when you push a `v*` tag or trigger the workflow by hand — not on
every commit, because the image is ~1.5 GB and that is Codeberg's entire
recommended attachment budget in one file.

 It looks like it should not fit — but of
the 769 store paths in its closure, 766 are stock nixpkgs that cache.nixos.org
serves and only `losos-ctl` costs anything to produce. The CI job takes that
one from the job that already builds it, as a 12 MiB artifact, so it never
compiles the crate twice. Squashfs and ISO assembly are 50 s on four cores.

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
