---
title: Install in depth
sidebar_position: 1
mdx:
  format: md
---

# Install in depth

## Get the installer

Download the ISO and its `.sha256` from the
[latest release](https://github.com/dasmatus/losos/releases/latest), or build it:

```sh
nix build .#nixosConfigurations.iso.config.system.build.isoImage
```

Write it to a USB stick and boot the target machine. The stick's volume
label is `LOSOS_INSTALLER`. The BIOS boot menu, the UEFI splash and the
console banner say LosOS and the release tag, and the host name is
`losos-installer`. Underneath it is NixOS, and os-release says so with
`ID_LIKE=nixos`. The installed box carries the same name and tag in its
boot menu, its console banners and os-release (`NAME=LosOS`,
`IMAGE_VERSION`), and keeps `ID=nixos`.

## Before you boot

- **Secure Boot: off, or enrol the LosOS certificate.** Release ISOs are
  signed. The stick's UEFI loader is one unified kernel image (kernel,
  initrd, command line) signed with the LosOS db certificate, and that
  loader checks the system image's hash before mounting it. A firmware that
  trusts only Microsoft's keys refuses the stick until the certificate from
  the stick's `EFI/losos/` or the release page is added to its `db`, next
  to Microsoft's certificates, never in place of them: Windows, other
  distributions' shim and graphics-card option ROMs need those. OVMF shows
  "Access Denied"; other firmware may skip the stick without a word.
  The installed box's loader is not signed, so the box itself needs Secure
  Boot off. The handbook's *Secure Boot and signed media* page has the steps
  and the `SHA256SUMS.sig` check. `tests/secure-boot.nix` is the proof.
- **UEFI or BIOS, both work.** On the installer ISO, a terminal menu lets you
  choose BIOS, UEFI, or autodetect (the firmware that booted the ISO). UEFI
  gets systemd-boot. Legacy BIOS gets GRUB plus a 1 MiB BIOS boot partition
  on the first disk. With no answer within 30 seconds the menu takes
  autodetect, so an unattended boot still installs. `losos-install --bios`
  and `--uefi` select a mode without showing the menu. UEFI, from the menu or
  `--uefi`, needs the stick itself booted in UEFI mode, because systemd-boot
  writes the firmware's boot entry. From a BIOS-booted stick the installer
  refuses it before touching any disk. BIOS is mainly for testing in a VM.
  The menu is part of the installer ISO only. The installed appliance never
  shows it.
- **Connect the network.** The installer clones the flake at run time.

## What the installer does

After the firmware choice, the installer runs unattended. It finds every fixed
disk, puts them in one LVM volume group, encrypts it with LUKS, formats
`/persist` as ext4, and installs.

- On a machine with a TPM 2.0 chip, the installer seals the disk key to the
  chip right after formatting (`systemd-cryptenroll`). Most mini-PCs have
  one in the firmware, Intel PTT or AMD fTPM, usually switched on. The box
  then boots unattended from the first boot and nothing secret is on the
  boot partition. The disk alone, pulled or cloned, is unreadable. The key
  is not bound to firmware measurements (PCRs), because the box updates its
  firmware and bootloader unattended and has no shell to recover from a
  lockout. The price is that a thief who takes the whole box with its chip
  can still get at the data. The same random key the volume was formatted
  with stays inside the encrypted volume at `/etc/keys/persist-keyfile` as a
  recovery slot.
- Without a chip, or with `losos-ctl install --no-tpm` from the installer's
  shell, that keyfile is baked into the initrd instead. It then sits on the
  unencrypted ESP, and anyone who takes the disk can read the data. The
  installer prints which of the two it chose. `--tpm` makes a missing chip
  an error rather than a silent keyfile install.
  [TPM and the disk key](../reference/tpm.md) explains what the chip is for and what a box
  without one gives up.
- In a VM, give it a TPM (swtpm, see below) or it installs in keyfile mode.

