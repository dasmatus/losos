---
title: What survives a reboot
sidebar_position: 2
---

# What survives a reboot

The system disk is a blank slate rebuilt at every boot. Only these
directories are kept, on the encrypted volume, and bind-mounted back into
place:

| Kept                       | Holds                                                                 |
| -------------------------- | --------------------------------------------------------------------- |
| `/home/notshared`          | LosOS cloud's data: your files, photos, database                      |
| `/home/shared`             | Room lent to the mesh (encrypted separately, unreadable while sharing is off) |
| `/var`                     | LosOS Git's data, the box's state and secrets, the two clusters' state, logs |
| `/nix`                     | The system itself, every version kept for the boot menu               |
| `/etc/nixos`               | The box's own description: its disks, firmware mode, unlock mode, your settings |
| `/etc/keys`                | The disk key's recovery copy                                          |
| `/etc/ssh`                 | Host keys (there is no SSH server; the keys identify the box)         |
| `/etc/rancher`             | The mesh membership's node password                                   |
| `/etc/machine-id`          | The box's identity                                                    |

![The system disk lives in RAM and is rebuilt every boot; nine directories are bind-mounted back from the encrypted /persist volume, and everything else is gone at the 00:07 restart.](../img/what-survives.svg)

Everything else, including anything an attacker or a bug wrote anywhere
else, is gone at 00:07.

## Two things that are documents, not settings

The **Look** (background, veil, hand-written widgets) and the board layout
are kept under `/var` and in your browser respectively, so both survive, but
neither is part of the box's description and neither needs a rebuild to
change.

## What a reinstall does

Wipes all of it. See [Reset and reinstall](../manual/reset-and-reinstall).
