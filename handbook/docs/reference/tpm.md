---
title: TPM and the disk key
sidebar_position: 4.5
slug: /reference/tpm
---

# TPM and the disk key

A box without a TPM chip is less secure than one with it. This page explains
why, and how to move a box to a TPM.

## What a TPM is

A TPM is a small security chip on the mainboard. On most recent machines it
is part of the processor's firmware instead. Intel calls it PTT and AMD calls
it fTPM. A machine sold with Windows 11 has one, because Windows 11 requires it.

The chip can lock a secret so that only that same chip can unlock it again.
A locked secret copied off the disk, or onto another machine, is useless.

## Why the box wants one

Your box encrypts the disk that holds your files. It has to unlock that disk
on every start, with nobody there to type a passphrase. So the key must be
somewhere the box can reach by itself.

- **With a TPM**, the key is locked into the chip. The disk on its own,
  pulled out or copied, is unreadable.
- **Without a TPM**, the key sits on the boot partition, which is not
  encrypted. Anyone who gets the disk can read the key and then your files.
  That includes a stolen box, a box sent off for repair, and an old disk sold
  or thrown away without a wipe.

Without a chip, two smaller keys lose their protection too. One guards the
passwords the box makes for itself, the other the space you lend to the mesh.
Both become plain files on the same disk as the data they protect.

Everything on the network side works the same either way.

## What a TPM does not do

The chip hands the key to anything started on that machine. LosOS does this on
purpose, because the box updates itself every night and a stricter chip would
lock it out of its own disk after an update. A thief who takes the whole box
can therefore still get at the data. The [security model](security-model)
says so too.

## How to tell

If the box has no TPM, it says so in three places:

- on the box's own screen, under its address;
- on the Security page of the settings;
- in the setup wizard, once you have chosen your password.

## Moving to a TPM

The unlock mode is chosen when the box is installed. The settings cannot
change it, so the way to a TPM is a reinstall.

1. Copy your files off the box. Reinstalling erases the disk.
2. Turn the TPM on in the machine's firmware setup. Look for "TPM",
   "Security Device", "Intel PTT" or "AMD fTPM", usually under Security or
   Advanced.
3. [Install](../start/install) again. The installer finds the chip and uses
   it.

A box with no private files on it, or one in a virtual machine for testing,
is fine without a TPM.
