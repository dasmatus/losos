# Edge federation, end to end (handbook/docs/in-depth/edge-federation.md): three
# VMs on one LAN playing the three machines the design has.
#
#   * `hub`   — an official edge as the internet would see it: the edge module
#               with closed enrolment and two tenant rows, the spoke (`acme`,
#               with relayZone acme.losos.cfd) and the box itself (`mattbox`,
#               the direct row it falls back to). No LAN advert.
#   * `spoke` — the gateway: `losos.edge.gateway.enable`, i.e. LAN advert, open
#               enrolment, the uplink read from /var/lib/losos-edge/uplink.json
#               that `losos-edge uplink set` writes, a root console.
#   * `box`   — lososd with the master proxy on: the edge scanner, the path
#               rule, and modules/proxy.nix's two units driven by it. Nginx on
#               :80 answers a fixed page so a request through the tunnels has
#               something to assert on.
#
# What it asserts, in order:
#   1. the box picks the LAN edge (the spoke) as its path with no
#      configuration: /api/edge `path.source == "lan"`, the env file lososd
#      wrote, the tunnel dialling the advertised `tunnel=` endpoint, the
#      spoke's tunnel certificate pinned under losos-edge-pins/;
#   2. the spoke enrolled the box on first contact (open enrolment) and
#      routes to it: one tunnel hop answers;
#   3. `losos-edge uplink set` on the spoke relays the box to the hub: the
#      hub routes `acme.mattbox` with the spoke's token and
#      Host(mattbox.acme.losos.cfd), the spoke's uplink tunnel file carries
#      the service, the hub's certificate is pinned on the spoke, and a request
#      entering the hub crosses two tunnels to the box;
#   4. the spoke goes away: the box falls back to the configured official
#      edge (`path.source == "configured"`), the units are restarted against
#      it, and the box's direct tenant row on the hub answers;
#   5. the hub goes away too: `path == null`, /run/losos/edge-none, both
#      units stopped, the sharing gate refuses and the market relay says
#      unavailable;
#   6. the spoke comes back: the path returns to the LAN and the units run.
#
# Everything that would need public DNS or a TLS certificate (Traefik's
# ACME, Host routing over 443) is out of reach in a VM, as in nix/tests/edge-vm.nix;
# the hub's Traefik file is asserted as text and the data path is driven
# through the tunnel's per-service loopback ports.
{ pkgs }:

let
  lososPkgs = import ../packages.nix { inherit pkgs; };

  # Fixtures the hub operator would hand out: the box's token (its direct
  # tenant row on the hub; the spoke learns the same token on first contact)
  # and the spoke's token (its tenant row on the hub). Written by tmpfiles
  # during early boot, like nix/tests/edge-vm.nix.
  proxyToken = "/var/secrets/losos-proxy-token";
  spokeToken = "/var/secrets/losos-tenant-acme";
  proxyTokenValue = "test-proxy-token-0123456789abcdef0123456789abcdef";
  spokeTokenValue = "test-spoke-token-0123456789abcdef0123456789abcdef";
  secretFiles = {
    systemd.tmpfiles.rules = [
      "d /var/secrets 0700 root root - -"
      "f ${proxyToken} 0600 root root - ${proxyTokenValue}"
      "f ${spokeToken} 0600 root root - ${spokeTokenValue}"
    ];
  };
  selfShim = {
    _module.args.self = {
      # The gateway preset turns on losos.edge.manage, whose page is the
      # admin UI's manage/ build.
      packages.x86_64-linux = {
        inherit (lososPkgs) losos-registrar losos-admin-ui;
      };
    };
  };
in

