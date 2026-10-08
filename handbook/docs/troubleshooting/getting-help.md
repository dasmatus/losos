---
title: Getting help
sidebar_position: 13
slug: /troubleshooting/getting-help
---

# Getting help

Send problems and questions to the project's
[issue tracker](https://github.com/dasmatus/losos/issues). The box has no
shell, so what you can collect from the LAN is all a maintainer can ask for.
Collect it before you write.

## What to include

![The About pane, whose rows go into a report: name, address, storage mode and mesh state.](../img/settings-about.png)

1. **What you did and what you saw**, with the exact text of any message
   and a screenshot if there was one.
2. **The banner's text**: the address and the `.local` name.
3. **The About pane**: name, address reached at, storage mode, mesh state.
4. **The answer of** `http://<address>/api/health` and
   `http://<address>/setup/state.json`. The second holds no secret, only a
   name, an address and a certificate fingerprint.
5. **The Changes widget's entries** if an Apply or an update is involved:
   the job name and its outcome text.
6. **How the box was installed**: TPM or keyfile, UEFI or BIOS, from which
   release, on which hardware or which VM software. The install printed the
   first two.
7. **When.** The time of day matters because of the 00:07 restart and the
   03:00 update.

## What not to include

Leave out the spare admin key, the owner's password and the contents of your
files. Nobody needs them, and the issue tracker is public.

## If it is urgent

A restart repairs whatever is not on the encrypted volume, whether it comes
at 00:07 or because you pull the power. A **Reset** puts the settings back.
You can do both tonight. A maintainer's answer may take longer.
