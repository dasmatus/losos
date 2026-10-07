# Edge federation, end to end (wiki/Edge-Federation.md): three VMs on one
# LAN playing the three machines the design has.
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
#      wrote, the tunnel dialling the advertised `rathole=` endpoint, the
#      spoke's Noise key pinned under losos-edge-pins/;
#   2. the spoke enrolled the box on first contact (open enrolment) and
#      routes to it: one tunnel hop answers;
#   3. `losos-edge uplink set` on the spoke relays the box to the hub: the
#      hub routes `acme.mattbox` with the spoke's token and
#      Host(mattbox.acme.losos.cfd), the spoke's uplink.toml carries the
#      service, the hub's Noise key is pinned on the spoke, and a request
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
# ACME, Host routing over 443) is out of reach in a VM, as in tests/edge-vm.nix;
# the hub's Traefik file is asserted as text and the data path is driven
# through rathole's per-service loopback ports.
{ pkgs }:

let
  lososPkgs = import ../flake/packages.nix { inherit pkgs; };

  # Fixtures the hub operator would hand out: the box's token (its direct
  # tenant row on the hub; the spoke learns the same token on first contact)
  # and the spoke's token (its tenant row on the hub). Written by tmpfiles
  # during early boot, like tests/edge-vm.nix.
  proxyToken = "/var/secrets/losos-proxy-token";
  bootstrapToken = "/var/secrets/losos-rathole-bootstrap";
  spokeToken = "/var/secrets/losos-tenant-acme";
  proxyTokenValue = "test-proxy-token-0123456789abcdef0123456789abcdef";
  bootstrapTokenValue = "test-bootstrap-0123456789abcdef0123456789abcdef01";
  spokeTokenValue = "test-spoke-token-0123456789abcdef0123456789abcdef";
  secretFiles = {
    systemd.tmpfiles.rules = [
      "d /var/secrets 0700 root root - -"
      "f ${proxyToken} 0600 root root - ${proxyTokenValue}"
      "f ${bootstrapToken} 0600 root root - ${bootstrapTokenValue}"
      "f ${spokeToken} 0600 root root - ${spokeTokenValue}"
    ];
  };
  selfShim = {
    _module.args.self = {
      packages.x86_64-linux.losos-registrar = lososPkgs.losos-registrar;
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
          ../modules/options.nix
          ../modules/edge.nix
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
          ratholePortRange = "50000-50010";
          # Short, so a spoke that stops relaying takes its boxes with it
          # inside the test's patience.
          heartbeatTtl = "30s";
          reconcileInterval = "2s";
          bootstrapTokenFile = bootstrapToken;
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
          ../modules/options.nix
          ../modules/edge.nix
          ../modules/edge-gateway.nix
          secretFiles
          selfShim
        ];
        networking.hostName = "spoke";
        # The gateway preset is the feature under test; the rest is the
        # test's impatience.
        # The test framework gives root its own password file; the preset's
        # first password would only draw a precedence warning beside it.
        users.users.root.initialPassword = lib.mkForce null;
        losos.edge = {
          gateway.enable = true;
          registrarApiPort = 8443;
          ratholePortRange = "50000-50010";
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
          ../modules/options.nix
          ../modules/daemon.nix
          ../modules/proxy.nix
          secretFiles
        ];
        networking.hostName = "mattbox";
        losos.backend.package = lososPkgs.losos-ctl;
        losos.proxy = {
          enable = true;
          # The official edge, as a real box has it configured: dialled by
          # node name over the test's IPv6, which both hub listeners bind.
          registrarUrl = "http://hub:8443";
          edgeRatholeEndpoint = "hub:2333";
          hostname = "mattbox.acme.losos.cfd";
          applianceId = "mattbox";
          tokenFile = proxyToken;
          bootstrapTokenFile = bootstrapToken;
          heartbeatInterval = "3s";
          registrar.package = lososPkgs.losos-registrar;
        };
        # What modules/configuration.nix gives the real box: Avahi, so the
        # LAN edge's advert is seen and its .local name resolves.
        services.avahi = {
          enable = true;
          nssmdns4 = true;
          openFirewall = true;
          publish = {
            enable = true;
            addresses = true;
          };
        };
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
        environment.systemPackages = [
          pkgs.avahi
          pkgs.jq
        ];
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
    hub.wait_for_unit("losos-rathole.service")
    hub.wait_until_succeeds("curl -fsS http://localhost:8443/health")
    hub_pub = hub.succeed("curl -fsS http://localhost:8443/noise-public-key").strip()
    assert len(hub_pub) == 44, hub_pub

    spoke.wait_for_unit("losos-edge-gateway-secrets.service")
    spoke.wait_for_unit("losos-registrar.service")
    spoke.wait_for_unit("losos-rathole.service")
    spoke.wait_for_unit("avahi-daemon.service")
    spoke.wait_until_succeeds("curl -fsS http://localhost:8443/health")
    spoke_pub = spoke.succeed("curl -fsS http://localhost:8443/noise-public-key").strip()
    # The gateway minted its own bootstrap token and expired the first
    # root password; sshd is installed and not running.
    assert spoke.succeed("stat -c %a /var/secrets/losos-rathole-bootstrap").strip() == "600"
    assert spoke.succeed("chage -l root | grep -i 'last password change'").strip().lower().endswith("password must be changed")
    assert spoke.succeed("systemctl is-active sshd.service || true").strip() == "inactive"
    assert "not set" in spoke.succeed("losos-edge status")

    box.wait_for_unit("avahi-daemon.service")
    box.wait_for_unit("nginx.service")
    box.wait_until_succeeds("losos-ctl state --json")
    box.wait_until_succeeds("curl -fsS 127.0.0.1:8082/api/health")
    token = box.succeed("cat /var/secrets/losos-admin-token").strip()
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
      "avahi-browse --parsable --resolve --terminate _losos-edge._tcp | grep '^=' | grep IPv4"
    )
    assert "url=http://spoke.local:8443" in browse, browse
    assert "rathole=spoke.local:2333" in browse, browse
    assert "enrol=open" in browse, browse

    wait_path("lan")
    doc = edge_doc()
    print("edge:", doc)
    assert doc["path"]["url"] == "http://spoke.local:8443", doc
    assert doc["path"]["rathole"] == "spoke.local:2333", doc
    assert doc["reachable"] is True, doc
    # Both edges answer; the configured one carries its endpoint too.
    by_source = {e["source"]: e for e in doc["edges"]}
    assert by_source["lan"]["rathole"] == "spoke.local:2333", doc
    assert by_source["configured"]["rathole"] == "hub:2333", doc

    env = box.wait_until_succeeds("cat /run/losos/edge-path.env")
    assert "LOSOS_EDGE_PATH_URL=http://spoke.local:8443\n" in env, env
    assert "LOSOS_EDGE_PATH_RATHOLE=spoke.local:2333\n" in env, env
    assert "LOSOS_EDGE_PATH_NOISE_PUB=/var/secrets/losos-edge-pins/spoke.local_8443.pub" in env, env
    box.fail("test -e /run/losos/edge-none")
    box.wait_until_succeeds("systemctl is-active losos-rathole-client.service")
    box.wait_until_succeeds("systemctl is-active losos-registrar-announce.service")
    client = box.wait_until_succeeds("grep -F 'remote_addr = \"spoke.local:2333\"' /run/losos-rathole/client.toml && cat /run/losos-rathole/client.toml")
    assert 'type = "noise"' in client, client
    pinned = box.succeed("cat /var/secrets/losos-edge-pins/spoke.local_8443.pub").strip()
    assert pinned == spoke_pub, f"box pinned {pinned!r}, spoke has {spoke_pub!r}"
    # The announce unit read the path too: its process carries the spoke's URL.
    assert "http://spoke.local:8443" in box.succeed("systemctl show -p ExecStart --value losos-registrar-announce.service || true") or \
      "http://spoke.local:8443" in box.succeed("cat /proc/$(systemctl show -p MainPID --value losos-registrar-announce.service)/cmdline | tr '\\0' ' '")

    # ── 2. Open enrolment, and one hop through the spoke ─────────────────
    spoke.wait_until_succeeds("grep -F -q '[server.services.mattbox]' /etc/rathole/server.toml")
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
      "--registrar http://hub:8443 --rathole hub:2333"
    )
    assert "uplink set: acme" in out, out
    shown = json.loads(spoke.succeed("losos-edge uplink show"))
    assert shown["registrar_url"] == "http://hub:8443" and shown["id"] == "acme", shown
    assert "bootstrap_token_file" not in shown, shown
    assert spoke.succeed("stat -c %a /var/lib/losos-edge/uplink.token").strip() == "600"

    hub.wait_until_succeeds("grep -F -q '[server.services.\"acme.mattbox\"]' /etc/rathole/server.toml")
    hub_toml = hub.succeed("cat /etc/rathole/server.toml")
    block = hub_toml.split('[server.services."acme.mattbox"]')[1]
    assert 'token = "${spokeTokenValue}"' in block, block
    hub_port = block.split('bind_addr = "127.0.0.1:')[1].split('"')[0]
    traefik = hub.succeed("cat /etc/traefik/dynamic/losos.yml")
    assert "Host(`mattbox.acme.losos.cfd`)" in traefik, traefik
    registry = json.loads(hub.succeed("cat /var/lib/losos-registrar/registry.json"))
    assert registry["tenants"]["acme.mattbox"]["via"] == "acme", registry

    uplink = spoke.wait_until_succeeds("grep -F -q 'acme.mattbox' /etc/rathole/uplink.toml && cat /etc/rathole/uplink.toml")
    assert f'local_addr = "127.0.0.1:{spoke_port}"' in uplink, uplink
    assert 'remote_addr = "hub:2333"' in uplink, uplink
    assert f'remote_public_key = "{hub_pub}"' in uplink, uplink
    assert spoke.succeed("cat /var/lib/losos-edge/hub-noise.pub").strip() == hub_pub
    spoke.wait_for_unit("losos-rathole-uplink.service")
    assert "relaying: 1 box" in spoke.succeed("losos-edge status")

    # Two tunnels, one request: in at the hub, out at the box's Nginx.
    assert through(hub, hub_port).strip() == "hello-losos"

    # ── 4. The spoke goes away: fall back to the official edge ───────────
    spoke.succeed("systemctl stop losos-registrar.service losos-rathole.service avahi-daemon.service avahi-daemon.socket")
    wait_path("configured")
    doc = edge_doc()
    assert doc["path"]["url"] == "http://hub:8443" and doc["path"]["rathole"] == "hub:2333", doc
    env = box.succeed("cat /run/losos/edge-path.env")
    assert "LOSOS_EDGE_PATH_RATHOLE=hub:2333\n" in env, env
    assert "LOSOS_EDGE_PATH_NOISE_PUB=/var/secrets/losos-rathole-noise-pub" in env, env
    box.wait_until_succeeds("grep -F -q 'remote_addr = \"hub:2333\"' /run/losos-rathole/client.toml")
    assert box.succeed("cat /var/secrets/losos-rathole-noise-pub").strip() == hub_pub
    # The direct tenant row answers; the relayed one expires with the spoke.
    hub.wait_until_succeeds("grep -F -q '[server.services.mattbox]' /etc/rathole/server.toml")
    direct_port = hub.succeed("cat /etc/rathole/server.toml").split("[server.services.mattbox]")[1].split('bind_addr = "127.0.0.1:')[1].split('"')[0]
    assert through(hub, direct_port).strip() == "hello-losos"
    hub.wait_until_succeeds("! grep -F -q 'acme.mattbox' /etc/rathole/server.toml", timeout=120)

    # ── 5. The hub goes away too: everything edge-dependent is off ───────
    hub.succeed("systemctl stop losos-registrar.service losos-rathole.service")
    wait_path(None)
    doc = edge_doc()
    assert doc["reachable"] is False and doc["edges"] == [], doc
    box.wait_until_succeeds("test -e /run/losos/edge-none")
    box.fail("test -e /run/losos/edge-path.env")
    box.wait_until_succeeds("test \"$(systemctl is-active losos-rathole-client.service)\" = inactive")
    box.wait_until_succeeds("test \"$(systemctl is-active losos-registrar-announce.service)\" = inactive")
    # A stopped unit stays stopped: Restart=always does not apply to a
    # `systemctl stop`, and the condition keeps a manual start from dialling.
    box.succeed("systemctl start losos-rathole-client.service || true")
    assert unit("losos-rathole-client.service") == "inactive"
    code, body = api("GET", "/api/market")
    assert json.loads(body)["available"] is False, body
    code, body = api("POST", "/api/change", {"mode": "mesh"})
    assert code == "409", f"change to mesh with no edge answered {code}: {body!r}"
    assert json.loads(body)["edgeRequired"] is True, body

    # ── 6. The spoke comes back: the LAN path again, units running ───────
    spoke.succeed("systemctl start avahi-daemon.service losos-rathole.service losos-registrar.service")
    spoke.wait_until_succeeds("curl -fsS http://localhost:8443/health")
    wait_path("lan")
    box.wait_until_succeeds("! test -e /run/losos/edge-none")
    box.wait_until_succeeds("systemctl is-active losos-rathole-client.service")
    box.wait_until_succeeds("systemctl is-active losos-registrar-announce.service")
    box.wait_until_succeeds("grep -F -q 'remote_addr = \"spoke.local:2333\"' /run/losos-rathole/client.toml")
    spoke.wait_until_succeeds("grep -F -q '[server.services.mattbox]' /etc/rathole/server.toml")
    assert through(spoke, spoke_port).strip() == "hello-losos"

    # `losos-edge uplink clear` stops relaying and leaves nothing behind.
    spoke.succeed("losos-edge uplink clear")
    spoke.fail("test -e /var/lib/losos-edge/uplink.json")
    spoke.fail("test -e /etc/rathole/uplink.toml")
    assert spoke.succeed("systemctl is-active losos-rathole-uplink.service || true").strip() == "inactive"
  '';
}
