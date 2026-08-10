# Project-wide option declarations for losos.
#
# `tgt_drive.nix` previously tried to declare an option by writing
# `config.drive = lib.mkOption { ... }` *inside* a `config` block, which is
# invalid — options must live under `options`, not `config`. This file replaces
# it with a proper `losos.*` option namespace used by the other modules.
{ lib, ... }:

{
  options.losos = {
    # Block device disko should partition. Override per host if the install
    # target is not /dev/sda (e.g. /dev/nvme0n1).
    targetDrive = lib.mkOption {
      type = lib.types.str;
      default = "/dev/sda";
      description = "Block device to partition with the disko layout.";
    };

    # TPM-vs-keyfile switch for unlocking the encrypted /persist partition.
    # Set false on hardware without a TPM2 chip; a keyfile is used instead.
    tpm.enable = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = ''
        Use a TPM2 chip to unlock the persistent LUKS partition at boot.
        Disable on machines without TPM2; a random keyfile is used instead.
      '';
    };

    # Original "share my storage" flag from defaults.nix, now a real option.
    sharingMyStorage = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = "Expose local storage to the Tahoe-LAFS grid as a storage server.";
    };

    nextcloud.hostName = lib.mkOption {
      type = lib.types.str;
      default = "localhost";
      description = "Hostname Nextcloud is served on (set to your domain for remote access).";
    };

    nextcloud.adminpassFile = lib.mkOption {
      type = lib.types.path;
      default = "/var/secrets/nextcloud-admin-pass";
      description = "Path to the file holding the Nextcloud admin password (created on first install). Persisted via /var.";
    };

    nextcloud.https = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = "Serve Nextcloud over HTTPS (requires a certificate / domain in production).";
    };

    tahoe.introducerFurl = lib.mkOption {
      type = lib.types.nullOr lib.types.str;
      default = null;
      description = ''
        Paste the introducer.furl printed by the local introducer after its
        first start. Leave null until then; the storage node won't connect
        to a grid until it is set.
      '';
    };

    upgradeFlakeUri = lib.mkOption {
      type = lib.types.str;
      default = "git+file:///etc/nixos";
      description = "Flake URI system.autoUpgrade rebuilds from. Use a github: URI for remote auto-updates.";
    };

    # The Haskell `losos-ctl` backend the Nextcloud app talks to (via sudo).
    # The user writes/implements it; leave null until then. When set, services.nix
    # installs it system-wide and grants the `nextcloud` user NOPASSWD sudo for
    # exactly that binary, so the PHP app can trigger privileged rebuilds with
    # no shell login and no broader root.
    backend.package = lib.mkOption {
      type = lib.types.nullOr lib.types.package;
      default = null;
      description = ''
        The losos-ctl backend derivation (Haskell). When non-null it is added to
        environment.systemPackages and a sudoers rule lets the `nextcloud` user
        run it as root with no password. The Nextcloud app's BackendService
        invokes it at /run/current-system/sw/bin/losos-ctl.
        Leave null to run without a backend (the app then reports "backend not
        installed").
      '';
    };
  };
}