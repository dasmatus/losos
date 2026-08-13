# Project-wide option declarations for losos.
{ lib, ... }:

{
  options.losos = {
    targetDrives = lib.mkOption {
      type = lib.types.listOf lib.types.str;
      default = [ "/dev/sda" ];
      defaultText = lib.literalExpression ''[ "/dev/sda" ]'';
      description = ''
        Block devices to pool into the LVM volume group with disko. The first
        entry also carries the ESP. One drive is fine (a VG with a single PV);
        list several to merge their capacity into one logical volume.
      '';
    };

    tpm.enable = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = ''
        Use a TPM2 chip to unlock the persistent LUKS partition at boot.
        Disable on machines without TPM2; a random keyfile is used instead.
      '';
    };

    cfd.enable = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = "Enables Cloudflared (CF tunnels). "; # TODO: Route this through my account and secure details both in GNOME keyring and in agenix.
    };

    hostName = lib.mkOption {
      type = lib.types.str;
      default = "mattbox";
      description = "System hostname. Avahi publishes <hostName>.local via mDNS.";
    };

    # sharing's caring btw
    sharingMyStorage = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = "Expose local storage to the Tahoe-LAFS grid as a storage server.";
    };

    # ── Deployment mode switches ──────────────────────────────────────────
    nextcloud.mode = lib.mkOption {
      type = lib.types.enum [
        "native"
        "aio"
      ];
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
      type = lib.types.enum [
        "native"
        "container"
      ];
      default = "container";
      description = ''
        "native" — run Forgejo as a native NixOS service (services.forgejo).
        "container" — run Forgejo as a rootless Podman container behind Nginx.
      '';
    };

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

    # ── Installer (the `losos-ctl install` subcommand) ──────────────────────
    installer.package = lib.mkOption {
      type = lib.types.nullOr lib.types.package;
      default = null;
      description = ''
        The losos-ctl derivation to draw the `losos-install` wrapper from. The
        cabal project builds one `losos-ctl` binary containing both the control
        backend and the `install` subcommand, so this is usually
        `self.packages.<system>.losos-ctl`. Set to null to ship no installer
        binary.
      '';
    };

    installer.autorun = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = ''
        Auto-run `losos-install` as root's login shell on tty1 at boot. This is
        what turns the installer ISO into a set-and-forget reinstall / factory-
        reset medium: insert it, boot, and the installer runs unattended. Leave
        false on a normal target system.
      '';
    };

    # ── Cloudflared (CF tunnels) ───────────────────────────────────────────
    #
    # Mirrors the common shape of:
    #   services.cloudflared.tunnels.<name>.<option>
    cfd.tunnels = lib.mkOption {
      type = lib.types.attrsOf (
        lib.types.submodule (
          { name, ... }: {
            options = {
              certificateFile = lib.mkOption {
                type = lib.types.nullOr lib.types.path;
                default = null;
              };

              credentialsFile = lib.mkOption {
                type = lib.types.nullOr lib.types.path;
                default = null;
              };

              default = lib.mkOption {
                type = lib.types.nullOr lib.types.bool;
                default = null;
              };

              edgeIPVersion = lib.mkOption {
                type = lib.types.nullOr (
                  lib.types.enum [
                    "4"
                    "6"
                  ]
                );
                default = null;
              };

              ingress = lib.mkOption {
                type = lib.types.nullOr (lib.types.listOf lib.types.attrs);
                default = null;
              };

              originRequest = lib.mkOption {
                type = lib.types.nullOr (
                  lib.types.submodule {
                    options = {
                      caPool = lib.mkOption {
                        type = lib.types.nullOr lib.types.path;
                        default = null;
                      };

                      connectTimeout = lib.mkOption {
                        type = lib.types.nullOr lib.types.str;
                        default = null;
                      };

                      disableChunkedEncoding = lib.mkOption {
                        type = lib.types.nullOr lib.types.bool;
                        default = null;
                      };

                      httpHostHeader = lib.mkOption {
                        type = lib.types.nullOr lib.types.str;
                        default = null;
                      };

                      keepAliveConnections = lib.mkOption {
                        type = lib.types.nullOr lib.types.int;
                        default = null;
                      };

                      keepAliveTimeout = lib.mkOption {
                        type = lib.types.nullOr lib.types.str;
                        default = null;
                      };

                      noHappyEyeballs = lib.mkOption {
                        type = lib.types.nullOr lib.types.bool;
                        default = null;
                      };

                      noTLSVerify = lib.mkOption {
                        type = lib.types.nullOr lib.types.bool;
                        default = null;
                      };

                      originServerName = lib.mkOption {
                        type = lib.types.nullOr lib.types.str;
                        default = null;
                      };

                      proxyAddress = lib.mkOption {
                        type = lib.types.nullOr lib.types.str;
                        default = null;
                      };

                      proxyPort = lib.mkOption {
                        type = lib.types.nullOr lib.types.int;
                        default = null;
                      };

                      proxyType = lib.mkOption {
                        type = lib.types.nullOr lib.types.str;
                        default = null;
                      };

                      tcpKeepAlive = lib.mkOption {
                        type = lib.types.nullOr lib.types.str;
                        default = null;
                      };

                      tlsTimeout = lib.mkOption {
                        type = lib.types.nullOr lib.types.str;
                        default = null;
                      };
                    };
                  }
                );
                default = null;
              };

              protocol = lib.mkOption {
                type = lib.types.nullOr (
                  lib.types.enum [
                    "http2"
                    "http"
                    "tcp"
                    "udp"
                  ]
                );
                default = null;
              };

              warp-routing.enabled = lib.mkOption {
                type = lib.types.nullOr lib.types.bool;
                default = null;
              };
            };
          }
        )
      );
      default = { };
    };
  };
}
