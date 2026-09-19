# losos

A NixOS appliance that turns a mini-PC into a private cloud and a node in a
community compute-and-storage mesh: Nextcloud for your own files, spare disk and
spare CPU contributed to a cluster of other boxes, the two users mutually
unreadable. No SSH and no login shell — administration happens over the web, and
upgrades run unattended.

The root filesystem is tmpfs, rebuilt on every boot. Only what is explicitly
listed as persistent survives, on an encrypted partition, so a powered-off box
gives up nothing to someone holding it.

## Why not just use something else

Most of what follows is available elsewhere. Four things are not, and they are
the reason this exists rather than a bookmark to someone else's project.

**Against a Synology, QNAP or other NAS appliance.** Those are the closest
thing in spirit: a box on a shelf, a web UI, no terminal. The difference is what
happens when one is compromised. A NAS runs a long-lived, mutable root that
accumulates state nobody enumerated — which is why a NAS ransomware event is
usually discovered months after the initial foothold. Here the root is tmpfs and
is discarded at every boot, and the only things that survive are the directories
named explicitly in `impermanence.nix`. An attacker who writes anywhere else has
until the next reboot, and `midnight-reboot.timer` fires at 00:07 whether anyone
is watching or not. The second difference is that you can read every line of
what your box runs, and rebuild it yourself from this repository.

**Against a VPS or a rented seedbox.** The machine this OS runs on is in your home and the disk
is LUKS-encrypted with the key sealed to its TPM, so the hosting provider,
their staff, and anyone who takes the hardware get an opaque block device rather
than your files. The trade you would normally make for that — losing a public
address — is handled by the optional master proxy: a rathole tunnel out to a
cheap VPS running Traefik, so the box is reachable from the internet without a
single inbound port open at home.

**Against plain NixOS, which this is built on.** Nothing here is unavailable to
someone willing to write it. What the appliance adds is that it is already
written and already tested: the two-user isolation, the encrypted-persistence
layout, the online disk growth, the hardening baseline and the mesh gating are
each asserted in a booted VM under `tests/`, not merely configured. And the
administration surface is deliberately tiny — a handful of settings over the
web, no SSH, no shell login — because the target reader is somebody who wants a
private cloud, not a second job.

**Against every self-hosting stack, on the mesh.** This is the part with no
real equivalent. A box can lend its spare disk and spare CPU to other people's
boxes, and it does so under two conditions that must both hold: a window you
set (permission) _and_ the box actually being idle (reality). Idleness can
withdraw availability inside your window; it can never grant it outside one,
because a quiet box at 14:00 is not consent. Every unknown — a missing
heartbeat, a stale report, an edge that just restarted — resolves to "busy".
The effect is that you can contribute real capacity without ever finding your
own machine slow while you are sitting at it.

**What this is not.** It is not a general-purpose server: there is no shell to
log into, by design, and adding one would defeat most of the above. It is not a
backup system — impermanence discards, it does not archive, and you still need
copies of `/persist` somewhere else. It is not finished; see the caveats
throughout this file, which are written to be read rather than buried.

## What runs on it

|                  |                                                                                                                                           |
| ---------------- | ----------------------------------------------------------------------------------------------------------------------------------------- |
| **Nextcloud**    | Files, calendar and contacts for the `notshared` user. Reached at `<host>.local/nextcloud`.                                               |
| **Mesh storage** | Contributes spare disk to the cluster as the `shared` user, replicated by Longhorn. The domain is fscrypt-locked whenever sharing is off. |
| **Mesh compute** | Optional. Contributes CPU to the cluster during a nightly window — "share my compute when I sleep".                                       |
| **Forgejo**      | Optional git hosting at `<host>.local/forgejo/`.                                                                                          |
| **Admin UI**     | A single-page app at `<host>.local`, LAN-only, for the handful of settings the box exposes.                                               |
| **Master proxy** | Optional. Reaches the appliance from the internet through a rathole tunnel to a VPS running Traefik, without opening a port at home.      |

The two data users have mode-`700` homes, each with its own primary group, no
passwords and no shell. Neither can read the other's files — asserted in a
booted VM by `losos-impermanence`, not just claimed.

## Install

Tagged releases carry a prebuilt image:

**<https://codeberg.org/dasmatus/losos/releases/latest>**

