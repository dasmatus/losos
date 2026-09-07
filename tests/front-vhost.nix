# nixos-test-vms config for the Nginx front door (modules/containers.nix).
#
# containers.nix is the only file on the box that decides what the outside
# world can reach, and both of its access rules have already been wrong once:
#
#   * `lanOnly` allowed 10.0.0.0/8, which *contains* the container subnets
#     10.231.1.2 / 10.231.2.2 — so a compromised Nextcloud or Forgejo could
#     talk to the root-equivalent admin API on the front vhost. It now denies
#     10.231.0.0/16 first (nginx takes the first matching rule).
#   * the whole vhost was gated on losos.admin.enable, so turning the
#     dashboard off silently took /nextcloud and /forgejo/ down with it. Only
#     the admin locations follow that flag now.
#
# Both fixes are invisible to `nix eval`; this test pins them down. It asserts,
# from three different *source addresses*:
#
#   loopback (127.0.0.1)   -> admin routes 403. Deliberate, and load-bearing:
#                             master-proxy tunnel traffic reaches this vhost
#                             from 127.0.0.1 (rathole's local_addr), so
#                             allowing loopback would publish the entire admin
#                             surface to the internet whenever
#                             losos.proxy.enable is on. A version of this test
#                             that expects 200 here is testing the bug.
#   LAN (192.168.1.x)      -> admin routes 200 (301 for the `= /settings`
#                             redirect, which runs in nginx's rewrite phase
#                             before allow/deny is consulted).
#   container (10.231.1.2) -> admin routes 403.
#
# plus: /nextcloud and /forgejo/ are *not* LAN-guarded (that is how tunnel
# traffic arrives, from loopback); the four security headers ride on both the
# LAN 200s and the loopback 403s (`always`) and are omitted on the two service
# routes; and with losos.admin.enable = false the service routes and :80 stay
# up.
#
# Two nodes, both real appliances, each acting as the other's LAN client:
#   appliance — losos.admin.enable = true  (192.168.1.1)
#   noadmin   — losos.admin.enable = false (192.168.1.2)
#
# Nothing heavy is built: the Nextcloud/Forgejo container payloads are replaced
# with empty NixOS systems and their backends are stubbed by two extra nginx
# server blocks on the addresses the front vhost proxies to. Booting the real
# stacks would cost hours and would not exercise a single line of routing.
{ pkgs }:

let
  lososPkgs = import ../flake/packages.nix { inherit pkgs; };

  # modules/containers.nix reads config.lososInternal.nextcloudStack, an option
  # declared by modules/nextcloud-common.nix. Importing that module would drag
  # nextcloud34 + postgres + 30 apps into the closure, and the only consumer of
  # the value is the container body replaced below — so declare the option and
  # leave it empty.
  nextcloudStackStub =
    { lib, ... }:
    {
      options.lososInternal.nextcloudStack = lib.mkOption {
        type = lib.types.attrs;
        internal = true;
        default = {
          enable = false;
        };
      };
    };

  # The routes under test are nginx's, not nspawn's. Keep
  # losos.<svc>.mode = "container" (that is what puts /nextcloud and /forgejo/
  # in the vhost) but throw the container payloads away.
  emptyContainers =
    { lib, ... }:
    {
      containers.nextcloud = {
        autoStart = lib.mkForce false;
        config = lib.mkForce (_: {
          system.stateVersion = "26.11";
        });
      };
      containers.forgejo = {
        autoStart = lib.mkForce false;
        config = lib.mkForce (_: {
          system.stateVersion = "26.11";
        });
      };
    };

  # nspawn would put 10.231.1.1 / 10.231.2.1 on the host end of each veth and
  # the containers would answer on .2. With the containers gone, park the two
  # backend addresses on a dummy link instead. This does double duty: the front
  # vhost's proxy_pass has something to reach, and the test can originate a
  # request *from* 10.231.1.2 — the source address the `deny 10.231.0.0/16`
  # rule exists to reject.
  stubNet =
    { pkgs, ... }:
    {
      boot.kernelModules = [ "dummy" ];
      systemd.services.losos-test-stub-net = {
        description = "Container-subnet addresses for the front-vhost test";
        wantedBy = [ "multi-user.target" ];
        # nginx binds 10.231.1.2:80 and 10.231.2.2:3000 below, so the addresses
        # have to exist before it starts.
        requiredBy = [ "nginx.service" ];
        before = [ "nginx.service" ];
        after = [ "network-pre.target" ];
        serviceConfig = {
          Type = "oneshot";
          RemainAfterExit = true;
        };
        path = [ pkgs.iproute2 ];
        script = ''
          ip link show losos-stub >/dev/null 2>&1 || ip link add losos-stub type dummy
          ip addr replace 10.231.1.2/16 dev losos-stub
          ip addr replace 10.231.2.2/16 dev losos-stub
          ip link set losos-stub up
        '';
      };

      # Stand-ins for the two container backends and for lososd's loopback API,
      # on exactly the addresses/ports modules/containers.nix proxies to. More
      # specific listen addresses than the front vhost's 0.0.0.0:80, so nginx
      # routes connections to them and not to the default_server.
      services.nginx.virtualHosts = {
        "stub-nextcloud" = {
          listen = [
            {
              addr = "10.231.1.2";
              port = 80;
            }
          ];
          locations."/".extraConfig = ''return 200 "stub-nextcloud\n";'';
        };
        "stub-forgejo" = {
          listen = [
            {
              addr = "10.231.2.2";
              port = 3000;
            }
          ];
          locations."/".extraConfig = ''return 200 "stub-forgejo\n";'';
        };
        "stub-lososd" = {
          listen = [
            {
              addr = "127.0.0.1";
              port = 8082;
            }
          ];
          locations."/".extraConfig = ''return 200 "stub-lososd\n";'';
        };
      };
    };

  applianceBase = {
    imports = [
      ../modules/options.nix
      ../modules/containers.nix
      nextcloudStackStub
      emptyContainers
      stubNet
    ];

    losos = {
      hostName = "mattbox";
      nextcloud.mode = "container";
      forgejo.mode = "container";
      forgejo.enable = true;
      # No /dev/dri in a VM, and the GPU bind mount is not what is under test.
      gpu.enable = false;
      # No lososd: /api/ is answered by the stub above. The escape hatch only
      # works because modules/defaults.nix wires the real package with
      # lib.mkDefault — at normal priority this was a conflict, not an opt-out.
      backend.package = null;
    };

    virtualisation = {
      memorySize = 1024;
      cores = 2;
    };
  };
