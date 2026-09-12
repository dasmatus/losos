# nixos-test-vms config for the Nginx front door (modules/containers.nix).
#
# containers.nix is the only file on the box that decides what the outside
# world can reach, and both of its access rules have already been wrong once:
#
#   * the whole vhost was gated on losos.admin.enable, so turning the dashboard
#     off silently took /nextcloud and /forgejo/ down with it. Only the admin
#     locations follow that flag now.
#   * `lanOnly` allowed 10.0.0.0/8, which *contained* the nspawn subnet the two
#     services used to live on (10.231.1.2 / 10.231.2.2) — so a compromised
#     Nextcloud or Forgejo could talk to the root-equivalent admin API. The fix
#     was `deny 10.231.0.0/16` as the first rule, and this test proved it by
#     curling the admin routes *from* that source address.
#
# The subnet and that deny rule are both gone, and this test must not pretend
# otherwise. `mode == "container"` now means a hostNetwork pod in the box's own
# k3s cluster (modules/workloads.nix): the pods share the host's network
# namespace, so nginx sees them as 127.0.0.1 or as the box's own LAN address,
# and there is no pod CIDR left to reject. containers.nix deleted the rule and
# forbids re-adding one, because a deny that cannot match reads like protection.
#
# What the guard lost, spelled out here so nobody has to reconstruct it from the
# diff: a pod that connects over loopback still matches no `allow` and falls
# through to `deny all`, which the loopback subtest below covers — but a pod
# that source-binds the box's own LAN address is indistinguishable from a laptop
# on that LAN and passes. The "LAN source, same box" subtest asserts that 200 on
# purpose, so the gap stays a tested fact rather than a surprise. What holds the
# line is no longer this guard: /api is Bearer-authed against
# losos.admin.tokenFile, which is 0600 root-only, and the workload pods run as
# uid 1002/1003 — so they cannot read the token and cannot drive the API from
# any source address at all. The static pages behind the other admin locations
# are not secrets.
#
# None of this is reachable from `nix eval` — an access rule only becomes real
# when a request carries a source address — so the test asserts from two of
# them:
#
#   loopback (127.0.0.1)   -> admin routes 403. Deliberate, and load-bearing:
#                             master-proxy tunnel traffic reaches this vhost
#                             from 127.0.0.1 (rathole's local_addr), so
#                             allowing loopback would publish the entire admin
#                             surface to the internet whenever
#                             losos.proxy.enable is on. A version of this test
#                             that expects 200 here is testing the bug.
#   LAN (192.168.1.x)      -> admin routes 200, including paths that exist
#                             only as client-side routes and are answered by
#                             the SPA fallback.
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
# Nothing heavy is built: no k3s, no kubelet, no workload pod. The routes under
# test are nginx's, and a hostNetwork pod is nothing but a process listening on
# the host's loopback, so two extra nginx server blocks on the ports the front
# vhost proxies to are a faithful stand-in rather than a simplification.
# Booting the real stacks would cost hours and would not exercise a single line
# of routing.
{ pkgs }:

