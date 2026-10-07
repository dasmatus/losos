# nixos-test-vms config for finding an edge on the LAN, and the sharing gate.
#
# Boots two VMs on one virtual network:
#   * `edge` — losos.edge.enable with losos.edge.lan.advertise: the registrar
#              API bound off-loopback and announced over mDNS as
#              `_losos-edge._tcp` with a `url=` record (modules/edge.nix).
#   * `box`  — the appliance's control plane (modules/daemon.nix, like
#              tests/admin-vm.nix) plus the Avahi the real box runs
#              (modules/configuration.nix gives it; this test enables the same
#              daemon by hand so it stays disko-free).
#
# Asserts what backend/src/edge.rs promises:
#   1. the edge's advert is on the wire: `avahi-browse` on the box resolves
#      the service with the url record;
#   2. lososd's scan finds it: `GET /api/edge` says reachable, source lan,
#      and names the advertised URL;
#   3. with the edge in reach, `change --mode mesh` is accepted (the gate is
#      open), and the rebuild it queues is left alone;
#   4. the edge goes away (its registrar is stopped — the advert can stay,
#      it is the /health probe that decides) and within two scan periods
#      `/api/edge` says no edge, `lanSearched` still true;
#   5. `POST /api/change {mesh}` is now 409 with `edgeRequired`, an apply
#      that turns `losos.cluster.enable` on is 409, an apply that changes only
#      the hostname is accepted, and state.json still says what it said;
#   6. the edge returns and the gate reopens.
#
# The configured URL is pointed at a port nothing listens on, so only the
# LAN road can open the gate here: that is the case the feature exists for
# (an edge beside the boxes, no internet), and a public edge that happened to
# answer from the test host would otherwise make step 4 impossible to reach.
{ pkgs }:

let
  lososPkgs = import ../flake/packages.nix { inherit pkgs; };
  proxyTokenValue = "test-proxy-token-0123456789abcdef";
  bootstrapTokenValue = "test-bootstrap-0123456789abcdef0123";
  proxyToken = "/var/secrets/losos-proxy-token";
  bootstrapToken = "/var/secrets/losos-rathole-bootstrap";
  secretFiles = {
    systemd.tmpfiles.rules = [
      "d /var/secrets 0700 root root - -"
      "f ${proxyToken} 0600 root root - ${proxyTokenValue}"
      "f ${bootstrapToken} 0600 root root - ${bootstrapTokenValue}"
    ];
  };
in

