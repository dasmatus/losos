# The Nginx front door.
#
# One default_server vhost on :80 carries everything the appliance publishes:
# the admin SPA at /, lososd's JSON API at /api/*, and the two service routes
# /nextcloud and /forgejo/. default_server because master-proxy
# (Traefik+rathole) traffic arrives with a public hostname, not
# <hostName>.local. Nothing but Nginx binds a public port. The admin surface
# (/, /assets/, /setup/, /api/) is LAN-only (see `lanOnly` below); only
# /nextcloud and /forgejo are reachable through the master-proxy tunnel.
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

  # Access guard for the admin surface (the SPA, its assets, the setup routes,
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
  # img-src carries `data:` for Vite: anything imported under
  # build.assetsInlineLimit (4 KiB by default) is emitted as a data: URI rather
  # than a file, and a CSS background-image is an img-src fetch. The SPA
  # imports no binary assets today, so the grant is currently unused — it is
  # kept because the first small inlined SVG would otherwise fail in a console
  # nobody is watching, on a box with no shell. To drop it, pair
  # `img-src 'self'` with `build.assetsInlineLimit = 0` in
  # admin-ui/app/vite.config.ts so the ban is enforced where it can be seen.
  # (It previously covered a chevron inlined by the plain-JS pages'
  # design-system stylesheet, which is no longer served.)
  #
  # connect-src is plain 'self': it used to also carry http(s)://$host:3456
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
    # The first-run wizard embeds /nextcloud so the handover from "choose a
    # password" to "here are your files" happens without leaving the page.
    #
    # It has to be spelled out: under CSP3 frame-src falls back to child-src
    # and then to default-src, which is 'none' above, so an absent frame-src is
    # a *block*, not an inherit.
    #
    # 'self' and nothing wider, and this is not merely tidiness. The framed
    # page is same-origin, which means it is NOT sandboxed from us: it can
    # reach `parent.document` and therefore the admin token in sessionStorage.
    # docs/security-model.md already accepts that an XSS in Nextcloud is a full
    # appliance compromise because the two share an origin; framing Nextcloud
    # inside the admin document does not create that exposure, but it does
    # remove the "an attacker must first land an XSS" step for anything
    # Nextcloud itself loads. That trade was made deliberately — see the
    # "Embedding Nextcloud in the wizard" section of the security model.
    "frame-src 'self'"
    "form-action 'none'"
    # Unchanged, and it governs the OTHER direction: who may frame the admin
    # page. Nothing about embedding Nextcloud needs this loosened, and
    # tests/front-vhost.nix asserts it stayed DENY precisely because reaching
    # for it is the obvious wrong way to unblock an iframe.
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
        admin SPA to serve and every admin route would 404.
        modules/defaults.nix wires losos.admin.ui to the flake's
        losos-admin-ui package; either restore that or set
        losos.admin.enable = false.
      '';
    }
  ];

  # ── Nginx front router ────────────────────────────────────────────────────
  # One default_server vhost on :80: the admin SPA at /, lososd's JSON API at
  # /api/*, and the two workload routes. losos.admin.ui is a built Vite tree
  # (index.html + hashed assets/ + theme-boot.js), so it is the vhost root
  # outright — there is no page pair to alias alongside each other any more,
  # and every asset the document loads is an absolute /assets/… path served
  # from that same root.
  services.nginx = {
    enable = true;
    recommendedProxySettings = true;

    # Header value selectors — see `adminCsp` above for why they are maps.
    appendHttpConfig = adminHeaderMaps;

    virtualHosts."losos-front" = {
      default = true; # default_server — master-proxy traffic arrives with a public hostname
      # mkDefault because modules/tls.nix restates this listener alongside its
      # :443 one, on the same vhost (deliberately — see that file's header for
      # why HTTPS must not be a second vhost). `listen` is a listOf, and two
      # plain definitions would *concatenate* rather than override: the merged
      # config would carry `listen 0.0.0.0:80 default_server;` twice and nginx
      # would refuse to start with "a duplicate listen 0.0.0.0:80".
      #
      # That failure is invisible at build time — the nginx.conf derivation runs
      # gixy, not `nginx -t` — so it lands in nginx.service's ExecStartPre at
      # boot, which on this box means the only front door crash-loops with no
      # shell to see it from. tests/setup.nix is what caught it, by being the
      # first test to import containers.nix and tls.nix together.
      listen = lib.mkDefault [
        {
          addr = "0.0.0.0";
          port = 80;
        }
      ];
      root = lib.mkIf adminEnabled "${adminUi}";
      extraConfig = adminHeaderDirectives;
      locations = lib.mkMerge [
        (lib.mkIf adminEnabled {
          # The whole admin surface is one location now: one SPA, one
          # document, real paths. LAN-only, like every admin location below.
          #
          # The `/index.html` arm is the load-bearing one. The app routes on
          # real paths rather than hashes (/storage, /settings/network), so
          # without it a deep link — or an ordinary reload of one — 404s.
          # admin-ui/app/src/App.tsx names this requirement in its own header.
          #
          # A consequence worth stating so nobody "fixes" it: an unknown path
          # under / now returns index.html with 200, and the SPA's catch-all
          # route renders "nothing here", where the old page pair returned
          # 404. That is what an SPA fallback means.
          "/" = {
            index = "index.html";
            tryFiles = "$uri $uri/ /index.html";
            extraConfig = lanOnly;
          };
          # Content-hashed filenames, so a year is safe: a changed bundle gets
          # a changed name. `expires`, not `add_header` — an add_header at
          # location scope REPLACES the inherited set rather than adding to
          # it, which would silently drop the security headers this vhost sets
          # at server level. `expires` is a different directive and leaves
          # them alone.
          #
          # theme-boot.js is deliberately NOT covered: it is the one unhashed
          # file in the bundle (admin-ui/app/vite.config.ts says why), so it
          # has to keep revalidating against its ETag.
          "/assets/" = {
            extraConfig = ''
              ${lanOnly}
              expires 1y;
            '';
          };
          # Fail closed rather than handing the SPA's index.html to a client
          # that asked for JSON. modules/setup.nix merges `= /setup/state.json`
          # and `= /setup/losos-ca.crt` into this same vhost; an exact match
          # beats this prefix, so where that module is imported nothing
          # changes. Where it is not, the first-run wizard gets a 404 — which
          # it has a branch for ("this box is too old to have a setup route")
          # — instead of an HTML document it would try to parse as JSON.
          "/setup/" = {
            tryFiles = "$uri =404";
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