The release pages link to versioned files in Codeberg's generic package
registry rather than attaching the multi-gigabyte media directly. The registry
holds the installer ISO and the demo QCOW2, and the release still carries the
small `.sha256`/`release.json` metadata. To build the installer ISO yourself:

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

### An ISO that carries the closure

```sh
nix build .#losos-disk-iso          # devenv shell build-media iso
```

The ordinary ISO carries the installer and nothing else, so the target machine
fetches and builds the whole appliance itself. Measured on a dev machine, that
closure is 5.78 GiB over 882 store paths. 1.57 GiB of it comes down from
cache.nixos.org. The box compresses the Nextcloud, Forgejo and pause image
tarballs itself, another 849 MiB, because no public cache has them, and
compiles `losos-ctl` for the same reason. This ISO puts the finished closure in
`isoImage.storeContents`, so `nixos-install` copies those paths off the medium
instead.

It still needs a network. The installer clones the flake at run time and
`nixos-install` evaluates it, which fetches nixpkgs. Carrying store paths does
not change that.

It also only pays off if the medium and the clone agree. `losos-install` runs
`git clone --depth 1` of `LOSOS_FLAKE_URL` and installs _that_ revision, so an
ISO built from a different commit evaluates to different store paths and the
copies go unused without saying so. Build it from the commit you mean to
install.

Build it locally and keep it there. The plain ISO is already about 1.5 GB,
Codeberg's whole recommended budget for packages and attachments, and the
closure adds up to 2.4 GiB on top of it once compressed at the level the
medium's squashfs uses. This is the offline half of what `modules/cache.nix`
does online. A reachable binary cache is cheaper; the medium works without one.

### A demo image for a VM

```sh
nix build .#losos-disk-qcow2        # devenv shell build-media qcow2
```

A preinstalled QCOW2 to boot in QEMU or virt-manager, so you can look at the
admin UI, Nextcloud and the settings page without finding a spare machine.
Tagged releases publish it beside the installer ISO in the generic registry.

It is not the appliance, and the difference is the security model rather than a
detail. A disk image cannot carry the TPM-sealed LUKS volume the installer
builds, so this one has no full-disk encryption at all. Whoever holds the file
can read everything in it. It also never runs the installer, so a QCOW2 that
boots is no evidence that an install works. `tests/install.nix` and a real box
remain the only things that say anything about that. Demo and development
only.

### Growing the disk later

The logical volume claims 90% of the volume group, not all of it
(`losos.storage.fillPercent`). The remainder exists so the disk can be grown
without opening the box:

```sh
losos-ctl grow
```

That runs `lvextend`, `cryptsetup resize` and `resize2fs` in that order, with
`/persist` still mounted and nothing restarted. It matters because `/persist`
_is_ `/nix` here — impermanence binds one over the other — so the store grows
with every generation and the 03:00 unattended rebuild is what eventually fills
it, on a machine with no shell to notice from.

Once the reserve is used up, add a disk and run it again:

```sh
pvcreate /dev/sdX && vgextend persist-vg /dev/sdX && losos-ctl grow
```

It refuses rather than reporting success when there is nothing left to claim.

## Hardening

The baseline is on by default: KSPP kernel parameters, sysctl tightening, a
kernel-module blacklist that also blocks explicit `modprobe`, tightened mount
options, dbus-broker, and systemd sandboxing on the services that face the
network. Four settings that can cost you something are opt-in:

```nix
losos.hardening.apparmor = true;   # mandatory access control
losos.hardening.malloc   = true;   # GrapheneOS hardened_malloc
losos.hardening.nosmt    = true;   # no SMT; roughly halves the core count
losos.hardening.usbguard = true;   # block USB devices not present at boot
```

Three settings every hardening guide recommends are deliberately left out,
because on this box they cost more than they buy: strict `rp_filter` (it drops
the mDNS replies that are the only way in, and breaks Calico), disabled user
namespaces (it stops both kubelets and containerd), and `noexec` on `/tmp` (nix
builds execute there, and the nightly rebuild is the only self-repair path).
`tests/hardening.nix` asserts all three are absent so they stay decisions
rather than regressions.

## The mesh

A box runs **two** Kubernetes instances, and they are deliberately different
clusters:

|                    | Local                                | Mesh                             |
| ------------------ | ------------------------------------ | -------------------------------- |
| What               | this box's own Nextcloud and Forgejo | Longhorn storage, shared compute |
| Server             | this box                             | the edge VPS                     |
| Needs the network? | **no**                               | yes                              |

