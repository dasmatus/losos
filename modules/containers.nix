# The Nginx front door.
#
# One default_server vhost on :80 carries everything the appliance publishes:
# the dashboard at /, the settings SPA at /settings, lososd's JSON API at
# /api/*, and the two service routes /nextcloud and /forgejo/. default_server
# because master-proxy (Traefik+rathole) traffic arrives with a public
# hostname, not <hostName>.local. Nothing but Nginx binds a public port. The
# admin surface (/, /settings, /ds, /common.js, /api) is LAN-only (see
# `lanOnly` below); only /nextcloud and /forgejo are reachable through the
# master-proxy tunnel.
#
# This file used to *define* the two services as well, as systemd-nspawn
# containers on private veths (10.231.1.2 and 10.231.2.2) with NAT for egress.
# All of that is gone. `losos.<svc>.mode == "container"` now means a static pod
# in this box's own local k3s cluster: modules/cluster.nix runs the cluster,
# modules/workloads.nix writes the manifests and the rendered config the pods
# mount, and modules/nextcloud-common.nix still holds the Nextcloud truth both
# modes share. The option value keeps the name "container" — it is the
# user-facing spelling in the settings SPA and there is no installed base to
# break by renaming it, but it no longer implies nspawn.
#
# The local cluster runs no CNI (--flannel-backend=none), so its pods share the
# host's network namespace and bind 127.0.0.1. That is why the two routes below
# proxy to loopback ports instead of container addresses — and why `lanOnly`
# lost a layer of depth. That loss is spelled out where the guard is defined;
# do not re-add a deny rule for a subnet that no longer exists.
#
# The vhost itself is unconditional: it owns the *service* routes, so gating it
# on losos.admin.enable (as it used to be) took Nextcloud and Forgejo offline
# along with the dashboard. Only the admin locations, the document root and the
# admin-only firewall entry follow losos.admin.enable.
{
  lib,
  config,
  ...
}:

let
  nextcloudWorkload = config.losos.nextcloud.mode == "container";
  # losos.forgejo.enable is consulted in *both* modes. It used to be read only
  # by the native path (modules/services.nix), so with the default
  # mode == "container" the option was dead: enable = false still ran the git
  # host and still published /forgejo/ through the tunnel.
  forgejoWorkload = config.losos.forgejo.mode == "container" && config.losos.forgejo.enable;

  # The Nextcloud pod's httpd listens here. The port is runtime-tunable from
  # the settings SPA, which is exactly why it is not baked into the image:
  # modules/workloads.nix renders listen.conf in module context and
  # hostPath-mounts it, so a changed port changes the manifest and the kubelet
  # restarts the pod. Both ends read the same option, so they cannot drift.
  apachePort = config.losos.nextcloud.apachePort;

  # Forgejo's port is a literal on both sides — here and in the losos.ini that
  # modules/workloads.nix mounts into the pod. There is deliberately no
  # losos.forgejo.httpPort: nothing off-box ever sees it (the prefix is
  # stripped here and ROOT_URL carries the public form), so an option would be
  # a knob with no reason to be turned.
  forgejoPort = 3000;

  adminUi = config.losos.admin.ui;
  adminApiPort = config.losos.admin.apiPort;
  adminEnabled = config.losos.admin.enable && adminUi != null;

  # Access guard for the admin surface (dashboard, settings SPA, their assets,
  # the lososd API): local network only. Master-proxy traffic must never reach
  # these routes — and it arrives from *loopback* (proxy.nix points rathole at
  # 127.0.0.1:80), so loopback is deliberately not allowed. That costs nothing
  # for humans: the box has no shell logins, so no legitimate client browses
  # from localhost. The vhost only listens on 0.0.0.0, so the IPv4 ranges below
  # are exhaustive; if an IPv6 listener is ever added, these rules fail closed
  # (LAN IPv6 clients get 403) rather than open. Link-local (169.254/16) is not
  # allowed at all — a self-assigned address is not a trust signal on an admin
  # plane.
  #
  # What this guard no longer does, honestly:
  #
  # Under nspawn the workloads had their own subnet, so this list began with
  # `deny 10.231.0.0/16;` — nginx takes the first matching rule, and a PHP RCE
  # in Nextcloud reaching Nginx on 0.0.0.0:80 *from* 10.231.1.2 was refused
  # before `allow 10.0.0.0/8` could wave it through. hostNetwork pods have no
  # address of their own, so there is nothing left to deny: nginx sees them as
  # 127.0.0.1 or as the box's own LAN address. A pod connecting over loopback
  # still matches none of the allows and falls through to `deny all`, so the
  # naive case is covered — but a compromised pod can source-bind the LAN
  # address and would then pass this guard. Do not paper that over with a deny
  # rule for a pod CIDR: there is no pod CIDR, and a rule that cannot match is
  # worse than an acknowledged gap, because it reads like protection.
  #
  # What is genuinely left: /api is Bearer-authed against
  # losos.admin.tokenFile, which is 0600 root-only, and the workload pods run
  # as uid 1002/1003 — so they cannot read the token and cannot drive the API
  # even from an address this guard accepts. The static admin pages behind the
  # other guarded locations are not secrets.
  #
  # And never "fix" any of this with `allow 127.0.0.1`: that hands the whole
  # admin surface to the internet the moment losos.proxy.enable is on.
  #
  # (A LAN that genuinely numbers itself inside RFC1918 is the normal case and
  # is what the allows are for; there is no longer a carve-out to collide with.)
  lanOnly = ''
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
  # img-src carries `data:` because admin-ui/design-system/losos.css inlines
  # the select chevron as a data: URI, and a CSS background-image is an img-src
  # fetch. connect-src is plain 'self': it used to also carry http(s)://$host:3456
  # for the dashboard's cross-origin probe of the Tahoe web UI, and Tahoe-LAFS
  # is gone — with it the :3456 vhost, the probe, and any reason for this page
  # to talk to a second origin.
  adminCsp = lib.concatStringsSep "; " [
    "default-src 'none'"
    "script-src 'self'"
    "style-src 'self'"
    "img-src 'self' data:"
    "font-src 'self'"
    "connect-src 'self'"
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

  # ── Nginx front router ────────────────────────────────────────────────────
  # One default_server vhost on :80: dashboard at /, settings SPA at
  # /settings, lososd JSON API at /api/*, and the two workload routes. The
  # pages reference their assets by absolute path (/ds/losos.css,
  # /settings/app.js), so dashboard/ is the vhost root, settings/ is aliased
  # alongside it, and the shared common.js (admin-ui root, used by both pages)
  # gets an explicit alias.
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
        (lib.mkIf nextcloudWorkload {
          # No URI part in proxyPass -> path preserved; the pod's
          # losos.config.php sets overwritewebroot = /nextcloud.
          #
          # Loopback, not a container address: the pod is hostNetwork, so its
          # httpd binds 127.0.0.1:<apachePort> on the host itself. The pod's
          # trusted_proxies must therefore list 127.0.0.1 rather than a veth
          # host address, or Nextcloud sees one client for every request and
          # its brute-force throttle protects nothing (modules/workloads.nix).
          "/nextcloud" = {
            proxyPass = "http://127.0.0.1:${toString apachePort}";
            proxyWebsockets = true;
            extraConfig = ''
              client_max_body_size 0;
              proxy_request_buffering off;
              proxy_read_timeout 86400s;
              proxy_send_timeout 86400s;
            '';
          };
        })
        (lib.mkIf forgejoWorkload {
          # Trailing slash on proxyPass -> /forgejo prefix stripped;
          # Forgejo's ROOT_URL is http://<host>.local/forgejo/ (or the
          # master-proxy hostname) so it generates prefixed links.
          "/forgejo/" = {
            proxyPass = "http://127.0.0.1:${toString forgejoPort}/";
            proxyWebsockets = true;
          };
        })
      ];
    };
  };

  # ── Firewall ──────────────────────────────────────────────────────────────
  # Nginx owns :80 and nothing else on the box holds a public port: both
  # workload pods bind 127.0.0.1 (hostNetwork, config-driven — see
  # modules/workloads.nix), the lososd API is loopback, and neither Kubernetes
  # instance adds to allowedTCPPorts, so 6443/9345/10250/10260 stay closed
  # against NixOS' default-deny firewall. The port is opened whenever the front
  # vhost has any route to serve — the admin SPA *or* either workload — because
  # gating it on the admin flag alone took the service routes down with the
  # dashboard.
  networking.firewall.allowedTCPPorts = lib.optional (
    adminEnabled || nextcloudWorkload || forgejoWorkload
  ) 80;
}
