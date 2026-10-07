---
title: Stuck at a passphrase prompt
sidebar_position: 6
slug: /troubleshooting/stuck-at-passphrase
---

# Stuck at a passphrase prompt

<div className="losos-symptom">

**What you see:** instead of the blue banner, the screen says
`Please enter passphrase for disk persist` (or similar) and waits. The box
never asks for a passphrase on a normal boot.

</div>

## Why it happens

The encrypted volume could not be unlocked the way the box was installed to
unlock it:

| Install              | What went wrong                                                                                                           |
| -------------------- | ------------------------------------------------------------------------------------------------------------------------- |
| **TPM**              | The chip no longer holds, or no longer releases, the key: the firmware's TPM was cleared or reset, the chip was replaced, the disk was moved to another machine, or (in a VM) the emulated TPM was started with a different state directory than at install time. |
| **Keyfile**          | The boot partition lost or damaged the key file, or the disk was partially cloned.                                         |
| **Any, before 2 October 2026** | A bug in older installers formatted the volume with one key and tried to unlock it with another (a stray byte was stripped on the way in). Every such box stopped at this prompt on its first boot. Fixed in pull request #47; install from a current ISO. |

## What you can do

There is no passphrase to type: the key is a random 4096-character string
that was never shown to anyone. The copy kept for recovery lives *inside* the
encrypted volume, so it cannot be used to open that volume.

1. **Power off and on once.** A TPM that failed to answer during a hurried
   boot sometimes answers the next time.
2. **In a VM**, start the emulated TPM with the same state directory used
   at install time, before starting the VM.
3. **On hardware**, check that the firmware's TPM is still enabled (Intel
   PTT or AMD fTPM) and was not cleared by a firmware update or a "reset to
   defaults".
4. Otherwise, **reinstall**. The data on the volume is unreadable without the
   key, which is exactly what the encryption promised; the copy you keep
   elsewhere is where your files are.

## Preventing the next time

Keep another copy of your data. Do not clear the TPM in the firmware. In a
VM, keep the TPM state directory with the disk image. The
[install variants](../types/install-variants#how-the-disk-is-unlocked) page
explains the trade-off the TPM mode makes.
