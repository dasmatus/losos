# nixos-test-vms config for a company edge managing the box behind it.
#
# Boots two VMs on one virtual network:
#   * `edge` — losos.edge.enable with losos.edge.lan.advertise and
#              losos.edge.manage.enable: the registrar serves the manage page
#              and its API (edge/registrar/src/server/manage.rs) under a key
#              it makes on first start.
#   * `box`  — lososd (modules/daemon.nix) with its option document
#              (modules/config-repo.nix), the mDNS the real box runs, and the
#              edge path (modules/proxy.nix): rathole client, announce, and
#              the loopback-only nginx server the edge's calls land on.
#
# Asserts what backend/src/edge.rs and backend/sections.json promise:
#   1. the box finds the company edge on the LAN, uses it, and says the
#      owner may let it manage the box; the edge makes its key at 0600;
#   2. before the owner's switch the edge lists the box but relays nothing,
#      and a call forged straight through the tunnel with the box's token is
#      refused by lososd;
#   3. the manage server on the box takes loopback only: the same call from
#      the LAN is turned away by nginx, and lososd ignores the edge header
#      without the token;
#   4. the owner switches it on: the marker appears, the next heartbeat
#      tells the edge, and the edge's calls reach lososd as the edge;
#   5. through the edge, LosOS Cloud and LosOS Git settings and the apps
#      answer; the owner's routes (options, apply, backups, people, the
#      switch itself, a new password) are 403;
#   6. a settings change through the edge rewrites only the line it names
#      and queues a rebuild; one naming anything else is refused and writes
#      nothing;
#   7. the owner switches it off and the edge loses access at once, before
#      its next heartbeat;
#   8. the page is served under its own policy, and deep links reload.
#
# The rebuild each change queues has no flake to build in the VM and fails;
# the test reads overrides.nix, which is what the change writes.
{ pkgs }:

let
  lososPkgs = import ../packages.nix { inherit pkgs; };
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
  # The page the edge serves, small: the admin UI's own bundle is
  # tests/manage.browser.mjs's business, the routes around it are this one's.
  ui = pkgs.runCommand "losos-manage-ui-fixture" { } ''
    mkdir -p $out/manage $out/assets
    echo '<!doctype html><title>LosOS edge</title>' > $out/manage/index.html
    echo 'void 0' > $out/assets/page-test.js
    echo 'void 0' > $out/theme-boot.js
  '';
  # The test network's /etc/hosts names both machines; mDNS finds the
  # advert, and the records it carries name the edge the same way.
  edgeUrl = "http://edge:8443";
in

