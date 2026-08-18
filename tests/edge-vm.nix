# nixos-test-vms config for the master-proxy edge + appliance.
#
# Boots TWO VMs:
#   * `edge`      — losos.edge.enable: Traefik (master) + rathole server +
#                   losos-registrar (serve). The registrar API is bound to
#                   0.0.0.0 (losos.edge.registrarApiBind) ONLY because there is
#                   no Traefik/TLS path in the VM (no LE, no DNS) — the
#                   appliance dials it directly over HTTP.
#   * `appliance` — losos.proxy.enable: rathole client (dials edge:2333) +
#                   losos-registrar announce (register/heartbeat) + a minimal
#                   Nginx on :80 (the slave proxy) serving a known page.
#
# Asserts the end-to-end master-proxy data path without TLS:
#   1. edge rathole-seed writes a declarative [server] base (bind_addr scalar);
#   2. the registrar API (/health) comes up on the edge;
#   3. the appliance announce service registers → the reconciler writes
#      /etc/traefik/dynamic/losos.yml (Host rule) and /etc/rathole/server.toml
#      ([server.services.mattbox]) and SIGHUPs rathole;
#   4. rathole forwards: `curl edge:127.0.0.1:<port>` reaches the appliance
#      Nginx over the tunnel (the L4 path Traefik would use);
#   5. POST /unregister tears the route down (curl then fails);
#   6. the upload endpoints validate-and-discard: /config parses a Traefik YAML
#      (primary_key "http"), /tahoe parses a tahoe.cfg INI (primary_key "node"),
#      and the spilled tmpfs file is gone after.
#
# Traefik's on-demand TLS (certResolver le) is NOT exercised — LE can't issue
# in a VM — so we test the L4 rathole tunnel + registrar directly. Traefik is
# left running (its ACME failures are logged, not fatal).
{ pkgs }:

pkgs.testers.nixosTest {
  name = "losos-edge-proxy";

  nodes =
    let
      # Shared test fixtures (store paths are identical across both nodes'
      # evaluations, so the appliance and edge see the same token bytes).
      proxyToken = pkgs.writeText "losos-proxy-token" "test-proxy-token-0123456789abcdef";
      bootstrapToken = pkgs.writeText "losos-rathole-bootstrap" "test-bootstrap-789";
      lososPkgs = import ../flake/packages.nix { inherit pkgs; };
    in
    {
      edge =
        { pkgs, ... }:
        {
          imports = [
            ../modules/options.nix
            ../modules/edge.nix
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
            registrarApiBind = "0.0.0.0"; # test-only: appliance dials the API directly
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

          virtualisation = {
            memorySize = 1024;
            cores = 2;
          };
        };

      appliance =
        { pkgs, ... }:
        {
          imports = [
            ../modules/options.nix
            ../modules/proxy.nix
          ];

          losos = {
            hostName = "mattbox";
            proxy = {
              enable = true;
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
    #    bind_addr (NOT a `bind` list) before rathole started.
    edge.wait_for_unit("losos-rathole-seed.service")
    seed = edge.succeed("cat /etc/rathole/server.toml")
    assert "bind_addr = " in seed, f"seed missing bind_addr scalar: {seed!r}"
    assert "default_token = " in seed, f"seed missing default_token: {seed!r}"
    edge.wait_for_unit("losos-rathole.service")
    edge.wait_for_unit("losos-registrar.service")

    # 2. The registrar API is up.
    edge.wait_until_succeeds("curl -fsS http://127.0.0.1:8443/health")

    # Diagnostics: confirm the edge is listening on the inter-VM network and
    # the appliance resolves `edge` to that IP. (Printed so a connectivity
    # failure in a later step shows the resolution + listener state.)
    print("edge listeners:", edge.succeed("ss -tlnp 2>/dev/null | grep -E '2333|8443' || true"))
    print("appliance resolves edge:", appliance.succeed("getent hosts edge || true"))

    # 3. The appliance announce service registers, and the reconciler writes
    #    the Traefik router + rathole service. Reconcile runs on a notify kick
    #    (immediate) and every 2s; wait for the rathole service to appear.
    appliance.wait_for_unit("losos-registrar-announce.service")
    appliance.wait_for_unit("losos-rathole-client.service")

    edge.wait_until_succeeds(
      "grep -q '[server.services.mattbox]' /etc/rathole/server.toml"
    )
    edge.wait_until_succeeds(
      "grep -q 'Host(`mattbox.losos.cfd`)' /etc/traefik/dynamic/losos.yml"
    )
    toml = edge.succeed("cat /etc/rathole/server.toml")
    assert "bind_addr = \"127.0.0.1:50000\"" in toml, f"rathole service bind_addr: {toml!r}"

    # 4. End-to-end over the L4 tunnel: the rathole server's per-service
    #    bind (127.0.0.1:50000 on the edge) forwards through the tunnel to the
    #    appliance's Nginx. (Traefik would Hit this same address.) Rathole
    #    hot-reloaded on SIGHUP and the client service is now matched, so the
    #    forward succeeds after the control channel settles.
    edge.wait_until_succeeds("curl -fsS http://127.0.0.1:50000/ | grep -q hello-losos")

    # 5. /unregister tears the route down: the rathole service disappears and
    #    the forward stops. (Auth via JSON body, same token as announce.)
    hdr_id = "Content-Type: application/json"
    body = '{"appliance_id":"mattbox","token":"test-proxy-token-0123456789abcdef"}'
    edge.succeed(f"curl -fsS -X POST -H '{hdr_id}' -d '{body}' http://127.0.0.1:8443/unregister")
    edge.wait_until_succeeds("! grep -q '[server.services.mattbox]' /etc/rathole/server.toml")
    edge.wait_until_fails("curl -fsS http://127.0.0.1:50000/")

    # 6. Upload endpoints validate-and-discard. Re-register first so auth has a
    #    known tenant, then POST a Traefik YAML and a Tahoe INI.
    regbody = '{"appliance_id":"mattbox","token":"test-proxy-token-0123456789abcdef","hostname":"mattbox.losos.cfd"}'
    edge.succeed(f"curl -fsS -X POST -H '{hdr_id}' -d '{regbody}' http://127.0.0.1:8443/register")

    traefik_yaml = "http:\n  routers:\n    r:\n      rule: \"Host(`x.losos.cfd`)\"\n      service: r\n"
    out = edge.succeed(
      f"curl -fsS -X POST --data-binary '{traefik_yaml}' -H 'x-appliance-id: mattbox' -H 'x-appliance-token: test-proxy-token-0123456789abcdef' http://127.0.0.1:8443/config"
    )
    print("config:", out)
    assert '"http"' in out, f"/config did not report http primary key: {out!r}"

    tahoe_ini = "[node]\nnickname = mattbox\n[client]\nintroducer.furl = pb://x\n"
    out2 = edge.succeed(
      f"curl -fsS -X POST --data-binary '{tahoe_ini}' -H 'x-appliance-id: mattbox' -H 'x-appliance-token: test-proxy-token-0123456789abcdef' http://127.0.0.1:8443/tahoe"
    )
    print("tahoe:", out2)
    assert '"node"' in out2, f"/tahoe did not report node section: {out2!r}"

    # The spilled tmpfs upload dir is empty after each parse-and-discard.
    leftover = edge.succeed("ls -A /run/losos-registrar/ 2>/dev/null || true").strip()
    assert leftover == "", f"tmpfs upload dir not emptied: {leftover!r}"
  '';
}