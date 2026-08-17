# Impermanence: /persist (encrypted, see disko.nix) holds all durable state;
# the root is tmpfs and is rebuilt every boot. Directories listed here are
# bind-mounted from /persist back onto the volatile root.
{ config, ... }:

{
  environment.persistence."/persist" = {
    hideMounts = true;

    directories = [
      "/nix" # the nix store, so packages survive reboot
      "/var" # service state: postgres, nextcloud, tahoe-lafs, nextcloud-aio data, etc.
      "/etc/ssh" # host keys
      "/etc/keys" # LUKS keyfile source, so auto-upgrade rebuilds can re-bake the initrd
      "/etc/nixos" # the flake source, so auto-upgrade can rebuild without a remote
      "/home/notshared/data"
      "/home/shared/data"
      # The nspawn containers' state is all under /var/lib/{nextcloud,forgejo},
      # covered by the whole-/var bind mount above; lososd's
      # /var/lib/losos/state.json and the admin token at /var/secrets likewise.
    ];

    files = [
      "/etc/machine-id"
    ];
  };
}