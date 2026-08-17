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
      description = ''
        Enable Cloudflare Tunnel (cloudflared) on the host so the appliance's
        web services are reachable without opening public ports.

        Off by default. Turning it on is incomplete until you also populate
        `losos.cfd.tunnels.<name>` with real tunnel credentials — see that
        option's note on agenix — and `modules/containers.nix` actually passes
        the tunnel attrset to `services.cloudflared.tunnels` (currently a
        stub: `tunnels = {}`). The origin certificate from
        `cloudflared tunnel login` and your account routing are local-dev
        concerns on your workstation (GNOME keyring / `~/.cloudflared/`), not
        part of this appliance repo.
      '';
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
        "container"
      ];
      default = "container";
      description = ''
        "native" — run Nextcloud as a native NixOS service (services.nextcloud)
        on the host.
        "container" — run the same native stack inside a declarative
        systemd-nspawn container (containers.nextcloud, modules/containers.nix),
        fronted by Nginx path routing. This is the default deployment.
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
        "container" — run Forgejo inside a systemd-nspawn container
        (containers.forgejo) behind Nginx path routing.
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

    # Host loopback port the in-container Nextcloud publishes (port 80) on,
    # via containers.nextcloud.forwardPorts. Nginx proxies /nextcloud to the
    # container IP directly; this port is retained for direct local access.
    nextcloud.apachePort = lib.mkOption {
      type = lib.types.port;
      default = 11000;
      description = ''
        Host loopback port forwarded to the Nextcloud container's port 80
        (losos.nextcloud.mode == "container"). Nginx proxies to the container's
        private IP; this forward is a convenience for local debugging.
      '';
    };

    # ── GPU hardware acceleration for the Nextcloud container ──────────────
    gpu.enable = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = ''
        Enable host graphics (VA-API/Mesa) and pass /dev/dri into the Nextcloud
        nspawn container for GPU acceleration.
      '';
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

    # The Haskell control-plane package holding BOTH executables: the `lososd`
    # root daemon (D-Bus org.losos1 on the system bus + loopback Bearer-authed
    # admin HTTP API) and the `losos-ctl` facade CLI relaying to it. When
    # non-null the daemon runs system-wide (see modules/daemon.nix); set to
    # null to run without the control plane.
    backend.package = lib.mkOption {
      type = lib.types.nullOr lib.types.package;
      default = null;
      description = ''
        The losos-ctl/lososd derivation (Haskell). When non-null, lososd is
        enabled as a systemd daemon and losos-ctl is installed for root.
        Leave null to run without a backend.
      '';
    };

    # ── Standalone admin endpoint (lososd HTTP API + static admin UI) ──────
    admin.enable = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = ''
        Serve the standalone losos admin UI (dashboard + settings SPA) on the
        front Nginx vhost and run lososd's loopback JSON API (/api/*).
      '';
    };

    admin.apiPort = lib.mkOption {
      type = lib.types.port;
      default = 8082;
      description = "Loopback port lososd serves the Bearer-authed JSON API on. Nginx proxies /api/ here.";
    };

    admin.tokenFile = lib.mkOption {
      type = lib.types.path;
      default = "/var/secrets/losos-admin-token";
      description = ''
        Bearer token for the admin API; created randomly with mode 0600 by
        lososd on first start if absent. Persisted via /var.
      '';
    };

    # Path of the packaged static admin UI (built from this flake's ./admin-ui
    # by flake/packages.nix; wired in via modules/defaults.nix). Internal.
    admin.ui = lib.mkOption {
      type = lib.types.nullOr lib.types.path;
      default = null;
      internal = true;
      description = "Store path of the static admin UI (dashboard/ + settings/ subdirectories).";
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
                description = ''
                  Path to the Cloudflare tunnel origin certificate
                  (`cert.pem`). Pass an agenix secret's runtime path
                  (`config.age.secrets.<name>.path`) so the credential is not
                  copied world-readable into the Nix store.
                '';
              };

              credentialsFile = lib.mkOption {
                type = lib.types.nullOr lib.types.path;
                default = null;
                description = ''
                  Path to the tunnel credentials JSON
                  (`<tunnel-id>.json`). Use an agenix secret's runtime path
                  (`config.age.secrets.<name>.path`), not a store path, so the
                  tunnel ID + secret stay out of the world-readable store.
                '';
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
