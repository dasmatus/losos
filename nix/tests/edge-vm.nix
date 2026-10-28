# nixos-test-vms config for the master-proxy edge + appliance.
#
# Boots two VMs:
#   * `edge`      — losos.edge.enable: Traefik (master) + tunnel server +
#                   losos-registrar (serve). The registrar API is bound to
#                   `::` (losos.edge.registrarApiBind, dual-stack) only because
#                   there is no Traefik/TLS path in the VM (no LE, no DNS) —
#                   the appliance dials it directly over HTTP.
#   * `appliance` — losos.proxy.enable: tunnel client (dials edge:2333) +
#                   losos-registrar announce (register/heartbeat) + a minimal
#                   Nginx on :80 (the slave proxy) serving a known page.
#
# Asserts the end-to-end master-proxy data path without TLS:
#   1. the edge makes its tunnel certificate on first start, the registrar
#      serves its pin, and the appliance pins it before its first dial;
#   2. the registrar API (/health) comes up on the edge;
#   3. the appliance announce service registers → the registrar publishes
#      the Host rule on its provider listener and writes
#      /etc/losos-tunnel/server.json (the `mattbox` service), which the
#      tunnel server re-reads within a second;
#   4. the tunnel forwards: `curl edge:127.0.0.1:<port>` reaches the
#      appliance Nginx over QUIC, and Traefik, having discovered the box
#      from the registrar, routes https://mattbox.losos.cfd to it over TCP
#      and over HTTP/3 (QUIC, UDP 443), the latter from the appliance across
#      the firewall; with the registrar stopped Traefik keeps the route; with
#      the tunnel down a visitor gets the edge's offline page, not a bare
#      502; an old edge's leftover losos.yml is removed on the next switch;
#      with UDP to the tunnel's port dropped, the client falls back to TLS
#      on TCP and the same curl works again;
#   5. POST /unregister tears the route down (curl then fails, Traefik
#      answers 404 for the name);
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
# Traefik's on-demand certificates (certResolver le) are not exercised: Let's
# Encrypt cannot issue in a VM, so Traefik answers with its default
# self-signed certificate and the curls below pass -k. Everything else about
# the route is the real thing.
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

  # Runtime paths, NOT pkgs.writeText store paths. options.nix now warns when
  # a secret option points into /nix/store, because the store is world-
  # readable and a token there is published to every process on the box. This
  # test used to demonstrate exactly that anti-pattern, which both fired four
  # warnings on every eval — training the reader to ignore them — and modelled
  # the wrong thing for anyone copying it. tmpfiles writes the fixtures during
  # early boot, before any unit that reads them, which is how a real appliance
  # provisions them out of band.
  proxyToken = "/var/secrets/losos-proxy-token";
  secretFiles = {
    systemd.tmpfiles.rules = [
      "d /var/secrets 0700 root root - -"
      "f ${proxyToken} 0600 root root - ${proxyTokenValue}"
    ];
  };
  lososPkgs = import ../packages.nix { inherit pkgs; };
in

