# losos

A NixOS appliance turning a mini-PC into a private cloud and a community
storage node: Nextcloud for your files, Tahoe-LAFS contributing spare disk
to a distributed grid, the two users mutually unreadable. No SSH and no
login shell: administration happens in web UIs; upgrades run unattended.

The root filesystem is tmpfs, rebuilt on every boot. Persistent state lives
on an encrypted partition, so a powered-off or stolen box exposes nothing.

## Install

```sh
nix build .#nixosConfigurations.iso.config.system.build.isoImage
```

Boot the image; the unattended installer partitions and encrypts all
attached disks and installs the system. Development shell: `nix develop .#`