pkgs.testers.nixosTest {
  name = "losos-edge-manage";

  nodes = {
    edge =
      { ... }:
      {
        imports = [
          ../../modules/options.nix
          ../../modules/edge.nix
          secretFiles
        ];
        _module.args.self = {
          packages.x86_64-linux = {
            losos-registrar = lososPkgs.losos-registrar;
            losos-admin-ui = ui;
          };
        };
        networking.hostName = "edge";
        losos.edge = {
          enable = true;
          acmeEmail = "test@losos.cfd";
          registrarApiPort = 8443;
          ratholeBindPort = 2333;
          ratholePortRange = "50000-50010";
          reconcileInterval = "2s";
          heartbeatTtl = "30s";
          bootstrapTokenFile = bootstrapToken;
          tenants.office-box = {
            hostname = "office-box.example.test";
            tokenFile = proxyToken;
          };
          lan = {
            advertise = true;
            url = edgeUrl;
            ratholeEndpoint = "edge:2333";
          };
          manage = {
            enable = true;
            ui = ui;
          };
        };
        environment.systemPackages = [ pkgs.jq ];
        virtualisation = {
          memorySize = 1024;
          cores = 2;
        };
      };

    box =
      { ... }:
      {
        imports = [
          ../../modules/options.nix
          ../../modules/daemon.nix
          ../../modules/config-repo.nix
          ../../modules/mdns.nix
          ../../modules/proxy.nix
          secretFiles
        ];
        networking.hostName = "office-box";
        losos = {
          backend.package = lososPkgs.losos-ctl;
          # No LosOS Git in this VM: the changes are committed nowhere.
          configRepo.enable = false;
          mdns.enable = true;
          proxy = {
            enable = true;
            edgeRatholeEndpoint = "edge:2333";
            registrarUrl = edgeUrl;
            hostname = "office-box.example.test";
            applianceId = "office-box";
            tokenFile = proxyToken;
            bootstrapTokenFile = bootstrapToken;
            heartbeatInterval = "3s";
            registrar.package = lososPkgs.losos-registrar;
          };
        };
        # The manage server proxy.nix adds; port 80 open so step 3 can try
        # it from the LAN.
        services.nginx.enable = true;
        networking.firewall.allowedTCPPorts = [ 80 ];
        environment.systemPackages = [ pkgs.jq ];
        virtualisation = {
          memorySize = 1536;
          cores = 2;
        };
      };
  };

  testScript = ''
    import json
    import shlex

    TOKEN = "${proxyTokenValue}"

    edge.start()
    box.start()

    edge.wait_for_unit("losos-registrar.service")
    edge.wait_until_succeeds("curl -fsS http://localhost:8443/health")
    key = edge.succeed("cat /var/lib/losos-registrar/manage-key").strip()
    assert len(key) == 64, f"manage key looks wrong: {key!r}"
    assert edge.succeed("stat -c %a /var/lib/losos-registrar/manage-key").strip() == "600"

    box.wait_for_unit("lososd.service")
    box.wait_until_succeeds("curl -fsS 127.0.0.1:8082/api/health")
    admin = box.succeed("systemd-creds decrypt --name=admin-token /etc/credstore.encrypted/admin-token -").strip()

    def call(machine, url, method="GET", data=None, headers=()):
        hs = " ".join(f"-H {shlex.quote(h)}" for h in headers)
        body = ""
        if data is not None:
            body = f"-H 'Content-Type: application/json' --data-binary {shlex.quote(json.dumps(data))}"
        code = machine.succeed(
            f"curl -s -o /tmp/body -w '%{{http_code}}' -X {method} {hs} {body} {shlex.quote(url)}"
        ).strip()
        return code, machine.succeed("cat /tmp/body")

    def owner(method, path, data=None):
        return call(box, f"http://127.0.0.1:8082{path}", method, data, [f"Authorization: Bearer {admin}"])

    def manage(method, path, data=None, k=None):
        return call(edge, f"http://127.0.0.1:8443{path}", method, data, [f"Authorization: Bearer {k or key}"])

    def overrides():
        return box.succeed("cat /etc/nixos/modules/overrides.nix 2>/dev/null || true")

    # 1. The company edge is found on the LAN and used; the box may let it manage.
    box.wait_until_succeeds(
      f"curl -fsS -H 'Authorization: Bearer {admin}' 127.0.0.1:8082/api/edge"
      " | jq -e '.path.source == \"lan\" and .manage.candidate != null"
      " and .manage.refused == null and .manage.managedBy == null and .manage.active == false'",
      timeout=600,
    )
    box.wait_for_unit("losos-rathole-client.service")
    box.wait_for_unit("losos-registrar-announce.service")
    edge.wait_until_succeeds("grep -F -q '[server.services.office-box]' /etc/rathole/server.toml", timeout=300)
    port = edge.succeed(
      "grep -A3 -F '[server.services.office-box]' /etc/rathole/server.toml"
      " | sed -n 's/.*bind_addr = \"127.0.0.1:\\([0-9]*\\)\".*/\\1/p'"
    ).strip()
    assert port, "no rathole port for the box"
    # The tunnel carries the box's own port 80.
    edge.wait_until_succeeds(f"curl -s -o /dev/null -w '%{{http_code}}' -H 'Host: losos-manage.invalid' 127.0.0.1:{port}/ | grep -q 404")

    # 2. Listed, not managed, nothing relayed; a forged call through the
    #    tunnel with the box's own token is refused by lososd.
    code, body = manage("GET", "/manage/api/boxes")
    assert code == "200", (code, body)
    boxes = json.loads(body)["boxes"]
    assert [b["id"] for b in boxes] == ["office-box"], boxes
    assert boxes[0]["managed"] is False, boxes
    code, _ = manage("GET", "/manage/api/boxes/office-box/api/site")
    assert code == "403", code
    code, _ = manage("GET", "/manage/api/boxes", k="0" * 64)
    assert code == "401", code
    code, body = call(edge, f"http://127.0.0.1:{port}/api/site", headers=["Host: losos-manage.invalid", f"X-Losos-Edge: {TOKEN}"])
    assert code == "401", (code, body)

    # 3. Loopback only, and the header alone is nothing.
    code, _ = call(edge, "http://office-box/api/site", headers=["Host: losos-manage.invalid", f"X-Losos-Edge: {TOKEN}"])
    assert code == "403", code
    code, _ = call(box, "http://127.0.0.1:8082/api/site", headers=["X-Losos-Via-Edge: 1", "X-Losos-Edge: wrong"])
    assert code == "401", code

    # 4. The owner switches it on, for the edge in use.
    code, body = owner("POST", "/api/edge/manage", {"enabled": True})
    assert code == "200", (code, body)
    m = json.loads(body)["manage"]
    assert m["active"] is True and m["managedBy"] == "${edgeUrl}", m
    box.succeed("test -e /run/losos/edge-managed")
    edge.wait_until_succeeds(
      f"curl -fsS -H 'Authorization: Bearer {key}' 127.0.0.1:8443/manage/api/boxes"
      " | jq -e '.boxes[0].managed == true'",
      timeout=60,
    )

    # 5. What the edge may reach, and what it may not.
    code, body = manage("GET", "/manage/api/boxes/office-box/api/me")
    assert code == "200", (code, body)
    me = json.loads(body)
    assert me.get("edge") is True and me["admin"] is False, me
    assert "apps" in me["sections"] and "backup" not in me["sections"], me
    code, body = manage("GET", "/manage/api/boxes/office-box/api/site")
    assert code == "200", (code, body)
    names = [o["name"] for o in json.loads(body)["options"]]
    assert "cloud.trashDays" in names and "git.site.defaultPrivate" in names, names
    assert all(n == "cloud.trashDays" or n.startswith("git.site.") for n in names), names
    code, body = manage("GET", "/manage/api/boxes/office-box/api/apps")
    assert code == "200", (code, body)
    for method, path, data in [
        ("GET", "/api/options", None),
        ("POST", "/api/apply", {"x": 1}),
        ("GET", "/api/backup", None),
        ("GET", "/api/people", None),
        ("POST", "/api/edge/manage", {"enabled": False}),
        ("POST", "/api/set-password", {"password": "x"}),
        ("POST", "/api/change", {"mode": "local"}),
    ]:
        code, body = manage(method, f"/manage/api/boxes/office-box{path}", data)
        assert code == "403", (method, path, code, body)

    # 6. A change of a site setting writes only its line; anything else is
    #    refused and writes nothing.
    before = overrides()
    code, body = manage("POST", "/manage/api/boxes/office-box/api/site", {"set": {"hostName": "\"evil\""}})
    assert code == "400", (code, body)
    assert overrides() == before
    code, body = manage("POST", "/manage/api/boxes/office-box/api/site", {"set": {"cloud.trashDays": "45"}})
    assert code == "200", (code, body)
    assert json.loads(body)["job"], body
    after = overrides()
    assert "losos.cloud.trashDays = 45;" in after, after
    assert 'losos.hostName = "mattbox";' in after, after
    assert "evil" not in after, after

    # 7. Switched off: the edge loses access at once, before it hears.
    code, body = owner("POST", "/api/edge/manage", {"enabled": False})
    assert code == "200", (code, body)
    box.fail("test -e /run/losos/edge-managed")
    code, _ = manage("GET", "/manage/api/boxes/office-box/api/site")
    assert code == "403", code
    edge.wait_until_succeeds(
      f"curl -fsS -H 'Authorization: Bearer {key}' 127.0.0.1:8443/manage/api/boxes"
      " | jq -e '.boxes[0].managed == false'",
      timeout=60,
    )

    # 8. The page, under its own policy; a deep link is the page too.
    headers = edge.succeed("curl -fsS -D - -o /tmp/page http://127.0.0.1:8443/manage/")
    assert "frame-ancestors 'none'" in headers, headers
    assert "LosOS edge" in edge.succeed("cat /tmp/page")
    assert "LosOS edge" in edge.succeed("curl -fsS http://127.0.0.1:8443/manage/boxes/office-box")
    # From the LAN too: the edge's operators open it from their own machines.
    assert "LosOS edge" in box.succeed("curl -fsS ${edgeUrl}/manage/")
  '';
}