pkgs.testers.nixosTest {
  name = "losos-edge-lan";

  nodes = {
    edge =
      { ... }:
      {
        imports = [
          ../modules/options.nix
          ../modules/edge.nix
          secretFiles
        ];
        _module.args.self = {
          packages.x86_64-linux.losos-registrar = lososPkgs.losos-registrar;
        };
        networking.hostName = "edge";
        losos.edge = {
          enable = true;
          acmeEmail = "test@losos.cfd";
          registrarApiPort = 8443;
          ratholePortRange = "50000-50010";
          reconcileInterval = "2s";
          bootstrapTokenFile = bootstrapToken;
          tenants.mattbox = {
            hostname = "mattbox.losos.cfd";
            tokenFile = proxyToken;
          };
          # The feature under test. The url defaults to the edge's own
          # mDNS name; the box resolves it through nss-mdns.
          lan.advertise = true;
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
        ];
        networking.hostName = "mattbox";
        losos.backend.package = lososPkgs.losos-ctl;
        # Nothing at the configured URL: see the header.
        losos.proxy.registrarUrl = "http://127.0.0.1:9";
        # What modules/configuration.nix gives the real box.
        services.avahi = {
          enable = true;
          nssmdns4 = true;
          openFirewall = true;
          publish = {
            enable = true;
            addresses = true;
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

    edge.start()
    box.start()

    edge.wait_for_unit("losos-registrar.service")
    edge.wait_for_unit("avahi-daemon.service")
    edge.wait_until_succeeds("curl -fsS http://localhost:8443/health")

    box.wait_for_unit("avahi-daemon.service")
    box.wait_until_succeeds("losos-ctl state --json")
    box.wait_until_succeeds("curl -fsS 127.0.0.1:8082/api/health")
    token = box.succeed("cat /var/secrets/losos-admin-token").strip()
    hdr = f"Authorization: Bearer {token}"

    def api(method, path, data=None, ctype="application/json"):
        extra = f"-H 'Content-Type: {ctype}' --data-binary @- " if data is not None else ""
        cmd = (
            f"curl -s -o /tmp/body -w '%{{http_code}}' -X {method} -H '{hdr}' {extra}"
            f"127.0.0.1:8082{path}"
        )
        if data is not None:
            payload = json.dumps(data) if ctype == "application/json" else data
            cmd = f"printf %s {shlex.quote(payload)} | " + cmd
        code = box.succeed(cmd).strip()
        body = box.succeed("cat /tmp/body")
        return code, body

    # 1. The advert is on the wire and carries the url record.
    browse = box.wait_until_succeeds(
      "avahi-browse --parsable --resolve --terminate _losos-edge._tcp | grep '^=' | grep IPv4"
    )
    print("browse:", browse)
    assert "url=http://edge.local:8443" in browse, f"advert without the url record: {browse!r}"

    # 2. lososd's scan finds the edge over the LAN road.
    def edge_doc():
        code, body = api("GET", "/api/edge")
        assert code == "200", f"/api/edge answered {code}: {body!r}"
        return json.loads(body)

    def wait_edge(reachable):
        def probe(_):
            return edge_doc()["reachable"] == reachable
        with box.nested(f"waiting for /api/edge reachable={reachable}"):
            retry(probe, timeout_seconds=90)

    wait_edge(True)
    doc = edge_doc()
    print("edge:", doc)
    assert doc["lanSearched"] is True, doc
    assert doc["edges"][0]["source"] == "lan", doc
    assert doc["edges"][0]["url"] == "http://edge.local:8443", doc
    assert doc["edges"][0]["name"].startswith("losos edge on"), doc
    assert doc["configuredUrl"] == "http://127.0.0.1:9", doc
    facade = json.loads(box.succeed("losos-ctl edge --json"))
    assert facade["reachable"] is True, facade

    # 3. The gate is open: mesh mode is accepted. The rebuild it queues runs
    #    `nixos-rebuild` against no flake and fails on its own; that is the
    #    supervisor's business and not this test's. Reset the posture after.
    code, body = api("POST", "/api/change", {"mode": "mesh"})
    if code != "200":
        print(box.execute("journalctl -u lososd --no-pager -n 40")[1])
        print(box.execute("cat /var/lib/losos/state.json /var/lib/losos/rebuild.log")[1])
    assert code == "200", f"change to mesh with an edge in reach answered {code}: {body!r}"
    state = json.loads(box.succeed("losos-ctl state --json"))
    assert state["mode"] == "mesh" and state["sharing"] is True, state
    code, body = api("POST", "/api/change", {"mode": "local"})
    assert code == "200", f"change to local answered {code}: {body!r}"

    # 4. The edge goes away. The advert may linger in caches; the /health
    #    probe is what decides, so stopping the registrar is enough.
    edge.succeed("systemctl stop losos-registrar.service")
    wait_edge(False)
    doc = edge_doc()
    assert doc["edges"] == [], doc
    assert doc["lanSearched"] is True, doc

    # 5. Refusals, in the daemon's words, with nothing written.
    # The rebuild the accepted change queued fails on its own (no flake in
    # this VM) and its watcher rewrites the record when it does; let it
    # settle so the snapshot below moves only if the refused request moves it.
    box.wait_until_succeeds(
      "losos-ctl status --json | jq -e '.state != \"building\"'", timeout=300
    )
    before = box.succeed("cat /var/lib/losos/state.json")
    code, body = api("POST", "/api/change", {"mode": "mesh"})
    assert code == "409", f"change to mesh with no edge answered {code}: {body!r}"
    refused = json.loads(body)
    assert refused["edgeRequired"] is True, refused
    assert refused["setting"] == "losos.sharingMyStorage", refused
    assert "no edge proxy is reachable" in refused["error"], refused
    assert "Local use keeps working" in refused["error"], refused

    settings = json.loads(box.succeed("losos-ctl settings --json"))
    assert settings["clusterEnable"] is False, settings
    overrides = box.succeed("cat /etc/nixos/modules/overrides.nix 2>/dev/null || true")
    joining = (
      "{ ... }:\n{\n"
      "  losos.sharingMyStorage = false;\n"
      "  losos.cluster.enable = true;\n"
      "  losos.hostName = \"mattbox\";\n"
      "}\n"
    )
    code, body = api("POST", "/api/apply", joining, ctype="text/plain")
    assert code == "409", f"apply joining the mesh with no edge answered {code}: {body!r}"
    assert json.loads(body)["setting"] == "losos.cluster.enable", body
    after = box.succeed("cat /var/lib/losos/state.json")
    assert before == after, f"state.json changed on a refused request:\n{before}\n{after}"
    assert "cluster.enable = true" not in box.succeed(
      "cat /etc/nixos/modules/overrides.nix 2>/dev/null || true"
    ), "overrides.nix was written for a refused apply"

    # The gate is about sharing: an apply that only renames the box passes.
    renaming = (
      "{ ... }:\n{\n"
      "  losos.sharingMyStorage = false;\n"
      "  losos.cluster.enable = false;\n"
      "  losos.hostName = \"salmon\";\n"
      "}\n"
    )
    code, body = api("POST", "/api/apply", renaming, ctype="text/plain")
    assert code == "200", f"a hostname-only apply with no edge answered {code}: {body!r}"
    settings = json.loads(box.succeed("losos-ctl settings --json"))
    assert settings["hostName"] == "salmon", settings

    # The facade sees the same refusal over the bus.
    out = box.fail("losos-ctl change --mode mesh 2>&1")
    assert "no edge proxy is reachable" in out, out

    # 6. The edge comes back; the gate reopens.
    edge.succeed("systemctl start losos-registrar.service")
    edge.wait_until_succeeds("curl -fsS http://localhost:8443/health")
    wait_edge(True)
    code, body = api("POST", "/api/change", {"mode": "mesh"})
    assert code == "200", f"change to mesh after the edge returned answered {code}: {body!r}"
  '';
}
