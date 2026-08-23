# Declarative NixOS Containers (systemd-nspawn) + the Nginx front router.
#
# Active only when the corresponding losos.*.mode selects the container path:
#   losos.nextcloud.mode == "container" -> the native Nextcloud stack inside
#     containers.nextcloud (10.231.1.2), served under /nextcloud by the front
#     vhost
#   losos.forgejo.mode  == "container" -> services.forgejo inside
#     containers.forgejo (10.231.2.2), served under /forgejo/ (prefix stripped)
# The native service configs shared by both modes live in
# modules/nextcloud-common.nix (Nextcloud) and modules/services.nix (Forgejo),
# gated on the *other* mode value, so you flip the whole deployment back and
# forth by changing one option. (Rootless Podman is gone — the containers run
# the pinned nixpkgs stacks, so the closure is fully flake-pinned and nothing
# is pulled from a registry at runtime.)
#
# Nginx is the single front door: one default_server vhost on :80 with
# path-based routing — / (dashboard), /settings (settings SPA), /api/* (lososd),
# /nextcloud (container), /forgejo/ (container). default_server because
# master-proxy (Traefik+rathole) traffic arrives with a public hostname, not
# <hostName>.local. Nothing but Nginx binds a public port. The admin surface
# (/, /settings, /ds, /common.js, /api) is LAN-only (see `lanOnly` below);
# only /nextcloud and /forgejo are reachable through the master-proxy tunnel.
{
  pkgs,
  lib,
  config,
  ...
}:

let
  nextcloudContainer = config.losos.nextcloud.mode == "container";
  forgejoContainer = config.losos.forgejo.mode == "container";

  # Host values captured for the in-container configs (container modules are a
  # separate NixOS evaluation that cannot see losos.* options).
  hostName = config.losos.hostName;
  nextcloudStack = config.lososInternal.nextcloudStack;
  apachePort = config.losos.nextcloud.apachePort;
  adminpassFile = config.losos.nextcloud.adminpassFile;
  adminUi = config.losos.admin.ui;
  adminApiPort = config.losos.admin.apiPort;

  # Access guard for the admin surface (dashboard, settings SPA, their assets,
  # the lososd API): local network only. Master-proxy traffic must never reach
  # these routes — and it arrives from *loopback* (proxy.nix points rathole at
  # 127.0.0.1:80), so loopback is deliberately not allowed. That costs nothing:
  # the box has no shell logins, so no legitimate client browses from
  # localhost. The vhost only listens on 0.0.0.0, so the IPv4 ranges below are
  # exhaustive; if an IPv6 listener is ever added, these rules fail closed
  # (LAN IPv6 clients get 403) rather than open.
  lanOnly = ''
    allow 10.0.0.0/8;
    allow 172.16.0.0/12;
    allow 192.168.0.0/16;
    allow 169.254.0.0/16;
    deny all;
  '';
