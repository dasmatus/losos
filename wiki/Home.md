# losos

losos is a NixOS appliance for a mini-PC. It runs Nextcloud for your own files
and can lend spare disk and CPU to a mesh of other losos boxes. There is no SSH
and no login shell. You administer it from a web page, and it upgrades itself.

The root filesystem is a tmpfs that is rebuilt on every boot. Only the
directories listed in `modules/impermanence.nix` survive, on an encrypted
`/persist` partition.

The owner's manual is the [handbook](https://losos.dasmat.us) (`handbook/` in
the repository). Every box also serves it at `http://<address>/handbook/`, so
it can be read from the LAN alone. The pages below are for people who build
and run the project.

## Pages

- [Install](Install). The installer ISO, Secure Boot, TPM, the demo image,
  growing the disk.
- [Administration](Administration). The admin UI, settings, upgrades and the
  nightly reboot.
- [Mesh](Mesh). Contributing storage and compute to other boxes.
- [Master proxy](Master-Proxy). Reaching the box from the internet without
  opening a port.
- [Edge federation](Edge-Federation). Your own edge on the LAN, relayed through the official ones.
- [Hardening](Hardening). The default hardening baseline and the opt-in flags.
- [Security model](Security-Model). What is and is not defended.
- [Architecture](Architecture). How the pieces fit together.
- [Development](Development). The dev shell, tests, lock files.
- [CI and releases](CI-and-Releases). CI jobs, release media, the binary cache.

## Compared with alternatives

- **A NAS (Synology, QNAP).** Same idea, a box with a web UI. The difference
  is that the root here is discarded on every boot, so an attacker's changes
  last until the next reboot, which comes at 00:07 each night at the latest.
  You can also read and rebuild everything the box runs.
- **A VPS.** The box is in your home and its disk is encrypted. The key is
  sealed to the box's TPM chip, or kept on the boot partition on a machine
  without one. The optional [master proxy](Master-Proxy) gives it a public
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
