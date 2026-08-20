# Project-wide option declarations for losos.
{
  lib,
  pkgs,
  config,
  ...
}:

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

    # ── Master proxy (appliance side) ───────────────────────────────────────
    # Replaces the retired losos.cfd (Cloudflare Tunnel). The appliance dials
    # out to the edge over rathole (no inbound public port) and announces
    # itself to the edge's losos-registrar, which rewrites Traefik + rathole
    # config there. See docs/superpowers/specs/2026-08-17-master-proxy-design.md
    # and modules/proxy.nix. The edge side of the contract is losos.edge.*
    # (below). The on-box Nginx stays the slave proxy on :80.
    proxy.enable = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = ''
        Enable the master-proxy appliance side: a rathole client that dials
        out to the edge (losos.proxy.edgeRatholeEndpoint) and a
        losos-registrar announce service that registers/heartbeats this
        appliance with the edge. The on-box Nginx stays the slave proxy on
        :80; rathole forwards the tunnel to it. No inbound public port is
        opened — the no-SSH/no-public-ports invariant is preserved.
      '';
    };

    proxy.edgeRatholeEndpoint = lib.mkOption {
      type = lib.types.str;
      default = "edge.losos.cfd:2333";
      description = "host:port the rathole client dials (the edge rathole server's [server] bind).";
    };

    proxy.registrarUrl = lib.mkOption {
      type = lib.types.str;
      default = "https://register.losos.cfd";
      description = "Base URL of the edge losos-registrar HTTP API (fronted by Traefik at a static hostname).";
    };

    proxy.hostname = lib.mkOption {
      type = lib.types.str;
      default = "${config.losos.hostName}.losos.cfd";
      defaultText = lib.literalExpression "\${config.losos.hostName}.losos.cfd";
      description = "Public hostname this appliance registers; Traefik routes Host(<hostname>) through the tunnel to this box's Nginx.";
    };

    proxy.applianceId = lib.mkOption {
      type = lib.types.str;
      default = config.losos.hostName;
      defaultText = lib.literalExpression "config.losos.hostName";
      description = "Stable appliance id; the registry key and the rathole service name.";
    };

    proxy.tokenFile = lib.mkOption {
      type = lib.types.path;
      default = "/var/secrets/losos-proxy-token";
      description = ''
        Per-appliance shared secret (0600, persisted via /var). Used both to
        authenticate /register and as the rathole service token. Must match
        losos.edge.tenants.<applianceId>.tokenFile on the edge. Provision out
        of band (agenix or a manual write), not via the nix store.
      '';
    };

    proxy.bootstrapTokenFile = lib.mkOption {
      type = lib.types.path;
      default = "/var/secrets/losos-rathole-bootstrap";
      description = ''
        The rathole default_token (0600, persisted via /var) — the shared
        transport secret. Must match losos.edge.bootstrapTokenFile on the
        edge. Provision out of band, not via the nix store.
      '';
    };

    proxy.heartbeatInterval = lib.mkOption {
      type = lib.types.str;
      default = "30s";
      description = "Cadence losos-registrar announce POSTs /heartbeat. Must be well under losos.edge.heartbeatTtl.";
    };

    proxy.rathole.package = lib.mkOption {
      type = lib.types.package;
      default = pkgs.rathole;
      defaultText = lib.literalExpression "pkgs.rathole";
      description = "rathole derivation for the appliance-side client.";
    };

    proxy.registrar.package = lib.mkOption {
      type = lib.types.nullOr lib.types.package;
      default = null;
      description = ''
        The losos-registrar derivation (Rust). When non-null, the announce
        service runs; wired by modules/defaults.nix to
        self.packages.<system>.losos-registrar. Leave null to run without.
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

    # The Haskell control-plane package holding both executables: the `lososd`
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

    # ── Master proxy (edge side) ───────────────────────────────────────────
    # Options for the edge system (flake output `nixosModules.edge`), which
    # runs Traefik (master proxy), a rathole server, and losos-registrar
    # (serve). Only used by that module; the appliance uses losos.proxy.* above.
    edge.enable = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = "Configure this system as the losos master-proxy edge (Traefik + rathole server + losos-registrar).";
    };

    edge.publicDomain = lib.mkOption {
      type = lib.types.str;
      default = "losos.cfd";
      description = "Apex domain. The static registration router is register.<publicDomain>; per-appliance hostnames live under it.";
    };

    edge.ratholeBindAddr = lib.mkOption {
      type = lib.types.str;
      default = "::";
      description = ''
        Address the rathole server listens on for appliance clients to dial.
        Defaults to `::` (IPv6 any), which on Linux with the default
        `net.ipv6.bindv6only=0` binds dual-stack — so appliances that resolve
        the edge over IPv6 (the common case, and the nixosTest inter-VM path)
        and over IPv4 both reach the tunnel. Serialised bracketed for IPv6
        (`[::]:<port>`) by the `fmtBind` helper in modules/edge.nix and the
        `format_bind` helper in backend-registrar/src/config.rs, which must
        agree byte-for-byte.
      '';
    };

    edge.ratholeBindPort = lib.mkOption {
      type = lib.types.port;
      default = 2333;
      description = "Port appliance rathole clients dial (rathole [server] bind).";
    };

    edge.ratholePortRange = lib.mkOption {
      type = lib.types.str;
      default = "50000-50100";
      description = "lo-hi range the registrar allocates per-appliance rathole edge ports from.";
    };

    edge.heartbeatTtl = lib.mkOption {
      type = lib.types.str;
      default = "120s";
      description = "Tenants with no heartbeat within this TTL are pruned (their Traefik router + rathole service removed).";
    };

    edge.reconcileInterval = lib.mkOption {
      type = lib.types.str;
      default = "15s";
      description = "How often the registrar reconciler re-derives Traefik + rathole config from the registry.";
    };

    edge.registrarApiPort = lib.mkOption {
      type = lib.types.port;
      default = 8443;
      description = "Loopback port the losos-registrar HTTP API listens on; Traefik forwards register.<publicDomain> here.";
    };

    edge.registrarApiBind = lib.mkOption {
      type = lib.types.str;
      default = "127.0.0.1";
      description = ''
        Address the losos-registrar HTTP API binds. Defaults to loopback — in
        production only Traefik (fronting register.<publicDomain>) reaches it.
        Set to `0.0.0.0` only in tests where there is no Traefik/TLS path and
        the appliance VM must dial the registrar directly; never expose it
        publicly in deployment.
      '';
    };

    edge.acmeEmail = lib.mkOption {
      type = lib.types.nullOr lib.types.str;
      default = null;
      description = "Let's Encrypt account email for the on-demand cert resolver. Required when losos.edge.enable.";
    };

    edge.bootstrapTokenFile = lib.mkOption {
      type = lib.types.path;
      default = "/var/secrets/losos-rathole-bootstrap";
      description = "rathole default_token (0600). Shared by all appliance tunnels as the transport Noise bootstrap.";
    };

    edge.tenants = lib.mkOption {
      type = lib.types.attrsOf (lib.types.submodule {
        options = {
          hostname = lib.mkOption {
            type = lib.types.str;
            description = "Public hostname Traefik routes to this appliance.";
          };
          tokenFile = lib.mkOption {
            type = lib.types.path;
            description = ''
              Path to this appliance's token (0600); must match the
              appliance's losos.proxy.tokenFile. Use an agenix/runtime secret
              path, not a store path, so the token stays out of the
              world-readable nix store.
            '';
          };
        };
      });
      default = {};
      description = "Closed-enrollment whitelist of appliances permitted to register. The registrar only ever writes Traefik routers for ids listed here.";
    };

    edge.rathole.package = lib.mkOption {
      type = lib.types.package;
      default = pkgs.rathole;
      defaultText = lib.literalExpression "pkgs.rathole";
      description = "rathole derivation for the edge-side server.";
    };

    edge.registrar.package = lib.mkOption {
      type = lib.types.nullOr lib.types.package;
      default = null;
      description = "The losos-registrar derivation (Rust). Wired by the edge module to self.packages.<system>.losos-registrar.";
    };
  };
}
