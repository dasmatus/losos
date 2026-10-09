---
title: In depth
sidebar_position: 0
sidebar_label: Overview
slug: /in-depth
mdx:
  format: md
---

# In depth

The rest of this handbook is for the owner of a box. This chapter is for the
people who build LosOS, run an edge or cut a release: how the pieces fit
together, why each one sits where it does, and how to change them.

LosOS is a NixOS appliance for a mini-PC. It runs Nextcloud for your own files
and can lend spare disk and CPU to a mesh of other LosOS boxes. There is no SSH
and no login shell. You administer it from a web page, and it upgrades itself.

The root filesystem is a tmpfs that is rebuilt on every boot. Only the
directories listed in `modules/impermanence.nix` survive, on an encrypted
`/persist` partition.

## Pages

- [Install in depth](install.md). The installer ISO, Secure Boot, TPM, the demo image,
  growing the disk.
- [Administration](administration.md). The admin UI, settings, upgrades and the
  nightly reboot.
- [Mesh](mesh.md). Contributing storage and compute to other boxes.
- [Market](market.md). Selling the disk and CPU a box already shares
  (planned).
- [Virtual machines](virtual-machines.md). Renting machines on the mesh and
  hosting other boxes' machines.
- [Master proxy](master-proxy.md). Reaching the box from the internet without
  opening a port.
- [Edge federation](edge-federation.md). Your own edge on the LAN, relayed through the official ones.
- [Lab](lab.md). The setup visualiser at `/lab/`, its simulated and real
  guests.
- [Hardening](hardening.md). The default hardening baseline and the opt-in flags.
- [TPM and the disk key](../reference/tpm.md). What the chip does, and what a box without
  one gives up.
- [Security model](../reference/security-model.md). What is and is not defended.
- [Architecture](architecture.md). How the pieces fit together.

## Compared with alternatives

- **A NAS (Synology, QNAP).** Same idea, a box with a web UI. The difference
  is that the root here is discarded on every boot, so an attacker's changes
  last until the next reboot, which comes at 00:07 each night at the latest.
  You can also read and rebuild everything the box runs.
- **A VPS.** The box is in your home and its disk is encrypted. The key is
  sealed to the box's TPM chip, or kept on the boot partition on a machine
  without one. The optional [master proxy](master-proxy.md) gives it a public
  address without an inbound port.
- **Plain NixOS.** You could write all of this yourself. Here it is already
  written, and the user isolation, encrypted persistence, disk growth,
  hardening and mesh gating each have a VM test.
- **Other self-hosting stacks.** None of them lend capacity to a mesh only
  while the owner's time window is open *and* the box is idle.

## What it is not

- Not a general-purpose server. There is no shell by design.
- Not a backup. Keep copies of `/persist` somewhere else.
- Not finished.
