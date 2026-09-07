# Declarative NixOS Containers (systemd-nspawn) + the Nginx front router.
#
# Active only when the corresponding losos.*.mode selects the container path:
#   losos.nextcloud.mode == "container" -> the native Nextcloud stack inside
#     containers.nextcloud (10.231.1.2), served under /nextcloud by the front
#     vhost
#   losos.forgejo.mode  == "container" && losos.forgejo.enable -> services.forgejo
#     inside containers.forgejo (10.231.2.2), served under /forgejo/ (prefix
#     stripped)
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
#
# The vhost itself is unconditional: it owns the *service* routes, so gating it
# on losos.admin.enable (as it used to be) took Nextcloud and Forgejo offline
# along with the dashboard. Only the admin locations, the document root and the
# admin-only firewall entry follow losos.admin.enable.
{
  pkgs,
  lib,
  config,
  ...
}:

let
  nextcloudContainer = config.losos.nextcloud.mode == "container";
  # losos.forgejo.enable is consulted in *both* modes. It used to be read only
  # by the native path (modules/services.nix), so with the default
  # mode == "container" the option was dead: enable = false still ran the git
  # host and still published /forgejo/ through the tunnel.
  forgejoEnabled = config.losos.forgejo.mode == "container" && config.losos.forgejo.enable;

  # Host values captured for the in-container configs (container modules are a
  # separate NixOS evaluation that cannot see losos.* options).
  hostName = config.losos.hostName;
  nextcloudStack = config.lososInternal.nextcloudStack;
  apachePort = config.losos.nextcloud.apachePort;
  adminpassFile = config.losos.nextcloud.adminpassFile;
  adminUi = config.losos.admin.ui;
  adminApiPort = config.losos.admin.apiPort;
  adminEnabled = config.losos.admin.enable && adminUi != null;

  proxied = config.losos.proxy.enable;

  # The host end of each container's veth. Nginx connects to the container
  # from this address, so it is what the backend sees as the client.
  nextcloudHostAddress = "10.231.1.1";
  forgejoHostAddress = "10.231.2.1";
  containerSubnet = "10.231.0.0/16";

  # Access guard for the admin surface (dashboard, settings SPA, their assets,
  # the lososd API): local network only. Master-proxy traffic must never reach
  # these routes — and it arrives from *loopback* (proxy.nix points rathole at
  # 127.0.0.1:80), so loopback is deliberately not allowed. That costs nothing:
  # the box has no shell logins, so no legitimate client browses from
  # localhost. The vhost only listens on 0.0.0.0, so the IPv4 ranges below are
  # exhaustive; if an IPv6 listener is ever added, these rules fail closed
  # (LAN IPv6 clients get 403) rather than open.
  #
  # The container subnet is denied *first* (nginx takes the first matching
  # rule): it sits inside 10.0.0.0/8, so allowing RFC1918 wholesale handed the
  # root-equivalent /api to the two most internet-exposed processes on the box.
  # A PHP RCE in Nextcloud reaches Nginx on 0.0.0.0:80 from 10.231.1.2 and
  # would otherwise pass the guard. Link-local (169.254/16) is not allowed at
  # all — a self-assigned address is not a trust signal on an admin plane.
  # (A LAN that genuinely numbers itself inside 10.231.0.0/16 loses admin
  # access; renumber the containers here if you run one.)
  lanOnly = ''
    deny ${containerSubnet};
    allow 10.0.0.0/8;
    allow 172.16.0.0/12;
    allow 192.168.0.0/16;
    deny all;
  '';

  # ── Security response headers ─────────────────────────────────────────────
  # The admin UI, /api/, /nextcloud and /forgejo/ share one origin by design,
  # so a strict CSP is the only thing standing between an injected script on
  # the admin pages and the Bearer token the SPA keeps in sessionStorage.
  #
  # It cannot be applied to Nextcloud or Forgejo: both ship their own CSP and
  # both need inline script, and `add_header` at server level is inherited by
  # every location that does not set one of its own — including the two proxy
  # locations. So the values are selected by a `map` on $uri and the two
  # service prefixes map to the empty string; nginx omits a header whose value
  # is empty. Fail-closed on purpose: the default arm is the strict policy, so
  # a new admin route is covered without anyone remembering to add it.
  #
  # (Chosen over per-location `add_header` because that form *replaces* the
  # inherited set, so any header added at server level later would silently
  # vanish from exactly the locations that need it most.)
  #
  # Two policy constraints come from the UI itself:
  #   * admin-ui/design-system/losos.css inlines the select chevron as a
  #     data: URI, so img-src needs `data:` (a CSS background-image is an
  #     img-src fetch).
  #   * admin-ui/dashboard/app.js probes the Tahoe WUI cross-origin at
  #     <host>:3456, so connect-src must carry that origin. The hostname is
  #     whatever the client used (mDNS name, LAN IP, or the tunnel hostname),
  #     so it is spelled with nginx's $host — which nginx validates as a
  #     hostname, and which is exactly what the page's location.hostname is.
  adminCsp = lib.concatStringsSep "; " [
    "default-src 'none'"
    "script-src 'self'"
    "style-src 'self'"
    "img-src 'self' data:"
    "font-src 'self'"
    "connect-src 'self' http://$host:3456 https://$host:3456"
    "form-action 'none'"
    "frame-ancestors 'none'"
    "base-uri 'none'"
  ];

  # One map per header: value on the admin surface, empty (header omitted) on
  # the two proxied service routes.
  adminHeaderMap = variable: value: ''
    map $uri ${variable} {
        default        "${value}";
        ~^/nextcloud   "";
        ~^/forgejo     "";
    }
  '';

  adminHeaders = [
    {
      name = "Content-Security-Policy";
      variable = "$losos_csp";
      value = adminCsp;
    }
    {
      name = "X-Frame-Options";
      variable = "$losos_frame_options";
      value = "DENY";
    }
    {
      name = "X-Content-Type-Options";
      variable = "$losos_content_type_options";
      value = "nosniff";
    }
    {
      name = "Referrer-Policy";
      variable = "$losos_referrer_policy";
      value = "no-referrer";
    }
  ];

  adminHeaderMaps = lib.concatMapStrings (h: adminHeaderMap h.variable h.value) adminHeaders;

  # `always` so the headers ride on the 403s the lanOnly guard emits too.
  adminHeaderDirectives = lib.concatMapStrings (
    h: "add_header ${h.name} ${h.variable} always;\n"
  ) adminHeaders;