`/persist` is ext4 with the `encrypt` feature because fscrypt needs it and
btrfs does not support it. You lose compression and data checksums.

After the reboot, tty1 shows a full-screen banner with the box's IP address
and `<hostname>.local`. Open the IP address in a browser on any computer on
the same network. The `.local` name works too wherever the computer resolves
mDNS names. Windows, macOS, phones and most Linux desktops do; the host of a
libvirt or VirtualBox NAT guest usually does not, so use the address there.
Both reach the same pages. The banner updates when the address changes.

The first page is the setup wizard. Its first step is trusting the box's own
certificate, so that the rest of the setup, and every later sign-in, travels
over HTTPS and your browser can offer a passkey. The step shows one line to
paste into a terminal, picked for the computer you are on. On macOS and Linux
it is `curl -fsSL http://<address>/setup/trust.sh | sh`, on Windows
`irm http://<address>/setup/trust.ps1 | iex`. The box itself serves the
script as plain text, so open the link in a tab to read it first. The script
adds the one certificate to the stores your browsers read, for your user
only: the login keychain on macOS, the user's Trusted Root store on Windows,
the NSS stores Chrome and Firefox use on Linux. It installs nothing else,
never asks for administrator rights, and prints the certificate's SHA-256
fingerprint so you can compare it with the one on the page. Phones get the
plain download instead. The manual route stays below it: download
`losos-ca.crt` and add it as a trusted authority yourself.

If reading a screen and typing an address is the scary part, open
[losos-edge.dasmat.us/find](https://losos-edge.dasmat.us/find) in Chrome
instead and press **Find my box**. Chrome asks once whether the page may look
for devices on your local network (its *Local Network Access* permission,
Chrome 142 or newer). Say yes, and the page finds the box by its name and
links you to its setup. The lookup runs inside your browser, between your
computer and the box. The page reads the box's `/setup/state.json`, which
holds its name and certificate fingerprint and nothing more, and nothing
about your network leaves your machine. The box lets only that one page read
the document (`losos.setup.finderOrigins`). Firefox and Safari have no such
permission and get the typed-address instructions instead.

## Try it in a VM (BIOS)

For the TPM path, the VM needs an emulated chip at install time *and* at
every boot, with the same state directory, or the installed system will not
find the key it sealed. With swtpm:

```sh
mkdir -p tpm
swtpm socket --tpmstate dir=tpm --ctrl type=unixio,path=tpm/sock --tpm2 --daemon
qemu-system-x86_64 ... \
  -chardev socket,id=chrtpm,path=tpm/sock -tpmdev emulator,id=tpm0,chardev=chrtpm \
  -device tpm-tis,tpmdev=tpm0
```

Add those three lines to both the installer run and the later boots. Without
them the installer says `unlock: keyfile in the initrd` and the box works, in
keyfile mode.

```sh
qemu-img create -f raw disk.img 40G
qemu-system-x86_64 -m 4096 -smp 2 -enable-kvm -machine q35 \
  -drive file=losos.iso,media=cdrom,readonly=on \
  -drive file=disk.img,format=raw,if=virtio \
  -nic user,hostfwd=tcp::8080-:80
```

Without `-bios` QEMU boots SeaBIOS, so the installer picks GRUB. Once it
finishes, shut the VM down and boot again without the `-drive …media=cdrom`
line. The VM's address is reachable from the host only through the forward,
so open `http://localhost:8080`.

## ISO with the full closure

```sh
nix build .#losos-disk-iso          # or: devenv shell build-media iso
```

The normal ISO makes the target download and build the whole system, about
5.8 GiB. This ISO carries the built closure, so `nixos-install` copies it
from the stick.

- It still needs a network to clone the flake and fetch nixpkgs.
- Build it from the same commit the installer will clone
  (`LOSOS_FLAKE_URL`, `--depth 1`). Otherwise the store paths differ and the
  copies go unused.
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
