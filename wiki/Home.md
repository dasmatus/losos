# losos

losos is a NixOS appliance for a mini-PC. It runs Nextcloud for your own files
and can lend spare disk and CPU to a mesh of other losos boxes. There is no SSH
and no login shell: you administer it from a web page, and it upgrades itself.

The root filesystem is a tmpfs that is rebuilt on every boot. Only directories
listed in `modules/impermanence.nix` survive, on an encrypted `/persist`
partition.

## Pages

- [Install](Install) — installer ISO, Secure Boot, TPM, demo image, growing the disk
- [Administration](Administration) — the admin UI, settings, upgrades and the nightly reboot
- [Mesh](Mesh) — contributing storage and compute to other boxes
- [Master proxy](Master-Proxy) — reaching the box from the internet without opening a port
- [Edge federation](Edge-Federation) — your own edge on the LAN, relayed through the official ones
- [Hardening](Hardening) — the default hardening baseline and the opt-in flags
- [Security model](Security-Model) — what is and is not defended
- [Architecture](Architecture) — how the pieces fit together
- [Development](Development) — dev shell, tests, lock files
- [CI and releases](CI-and-Releases) — CI jobs, release media, the binary cache

## Compared with alternatives

- **A NAS (Synology, QNAP).** Same idea of a box with a web UI. The difference
  is that the root here is discarded on every boot, so an attacker's changes
  last until the next reboot (at 00:07 each night at the latest). You can also
  read and rebuild everything the box runs.
- **A VPS.** The box is in your home and its disk is encrypted; the key is
  sealed to the box's TPM chip (or, on a machine without one, kept on the
  boot partition). The optional [master proxy](Master-Proxy) gives it a
  public address without an inbound port.
- **Plain NixOS.** You could write all of this yourself. Here it is already
  written, and the user isolation, encrypted persistence, disk growth,
  hardening and mesh gating are each tested in a VM.
- **Other self-hosting stacks.** None of them lend capacity to a mesh only
  when the owner's time window is open *and* the box is idle.

## What it is not

- Not a general-purpose server. There is no shell by design.
- Not a backup. Keep copies of `/persist` somewhere else.
- Not finished.