in
{
  assertions = [
    {
      assertion = config.losos.admin.enable -> config.losos.admin.ui != null;
      message = ''
        losos.admin.enable is on but losos.admin.ui is null, so there is no
        admin SPA to serve and the dashboard/settings routes would 404.
        modules/defaults.nix wires losos.admin.ui to the flake's
        losos-admin-ui package; either restore that or set
        losos.admin.enable = false.
      '';
    }
  ];

  # ── Nextcloud (nspawn) ─────────────────────────────────────────────────────
  containers.nextcloud = lib.mkIf nextcloudContainer {
    autoStart = true;
    privateNetwork = true;
    hostAddress = nextcloudHostAddress;
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
            "overwrite.cli.url" =
              if proxied then
                "https://${config.losos.proxy.hostname}/nextcloud"
              else
                "http://${hostName}.local/nextcloud";
            # Requests arrive with the appliance's mDNS name (direct) or the
            # master-proxy public hostname (tunnel) — both must be trusted,
            # or Nextcloud rejects tunnel traffic with "Untrusted domain".
            trusted_domains =
              [ "${hostName}.local" ]
              ++ lib.optional proxied config.losos.proxy.hostname;
            # Every request is proxied by the host's Nginx, which reaches the
            # container from the host end of the veth. Without this Nextcloud
            # sees that one address as the client for *all* traffic, so the
            # brute-force throttle and per-IP blocking protect nothing and the
            # audit log records a single source.
            trusted_proxies = [ nextcloudHostAddress ];
          }
          # Behind the master proxy, Traefik terminates TLS and the container
          # is reached over plain HTTP. Without these Nextcloud derives http://
          # absolute URLs and embeds them in an https:// page — mixed content,
          # blocked by browsers, and clients redirected back to http.
          // lib.optionalAttrs proxied {
            overwriteprotocol = "https";
            overwritehost = config.losos.proxy.hostname;
          };
        };
      };
  };

  # ── Forgejo (nspawn) ───────────────────────────────────────────────────────
  containers.forgejo = lib.mkIf forgejoEnabled {
    autoStart = true;
    privateNetwork = true;
    hostAddress = forgejoHostAddress;
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
              ROOT_URL =
                if proxied then
                  "https://${config.losos.proxy.hostname}/forgejo/"
                else
                  "http://${hostName}.local/forgejo/";
            };
            service = {
              # /forgejo/ is the one admin-free route published through the
              # master-proxy tunnel, and Forgejo's default is open sign-up.
              # Without this anyone on the internet could create an account and
              # push to this box. Accounts are made by the operator with
              # `forgejo admin user create` inside the container.
              DISABLE_REGISTRATION = true;
            };
            security = {
              # Close the first-run installer page. It is normally locked by
              # completing the wizard, but a declarative deployment never runs
              # it — leaving /forgejo/install reachable, and that page rewrites
              # the database and admin credentials.
              INSTALL_LOCK = true;
            };
            # Actions is remote code execution by design and this appliance
            # registers no runner (there is no shell and no runner unit), so
            # enabling it on an internet-reachable route buys nothing and
            # exposes the runner-registration API. Flip to true together with
            # an actual runner. (Native mode, which is LAN-only, keeps it on —
            # see modules/services.nix.)
            actions.ENABLED = false;
          };
        };
      };
  };

  # ── Container egress ──────────────────────────────────────────────────────
  # nixpkgs' container module already gives each container an address, a
  # default route via the host end of the veth, and a copy of the host's
  # /etc/resolv.conf — but nothing masquerades the private 10.231.0.0/16
  # source addresses, so without NAT the containers have a gateway, DNS and no
  # reachable internet.
  #
  # Egress is deliberate here, not incidental: nextcloud-common.nix ships
  # appstoreEnable = true (so the admin can install the heavy apps on demand)
  # plus apps that fetch at runtime, and Nextcloud's update check, federation
  # and outbound mail all dial out. Isolating the containers instead would mean
  # turning those off, which is not what this deployment wants.
  #
  # externalInterface stays null on purpose: this is a repurposed mini-PC whose
  # NIC name is not known at build time (same reason avahi no longer pins an
  # interface list), so the masquerade rule matches whichever interface carries
  # the default route.
  networking.nat = lib.mkIf (nextcloudContainer || forgejoEnabled) {
    enable = true;
    internalInterfaces = [ "ve-+" ];
  };

  # nspawn bind-mounts the *source* paths below into the containers, and it
  # does not create them: on a fresh /persist neither exists, so
  # container@nextcloud / container@forgejo fail to start at all. Mode and
  # owner are left as `-` so tmpfiles creates them if missing and never
  # rewrites the ownership the service inside the container sets afterwards.
  systemd.tmpfiles.rules =
    lib.optional nextcloudContainer "d /var/lib/nextcloud - - - -"
    ++ lib.optional forgejoEnabled "d /var/lib/forgejo - - - -";

  # GPU device access for the nextcloud container's unit (replaces the old
  # rootless --device/--group-add propagation), plus the ordering that makes
  # the admin-password bind source exist before nspawn tries to mount it
  # (the generator lives in modules/nextcloud-common.nix).
  systemd.services."container@nextcloud" = lib.mkIf nextcloudContainer {
    after = [ "losos-nextcloud-adminpass.service" ];
    requires = [ "losos-nextcloud-adminpass.service" ];
    serviceConfig.DeviceAllow = lib.mkIf config.losos.gpu.enable [ "char-drm rw" ];
  };

  # ── Nginx front router ────────────────────────────────────────────────────
  # One default_server vhost on :80: dashboard at /, settings SPA at
  # /settings, lososd JSON API at /api/*, and the container routes. The pages
  # reference their assets by absolute path (/ds/losos.css, /settings/app.js), so
  # dashboard/ is the vhost root, settings/ is aliased alongside it, and the
  # shared common.js (admin-ui root, used by both pages) gets an explicit
  # alias.
  services.nginx = {
    enable = true;
    recommendedProxySettings = true;

    # Header value selectors — see `adminCsp` above for why they are maps.
    appendHttpConfig = adminHeaderMaps;

    virtualHosts."losos-front" = {
      default = true; # default_server — master-proxy traffic arrives with a public hostname
      listen = [
        {
          addr = "0.0.0.0";
          port = 80;
        }
      ];
      root = lib.mkIf adminEnabled "${adminUi}/dashboard";
      extraConfig = adminHeaderDirectives;
      locations = lib.mkMerge [
        (lib.mkIf adminEnabled {
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
        })
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
        (lib.mkIf forgejoEnabled {
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
  # not public. The port is opened whenever the front vhost has any route to
  # serve — the admin SPA *or* either container — because gating it on the
  # admin flag alone took the service routes down with the dashboard.
  networking.firewall.allowedTCPPorts = lib.optional (
    adminEnabled || nextcloudContainer || forgejoEnabled
  ) 80;
}
