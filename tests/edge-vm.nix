# nixos-test-vms config for the master-proxy edge + appliance.
#
# Boots two VMs:
#   * `edge`      — losos.edge.enable: Traefik (master) + rathole server +
#                   losos-registrar (serve). The registrar API is bound to
#                   `::` (losos.edge.registrarApiBind, dual-stack) only because
#                   there is no Traefik/TLS path in the VM (no LE, no DNS) —
#                   the appliance dials it directly over HTTP.
#   * `appliance` — losos.proxy.enable: rathole client (dials edge:2333) +
#                   losos-registrar announce (register/heartbeat) + a minimal
#                   Nginx on :80 (the slave proxy) serving a known page.
#
# Asserts the end-to-end master-proxy data path without TLS:
#   1. edge rathole-seed writes a declarative [server] base (bind_addr scalar);
#   2. the registrar API (/health) comes up on the edge;
#   3. the appliance announce service registers → the reconciler writes
#      /etc/traefik/dynamic/losos.yml (Host rule) and /etc/rathole/server.toml
#      ([server.services.mattbox]); rathole's file-watcher hot-reloads;
#   4. rathole forwards: `curl edge:127.0.0.1:<port>` reaches the appliance
#      Nginx over the tunnel (the L4 path Traefik would use);
#   5. POST /unregister tears the route down (curl then fails);
#   6. enrollment is actually closed: an unknown appliance id, a known id with
#      the wrong token, and a blank token are each rejected.
#
# Step 6 replaces the old /config and /tahoe upload assertions. Those
# endpoints were removed: they accepted an arbitrary body, wrote it to disk as
# root, parsed it and threw it away, reporting back only what the caller had
# just sent. They appear nowhere in the master-proxy design, and they put an
# untrusted serde_yaml parse on the public edge for no functionality. What
# went untested all along is the thing options.nix calls the security
# boundary — that the registrar only ever serves whitelisted tenants — so the
# coverage moves there.
#
# Traefik's on-demand TLS (certResolver le) is not exercised — LE can't issue
# in a VM — so we test the L4 rathole tunnel + registrar directly. Traefik is
# left running (its ACME failures are logged, not fatal).
{ pkgs }:

let
  # Shared test fixtures (store paths are identical across both nodes'
  # evaluations, so the appliance and edge see the same token bytes).
  #
  # Bound once and interpolated everywhere, including into testScript. The
  # token used to appear as a literal here and again in four Python
  # assertions, so changing it in one place left the others silently testing
  # the wrong value. These live in the top-level `let` rather than inside
  # `nodes` because testScript is a sibling of `nodes`, not a child, and could
  # not otherwise see them.
  proxyTokenValue = "test-proxy-token-0123456789abcdef";
  # 32 chars minimum: the registrar refuses to start on a bootstrap token
  # shorter than that, because a short or truncated token file is a
  # guessable secret rather than a credential. The old fixture was 18
  # characters and crash-looped the service the moment that check landed.
  bootstrapTokenValue = "test-bootstrap-0123456789abcdef0123";

  # Runtime paths, NOT pkgs.writeText store paths. options.nix now warns when
  # a secret option points into /nix/store, because the store is world-
  # readable and a token there is published to every process on the box. This
  # test used to demonstrate exactly that anti-pattern, which both fired four
  # warnings on every eval — training the reader to ignore them — and modelled
  # the wrong thing for anyone copying it. tmpfiles writes the fixtures during
  # early boot, before any unit that reads them, which is how a real appliance
  # provisions them out of band.
  proxyToken = "/var/secrets/losos-proxy-token";
  bootstrapToken = "/var/secrets/losos-rathole-bootstrap";
  secretFiles = {
    systemd.tmpfiles.rules = [
      "d /var/secrets 0700 root root - -"
      "f ${proxyToken} 0600 root root - ${proxyTokenValue}"
      "f ${bootstrapToken} 0600 root root - ${bootstrapTokenValue}"
    ];
  };
  lososPkgs = import ../flake/packages.nix { inherit pkgs; };
in