pkgs.testers.nixosTest {
  name = "losos-edge-federation";

  nodes = {
    hub =
      { ... }:
      {
        imports = [
          ../../modules/options.nix
          ../../modules/edge.nix
          secretFiles
          selfShim
        ];
        networking.hostName = "hub";
        losos.edge = {
          enable = true;
          acmeEmail = "test@losos.cfd";
          # The test framework resolves node names to IPv6 (2001:db8::/64);
          # bind dual-stack so `hub:8443` and `hub:2333` reach it.
          registrarApiBind = "::";
          registrarApiPort = 8443;
          tunnelPortRange = "50000-50010";
          # Short, so a spoke that stops relaying takes its boxes with it
          # inside the test's patience.
          heartbeatTtl = "30s";
          reconcileInterval = "2s";
          tenants = {
            acme = {
              hostname = "acme.losos.cfd";
              tokenFile = spokeToken;
              relayZone = "acme.losos.cfd";
            };
            mattbox = {
              hostname = "mattbox.acme.losos.cfd";
              tokenFile = proxyToken;
            };
          };
        };
        virtualisation = {
          memorySize = 1024;
          cores = 2;
        };
      };

    spoke =
      { lib, ... }:
      {
        imports = [
          ../../modules/options.nix
          ../../modules/edge.nix
          ../../modules/edge-gateway.nix
          secretFiles
          selfShim
        ];
        networking.hostName = "spoke";
        # The gateway preset is the feature under test; the rest is the
        # test's impatience.
        # The test framework gives root its own password file; the preset's
        # first password would only draw a precedence warning beside it.
        users.users.root.initialPassword = lib.mkForce null;
        # The screenshot below is of the banner. The test instrumentation
        # raises the console log level to 7 (plain assignment, above the
        # preset's mkDefault 3), so the image's quiet console is restored
        # here or the kernel's chatter lands across the banner.
        boot.consoleLogLevel = lib.mkForce 3;
        losos.edge = {
          gateway.enable = true;
          registrarApiPort = 8443;
          tunnelPortRange = "50000-50010";
          reconcileInterval = "2s";
          uplink.interval = "3s";
        };
        virtualisation = {
          memorySize = 1024;
          cores = 2;
        };
      };

    box =
      { pkgs, ... }:
      {
        imports = [
          ../../modules/options.nix
          ../../modules/daemon.nix
          ../../modules/proxy.nix
          ../../modules/mdns.nix
          secretFiles
        ];
        networking.hostName = "mattbox";
        losos.backend.package = lososPkgs.losos-ctl;
        losos.proxy = {
          enable = true;
          # The official edge, as a real box has it configured: dialled by
          # node name over the test's IPv6, which both hub listeners bind.
          registrarUrl = "http://hub:8443";
          edgeTunnelEndpoint = "hub:2333";
          hostname = "mattbox.acme.losos.cfd";
          applianceId = "mattbox";
          tokenFile = proxyToken;
          heartbeatInterval = "3s";
          registrar.package = lososPkgs.losos-registrar;
        };
        # What modules/configuration.nix gives the real box: mDNS through
        # resolved, so the LAN edge's advert is seen and its .local name
        # resolves.
        losos.mdns.enable = true;
        # The slave proxy: a minimal Nginx on :80 that the tunnels end at.
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
        environment.systemPackages = [ pkgs.jq ];
        virtualisation = {
          memorySize = 1024;
          cores = 2;
        };
      };
  };

  testScript = ''
    import json
    import shlex

    hub.start()
    spoke.start()
    box.start()

    hub.wait_for_unit("losos-registrar.service")
    hub.wait_for_unit("losos-tunnel.service")
    hub.wait_until_succeeds("curl -fsS http://localhost:8443/health")
    hub.wait_until_succeeds("curl -fsS http://localhost:8443/tunnel-pin")
    hub_pin = hub.succeed("curl -fsS http://localhost:8443/tunnel-pin").strip()
    assert len(hub_pin) == 64, hub_pin

    spoke.wait_for_unit("losos-registrar.service")
    spoke.wait_for_unit("losos-tunnel.service")
    spoke.wait_for_unit("systemd-resolved.service")
    spoke.wait_until_succeeds("curl -fsS http://localhost:8443/health")
    spoke.wait_until_succeeds("curl -fsS http://localhost:8443/tunnel-pin")
    spoke_pin = spoke.succeed("curl -fsS http://localhost:8443/tunnel-pin").strip()
    assert spoke_pin != hub_pin, "two edges made the same certificate"
    # The gateway expired the first root password; sshd is installed and
    # not running.
    assert spoke.succeed("chage -l root | grep -i 'last password change'").strip().lower().endswith("password must be changed")
    assert spoke.succeed("systemctl is-active sshd.service || true").strip() == "inactive"
    assert "not set" in spoke.succeed("losos-edge status")
    # What a person sees on the gateway's tty1 before logging in.
    spoke.screenshot("edge-gateway-tty1")

    box.wait_for_unit("systemd-resolved.service")
    box.wait_for_unit("nginx.service")
    box.wait_until_succeeds("losos-ctl state --json")
    box.wait_until_succeeds("curl -fsS 127.0.0.1:8082/api/health")
    token = box.succeed("systemd-creds decrypt --name=admin-token /etc/credstore.encrypted/admin-token -").strip()
    hdr = f"Authorization: Bearer {token}"

    def api(method, path, data=None):
        extra = "-H 'Content-Type: application/json' --data-binary @- " if data is not None else ""
        cmd = (
            f"curl -s -o /tmp/body -w '%{{http_code}}' -X {method} -H '{hdr}' {extra}"
            f"127.0.0.1:8082{path}"
        )
        if data is not None:
            cmd = f"printf %s {shlex.quote(json.dumps(data))} | " + cmd
        code = box.succeed(cmd).strip()
        return code, box.succeed("cat /tmp/body")

    def edge_doc():
        code, body = api("GET", "/api/edge")
        assert code == "200", f"/api/edge answered {code}: {body!r}"
        return json.loads(body)

    def wait_path(source):
        def probe(_):
            p = edge_doc()["path"]
            return (p is None and source is None) or (p is not None and p["source"] == source)
        with box.nested(f"waiting for the edge path to be {source}"):
            retry(probe, timeout_seconds=180)

    def unit(name):
        return box.succeed(f"systemctl is-active {name} || true").strip()

    # ── 1. The LAN edge is the path, with no configuration ────────────────
    browse = box.wait_until_succeeds(
      "resolvectl service 'losos edge on spoke' _losos-edge._tcp local"
    )
    assert "url=http://spoke.local:8443" in browse, browse
    assert "tunnel=spoke.local:2333" in browse, browse
    assert "enrol=open" in browse, browse

    wait_path("lan")
    doc = edge_doc()
    print("edge:", doc)
    assert doc["path"]["url"] == "http://spoke.local:8443", doc
    assert doc["path"]["tunnel"] == "spoke.local:2333", doc
    assert doc["reachable"] is True, doc
    # Both edges answer; the configured one carries its endpoint too.
    by_source = {e["source"]: e for e in doc["edges"]}
    assert by_source["lan"]["tunnel"] == "spoke.local:2333", doc
    assert by_source["configured"]["tunnel"] == "hub:2333", doc

    env = box.wait_until_succeeds("cat /run/losos/edge-path.env")
    assert "LOSOS_EDGE_PATH_URL=http://spoke.local:8443\n" in env, env
    assert "LOSOS_EDGE_PATH_TUNNEL=spoke.local:2333\n" in env, env
    assert "LOSOS_EDGE_PATH_TUNNEL_PIN=/var/secrets/losos-edge-pins/spoke.local_8443.pin" in env, env
    box.fail("test -e /run/losos/edge-none")
    box.wait_until_succeeds("systemctl is-active losos-tunnel-client.service")
    box.wait_until_succeeds("systemctl is-active losos-registrar-announce.service")
    def client_file():
        return json.loads(box.succeed("cat /run/losos-tunnel/client.json"))

    box.wait_until_succeeds("grep -F -q '\"remote\": \"spoke.local:2333\"' /run/losos-tunnel/client.json")
    client = client_file()
    pinned = box.succeed("cat /var/secrets/losos-edge-pins/spoke.local_8443.pin").strip()
    assert pinned == spoke_pin == client["pin"], f"box pinned {pinned!r}, spoke has {spoke_pin!r}"
    # The announce unit read the path too: its process carries the spoke's URL.
    assert "http://spoke.local:8443" in box.succeed("systemctl show -p ExecStart --value losos-registrar-announce.service || true") or \
      "http://spoke.local:8443" in box.succeed("cat /proc/$(systemctl show -p MainPID --value losos-registrar-announce.service)/cmdline | tr '\\0' ' '")

    # ── 2. Open enrolment, and one hop through the spoke ─────────────────
    def services(machine):
        return json.loads(machine.succeed("cat /etc/losos-tunnel/server.json"))["services"]

    spoke.wait_until_succeeds("grep -F -q '\"mattbox\":' /etc/losos-tunnel/server.json")
    enrolled = spoke.succeed("losos-registrar enrol list --dir /var/lib/losos-registrar/enrolled")
    assert enrolled.strip() == "mattbox mattbox.acme.losos.cfd", enrolled
    assert spoke.succeed("stat -c %a /var/lib/losos-registrar/enrolled/mattbox.token").strip() == "600"
    assert spoke.succeed("cat /var/lib/losos-registrar/enrolled/mattbox.token").strip() == "${proxyTokenValue}"
    boxes = spoke.succeed("losos-edge boxes")
    assert boxes.startswith("mattbox\tmattbox.acme.losos.cfd\tport "), boxes
    spoke_port = boxes.split("port ")[1].strip()

    def through(machine, port):
        return machine.wait_until_succeeds(f"curl -fsS --max-time 5 http://127.0.0.1:{port}/", timeout=120)

    assert through(spoke, spoke_port).strip() == "hello-losos"

    # ── 3. The uplink: the box reaches the hub through the spoke ─────────
    out = spoke.succeed(
      "losos-edge uplink set --id acme --token-file ${spokeToken} "
      "--registrar http://hub:8443 --tunnel hub:2333"
    )
    assert "uplink set: acme" in out, out
    shown = json.loads(spoke.succeed("losos-edge uplink show"))
    assert shown["registrar_url"] == "http://hub:8443" and shown["id"] == "acme", shown
    assert shown["tunnel_endpoint"] == "hub:2333", shown
    assert spoke.succeed("stat -c %a /var/lib/losos-edge/uplink.token").strip() == "600"

    hub.wait_until_succeeds("grep -F -q '\"acme.mattbox\":' /etc/losos-tunnel/server.json")
    relayed = services(hub)["acme.mattbox"]
    assert relayed["token"] == "${spokeTokenValue}", relayed
    hub_port = relayed["bind"].rsplit(":", 1)[1]
    traefik = hub.succeed("cat /etc/traefik/dynamic/losos.yml")
    assert "Host(`mattbox.acme.losos.cfd`)" in traefik, traefik
    registry = json.loads(hub.succeed("cat /var/lib/losos-registrar/registry.json"))
    assert registry["tenants"]["acme.mattbox"]["via"] == "acme", registry

    spoke.wait_until_succeeds("grep -F -q 'acme.mattbox' /etc/losos-tunnel/uplink.json")
    uplink = json.loads(spoke.succeed("cat /etc/losos-tunnel/uplink.json"))
    assert uplink["services"]["acme.mattbox"]["local"] == f"127.0.0.1:{spoke_port}", uplink
    assert uplink["remote"] == "hub:2333" and uplink["pin"] == hub_pin, uplink
    assert spoke.succeed("cat /var/lib/losos-edge/hub-tunnel.pin").strip() == hub_pin
    spoke.wait_for_unit("losos-tunnel-uplink.service")
    assert "relaying: 1 box" in spoke.succeed("losos-edge status")

    # Two tunnels, one request: in at the hub, out at the box's Nginx.
    assert through(hub, hub_port).strip() == "hello-losos"

    # ── 4. The spoke goes away: fall back to the official edge ───────────
    # resolved and both its sockets, or a lookup on the spoke starts it again.
    spoke.succeed(
        "systemctl stop losos-registrar.service losos-tunnel.service "
        "systemd-resolved.service systemd-resolved-varlink.socket systemd-resolved-monitor.socket"
    )
    wait_path("configured")
    doc = edge_doc()
    assert doc["path"]["url"] == "http://hub:8443" and doc["path"]["tunnel"] == "hub:2333", doc
    env = box.succeed("cat /run/losos/edge-path.env")
    assert "LOSOS_EDGE_PATH_TUNNEL=hub:2333\n" in env, env
    assert "LOSOS_EDGE_PATH_TUNNEL_PIN=/var/secrets/losos-tunnel-pin" in env, env
    box.wait_until_succeeds("grep -F -q '\"remote\": \"hub:2333\"' /run/losos-tunnel/client.json")
    assert box.succeed("cat /var/secrets/losos-tunnel-pin").strip() == hub_pin == client_file()["pin"]
    # The direct tenant row answers; the relayed one expires with the spoke.
    hub.wait_until_succeeds("grep -F -q '\"mattbox\":' /etc/losos-tunnel/server.json")
    direct_port = services(hub)["mattbox"]["bind"].rsplit(":", 1)[1]
    assert through(hub, direct_port).strip() == "hello-losos"
    hub.wait_until_succeeds("! grep -F -q 'acme.mattbox' /etc/losos-tunnel/server.json", timeout=120)

    # ── 5. The hub goes away too: everything edge-dependent is off ───────
    hub.succeed("systemctl stop losos-registrar.service losos-tunnel.service")
    wait_path(None)
    doc = edge_doc()
    assert doc["reachable"] is False and doc["edges"] == [], doc
    box.wait_until_succeeds("test -e /run/losos/edge-none")
    box.fail("test -e /run/losos/edge-path.env")
    box.wait_until_succeeds("test \"$(systemctl is-active losos-tunnel-client.service)\" = inactive")
    box.wait_until_succeeds("test \"$(systemctl is-active losos-registrar-announce.service)\" = inactive")
    # A stopped unit stays stopped: Restart=always does not apply to a
    # `systemctl stop`, and the condition keeps a manual start from dialling.
    box.succeed("systemctl start losos-tunnel-client.service || true")
    assert unit("losos-tunnel-client.service") == "inactive"
    code, body = api("GET", "/api/market")
    assert json.loads(body)["available"] is False, body
    code, body = api("POST", "/api/change", {"mode": "mesh"})
    assert code == "409", f"change to mesh with no edge answered {code}: {body!r}"
    assert json.loads(body)["edgeRequired"] is True, body

    # ── 6. The spoke comes back: the LAN path again, units running ───────
    spoke.succeed("systemctl start systemd-resolved.service losos-tunnel.service losos-registrar.service")
    spoke.wait_until_succeeds("curl -fsS http://localhost:8443/health")
    wait_path("lan")
    box.wait_until_succeeds("! test -e /run/losos/edge-none")
    box.wait_until_succeeds("systemctl is-active losos-tunnel-client.service")
    box.wait_until_succeeds("systemctl is-active losos-registrar-announce.service")
    box.wait_until_succeeds("grep -F -q '\"remote\": \"spoke.local:2333\"' /run/losos-tunnel/client.json")
    spoke.wait_until_succeeds("grep -F -q '\"mattbox\":' /etc/losos-tunnel/server.json")
    assert through(spoke, spoke_port).strip() == "hello-losos"

    # `losos-edge uplink clear` stops relaying and leaves nothing behind.
    spoke.succeed("losos-edge uplink clear")
    spoke.fail("test -e /var/lib/losos-edge/uplink.json")
    spoke.fail("test -e /etc/losos-tunnel/uplink.json")
    spoke.wait_until_succeeds("test \"$(systemctl is-active losos-tunnel-uplink.service)\" = inactive")
  '';
}
