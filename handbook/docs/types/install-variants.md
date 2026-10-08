---
title: Install variants
sidebar_position: 4
---

# Install variants

The installer makes two decisions and you make one. It records
all three in a file on the box, `modules/install-target.nix`. A reinstall
from the same medium repeats them, and a nightly update never undoes them.

## How the disk is unlocked

Every box has an encrypted disk. The installer formats it unattended, from
a random key it generates. The variants differ in where that key lives at
boot.

| Variant                    | When                                                     | What it means                                                                                                                         |
| -------------------------- | -------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------- |
| **TPM** (the default)      | The installer found a TPM 2.0 chip                        | The key is sealed into the chip. The boot partition carries no secret. The disk on its own, pulled or cloned, is unreadable. The box still boots unattended. |
| **Keyfile**                | No chip, or `losos-ctl install --no-tpm`                  | The key sits on the unencrypted boot partition, so the box can boot unattended. Whoever takes the disk has the data. Fine for a VM or a test box. |

The installer prints which one it chose: `unlock: TPM` or `unlock: keyfile
in the initrd`. The TPM key is **not** bound to firmware measurements,
on purpose. The box updates its firmware, bootloader and kernel on its own
and has no shell to recover from a lockout. The cost is that a thief who
takes the whole box, chip included, can still boot it. The
[security model](../reference/security-model) says so plainly.

![Proof from an install in a virtual machine with an emulated TPM: the encrypted volume has a systemd-tpm2 token in key slot 1, and the key file stays on the encrypted volume only.](../img/install-done-tpm.png)

A box never asks for a passphrase at boot. If yours does, something is
wrong; see [Stuck at a passphrase prompt](../troubleshooting/stuck-at-passphrase).

## UEFI or BIOS

This is the only question the installer asks. **UEFI** gets systemd-boot.
**BIOS** gets GRUB and a 1 MiB boot partition on the first disk.
**Autodetect** picks the mode the stick was booted in, and the menu takes it
after 30 seconds, so an unattended boot still installs. UEFI needs the stick
itself booted in UEFI mode. On a BIOS-booted stick the installer refuses UEFI
before it touches any disk. BIOS is mainly for virtual machines. Under UEFI
the stick is signed, and a firmware with the LosOS certificate enrolled
verifies it. The installed box's own loader is not signed, so the box needs
Secure Boot off. See
[Secure Boot and signed media](../start/secure-boot).

![The installer's firmware menu: 1 BIOS, 2 UEFI, 3 Autodetect, chosen automatically after 30 seconds.](../img/installer-menu.png)

## The three media

| Medium                       | Size      | Use                                                                                          |
| ---------------------------- | --------- | -------------------------------------------------------------------------------------------- |
| **Installer ISO**            | small     | The normal one, attached to every release. The target downloads the system it installs.      |
| **ISO with the full system** | large     | Carries the built system, so the install copies instead of downloads. Build it yourself.     |
| **Demo disk image** (QCOW2)  | large     | A preinstalled virtual disk for QEMU or virt-manager. No encryption, no installer. For demos. |

## Growing into the rest of the disk

The installer leaves 10 % of the disk unused on purpose. The box can grow
into that reserve later without anyone opening the case. The Storage pane's **Use the
reserve** button claims it; see [Disk growth](../manual/disk-growth).

![The disk from the outside in: one volume group holding the logical volume, the encryption layer inside it and the filesystem inside that, with a 10 % reserve beside them.](../img/disk-growth.svg)