pkgs.testers.nixosTest {
  name = "losos-edge-proxy";

  nodes = {
    edge =
      { ... }:
      {
        imports = [
          ../modules/options.nix
          ../modules/edge.nix
          secretFiles
        ];

        # edge.nix wires losos.edge.registrar.package = self.packages...;
        # testers.nixosTest has no flake `self`, so supply a shim pointing at
        # the same package the flake would build.
        _module.args.self = {
          packages.x86_64-linux.losos-registrar = lososPkgs.losos-registrar;
        };

        losos.edge = {
          enable = true;
          acmeEmail = "test@losos.cfd"; # satisfies the assertion; LE won't run usefully in-VM
          # test-only: appliance dials the API directly over the inter-VM
          # link. The nixosTest framework maps `edge` to an IPv6 address
          # (2001:db8::/64), so bind `::` (dual-stack) — the appliance's
          # native `getent hosts edge` resolution reaches it without any
          # IPv4 fallback. Production fronts the API behind Traefik/TLS.
          registrarApiBind = "::";
          registrarApiPort = 8443;
          ratholeBindPort = 2333;
          ratholePortRange = "50000-50010";
          heartbeatTtl = "120s";
          reconcileInterval = "2s"; # fast for the test
          bootstrapTokenFile = bootstrapToken;
          tenants.mattbox = {
            hostname = "mattbox.losos.cfd";
            tokenFile = proxyToken;
          };
        };

        # ratholeBindAddr defaults to `::` (dual-stack), so the IPv6
        # resolution the framework gives `edge` reaches rathole — no IPv4
        # override on eth1 is needed.

        virtualisation = {
          memorySize = 1024;
          cores = 2;
        };
      };

    appliance =
      { ... }:
      {
        imports = [
          ../modules/options.nix
          ../modules/proxy.nix
          secretFiles
        ];

        losos = {
          hostName = "mattbox";
          proxy = {
            enable = true;
            # Dial the edge by its node name. The nixosTest framework's
            # /etc/hosts maps `edge` to an IPv6 2001:db8::/64 address, and
            # rathole binds `::` (dual-stack) by default, so the appliance's
            # native resolution reaches the tunnel + registrar API directly.
            edgeRatholeEndpoint = "edge:2333";
            registrarUrl = "http://edge:8443"; # direct, no Traefik/TLS in-VM
            hostname = "mattbox.losos.cfd";
            applianceId = "mattbox";
            tokenFile = proxyToken;
            bootstrapTokenFile = bootstrapToken;
            heartbeatInterval = "3s";
            registrar.package = lososPkgs.losos-registrar;
          };
        };

        # The slave proxy: a minimal Nginx on :80 that rathole forwards to.
        # Serves a fixed page so the end-to-end curl through the tunnel has a
        # recognisable body to assert on.
        services.nginx = {
          enable = true;
          virtualHosts."losos-front" = {
            default = true;
            listen = [
              {
                addr = "0.0.0.0";
                port = 80;
              }
            ];
            locations."/".extraConfig = ''
              return 200 'hello-losos\n';
            '';
          };
        };

        virtualisation = {
          memorySize = 1024;
          cores = 2;
        };
      };
  };

  testScript = ''
    edge.start()
    appliance.start()

    # 1. The rathole-seed wrote a declarative [server] base with the scalar
    #    bind_addr (not a `bind` list) before rathole started.
    edge.wait_for_unit("losos-rathole-seed.service")
    seed = edge.succeed("cat /etc/rathole/server.toml")
    assert "bind_addr = " in seed, f"seed missing bind_addr scalar: {seed!r}"
    assert "default_token = " in seed, f"seed missing default_token: {seed!r}"
    edge.wait_for_unit("losos-rathole.service")
    edge.wait_for_unit("losos-registrar.service")

    # 2. The registrar API is up.
    edge.wait_until_succeeds("curl -fsS http://127.0.0.1:8443/health")

    # Diagnostics: confirm the edge is listening (dual-stack `::` binds show as
    # `*:2333` / `*:8443` in ss) and the appliance resolves `edge` to the IPv6
    # address the framework's /etc/hosts maps it to. (Printed so a connectivity
    # failure in a later step shows the resolution + listener state.)
    print("edge listeners:", edge.succeed("ss -tlnp 2>/dev/null | grep -E '2333|8443' || true"))
    print("edge eth1 ipv6:", edge.succeed("ip -6 addr show eth1 2>/dev/null | grep inet6 || true"))
    print("appliance resolves edge:", appliance.succeed("getent hosts edge || true"))

    # 3. The appliance announce service registers, and the reconciler writes
    #    the Traefik router + rathole service. Reconcile runs on a notify kick
    #    (immediate) and every 2s; wait for the rathole service to appear.
    appliance.wait_for_unit("losos-registrar-announce.service")
    appliance.wait_for_unit("losos-rathole-client.service")

    # grep -F (fixed-string) is load-bearing: `[server.services.mattbox]` is a
    # TOML section header, and without -F the `[...]` is a regex character class
    # that matches any line containing the letters s/e/r/v/m/a/t/b/o/x — i.e.
    # almost every line in server.toml. That makes the positive grep a tautology
    # and `! grep` (step 5's teardown assertion) always-false → 900s timeout.
    # -F matches the literal header so the assertions actually test the block.
    edge.wait_until_succeeds(
      "grep -F -q '[server.services.mattbox]' /etc/rathole/server.toml"
    )
    edge.wait_until_succeeds(
      "grep -q 'Host(`mattbox.losos.cfd`)' /etc/traefik/dynamic/losos.yml"
    )
    toml = edge.succeed("cat /etc/rathole/server.toml")
    assert "bind_addr = \"127.0.0.1:50000\"" in toml, f"rathole service bind_addr: {toml!r}"

    # 4. End-to-end over the L4 tunnel: the rathole server's per-service
    #    bind (127.0.0.1:50000 on the edge) forwards through the tunnel to the
    #    appliance's Nginx. (Traefik would Hit this same address.) Rathole's
    #    `notify` file-watcher hot-reloaded server.toml the instant the
    #    registrar rewrote it, so the client service is now matched and the
    #    forward succeeds after the control channel settles.
    edge.wait_until_succeeds("curl -fsS http://127.0.0.1:50000/ | grep -q hello-losos")

    # 5. /unregister tears the route down: the rathole service disappears and
    #    the forward stops. Stop the appliance's announce loop first: it
    #    re-registers every heartbeatInterval (3s here — a 404 heartbeat
    #    triggers re-enroll), so without stopping it mattbox would reappear
    #    before the reconciler prunes it and the [server.services.mattbox]
    #    block would never leave server.toml. (Auth via JSON body, same token
    #    as announce.)
    appliance.succeed("systemctl stop losos-registrar-announce.service")
    hdr_id = "Content-Type: application/json"
    body = '{"appliance_id":"mattbox","token":"${proxyTokenValue}"}'
    edge.succeed(f"curl -fsS -X POST -H '{hdr_id}' -d '{body}' http://127.0.0.1:8443/unregister")
    # The reconciler prunes mattbox and rewrites server.toml without the
    # [server.services.mattbox] block; rathole's `notify` file-watcher
    # hot-reloads (no signal) and closes the per-service 127.0.0.1:50000
    # listener.
    edge.wait_until_succeeds("! grep -F -q '[server.services.mattbox]' /etc/rathole/server.toml")
    edge.wait_until_fails("curl -fsS http://127.0.0.1:50000/")

    # 6. Enrollment is closed. options.nix calls losos.edge.tenants the
    #    security boundary — "the registrar only ever writes Traefik routers
    #    for ids listed here" — but every assertion above registers `mattbox`,
    #    which IS whitelisted. These are the negative cases.
    #
    #    `curl -o /dev/null -w %{http_code}` rather than `-f`, so a wrong
    #    status is reported as a status instead of a bare non-zero exit.
    def status(body):
        return edge.succeed(
            "curl -s -o /dev/null -w '%{http_code}' -X POST "
            f"-H '{hdr_id}' -d '{body}' http://127.0.0.1:8443/register"
        ).strip()

    # An id that is not in tenants.json, with a well-formed token.
    unknown = '{"appliance_id":"attacker","token":"${proxyTokenValue}","hostname":"evil.losos.cfd"}'
    assert status(unknown) == "401", f"unknown appliance id was not rejected: {status(unknown)}"

    # A whitelisted id with the wrong token.
    wrongtok = '{"appliance_id":"mattbox","token":"wrong-token-0000000000000000000","hostname":"mattbox.losos.cfd"}'
    assert status(wrongtok) == "401", f"wrong token was not rejected: {status(wrongtok)}"

    # A blank token. This is the one that mattered: authenticate() used to
    # compare the supplied token against the trimmed contents of the tenant's
    # token file with no validation, so a zero-byte or whitespace-only token
    # file authenticated anybody sending "".
    blank = '{"appliance_id":"mattbox","token":"","hostname":"mattbox.losos.cfd"}'
    assert status(blank) == "401", f"blank token was not rejected: {status(blank)}"

    # And the whitelisted tenant still works, so the guards above are not
    # rejecting everything indiscriminately.
    good = '{"appliance_id":"mattbox","token":"${proxyTokenValue}","hostname":"mattbox.losos.cfd"}'
    assert status(good) == "200", f"legitimate registration broke: {status(good)}"
  '';
}