pkgs.testers.nixosTest {
  name = "losos-edge-proxy";

  nodes = {
    edge =
      { ... }:
      {
        imports = [
          ../../modules/options.nix
          ../../modules/edge.nix
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
          tunnelBindPort = 2333;
          tunnelPortRange = "50000-50010";
          heartbeatTtl = "120s";
          reconcileInterval = "2s"; # fast for the test
          tenants.mattbox = {
            hostname = "mattbox.losos.cfd";
            tokenFile = proxyToken;
          };
        };

        environment.systemPackages = [ pkgs.jq ];

        # tunnelBindAddr defaults to `::` (dual-stack), so the IPv6
        # resolution the framework gives `edge` reaches the tunnel — no IPv4
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
          ../../modules/options.nix
          ../../modules/proxy.nix
          secretFiles
        ];

        losos = {
          hostName = "mattbox";
          proxy = {
            enable = true;
            # Dial the edge by its node name. The nixosTest framework's
            # /etc/hosts maps `edge` to an IPv6 2001:db8::/64 address, and
            # the tunnel binds `::` (dual-stack) by default, so the appliance's
            # native resolution reaches the tunnel + registrar API directly.
            edgeTunnelEndpoint = "edge:2333";
            registrarUrl = "http://edge:8443"; # direct, no Traefik/TLS in-VM
            hostname = "mattbox.losos.cfd";
            applianceId = "mattbox";
            tokenFile = proxyToken;
            heartbeatInterval = "3s";
            registrar.package = lososPkgs.losos-registrar;
          };
        };

        # The slave proxy: a minimal Nginx on :80 that the tunnel forwards to.
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

    # 1. The tunnel server made its certificate on its first start and
    #    wrote its pin; the registrar serves it, and the appliance pinned it
    #    before its first dial. Nothing about the tunnel's identity is
    #    provisioned in this test.
    edge.wait_for_unit("losos-tunnel.service")
    edge.wait_for_unit("losos-registrar.service")
    edge.wait_until_succeeds("test -s /var/lib/losos-tunnel/pin")
    edge_pin = edge.succeed("cat /var/lib/losos-tunnel/pin").strip()
    assert len(edge_pin) == 64, f"edge pin looks wrong: {edge_pin!r}"
    assert edge.succeed("stat -c %a /var/lib/losos-tunnel/key.der").strip() == "600"
    edge.wait_until_succeeds("curl -fsS http://127.0.0.1:8443/tunnel-pin")
    assert edge.succeed("curl -fsS http://127.0.0.1:8443/tunnel-pin").strip() == edge_pin
    appliance.wait_for_unit("losos-tunnel-client.service")
    pinned = appliance.succeed("cat /var/secrets/losos-tunnel-pin").strip()
    assert pinned == edge_pin, f"appliance pinned {pinned!r}, edge has {edge_pin!r}"
    client = appliance.succeed("cat /run/losos-tunnel/client.json")
    assert edge_pin in client, f"client not pinned: {client!r}"
    assert appliance.succeed("stat -c %a /run/losos-tunnel/client.json").strip() == "600"

    # 2. The registrar API is up.
    edge.wait_until_succeeds("curl -fsS http://127.0.0.1:8443/health")

    # Diagnostics: confirm the edge is listening (dual-stack `::` binds show as
    # `*:2333` / `*:8443` in ss) and the appliance resolves `edge` to the IPv6
    # address the framework's /etc/hosts maps it to. (Printed so a connectivity
    # failure in a later step shows the resolution + listener state.)
    print("edge listeners:", edge.succeed("ss -tulnp 2>/dev/null | grep -E '2333|8443' || true"))
    print("edge eth1 ipv6:", edge.succeed("ip -6 addr show eth1 2>/dev/null | grep inet6 || true"))
    print("appliance resolves edge:", appliance.succeed("getent hosts edge || true"))

    # 3. The appliance announce service registers, and the reconciler writes
    #    the Traefik router + tunnel service. Reconcile runs on a notify kick
    #    (immediate) and every 2s; wait for the tunnel service to appear.
    appliance.wait_for_unit("losos-registrar-announce.service")
    appliance.wait_for_unit("losos-tunnel-client.service")

    edge.wait_until_succeeds(
      "jq -e '.services.mattbox.bind == \"127.0.0.1:50000\"' /etc/losos-tunnel/server.json"
    )
    edge.wait_until_succeeds(
      "curl -fsS http://127.0.0.1:8090/traefik | grep -q 'Host(`mattbox.losos.cfd`)'"
    )
    # Nothing is written into Traefik's directory any more.
    edge.fail("test -e /etc/traefik/dynamic/losos.yml")
    assert edge.succeed("stat -c %a /etc/losos-tunnel/server.json").strip() == "600"

    # 4. End-to-end over the tunnel: the tunnel server's per-service bind
    #    (127.0.0.1:50000 on the edge) forwards through the QUIC connection
    #    to the appliance's Nginx. (Traefik would hit this same address.)
    edge.wait_until_succeeds("curl -fsS http://127.0.0.1:50000/ | grep -q hello-losos")
    edge.wait_until_succeeds("journalctl -u losos-tunnel.service | grep -q 'holds mattbox'")
    print(edge.succeed("journalctl -u losos-tunnel.service | grep 'holds mattbox'"))
    assert "(quic)" in edge.succeed("journalctl -u losos-tunnel.service | grep 'holds mattbox' | tail -1")

    # 4b. Traefik discovered the box: the name routes over TCP on the edge
    #     itself, and over HTTP/3 from another machine, through the firewall's
    #     UDP 443. Traefik advertises HTTP/3 on every TCP answer.
    https = "curl -sSk --max-time 10 --resolve mattbox.losos.cfd:443:127.0.0.1 https://mattbox.losos.cfd/"
    edge.wait_until_succeeds(f"{https} | grep -q hello-losos", timeout=120)
    headers = edge.succeed(f"{https} -D - -o /dev/null")
    assert 'alt-svc: h3=":443"' in headers.lower(), headers
    edge_v4 = appliance.succeed("getent ahostsv4 edge | awk 'NR==1{print $1}'").strip()
    edge_addr = edge_v4 or "[" + appliance.succeed("getent ahostsv6 edge | awk 'NR==1{print $1}'").strip() + "]"
    h3 = (
        "${pkgs.curl}/bin/curl -sSk --max-time 10 --http3-only "
        f"--resolve mattbox.losos.cfd:443:{edge_addr} https://mattbox.losos.cfd/"
    )
    appliance.wait_until_succeeds(f"{h3} | grep -q hello-losos", timeout=120)
    version = appliance.succeed(f"{h3} -o /dev/null -w '%{{http_version}}'").strip()
    assert version == "3", f"not HTTP/3: {version!r}"

    # Traefik keeps what it discovered while the registrar is away.
    edge.succeed("systemctl stop losos-registrar.service")
    edge.succeed(f"{https} | grep -q hello-losos")
    appliance.succeed(f"{h3} | grep -q hello-losos")
    edge.succeed("systemctl start losos-registrar.service")
    edge.wait_until_succeeds("curl -fsS http://127.0.0.1:8090/traefik | grep -q 'Host(`mattbox.losos.cfd`)'")

    # The tunnel goes down: the visitor gets the edge's page, in their
    # language, with Traefik's status, not a bare "Bad Gateway".
    appliance.succeed("systemctl stop losos-tunnel-client.service")
    offline = f"{https} -H 'Accept-Language: de' -o /tmp/offline.html -w '%{{http_code}}'"
    edge.wait_until_succeeds(f"test \"$({offline})\" = 502", timeout=120)
    page = edge.succeed("cat /tmp/offline.html")
    assert '<html lang="de">' in page and "<strong>mattbox.losos.cfd</strong>" in page, page
    edge.succeed(f"{https} -o /tmp/offline-en.html")
    assert '<html lang="en">' in edge.succeed("cat /tmp/offline-en.html")
    # Kept in the test's output, for the screenshots of the change.
    edge.copy_from_machine("/tmp/offline.html", "offline-page")
    edge.copy_from_machine("/tmp/offline-en.html", "offline-page")
    appliance.succeed("systemctl start losos-tunnel-client.service")
    edge.wait_until_succeeds(f"{https} | grep -q hello-losos", timeout=120)

    # An edge upgraded from the file-written routes: the leftover is taken
    # away on the switch, so Traefik never routes a box twice.
    edge.succeed("echo 'http: {}' > /etc/traefik/dynamic/losos.yml")
    edge.succeed("systemctl restart systemd-tmpfiles-resetup.service")
    edge.fail("test -e /etc/traefik/dynamic/losos.yml")

    # 4c. A network that drops UDP: the client gives QUIC five seconds, then
    #     dials the same port over TCP, and the same request goes through.
    for t in ["iptables", "ip6tables"]:
        edge.succeed(f"{t} -I INPUT -p udp --dport 2333 -j DROP")
    appliance.succeed("systemctl restart losos-tunnel-client.service")
    edge.wait_until_succeeds(
      "journalctl -u losos-tunnel.service | grep 'holds mattbox' | tail -1 | grep -q '(tcp)'",
      timeout=120,
    )
    edge.wait_until_succeeds("curl -fsS http://127.0.0.1:50000/ | grep -q hello-losos")
    appliance.succeed("journalctl -u losos-tunnel-client.service | grep -q 'on TCP'")
    for t in ["iptables", "ip6tables"]:
        edge.succeed(f"{t} -D INPUT -p udp --dport 2333 -j DROP")
    appliance.succeed("systemctl restart losos-tunnel-client.service")
    edge.wait_until_succeeds(
      "journalctl -u losos-tunnel.service | grep 'holds mattbox' | tail -1 | grep -q '(quic)'",
      timeout=120,
    )

    # 5. /unregister tears the route down: the tunnel service disappears and
    #    the forward stops. Stop the appliance's announce loop first: it
    #    re-registers every heartbeatInterval (3s here — a 404 heartbeat
    #    triggers re-enroll), so without stopping it mattbox would reappear
    #    before the reconciler prunes it and would never leave server.json. (Auth via JSON body, same token
    #    as announce.)
    appliance.succeed("systemctl stop losos-registrar-announce.service")
    hdr_id = "Content-Type: application/json"
    body = '{"appliance_id":"mattbox","token":"${proxyTokenValue}"}'
    edge.succeed(f"curl -fsS -X POST -H '{hdr_id}' -d '{body}' http://127.0.0.1:8443/unregister")
    # The reconciler prunes mattbox and rewrites server.json without it; the
    # tunnel server re-reads the file within a second and closes the
    # per-service 127.0.0.1:50000 listener.
    edge.wait_until_succeeds("jq -e '.services.mattbox == null' /etc/losos-tunnel/server.json")
    edge.wait_until_fails("curl -fsS http://127.0.0.1:50000/")
    edge.wait_until_succeeds(f"test \"$({https} -o /dev/null -w '%{{http_code}}')\" = 404")

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
