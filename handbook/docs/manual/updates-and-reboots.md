---
title: Updates and reboots
sidebar_position: 6
---

# Updates and reboots

Two timers run on every box. The About pane shows both.

| Time      | What                                                                                                                                     |
| --------- | ---------------------------------------------------------------------------------------------------------------------------------------- |
| **00:07** | **Restart.** Unconditional. The system disk is a blank slate rebuilt from the box's description, so this restart is how the box repairs itself. A box that was off at 00:07 restarts as soon as it is on. |
| **03:00** | **Update.** The box rebuilds itself from its upgrade source. With the default source it only re-applies its own description; with a published source (a `github:` address) it also fetches new software. |
| **04:30** | **Clean-up.** Old versions of the system older than 14 days are deleted, and the boot menu keeps the last five, so the disk and the boot partition do not fill up. |

## What survives

Only what is on a short list: your files and LosOS Git's data, the system's
own store, the settings you chose, the box's keys and identity, the mesh
membership. Everything else is gone at 00:07. The
[reference page](../reference/what-survives-a-reboot) has the list.

## While it works

A box applying an update or a change stays reachable: the apps keep
answering until the moment the new system is switched in, which takes
seconds. An update that fails leaves the previous system running and the
next night tries again.

## Choosing the upgrade source

The upgrade source is a Nix setting (`losos.upgradeFlakeUri`), not on the
panes. The default, the box's own copy of its description, never changes the
software. Pointing it at the project's GitHub repository
(`github:dasmatus/losos#install`) makes the 03:00 run pull new versions. A
company can point it at a fork. Keep the `#install` suffix: without it the
rebuild fails, quietly, every night.

A remote source still uses this box's own disk list, firmware mode, unlock
mode and settings: the box reads those from itself, not from the repository.

## When the box is off

Nothing happens, and nothing is lost. Switch it on and it boots, restarts
if it missed a midnight, and updates at the next 03:00.
