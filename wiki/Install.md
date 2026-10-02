# Install

## Get the installer

Download the ISO and its `.sha256` from the
[latest release](https://github.com/dasmatus/losos/releases/latest), or build it:

```sh
nix build .#nixosConfigurations.iso.config.system.build.isoImage
```

Write it to a USB stick and boot the target machine.

## Before you boot

- **Turn Secure Boot off.** Nothing is signed. With Secure Boot on, the
  firmware refuses the stick (OVMF shows "Access Denied"; other firmware may
  skip it silently).
- **Boot in UEFI mode.** The ISO boots under UEFI and legacy BIOS, but the
  installed system uses systemd-boot and needs UEFI.
- **Connect the network.** The installer clones the flake at run time.

## What the installer does

It runs unattended. It finds every fixed disk, puts them in one LVM volume
group, encrypts it with LUKS, formats `/persist` as ext4, and installs.

- With a TPM, `/persist` unlocks from the TPM.
- Without a TPM, it unlocks from a keyfile in the initrd. That keyfile sits on
  an unencrypted ESP, so anyone who takes the machine can read the data.

`/persist` is ext4 with the `encrypt` feature because fscrypt needs it and
btrfs does not support it. You lose compression and data checksums.

## ISO with the full closure

```sh
nix build .#losos-disk-iso          # or: devenv shell build-media iso
```

The normal ISO makes the target download and build the whole system
(about 5.8 GiB). This ISO carries the built closure, so `nixos-install` copies
it from the stick.

- It still needs a network to clone the flake and fetch nixpkgs.
- Build it from the same commit the installer will clone
  (`LOSOS_FLAKE_URL`, `--depth 1`). Otherwise the store paths differ and the
  copies are not used.
- It is too large for a GitHub release asset. Build it yourself.

## Demo image for a VM

```sh
nix build .#losos-disk-qcow2        # or: devenv shell build-media qcow2
```

A preinstalled QCOW2 for QEMU or virt-manager. Tagged releases publish it to
GHCR. It has **no disk encryption** and never runs the installer, so use it
for demos and development only.

## Growing the disk

The logical volume uses 90% of the volume group (`losos.storage.fillPercent`).
To use the rest:

```sh
losos-ctl grow
```

This runs `lvextend`, `cryptsetup resize` and `resize2fs`, in that order, with
`/persist` mounted. `/persist` holds `/nix`, so it fills up over time.

When the reserve is used up, add a disk and grow again:

```sh
pvcreate /dev/sdX && vgextend persist-vg /dev/sdX && losos-ctl grow
```

`grow` fails if there is nothing left to claim.
