---
title: TPM and the disk key
sidebar_position: 4.5
slug: /reference/tpm
---

# TPM and the disk key

A box without a TPM chip is less secure than one with it. This page says what
a TPM is, what LosOS uses it for, and what a box without one gives up.

## What a TPM is

A TPM (Trusted Platform Module) is a small security chip on the mainboard. On
most recent machines it is part of the processor's firmware instead of a
separate chip. Intel calls that PTT and AMD calls it fTPM. Windows 11 requires
TPM 2.0, so a machine sold with Windows 11 has one. Older mini-PCs often have
one too, sometimes switched off in the firmware setup.

The chip holds a key of its own that never leaves it. Software can hand the
chip a secret to seal, and only that same chip can unseal it again. A sealed
secret copied to another machine, or read off the disk, is useless there.

That is what an unattended box needs. It has to unlock its encrypted disk at
every boot with nobody at the keyboard to type a passphrase. The key has to be
somewhere the box can reach on its own, and the TPM is the one place that is
not the disk itself.

## What LosOS uses it for

The data volume (`/persist`) is LUKS-encrypted on every box. The installer
makes a random key, formats the volume with it, and then seals the key to the
TPM with `systemd-cryptenroll`. At boot the initrd asks the chip for the key
and opens the volume. The boot partition carries no secret.

The box seals two smaller secrets to the chip as well:

- The secrets the box makes for itself, such as Nextcloud's first admin
  password (`modules/keyring.nix`, through `systemd-creds`).
- The key that encrypts the mesh's storage directory (`modules/fscrypt.nix`).

A copy of the disk key also stays inside the encrypted volume at
`/etc/keys/persist-keyfile`. `losos-ctl grow` uses it to resize the volume.
It cannot open the volume from outside, because it is inside it.

## What a box without a TPM gives up

The installer still encrypts the volume the same way. What changes is where
the key lives at boot. Without a chip, the installer puts the key into the
initrd as `/crypto_keyfile.bin`. The initrd lives on the boot partition (the
ESP), and the boot partition is not encrypted. The key sits next to the lock.

That has two consequences.

1. Anyone who gets the disk can read everything on it. That covers a disk
   pulled out of the box, a stolen box, a disk image, a box sent off for
   repair, and an old disk sold or thrown away without a wipe. In each case
   the reader mounts the ESP, copies the key out of the initrd and opens the
   volume. That needs
   ordinary Linux tools and a few minutes. It exposes the owner's files in
   LosOS cloud, the repositories in LosOS Git, the admin key and the rest of
   `/persist`.
2. The box's own secrets lose their chip. The keyring falls back to
   `systemd-creds`' host key, and the mesh storage key to a plain file at
   `/var/secrets/losos-shared-key`. Both sit on the same volume as the data
   they protect. A copy of `/persist` that leaves the box takes the keys with
   it, and a bug that lets a service read any file as root reads both halves.

The network side does not change. The LAN guard, the sign-in, the tunnel,
the hardening and the separation of the two accounts work the same with or
without a chip.

## What a TPM does not protect against

LosOS seals the key without binding it to firmware measurements (PCRs). The
box updates its bootloader and kernel every night with nobody watching. A key
bound to those measurements would lock the box out of its own disk after an
update, and the box has no shell to recover from that.

So the chip hands the key to anything booted on that machine, including a
thief's USB stick. A TPM keeps the disk on its own unreadable. It does not
help once someone has the whole box. The [security model](security-model.md)
lists this as a known limitation.

## How to tell which mode a box is in

- The installer prints `unlock: TPM2` or `unlock: keyfile in the initrd`, and
  on the keyfile path a warning just before it finishes.
- The box's screen shows a warning under its address.
- The admin pages show a warning on Settings, Security, and in the setup
  wizard once the password is set.
- Settings, Advanced lists `tpm.enable` and the value the box runs.

## Moving a box to a TPM

The installer writes the unlock mode into `modules/install-target.nix`. The
admin pages cannot change it, because a line for it in `overrides.nix` would
collide with the installer's. The way to a TPM is a reinstall.

1. Copy your files off the box. The reinstall formats the disk.
2. Turn the TPM on in the firmware setup. Look for "TPM", "Security Device",
   "Intel PTT" or "AMD fTPM", usually under Security or Advanced.
3. Reinstall from the [installer ISO](../start/install.md). The installer finds the chip
   and uses it. `losos-ctl install --tpm` makes a missing chip an error
   instead of a quiet keyfile install.

A VM needs an emulated chip at install time and at every boot after it. The
swtpm lines are in [Install in depth](/in-depth/install.md#try-it-in-a-vm-bios). Keep the swtpm
state directory with the disk image, or the VM cannot unlock its disk.

Keyfile mode is fine for a VM, a test box, or a box that holds nothing
private. For an owner's real files on a box that could ever leave the house,
use the TPM.
