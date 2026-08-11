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

    # System hostname (the bare label; Avahi publishes <hostName>.local on the
    # LAN). Routed through an option so the losos admin app can set it via the
    # override file; networking.hostName reads from here.
    hostName = lib.mkOption {
      type = lib.types.str;
      default = "mattbox";
      description = "System hostname. Avahi publishes <hostName>.local via mDNS.";
    };

    # Original "share my storage" flag from defaults.nix, now a real option.
    sharingMyStorage = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = "Expose local storage to the Tahoe-LAFS grid as a storage server.";
    };

    # ── Deployment mode switches ──────────────────────────────────────────
    # The appliance can run its Nextcloud and Forgejo either as native NixOS
    # services (the original losos design — the losos web-UI toggle + losos-ctl
    # sudoers bridge depend on native Nextcloud) or as rootless Podman
    # containers fronted by Nginx (the AIO path). The two never collide: each
    # module gates itself on its mode flag, so flipping one option switches the
    # whole deployment back and forth. Defaults reflect the AIO/container
    # pivot; set back to "native" to restore the original appliance behaviour.
    nextcloud.mode = lib.mkOption {
      type = lib.types.enum [ "native" "aio" ];
      default = "aio";
      description = ''
        "native" — run Nextcloud as a native NixOS service (services.nextcloud),
        with the losos admin plugin + losos-ctl sudoers bridge.
        "aio" — run Nextcloud as Nextcloud All-in-One in a rootless Podman
        container (nextcloud/all-in-one master container), fronted by Nginx.
        See modules/containers.nix.
      '';
    };

    forgejo.mode = lib.mkOption {
      type = lib.types.enum [ "native" "container" ];
      default = "container";
      description = ''
        "native" — run Forgejo as a native NixOS service (services.forgejo).
        "container" — run Forgejo as a rootless Podman container behind Nginx.
      '';
    };
    # Whether to enable or disable the local native Forgejo instance (only
    # consulted when losos.forgejo.mode == "native").
    forgejo.enable = lib.mkOption {
      type = lib.types.bool;
      default = false;
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

    # ── Rootless Podman container runtime ──────────────────────────────────
    containers.user = lib.mkOption {
      type = lib.types.str;
      default = "containers";
      description = "The unprivileged user that owns the rootless Podman runtime and its containers.";
    };

    containers.uid = lib.mkOption {
      type = lib.types.ints.u16;
      default = 1002;
      description = "uid of the rootless Podman user. Its home holds all image/volume storage and is persisted.";
    };

    # ── Nextcloud All-in-One (active when losos.nextcloud.mode == "aio") ───
    aio.apachePort = lib.mkOption {
      type = lib.types.port;
      default = 11000;
      description = "Host port the AIO Apache (the actual Nextcloud) publishes. Nginx proxies here. Must be >1024 for rootless.";
    };

    aio.interfacePort = lib.mkOption {
      type = lib.types.port;
      default = 8000;
      description = "Host port the AIO management interface listens on (used for the one-time initial setup: domain, TLS, master password). LAN-only.";
    };

    aio.datadir = lib.mkOption {
      type = lib.types.path;
      default = "/var/lib/nextcloud-aio/data";
      description = "Host path for AIO Nextcloud user data. Persisted via /var.";
    };

    # ── GPU hardware acceleration for the Nextcloud (AIO) container ────────
    # Enables host VA-API/Mesa graphics + the render/video group memberships the
    # rootless `containers` user needs to reach /dev/dri, and passes /dev/dri
    # into the AIO master container. NOTE: AIO's master spawns the actual
    # Nextcloud container itself; whether the GPU reaches it depends on AIO
    # propagating the device to its sibling containers — verify on first
    # deploy and adjust the AIO master run args (containers.nix) if needed.
    gpu.enable = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = "Enable host graphics (VA-API/Mesa) and pass /dev/dri into the AIO container for GPU acceleration.";
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