let
  lososPkgs = import ../flake/packages.nix { inherit pkgs; };

  # Stand-ins for the two workloads and for lososd's loopback API, on exactly
  # the addresses and ports modules/containers.nix proxies to. Separate ports,
  # so none of them collides with the front vhost's 0.0.0.0:80 default_server.
  #
  # The two tunable ports are read from the same options containers.nix reads,
  # and the appliance below sets a *non-default* apachePort on purpose: if
  # anyone ever bakes the 11000 literal into the proxy_pass, this test fails
  # instead of passing by coincidence. Forgejo's 3000 is a literal on both
  # sides — containers.nix explains why it has deliberately never been an
  # option.
  stubBackends =
    { config, ... }:
    {
      services.nginx.virtualHosts = {
        "stub-nextcloud" = {
          listen = [
            {
              addr = "127.0.0.1";
              port = config.losos.nextcloud.apachePort;
            }
          ];
          locations."/".extraConfig = ''return 200 "stub-nextcloud\n";'';
        };
        "stub-forgejo" = {
          listen = [
            {
              addr = "127.0.0.1";
              port = 3000;
            }
          ];
          locations."/".extraConfig = ''return 200 "stub-forgejo\n";'';
        };
        "stub-lososd" = {
          listen = [
            {
              addr = "127.0.0.1";
              port = config.losos.admin.apiPort;
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
      stubBackends
    ];

    losos = {
      hostName = "mattbox";
      # "container" is what puts /nextcloud and /forgejo/ in the vhost; the
      # workloads themselves are stubbed above, so neither modules/cluster.nix
      # nor modules/workloads.nix is imported.
      nextcloud.mode = "container";
      # Deliberately not the 11000 default — see stubBackends.
      nextcloud.apachePort = 11007;
      forgejo.mode = "container";
      forgejo.enable = true;
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

    # The admin surface: the SPA, a client-side-only deep link, the pre-paint
    # theme script, and the lososd API proxy.
    #
    # "/settings/network" is the one that earns its place. It exists ONLY as a
    # client-side route — there is no such file in the bundle — so it can only
    # be answered by the `/index.html` arm of try_files. Drop that arm and
    # every deep link and every reload 404s, which is precisely the regression
    # a curl of "/" alone cannot see.
    #
    # `= /settings` is gone along with the page pair: /settings is now an
    # ordinary guarded route rather than a rewrite-phase redirect that could
    # not be guarded at all.
    ADMIN = ["/", "/settings", "/settings/network", "/theme-boot.js", "/api/health"]
    # Not guarded, by design: this is the only traffic the master-proxy tunnel
    # is allowed to carry, and it arrives from loopback.
    SERVICE = ["/nextcloud", "/forgejo/"]

    # The appliance's own LAN address (node 1 on vlan 1), spelled out rather
    # than reached through its name. On the appliance itself the name is
    # ambiguous: NixOS maps every machine's own hostname to 127.0.0.2, and the
    # test framework adds 192.168.1.1 for the same name — so `http://appliance`
    # from this node could be answered as *loopback*, which is the one thing
    # this file exists to tell apart. From the other node the name resolves only
    # to the LAN address, so the cross-node subtests keep using it.
    LAN = "192.168.1.1"

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
        # This is also the naive on-box case: a hostNetwork pod that reaches
        # nginx over loopback looks exactly like tunnel traffic and lands in the
        # same `deny all`.
        for path in ADMIN:
            got = code(appliance, f"http://127.0.0.1{path}")
            assert got == "403", f"loopback {path}: expected 403, got {got}"

    with subtest("admin routes are 200 from the LAN"):
        for path in ADMIN:
            got = code(noadmin, f"http://appliance{path}")
            assert got == "200", f"LAN {path}: expected 200, got {got}"
        body = noadmin.succeed("curl -s http://appliance/")
        assert '<div id="root">' in body, f"LAN / did not serve the SPA: {body!r}"
        # The blocking classic script that stamps the theme before first paint.
        # It cannot be inline (script-src 'self') and it cannot be a module
        # (deferred, so it would run after the paint), so its <script src> tag
        # in the served document is the only evidence the arrangement survived
        # the build. See admin-ui/app/vite.config.ts.
        assert "/theme-boot.js" in body, f"the pre-paint theme script is not linked: {body!r}"
        # The fallback itself: a path that exists only as a client-side route
        # must return the SAME document, not a 404 and not a different page.
        deep = noadmin.succeed("curl -s http://appliance/settings/network")
        assert deep == body, "the SPA fallback did not serve index.html for a deep link"

    with subtest("LAN source, same box: 200 — the gap hostNetwork opened"):
        # Not a property anyone wants; the honest replacement for the
        # container-subnet 403 cases this test used to carry. The old nspawn
        # workloads answered from 10.231.x and the first `deny` refused them
        # before `allow 10.0.0.0/8` could wave them through. A hostNetwork pod
        # has no address of its own, so it can source-bind the appliance's LAN
        # address and is then indistinguishable from a laptop on the same LAN —
        # which is what this curl imitates.
        #
        # The admin token is what stops it going any further: 0600 and owned by
        # root, unreadable to a pod running as uid 1002/1003, so /api answers
        # 401 to anything the pod could send. If these ever start returning 403,
        # the guard grew a layer — check it is a real one (not a deny rule for a
        # CIDR that cannot match) and update this subtest deliberately.
        for path in ADMIN:
            got = code(appliance, f"http://{LAN}{path}", source=LAN)
            assert got == "200", f"LAN-source {path}: expected 200, got {got}"

    with subtest("/setup/ fails closed rather than answering the SPA"):
        # The SPA fallback must not swallow paths that carry data. The wizard
        # reads /setup/state.json BEFORE a token exists, and modules/setup.nix
        # merges that in as an EXACT match — exact matches beat this prefix, so
        # where that module is imported the file still wins. Here it is not
        # imported, so the correct answer is 404: the wizard has a branch for a
        # missing setup route, and none for an HTML document it tried to parse
        # as JSON.
        got = code(noadmin, "http://appliance/setup/state.json")
        assert got == "404", f"LAN /setup/state.json: expected 404, got {got}"

    with subtest("service routes are reachable from loopback and from the LAN"):
        for path in SERVICE:
            got = code(appliance, f"http://127.0.0.1{path}")
            assert got == "200", f"loopback {path}: expected 200, got {got}"
            got = code(noadmin, f"http://appliance{path}")
            assert got == "200", f"LAN {path}: expected 200, got {got}"
        # Proof the two proxy_pass targets are the loopback ports the workload
        # pods bind, not some address left over from the nspawn layout.
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
        # connect-src is plain 'self', and the CSP no longer interpolates $host.
        # The map value used to carry http://$host:3456 so the dashboard could
        # probe the Tahoe web UI cross-origin; Tahoe-LAFS is gone, along with
        # that vhost and the probe, so a :3456 entry here would be a grant to
        # nothing.
        csp = headers(noadmin, "http://appliance/")["content-security-policy"]
        assert "connect-src 'self';" in csp, f"connect-src is not plain 'self': {csp}"
        assert ":3456" not in csp, f"CSP still grants the retired Tahoe WUI: {csp}"

        # frame-src 'self' — the setup wizard embeds /nextcloud, and without
        # this the iframe is blocked. Under CSP3 frame-src falls back to
        # child-src and then to default-src, which is 'none' here, so the grant
        # has to be explicit; there is no "inherit" that would let it through.
        #
        # It is exactly 'self' and nothing wider. The admin page has no business
        # framing any other origin, and this appliance publishes only two
        # routes besides the admin surface.
        assert "frame-src 'self'" in csp, f"the wizard cannot embed Nextcloud: {csp}"
        assert "frame-src *" not in csp and "frame-src 'unsafe" not in csp, \
            f"frame-src is wider than 'self': {csp}"
        # X-Frame-Options governs who may frame US, not whom we may frame, so
        # embedding Nextcloud must not have loosened it. Asserted here because
        # confusing the two directions is the obvious way to "fix" a blocked
        # iframe and it would expose the admin plane to clickjacking instead.
        assert headers(noadmin, "http://appliance/")["x-frame-options"] == "DENY", \
            "X-Frame-Options was loosened; it controls the other direction"

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
        # what matters is that the admin SPA is gone, not the status code.
        # `<div id="root">` rather than the word "LosOS": the SPA's <title> is
        # not the only place that string can appear, but the React mount point
        # appears nowhere except the document this test is checking for.
        body = appliance.succeed("curl -s http://noadmin/")
        assert '<div id="root">' not in body, f"noadmin / still serving the admin SPA: {body!r}"
  '';
}