in
{
  # ── Nextcloud (nspawn) ─────────────────────────────────────────────────────
  containers.nextcloud = lib.mkIf nextcloudContainer {
    autoStart = true;
    privateNetwork = true;
    hostAddress = "10.231.1.1";
    localAddress = "10.231.1.2";
    # Durable state lives on the host (persisted via impermanence's /var); the
    # container root is disposable. The admin password file is mounted
    # read-only; /dev/dri only when GPU acceleration is on.
    bindMounts = {
      "/var/lib/nextcloud" = {
        hostPath = "/var/lib/nextcloud";
        isReadOnly = false;
      };
      "${toString adminpassFile}" = {
        hostPath = toString adminpassFile;
        isReadOnly = true;
      };
      "/dev/dri" = lib.mkIf config.losos.gpu.enable {
        hostPath = "/dev/dri";
        isReadOnly = false;
      };
    };
    # Convenience loopback forward (Nginx talks to 10.231.1.2 directly);
    # bound to 127.0.0.1 only, so nothing but Nginx holds a public port.
    extraFlags = [ "--port=127.0.0.1:${toString apachePort}:80" ];
    config =
      { ... }:
      {
        system.stateVersion = "26.11";
        networking.firewall.allowedTCPPorts = [ 80 ];
        services.nextcloud = nextcloudStack // {
          # Subpath deployment keys: the front vhost proxies /nextcloud with
          # the path preserved, so the in-container Nextcloud must generate
          # /nextcloud-prefixed URLs. (Kept out of nextcloudStack — in native
          # mode the same stack serves at the vhost root.)
          settings = {
            overwritewebroot = "/nextcloud";
            "htaccess.RewriteBase" = "/nextcloud";
            "overwrite.cli.url" = "http://${hostName}.local/nextcloud";
            # Requests arrive with the appliance's mDNS name (direct) or the
            # master-proxy public hostname (tunnel) — both must be trusted,
            # or Nextcloud rejects tunnel traffic with "Untrusted domain".
            trusted_domains =
              [ "${hostName}.local" ]
              ++ lib.optional config.losos.proxy.enable config.losos.proxy.hostname;
          };
        };
      };
  };

  # ── Forgejo (nspawn) ───────────────────────────────────────────────────────
  containers.forgejo = lib.mkIf forgejoContainer {
    autoStart = true;
    privateNetwork = true;
    hostAddress = "10.231.2.1";
    localAddress = "10.231.2.2";
    bindMounts."/var/lib/forgejo" = {
      hostPath = "/var/lib/forgejo";
      isReadOnly = false;
    };
    config =
      { ... }:
      {
        system.stateVersion = "26.11";
        networking.firewall.allowedTCPPorts = [ 3000 ];
        services.forgejo = {
          enable = true;
          lfs.enable = true;
          database.type = "postgres";
          stateDir = "/var/lib/forgejo";
          settings = {
            server = {
              HTTP_PORT = 3000;
              # The front vhost strips the /forgejo prefix; Forgejo must know
              # it is served under a subpath so it generates prefixed links.
              ROOT_URL = "http://${hostName}.local/forgejo/";
            };
            actions.ENABLED = true;
          };
        };
      };
  };

  # GPU device access for the nextcloud container's unit (replaces the old
  # rootless --device/--group-add propagation).
  systemd.services."container@nextcloud" =
    lib.mkIf (nextcloudContainer && config.losos.gpu.enable)
      {
        serviceConfig.DeviceAllow = [ "char-drm rw" ];
      };

  # ── Nginx front router ────────────────────────────────────────────────────
  # One default_server vhost on :80: dashboard at /, settings SPA at
  # /settings, lososd JSON API at /api/*, and the container routes. The pages
  # reference their assets by absolute path (/ds/losos.css, /settings/app.js), so
  # dashboard/ is the vhost root, settings/ is aliased alongside it, and the
  # shared common.js (admin-ui root, used by both pages) gets an explicit
  # alias.
  services.nginx =
    lib.mkIf (config.losos.admin.enable && adminUi != null)
      {
        enable = true;
        recommendedProxySettings = true;
        virtualHosts."losos-front" = {
          default = true; # default_server — master-proxy traffic arrives with a public hostname
          listen = [
            {
              addr = "0.0.0.0";
              port = 80;
            }
          ];
          root = "${adminUi}/dashboard";
          locations = lib.mkMerge [
            {
              # Static dashboard (index.html at the root). LAN-only, like every
              # admin location below.
              "/" = {
                index = "index.html";
                tryFiles = "$uri $uri/ =404";
                extraConfig = lanOnly;
              };
              # SPA deep link: /settings -> /settings/ so relative-looking
              # absolute asset paths resolve. Unguarded on purpose: `return`
              # runs in the rewrite phase, before allow/deny are consulted, so
              # a guard here would be dead config — and the redirect target is
              # guarded.
              "= /settings" = {
                extraConfig = "return 301 /settings/;";
              };
              "/settings/" = {
                alias = "${adminUi}/settings/";
                index = "index.html";
                tryFiles = "$uri $uri/ =404";
                extraConfig = lanOnly;
              };
              "/ds/" = {
                alias = "${adminUi}/ds/";
                extraConfig = lanOnly;
              };
              # Shared page chrome: both SPAs load these by absolute path,
              # but they live at the admin-ui root, not under dashboard/.
              "= /common.js" = {
                alias = "${adminUi}/common.js";
                extraConfig = lanOnly;
              };
              "/api/" = {
                proxyPass = "http://127.0.0.1:${toString adminApiPort}";
                proxyWebsockets = true;
                extraConfig = lanOnly;
              };
            }
            (lib.mkIf nextcloudContainer {
              # No URI part in proxyPass -> path preserved; the in-container
              # Nextcloud is configured with overwritewebroot = /nextcloud.
              "/nextcloud" = {
                proxyPass = "http://10.231.1.2";
                proxyWebsockets = true;
                extraConfig = ''
                  client_max_body_size 0;
                  proxy_request_buffering off;
                  proxy_read_timeout 86400s;
                  proxy_send_timeout 86400s;
                '';
              };
            })
            (lib.mkIf forgejoContainer {
              # Trailing slash on proxyPass -> /forgejo prefix stripped;
              # Forgejo's ROOT_URL is http://<host>.local/forgejo/ so it
              # generates prefixed links.
              "/forgejo/" = {
                proxyPass = "http://10.231.2.2:3000/";
                proxyWebsockets = true;
              };
            })
          ];
        };
      };

  # ── Firewall ──────────────────────────────────────────────────────────────
  # Nginx owns :80; the containers' private IPs and the loopback forward are
  # not public.
  networking.firewall.allowedTCPPorts =
    lib.optional (config.losos.admin.enable && adminUi != null) 80;
}
