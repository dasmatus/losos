# losos

A stateless NixOS appliance for repurposed mini-PCs. The root is tmpfs,
rebuilt fresh every boot; everything durable lives on an encrypted `/persist`.
Two isolated users get one service each: Nextcloud for private files,
Tahoe-LAFS for shared grid storage. No SSH, no shell logins — the box is
administered entirely through the service web UIs.

## Install

```sh
nix build .#nixosConfigurations.iso.config.system.build.isoImage
```

Write the ISO to USB and boot; `losos-install` runs automatically, merging
every fixed disk into one LUKS + btrfs pool. TPM2 machines unlock themselves;
without one, a keyfile at `/etc/keys/persist-keyfile` is used — back it up.

Develop in `nix develop .#`. Architecture, options, and runbook: `CLAUDE.md`.
