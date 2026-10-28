# The three tiers of an edge link (handbook/docs/in-depth/edge-federation.md,
# "Three tiers"), end to end: five VMs on one LAN.
#
#   * `hub1`, `hub2` — two official edges behind one load balancer, the
#                      official cluster `eu`. Each lists both as official
#                      peers, and carries the gateways' site (`acme`) and the
#                      box's own row (`mattbox`).
#   * `gwa`, `gwb`   — two gateways (losos.edge.gateway.enable) that form the
#                      secondary cluster `office` with `losos-edge cluster`
#                      and relay to `hub1`.
#   * `box`          — lososd with the master proxy on, configured for `hub1`.
#
# What it asserts, in order:
#   1. `losos-edge cluster create` on gwa and the printed `join` on gwb make
#      one cluster: each lists both members, and each says so at
#      GET /cluster/secondary;
#   2. both gateways uplink to hub1: their links are primary, in
#      main-cluster.json on the hub and in `losos-edge main`;
#   3. the box takes a secondary link through gwa (first by name), settles in
#      `office`, and one hop through gwa reaches it;
#   4. gwa goes away: the box fails over to gwb in the same cluster, and gwb
#      knows the box's token from the cluster's sync (no re-enrolment);
#   5. gwb goes away too: the box falls back to a tertiary link to hub1 and
#      lists hub2 as an official peer of cluster `eu`;
#   6. hub1 is banned on the box: it moves to hub2 (`leftBanned`), pins
#      hub2's key apart from hub1's, and hub2's row for it answers;
#   7. gwa comes back with hub1 banned on the gateway: the box returns to its
#      cluster, and gwa moves its uplink to hub2, which relays the box;
#   8. a custom domain holds the box: POST /api/edge/cluster answers 409
#      heldByDomain, the box stays on gwa while gwa is gone, and a ban on gwa
#      moves it with the hold following.
#
# As in nix/tests/edge-federation.nix, nothing needs public DNS or TLS: the
# data path is driven through rathole's per-service loopback ports. No edge
# is proved official (the test ships no root key), so the box's tertiary
# choice is the configured edge first; the load-balancing weight is covered
# by the unit tests in backend/src/edge.rs.
{ pkgs }:

let
  lososPkgs = import ../packages.nix { inherit pkgs; };

  proxyToken = "/var/secrets/losos-proxy-token";
  bootstrapToken = "/var/secrets/losos-rathole-bootstrap";
  siteToken = "/var/secrets/losos-tenant-acme";
  proxyTokenValue = "test-proxy-token-0123456789abcdef0123456789abcdef";
  bootstrapTokenValue = "test-bootstrap-0123456789abcdef0123456789abcdef01";
  siteTokenValue = "test-site-token-0123456789abcdef0123456789abcdef0";
  secretFiles = {
    systemd.tmpfiles.rules = [
      "d /var/secrets 0700 root root - -"
      "f ${proxyToken} 0600 root root - ${proxyTokenValue}"
      "f ${bootstrapToken} 0600 root root - ${bootstrapTokenValue}"
      "f ${siteToken} 0600 root root - ${siteTokenValue}"
    ];
  };
  selfShim = {
    _module.args.self = {
      packages.x86_64-linux = {
        inherit (lososPkgs) losos-registrar;
      };
    };
  };

  hub =
    name:
    { ... }:
    {
      imports = [
        ../../modules/options.nix
        ../../modules/edge.nix
        secretFiles
        selfShim
      ];
      networking.hostName = name;
      losos.edge = {
        enable = true;
        acmeEmail = "test@losos.cfd";
        registrarApiBind = "::";
        registrarApiPort = 8443;
        ratholePortRange = "50000-50010";
        heartbeatTtl = "30s";
        reconcileInterval = "2s";
        bootstrapTokenFile = bootstrapToken;
        official = {
          cluster = "eu";
          peers = [
            {
              cluster = "eu";
              url = "http://hub1:8443";
              tunnel = "hub1:2333";
            }
            {
              cluster = "eu";
              url = "http://hub2:8443";
              tunnel = "hub2:2333";
            }
          ];
        };
        tenants = {
          acme = {
            hostname = "acme.losos.cfd";
            tokenFile = siteToken;
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
        cores = 1;
      };
    };

  gateway =
    name:
    { lib, ... }:
    {
      imports = [
        ../../modules/options.nix
        ../../modules/edge.nix
        ../../modules/edge-gateway.nix
        secretFiles
        selfShim
      ];
      networking.hostName = name;
      users.users.root.initialPassword = lib.mkForce null;
      losos.edge = {
        gateway.enable = true;
        # The manage page (the admin UI's manage/ build) is
        # nix/tests/edge-manage.nix's business, and not this one's.
        manage.enable = false;
        registrarApiPort = 8443;
        ratholePortRange = "50000-50010";
        reconcileInterval = "2s";
        heartbeatTtl = "30s";
        uplink.interval = "3s";
        secondary.interval = "3s";
      };
      virtualisation = {
        memorySize = 1024;
        cores = 1;
      };
    };
in

pkgs.testers.nixosTest {
  name = "losos-edge-clusters";

  nodes = {
    hub1 = hub "hub1";
    hub2 = hub "hub2";
    gwa = gateway "gwa";
    gwb = gateway "gwb";

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
          registrarUrl = "http://hub1:8443";
          edgeRatholeEndpoint = "hub1:2333";
          hostname = "mattbox.acme.losos.cfd";
          applianceId = "mattbox";
          tokenFile = proxyToken;
          bootstrapTokenFile = bootstrapToken;
          heartbeatInterval = "3s";
          registrar.package = lososPkgs.losos-registrar;
        };
        losos.mdns.enable = true;
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
          cores = 1;
        };
      };
  };

  testScript = ''
    import json
    import shlex

    for m in (hub1, hub2, gwa, gwb):
        m.start()
    for h in (hub1, hub2):
        h.wait_for_unit("losos-registrar.service")
        h.wait_until_succeeds("curl -fsS http://localhost:8443/health")
    hub_pub = {
        h.name: h.succeed("curl -fsS http://localhost:8443/noise-public-key").strip()
        for h in (hub1, hub2)
    }
    assert hub_pub["hub1"] != hub_pub["hub2"], hub_pub

    # An official edge says what it is to a box, and lists its cluster.
    info = json.loads(hub1.succeed("curl -fsS http://localhost:8443/cluster/secondary"))
    print("hub1:", info)
    assert info["tier"] == "tertiary" and info["officialCluster"] == "eu", info
    assert {o["url"] for o in info["officials"]} == {"http://hub1:8443", "http://hub2:8443"}, info

    for g in (gwa, gwb):
        g.wait_for_unit("losos-registrar.service")
        g.wait_for_unit("systemd-resolved.service")
        g.wait_until_succeeds("curl -fsS http://localhost:8443/health")

    def secondary(g):
        return json.loads(g.succeed("curl -fsS http://localhost:8443/cluster/secondary"))

    # ── 1. Two gateways, one secondary cluster ───────────────────────────
    assert "in no secondary cluster" in gwa.succeed("losos-edge cluster show")
    out = gwa.succeed("losos-edge cluster create office")
    print(out)
    join = next(l.strip() for l in out.splitlines() if l.strip().startswith("losos-edge cluster join "))
    assert join.endswith(" http://gwa.local:8443"), join
    assert gwa.succeed("stat -c %a /var/lib/losos-edge/cluster.json").strip() == "600"
    gwa.fail("losos-edge cluster create office")
    gwb.succeed(join)

    def members(g):
        return sorted(x["member"] for x in secondary(g)["gateways"])

    for g in (gwa, gwb):
        retry(lambda _: members(g) == ["gwa", "gwb"], timeout_seconds=120)
        doc = secondary(g)
        assert doc["cluster"] == "office" and doc["tier"] == "secondary", doc
    shown = gwb.succeed("losos-edge cluster show")
    assert "cluster:   office" in shown and "member:    gwa" in shown, shown
    assert "secondary cluster office as gwa" in gwa.succeed("losos-edge status")

    # ── 2. Primary links into the main cluster ──────────────────────────
    for g in (gwa, gwb):
        g.succeed(
          "losos-edge uplink set --id acme --token-file ${siteToken} "
          "--registrar http://hub1:8443 --rathole hub1:2333"
        )

    def main_rows(h):
        try:
            return json.loads(h.succeed("cat /var/lib/losos-registrar/main-cluster.json"))
        except Exception:
            return []

    retry(lambda _: sorted(r["gateway"] for r in main_rows(hub1)) == ["gwa", "gwb"], timeout_seconds=120)
    rows = main_rows(hub1)
    assert all(r["tier"] == "primary" and r["cluster"] == "office" for r in rows), rows
    gwa.wait_until_succeeds("losos-edge main | grep -q 'gwb.*primary'")
    assert "primary to the hub" in gwa.succeed("losos-edge status")

    # ── 3. The box takes a secondary link and settles in the cluster ─────
    box.start()
    box.wait_for_unit("systemd-resolved.service")
    box.wait_for_unit("nginx.service")
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

    def wait_path(url, tier):
        def probe(_):
            p = edge_doc()["path"]
            return p is not None and p["url"] == url and p["tier"] == tier
        with box.nested(f"waiting for a {tier} path to {url}"):
            retry(probe, timeout_seconds=240)
        return edge_doc()

    def env():
        return box.succeed("cat /run/losos/edge-path.env")

    def through(machine, port):
        return machine.wait_until_succeeds(f"curl -fsS --max-time 5 http://127.0.0.1:{port}/", timeout=120)

    def box_port(machine, service):
        machine.wait_until_succeeds(f"grep -F -q '[server.services.{service}]' /etc/rathole/server.toml")
        toml = machine.succeed("cat /etc/rathole/server.toml")
        return toml.split(f"[server.services.{service}]")[1].split('bind_addr = "127.0.0.1:')[1].split('"')[0]

    doc = wait_path("http://gwa.local:8443", "secondary")
    print("edge:", doc)
    assert doc["path"]["cluster"] == "office" and doc["path"]["member"] == "gwa", doc
    assert doc["cluster"] == "office", doc
    assert {"name": "office", "gateways": ["gwa", "gwb"]} in [
        {"name": c["name"], "gateways": sorted(c["gateways"])} for c in doc["clusters"]
    ], doc
    assert box.succeed("cat /var/lib/losos/edge-cluster").strip() == "office"
    e = env()
    assert "LOSOS_EDGE_PATH_TIER=secondary\n" in e and "LOSOS_EDGE_PATH_CLUSTER=office\n" in e, e
    box.wait_until_succeeds("systemctl is-active losos-rathole-client.service")
    assert through(gwa, box_port(gwa, "mattbox")).strip() == "hello-losos"

    # ── 4. gwa goes away: failover inside the cluster ────────────────────
    stop_gw = (
        "systemctl stop losos-registrar.service losos-rathole.service "
        "systemd-resolved.service systemd-resolved-varlink.socket systemd-resolved-monitor.socket"
    )
    start_gw = "systemctl start systemd-resolved.service losos-rathole.service losos-registrar.service"
    gwa.succeed(stop_gw)
    doc = wait_path("http://gwb.local:8443", "secondary")
    assert doc["cluster"] == "office", doc
    box.wait_until_succeeds("grep -F -q 'remote_addr = \"gwb.local:2333\"' /run/losos-rathole/client.toml")
    # gwb learned the box from gwa's sync: the same token, no new enrolment.
    assert gwb.succeed("cat /var/lib/losos-registrar/enrolled/mattbox.token").strip() == "${proxyTokenValue}"
    assert through(gwb, box_port(gwb, "mattbox")).strip() == "hello-losos"

    # ── 5. gwb goes away too: a tertiary link to the official edge ───────
    gwb.succeed(stop_gw)
    doc = wait_path("http://hub1:8443", "tertiary")
    assert doc["path"]["officialCluster"] == "eu", doc
    assert doc["cluster"] == "office", doc
    by_url = {x["url"]: x for x in doc["edges"]}
    assert by_url["http://hub2:8443"]["officialCluster"] == "eu", doc
    assert by_url["http://hub2:8443"]["tier"] == "tertiary", doc
    e = env()
    assert "LOSOS_EDGE_PATH_TIER=tertiary\n" in e and "LOSOS_EDGE_PATH_OFFICIAL_CLUSTER=eu\n" in e, e
    assert through(hub1, box_port(hub1, "mattbox")).strip() == "hello-losos"

    # ── 6. hub1 is banned: the box moves to hub2 on its own ──────────────
    box.succeed("install -d -m 0755 /var/lib/losos && echo '{\"edges\":[\"http://hub1:8443\"]}' > /var/lib/losos/edge-bans.json")
    doc = wait_path("http://hub2:8443", "tertiary")
    assert doc["leftBanned"] == "http://hub1:8443", doc
    e = env()
    assert "LOSOS_EDGE_PATH_RATHOLE=hub2:2333\n" in e, e
    assert "LOSOS_EDGE_PATH_NOISE_PUB=/var/secrets/losos-edge-pins/hub2_8443.pub" in e, e
    box.wait_until_succeeds("grep -F -q 'remote_addr = \"hub2:2333\"' /run/losos-rathole/client.toml")
    assert box.succeed("cat /var/secrets/losos-edge-pins/hub2_8443.pub").strip() == hub_pub["hub2"]
    assert through(hub2, box_port(hub2, "mattbox")).strip() == "hello-losos"

    # ── 7. gwa comes back; hub1 banned on the gateway moves its uplink ───
    gwa.succeed("install -d /var/lib/losos-registrar && echo '{\"edges\":[\"http://hub1:8443\"]}' > /var/lib/losos-registrar/bans.json")
    gwa.succeed(start_gw)
    doc = wait_path("http://gwa.local:8443", "secondary")
    assert doc["leftBanned"] is None, doc
    gwa.wait_until_succeeds("jq -e '.registrar_url == \"http://hub2:8443\"' /var/lib/losos-edge/uplink.json", timeout=120)
    hub2.wait_until_succeeds("grep -F -q '[server.services.\"acme.mattbox\"]' /etc/rathole/server.toml", timeout=180)
    registry = json.loads(hub2.succeed("cat /var/lib/losos-registrar/registry.json"))
    assert registry["tenants"]["acme.mattbox"]["via"] == "acme", registry
    gwa.wait_until_succeeds("grep -F -q 'remote_addr = \"hub2:2333\"' /etc/rathole/uplink.toml")

    # ── 8. A custom domain holds the box to its edge ─────────────────────
    box.succeed("install -d /var/lib/losos-public-names && echo '[\"mattbox.example\"]' > /var/lib/losos-public-names/domains.json")
    retry(lambda _: edge_doc()["held"] == "http://gwa.local:8443", timeout_seconds=120)
    code, body = api("POST", "/api/edge/cluster", {"cluster": None})
    assert code == "409" and json.loads(body).get("heldByDomain") is True, (code, body)
    # gwa goes away again; the domain keeps the box on it, no failover.
    gwa.succeed(stop_gw)
    box.sleep(30)
    doc = edge_doc()
    assert doc["path"]["url"] == "http://gwa.local:8443" and doc["held"] == "http://gwa.local:8443", doc
    # A ban is the one thing that moves a held box; the hold goes with it.
    box.succeed("echo '{\"edges\":[\"http://hub1:8443\",\"http://gwa.local:8443\"]}' > /var/lib/losos/edge-bans.json")
    doc = wait_path("http://hub2:8443", "tertiary")
    assert doc["leftBanned"] == "http://gwa.local:8443", doc
    retry(lambda _: edge_doc()["held"] == "http://hub2:8443", timeout_seconds=60)
    assert "LOSOS_EDGE_PATH_URL=http://hub2:8443" in box.succeed("cat /var/lib/losos/edge-hold.env")
  '';
}