The split is the whole point. A Kubernetes agent cannot start its kubelet while
its server is unreachable, and this box reboots itself every night — so if your
own files lived in the edge's cluster, any outage spanning midnight would take
them offline until it ended, on a machine with no shell to fix it from. Your
data stays in the local cluster, which depends on nothing off-box.

Joining the mesh is off by default. Two switches in the settings page control
it: one to join at all, one to contribute CPU.

The second lends the machine out only when **both** of two things hold: the
nightly window you set, and the box actually being idle. The two are not
interchangeable, and the direction matters. A window is a guess about a
routine — set 23:00–07:00, then stay up editing photos, and you have told the
mesh your box is free while you are sitting at it. So idleness can _withdraw_
availability inside the window. It can never _grant_ it outside one: an owner
who set a window meant it, and a box that happens to be quiet at 14:00 has not
consented to anything.

Every unknown resolves to busy. A box too old to report idleness, a report
older than the heartbeat's time-to-live, an edge that has just restarted and
holds no live state — each reads as "in use", so the failure mode is that you
contribute less than you offered, never that a stranger's job lands on a
machine you are working at.

Storage you contribute is locked when you are not contributing it. The `shared`
domain sits under an fscrypt policy whose key is sealed to the TPM; with sharing
off the key is not in the kernel keyring and the directory is unreadable **even
to root on the running machine**. That is a layer on top of the disk encryption,
not a replacement for it: LUKS protects a box that is switched off, fscrypt
protects this domain from the box that is switched on.

Contributed files are namespaced per machine, so a pooled volume stays
attributable to the box that supplied it.

## How it works

Four mechanisms carry most of the behaviour above. None of them is exotic; what
matters is which one owns what.

**The root is thrown away, and a list decides what isn't.** `/` is a tmpfs, so
it starts empty at every boot. `/persist` is a LUKS-encrypted ext4 partition,
and `impermanence` bind-mounts a fixed set of directories out of it back into
place: `/nix`, `/var`, `/etc/ssh`, `/etc/keys`, `/etc/nixos`, `/etc/rancher`,
the two data homes, and `machine-id`. That list, in `modules/impermanence.nix`,
is the whole contract — **anything not on it is gone at the next reboot**,
which is the point for an attacker's foothold and the trap for a new feature
that quietly writes state somewhere else. `/persist` is marked `neededForBoot`
so it is available before those mounts resolve. It is unlocked from the TPM, or
on hardware without one from a keyfile in the initrd — and that second path
does not survive physical theft, which `docs/security-model.md` says plainly.

**Administration is a daemon, not a login.** There is no SSH and no shell, so
every privileged action goes through `lososd`, a root systemd service. It owns
`/var/lib/losos/state.json` as sole writer, exposes one method per operation on
the system D-Bus, and serves a token-authenticated JSON API on loopback only.
Nginx proxies `/api/*` to it from the LAN-only admin vhost; the token is 64
random hex characters that `lososd` itself writes, mode 0600, on first start.
Changing a setting writes `modules/overrides.nix` and starts a supervised
`nixos-rebuild switch` as a transient unit — which restarts `lososd` mid-flight,
so it re-attaches to the running rebuild when it comes back up rather than
reporting "building" forever.

**Settings are Nix, not a database.** The settings page generates the body of
`modules/overrides.nix` and posts it; there is no other write path. That is why
a change costs a rebuild rather than a restart, and why the box can always be
reproduced from this repository plus that one file.

**It updates itself, and reboots whether or not that worked.**
`system.autoUpgrade` rebuilds from `losos.upgradeFlakeUri` at 03:00. The default
only advances the system consistently from the local checkout; point it at a
`github:` URI to actually pull new packages. Separately,
`midnight-reboot.timer` reboots unconditionally at 00:07, with `Persistent=true`
so a box that was switched off catches up. The nightly reboot is not
housekeeping — on a machine whose root is discarded at boot, it is the
self-repair path, and it is why nothing here needs a shell to recover.

## Development

```sh
devenv shell        # or: direnv allow
```

Everything is a named script:

