---
title: Getting help
sidebar_position: 12
---

# Getting help

Problems and questions go to the project's
[issue tracker](https://github.com/dasmatus/losos/issues). Because the box
has no shell, what you can collect from the LAN is also everything a
maintainer can ask for, so collect it first.

## What to include

1. **What you did and what you saw**, with the exact text of any message
   and a screenshot if there was one.
2. **The banner's text**: the address and the `.local` name.
3. **The About pane**: name, address reached at, storage mode, mesh state.
4. **The answer of** `http://<address>/api/health` and
   `http://<address>/setup/state.json` (the second holds no secret: a name,
   an address and a certificate fingerprint).
5. **The Changes widget's entries** if an Apply or an update is involved:
   the job name and its outcome text.
6. **How the box was installed**: TPM or keyfile, UEFI or BIOS, from which
   release, on which hardware or which VM software. The install printed the
   first two.
7. **When**: the time of day matters, because of the 00:07 restart and the
   03:00 update.

## What not to include

The spare admin key, the owner's password, the contents of your files. Nobody
needs them and the issue tracker is public.

## If it is urgent

The box is designed so that a restart (00:07, or pull the power) repairs
whatever is not on the encrypted volume, and a **Reset** puts the settings
back. Those two are available to you tonight; a maintainer's answer may not
be.
