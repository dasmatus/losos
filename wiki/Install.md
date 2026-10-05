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
- **UEFI or BIOS, both work.** On the installer ISO, a terminal menu lets you
  choose BIOS, UEFI, or autodetect (the firmware that booted the ISO). UEFI gets
  systemd-boot; legacy BIOS gets GRUB plus a 1 MiB BIOS boot partition on the
  first disk. With no answer within 30 seconds the menu takes autodetect, so
  an unattended boot still installs. `losos-install --bios` and `--uefi`
  select a mode without showing the menu. UEFI, from the menu or `--uefi`,
  needs the stick itself booted in UEFI mode, because systemd-boot writes the
  firmware's boot entry; from a BIOS-booted stick the installer refuses it
  before touching any disk. BIOS is mainly for testing in a VM. This menu is
  only part of the installer ISO; it is not shown on the installed appliance.
- **Connect the network.** The installer clones the flake at run time.

## What the installer does

After the firmware choice, the installer runs unattended. It finds every fixed
disk, puts them in one LVM volume group, encrypts it with LUKS, formats
`/persist` as ext4, and installs.

- By default `/persist` unlocks from a keyfile in the initrd. That keyfile
  sits on an unencrypted ESP, so anyone who takes the machine can read the
  data. This is what the installer ISO does, TPM or not.
- Unlocking from the TPM instead is opt-in: run `losos-ctl install --tpm`
  from the installer's shell. It asks for a passphrase at format time, and
  until you run `systemd-cryptenroll --tpm2-device=auto --tpm2-pcrs=0+7
  /dev/persist-vg/persist` on the installed system, every boot stops at that
  passphrase. Nothing automates that step yet, which is why it is not the
  default on a box with no shell.

`/persist` is ext4 with the `encrypt` feature because fscrypt needs it and
btrfs does not support it. You lose compression and data checksums.

After the reboot, tty1 shows a full-screen banner with the box's IP address and
`<hostname>.local`. Open that address in a browser on any computer on the same
network. The banner updates when the address changes.

## Try it in a VM (BIOS)

```sh
qemu-img create -f raw disk.img 40G
qemu-system-x86_64 -m 4096 -smp 2 -enable-kvm -machine q35 \
  -drive file=losos.iso,media=cdrom,readonly=on \
  -drive file=disk.img,format=raw,if=virtio \
  -nic user,hostfwd=tcp::8080-:80
```

Without `-bios` QEMU boots SeaBIOS, so the installer picks GRUB. Once it
finishes, shut the VM down and boot again without the `-drive …media=cdrom`
line. The VM's address is only reachable from the host through the forward:
open `http://localhost:8080`.

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