|                                |                                                                                     |
| ------------------------------ | ----------------------------------------------------------------------------------- |
| `fmt` · `lint` · `test-rust`   | Format, lint and test both Rust crates                                              |
| `check-flake` · `check-eval`   | Evaluate the flake; force the full module merge                                     |
| `build-pkgs` · `build-iso`     | Build the three packages; build the installer ISO                                   |
| `build-images` · `build-media` | The OCI images; the demo QCOW2 and the closure-carrying ISO. Gigabytes each, opt-in |
| `vm-tests`                     | Every `nixos-test` VM the flake exports. Needs `/dev/kvm`                           |
| `devenv test`                  | Everything except the VM tests, the images and the media                            |

`nix develop` still works and gives a toolchain-only shell for anyone without
the devenv CLI.

Formatting is not your problem: a CI job reformats and pushes back on every
push to `main`, and its commit carries `[skip ci]` so it does not retrigger
the workflow. Clippy is _not_ auto-fixed — it rewrites code rather than
whitespace, so it stays a gate you have to satisfy yourself.

CI does **not** go through devenv, and spells the same commands out directly.
It was tried: every job first realises the whole dev toolchain, and both
devenv jobs hit Codeberg's 10-minute cap (9m31s and 10m05s) while the one
non-devenv job took 1m32s. The duplication between `devenv.nix` and
`.forgejo/workflows/ci.yml` is the price of that cap, and the two have to be
kept in step by hand.

There are two CI lanes, running the same gates against the same flake:
`.forgejo/workflows/` on Codeberg and `.github/workflows/ci.yml` on GitHub.
They too are kept in step by hand. Four things differ, and each is a platform
difference rather than a preference:

- The GitHub lane runs on `ubuntu-latest` with Nix installed by
  `.github/actions/setup-nix`, not inside `docker.io/nixos/nix`. GitHub injects
  a glibc-linked Node into container jobs to run JavaScript actions, and that
  image is Alpine/musl, so `actions/checkout` could not start at all.
- That action writes the substituters to `/etc/nix/nix.conf` before the daemon
  starts, rather than passing `NIX_CONFIG`. A multi-user Nix daemon discards
  cache settings from an untrusted client — with a warning, and a green build
  that compiled everything from source.
- GitHub caps jobs at six hours rather than ten minutes, so the Nextcloud image
  — the one most likely to break, and the hole the Codeberg lane documents —
  also builds on the weekly schedule there, not only on request.
- Releases differ; see [Testing](#testing).

Two lock files exist: `flake.lock` pins the nixpkgs that _builds_ the
appliance, `devenv.lock` the one that _lints and tests_ it. They must be
bumped together; `check-pins` fails if they disagree.

## Testing

The VM tests are the real gate, and CI cannot run them — Codeberg's runners
cap at 10 minutes and 8 GB, and the tests need KVM. Run them locally:

|                      |                                                                                          |
| -------------------- | ---------------------------------------------------------------------------------------- |
| `losos-install`      | Boots three empty disks and runs the installer through disko, LVM and LUKS               |
| `losos-admin-daemon` | `lososd` over D-Bus, its HTTP API, and the Bearer token                                  |
| `losos-edge-proxy`   | Two VMs: the master-proxy register/reconcile/forward path, and that enrollment is closed |
| `losos-ds-render`    | Renders every design-system component in headless Chromium                               |

The `install` system closure is a local-only gate; build it before merging
anything that touches the module set.

The installer ISO **is** built in CI on every push, in both lanes, and
published when you push a `v*` tag. Publishing stays rationed to tags because a
1.5 GB upload per commit to a free host would be antisocial.

Where the bytes land is now the one intentional extra difference between the
two lanes. The ordinary CI gates still run on both forges, but release
publication happens from GitHub's larger runners: they build the installer ISO
and the demo QCOW2, upload both to Codeberg's generic package registry, then
update both release pages to point there. That keeps one durable, versioned set
of downloads while avoiding GitHub's per-asset release limit and Codeberg's
smaller build runners. The release pages themselves still only carry the small
`.sha256` and `release.json` metadata.

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
admin-ui/app/      the admin SPA the box serves (React + Vite + Tailwind)
admin-ui/design-system/  dev-only tokens and a React wrapper; not shipped
tests/             nixos-test VMs
docs/              security model, baseline, design specs and plans
```

`CLAUDE.md` covers the cross-file architecture and the failure modes that bite
silently.

## Licence

AGPL-3.0-or-later. The repository is [REUSE](https://reuse.software)-compliant;
see `REUSE.toml` and `LICENSES/`.