in

pkgs.testers.nixosTest {
  name = "losos-front-vhost";

  nodes = {
    appliance = _: {
      imports = [ applianceBase ];
      losos.admin.enable = true;
      losos.admin.ui = lososPkgs.losos-admin-ui;
    };

    # Same box with the dashboard switched off. losos.admin.ui stays null,
    # which the assertion in containers.nix permits only while admin.enable is
    # false — so this node also covers that assertion's true branch.
    noadmin = _: {
      imports = [ applianceBase ];
      losos.admin.enable = false;
    };
  };

  testScript = ''
    start_all()
    appliance.wait_for_unit("nginx.service")
    noadmin.wait_for_unit("nginx.service")
    appliance.wait_for_open_port(80)
    noadmin.wait_for_open_port(80)

    # The admin surface: dashboard, settings SPA, their shared assets, and the
    # lososd API proxy. `= /settings` is handled separately — it is a rewrite-
    # phase redirect and therefore not guarded (see containers.nix).
    ADMIN = ["/", "/settings/", "/ds/losos.css", "/common.js", "/api/health"]
    # Not guarded, by design: this is the only traffic the master-proxy tunnel
    # is allowed to carry, and it arrives from loopback.
    SERVICE = ["/nextcloud", "/forgejo/"]

    def code(node, url, source=None):
        src = f"--interface {source} " if source else ""
        return node.succeed(
            f"curl -s -o /dev/null -w '%{{http_code}}' {src}{url}"
        ).strip()

    def headers(node, url, source=None):
        src = f"--interface {source} " if source else ""
        raw = node.succeed(f"curl -s -D - -o /dev/null {src}{url}")
        out = {}
        for line in raw.splitlines()[1:]:
            if ":" in line:
                k, _, v = line.partition(":")
                out[k.strip().lower()] = v.strip()
        return out

    with subtest("admin routes are 403 from loopback (tunnel traffic arrives here)"):
        for path in ADMIN:
            got = code(appliance, f"http://127.0.0.1{path}")
            assert got == "403", f"loopback {path}: expected 403, got {got}"

    with subtest("admin routes are 403 from a container-subnet source address"):
        # The exact thing the first `deny` in lanOnly exists for: a compromised
        # Nextcloud/Forgejo reaching the front vhost from 10.231.x. Without that
        # deny, `allow 10.0.0.0/8` would let this through.
        for path in ADMIN:
            got = code(appliance, f"http://appliance{path}", source="10.231.1.2")
            assert got == "403", f"container-source {path}: expected 403, got {got}"

    with subtest("admin routes are 200 from the LAN"):
        for path in ADMIN:
            got = code(noadmin, f"http://appliance{path}")
            assert got == "200", f"LAN {path}: expected 200, got {got}"
        body = noadmin.succeed("curl -s http://appliance/")
        assert "LosOS &mdash; home" in body, f"LAN / did not serve the dashboard: {body!r}"
        body = noadmin.succeed("curl -s http://appliance/settings/")
        assert "losos &mdash; settings" in body, f"LAN /settings/ is not the SPA: {body!r}"

    with subtest("`= /settings` redirects from every source"):
        # `return` runs in the rewrite phase, before allow/deny, so a guard on
        # this location would be dead config. The redirect target *is* guarded,
        # which is what the loopback 403 on /settings/ above proves.
        for label, node, url, source in (
            ("loopback", appliance, "http://127.0.0.1/settings", None),
            ("container", appliance, "http://appliance/settings", "10.231.1.2"),
            ("LAN", noadmin, "http://appliance/settings", None),
        ):
            got = code(node, url, source=source)
            assert got == "301", f"{label} /settings: expected 301, got {got}"

    with subtest("service routes are reachable from loopback and from the LAN"):
        for path in SERVICE:
            got = code(appliance, f"http://127.0.0.1{path}")
            assert got == "200", f"loopback {path}: expected 200, got {got}"
            got = code(noadmin, f"http://appliance{path}")
            assert got == "200", f"LAN {path}: expected 200, got {got}"
        assert "stub-nextcloud" in appliance.succeed("curl -s http://127.0.0.1/nextcloud")
        assert "stub-forgejo" in appliance.succeed("curl -s http://127.0.0.1/forgejo/")

    with subtest("security headers ride on admin responses, including the 403s"):
        want = {
            "content-security-policy": "default-src 'none'",
            "x-content-type-options": "nosniff",
            "referrer-policy": "no-referrer",
            "x-frame-options": "DENY",
        }
        for label, hdrs in (
            ("LAN 200", headers(noadmin, "http://appliance/")),
            ("loopback 403", headers(appliance, "http://127.0.0.1/")),
        ):
            for name, needle in want.items():
                assert name in hdrs, f"{label}: missing {name} ({sorted(hdrs)})"
                assert needle in hdrs[name], \
                    f"{label}: {name} = {hdrs[name]!r} lacks {needle!r}"
        # $host interpolation in the CSP map value — the dashboard probes the
        # Tahoe WUI cross-origin at <host>:3456 and would be blocked without it.
        csp = headers(noadmin, "http://appliance/")["content-security-policy"]
        assert "connect-src 'self' http://appliance:3456" in csp, f"CSP host not filled in: {csp}"

    with subtest("the admin CSP is not applied to the two service routes"):
        # Nextcloud and Forgejo ship their own CSP and both need inline script;
        # the map's ~^/nextcloud and ~^/forgejo arms exist to omit these.
        for path in SERVICE:
            hdrs = headers(appliance, f"http://127.0.0.1{path}")
            for name in ("content-security-policy", "x-frame-options"):
                assert name not in hdrs, f"{path}: {name} should be omitted, got {hdrs[name]!r}"

    with subtest("admin.enable = false keeps the service routes and :80 up"):
        for path in SERVICE:
            got = code(noadmin, f"http://127.0.0.1{path}")
            assert got == "200", f"noadmin loopback {path}: expected 200, got {got}"
            # From the LAN too, which is only possible if :80 is still open.
            got = code(appliance, f"http://noadmin{path}")
            assert got == "200", f"noadmin LAN {path}: expected 200, got {got}"
        assert "stub-forgejo" in appliance.succeed("curl -s http://noadmin/forgejo/")

    with subtest("admin.enable = false removes the admin routes entirely"):
        for path in [p for p in ADMIN if p != "/"]:
            got = code(appliance, f"http://noadmin{path}")
            assert got in ("403", "404"), \
                f"noadmin {path}: admin route still answering ({got})"
        # "/" has no location left either, so the request falls through to
        # nginx's own stock index page. That is a 200, so assert on the body:
        # what matters is that the dashboard is gone, not the status code.
        body = appliance.succeed("curl -s http://noadmin/")
        assert "LosOS" not in body, f"noadmin / still serving the dashboard: {body!r}"
  '';
}
