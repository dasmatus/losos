# Impermanence: /persist (encrypted, see disko.nix) holds all durable state;
# the root is tmpfs and is rebuilt every boot. Directories listed here are
# bind-mounted from /persist back onto the volatile root.
{ ... }:

{
  environment.persistence."/persist" = {
    hideMounts = true;

    directories = [
      "/nix" # the nix store, so packages survive reboot
      "/var" # service state: postgres, nextcloud, tahoe-lafs, etc.
      "/etc/ssh" # host keys
      "/etc/keys" # LUKS keyfile source, so auto-upgrade rebuilds can re-bake the initrd
      "/etc/nixos" # the flake source, so auto-upgrade can rebuild without a remote
      "/home/notshared/data"
      "/home/shared/data"
    ];

    files = [
      "/etc/machine-id"
    ];
  };
}