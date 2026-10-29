# The three tiers of an edge link (handbook/docs/in-depth/edge-federation.md,
# "Three tiers"), end to end: five VMs on one LAN.
#
#   * `hub1`, `hub2` — two official edges behind one load balancer, the
#                      official cluster `eu`. Each lists both as official
#                      peers, and carries the gateways' site (`acme`) and the
#                      box's own row (`mattbox`).
#   * `gwa`, `gwb`   — two gateways (losos.edge.gateway.enable) that form the
#                      secondary cluster `office` through their manage API
#                      (what the web UI calls) and relay to `hub1`.
#   * `box`          — lososd with the master proxy on, configured for `hub1`.
#
# What it asserts, in order:
#   1. starting a cluster on gwa's manage API and joining it on gwb's with
#      the secret gwa's page shows make one cluster: each lists both
#      members, and each says so at GET /cluster/secondary;
#   2. both gateways uplink to hub1 from the manage API: their links are
#      primary, in main-cluster.json on the hub and in the gateway's own
#      GET /manage/api/gateway;
#   3. the box takes a secondary link through gwa (first by name), settles in
#      `office`, and one hop through gwa reaches it;
#   4. gwa goes away: the box fails over to gwb in the same cluster, and gwb
#      knows the box's token from the cluster's sync (no re-enrolment);
#   5. gwb goes away too: the box falls back to a tertiary link to hub1 and
#      lists hub2 as an official peer of cluster `eu`;
#   6. hub1 is banned on the box: it moves to hub2 (`leftBanned`), pins
#      hub2's tunnel certificate apart from hub1's, and hub2's row for it
#      answers;
#   7. gwa comes back with hub1 banned on the gateway: the box returns to its
#      cluster, and gwa moves its uplink to hub2, pins hub2's certificate
#      afresh, and hub2 relays the box;
#   8. a custom domain holds the box: POST /api/edge/cluster answers 409
#      heldByDomain, the box stays on gwa while gwa is gone, and a ban on gwa
#      moves it with the hold following.
#
# As in nix/tests/edge-federation.nix, nothing needs public DNS or TLS: the
# data path is driven through the tunnel's per-service loopback ports. The two
# hubs are open gateways the box vets for itself (nix/tests/open-gateway.nix:
# a TPM, an owner's passport, LosOS's payout addresses), which is what makes
# them official and their cluster `eu` count; the box's tertiary choice is
# still the configured edge first, and the load-balancing weight is covered
# by the unit tests in backend/src/edge.rs.
{ pkgs }:

let
  lososPkgs = import ../packages.nix { inherit pkgs; };
  openGateway = import ./open-gateway.nix { inherit pkgs lososPkgs; };

  proxyToken = "/var/secrets/losos-proxy-token";
  siteToken = "/var/secrets/losos-tenant-acme";
  proxyTokenValue = "test-proxy-token-0123456789abcdef0123456789abcdef";
  siteTokenValue = "test-site-token-0123456789abcdef0123456789abcdef0";
  secretFiles = {
    systemd.tmpfiles.rules = [
      "d /var/secrets 0700 root root - -"
      "f ${proxyToken} 0600 root root - ${proxyTokenValue}"
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
        (openGateway.edge "http://${name}:8443")
        secretFiles
        selfShim
      ];
      networking.hostName = name;
      losos.edge = {
        enable = true;
        acmeEmail = "test@losos.cfd";
        registrarApiBind = "::";
        registrarApiPort = 8443;
        tunnelPortRange = "50000-50010";
        heartbeatTtl = "30s";
        reconcileInterval = "2s";
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
        # The gateway is set up through its manage API; the page itself
        # (the admin UI's manage/ build) is nix/tests/edge-manage.nix's.
        manage.enable = true;
        manage.ui = null;
        registrarApiPort = 8443;
        tunnelPortRange = "50000-50010";
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
          openGateway.box
          secretFiles
        ];
        networking.hostName = "mattbox";
        losos.backend.package = lososPkgs.losos-ctl;
        losos.proxy = {
          enable = true;
          registrarUrl = "http://hub1:8443";
          edgeTunnelEndpoint = "hub1:2333";
          hostname = "mattbox.acme.losos.cfd";
          applianceId = "mattbox";
          tokenFile = proxyToken;
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

  testScript = openGateway.script + ''
    import hashlib
    import json
    import shlex

    # One after another: four machines booting at once under TCG miss the
    # driver's wait for a shell.
    for m in (hub1, hub2, gwa, gwb):
        m.start()
        m.succeed("true")
    for h in (hub1, hub2):
        h.wait_for_unit("losos-registrar.service")
        h.wait_until_succeeds("curl -fsS http://localhost:8443/health")
    for h in (hub1, hub2):
        h.wait_for_unit("losos-tunnel.service")
        h.wait_until_succeeds("curl -fsS http://localhost:8443/tunnel-pin")
    hub_pin = {
        h.name: h.succeed("curl -fsS http://localhost:8443/tunnel-pin").strip()
        for h in (hub1, hub2)
    }
    assert hub_pin["hub1"] != hub_pin["hub2"], hub_pin

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

    # The gateway has no shell command: everything goes through the manage
    # API the web UI uses, signed in with the key tty1 shows.
    def manage(g, method, path, data=None):
        key = g.succeed("cat /var/lib/losos-registrar/manage-key").strip()
        extra = "-H 'Content-Type: application/json' --data-binary @- " if data is not None else ""
        cmd = (
            f"curl -s -o /tmp/body -w '%{{http_code}}' -X {method} "
            f"-H 'Authorization: Bearer {key}' {extra}http://localhost:8443{path}"
        )
        if data is not None:
            cmd = f"printf %s {shlex.quote(json.dumps(data))} | " + cmd
        code = g.succeed(cmd).strip()
        return code, g.succeed("cat /tmp/body")

    def gateway(g):
        code, body = manage(g, "GET", "/manage/api/gateway")
        assert code == "200", f"{g.name}: /manage/api/gateway answered {code}: {body!r}"
        return json.loads(body)

    # ── 1. Two gateways, one secondary cluster ───────────────────────────
    assert gateway(gwa)["cluster"] == {"joined": None}, gateway(gwa)
    code, body = manage(gwa, "PUT", "/manage/api/gateway/cluster", {"name": "office"})
    assert code == "200", f"create answered {code}: {body!r}"
    made = json.loads(body)
    print("created:", made)
    assert made["name"] == "office" and made["member"] == "gwa", made
    assert made["url"] == "http://gwa.local:8443", made
    assert gwa.succeed("stat -c %a /var/lib/losos-edge/cluster.json").strip() == "600"
    code, body = manage(gwa, "PUT", "/manage/api/gateway/cluster", {"name": "office"})
    assert code == "409", f"a second create answered {code}: {body!r}"
    code, body = manage(
        gwb, "PUT", "/manage/api/gateway/cluster",
        {"name": "office", "secret": made["secret"], "peer": made["url"]},
    )
    assert code == "200", f"join answered {code}: {body!r}"
    assert json.loads(body)["member"] == "gwb", body

    def members(g):
        return sorted(x["member"] for x in secondary(g)["gateways"])

    for g in (gwa, gwb):
        retry(lambda _: members(g) == ["gwa", "gwb"], timeout_seconds=120)
        doc = secondary(g)
        assert doc["cluster"] == "office" and doc["tier"] == "secondary", doc
    retry(
        lambda _: sorted(x["member"] for x in gateway(gwb)["cluster"]["joined"]["gateways"]) == ["gwa", "gwb"],
        timeout_seconds=60,
    )
    joined = gateway(gwb)["cluster"]["joined"]
    assert joined["name"] == "office" and joined["secret"] == made["secret"], joined

    # ── 2. Primary links into the main cluster ──────────────────────────
    site_token = gwa.succeed("cat ${siteToken}").strip()
    for g in (gwa, gwb):
        code, body = manage(
            g, "PUT", "/manage/api/gateway/uplink",
            {
                "id": "acme",
                "token": site_token,
                "registrarUrl": "http://hub1:8443",
                "tunnelEndpoint": "hub1:2333",
            },
        )
        assert code == "200", f"{g.name}: uplink answered {code}: {body!r}"

    def main_rows(h):
        try:
            return json.loads(h.succeed("cat /var/lib/losos-registrar/main-cluster.json"))
        except Exception:
            return []

    retry(lambda _: sorted(r["gateway"] for r in main_rows(hub1)) == ["gwa", "gwb"], timeout_seconds=120)
    rows = main_rows(hub1)
    assert all(r["tier"] == "primary" and r["cluster"] == "office" for r in rows), rows

    def gwb_primary_on_gwa(_):
        return any(r["gateway"] == "gwb" and r["tier"] == "primary" for r in gateway(gwa)["main"])

    retry(gwb_primary_on_gwa, timeout_seconds=120)
    up = gateway(gwa)["uplink"]
    assert up["set"]["id"] == "acme" and up["set"]["registrarUrl"] == "http://hub1:8443", up

    # ── 3. The box takes a secondary link and settles in the cluster ─────
    box.start()
    box.wait_for_unit("systemd-resolved.service")
    box.wait_for_unit("nginx.service")
    box.wait_until_succeeds("curl -fsS 127.0.0.1:8082/api/health")
    # The box vets the hubs against the image they booted.
    trust(hub1, box)
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
        machine.wait_until_succeeds(f"grep -F -q '\"{service}\":' /etc/losos-tunnel/server.json")
        services = json.loads(machine.succeed("cat /etc/losos-tunnel/server.json"))["services"]
        return services[service]["bind"].rsplit(":", 1)[1]

    def client_remote(remote):
        box.wait_until_succeeds(f"grep -F -q '\"remote\": \"{remote}\"' /run/losos-tunnel/client.json")

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
    box.wait_until_succeeds("systemctl is-active losos-tunnel-client.service")
    assert through(gwa, box_port(gwa, "mattbox")).strip() == "hello-losos"

    # ── 4. gwa goes away: failover inside the cluster ────────────────────
    stop_gw = (
        "systemctl stop losos-registrar.service losos-tunnel.service "
        "systemd-resolved.service systemd-resolved-varlink.socket systemd-resolved-monitor.socket"
    )
    start_gw = "systemctl start systemd-resolved.service losos-tunnel.service losos-registrar.service"
    gwa.succeed(stop_gw)
    doc = wait_path("http://gwb.local:8443", "secondary")
    assert doc["cluster"] == "office", doc
    client_remote("gwb.local:2333")
    # gwb learned the box from gwa's sync: the same token, no new enrolment.
    assert gwb.succeed("cat /var/lib/losos-registrar/enrolled/mattbox.token").strip() == "${proxyTokenValue}"
    assert through(gwb, box_port(gwb, "mattbox")).strip() == "hello-losos"

    # ── 5. gwb goes away too: a tertiary link to the official edge ───────
    gwb.succeed(stop_gw)
    # Both hubs are official and in one cluster, so the box spreads over
    # them by rendezvous weight of its name (backend/src/edge.rs `weight`),
    # not by which one it was configured with.
    def weight(url):
        return hashlib.sha256(b"mattbox\0" + url.encode()).digest()
    hubs = {"http://hub1:8443": hub1, "http://hub2:8443": hub2}
    near, far = sorted(hubs, key=weight, reverse=True)
    doc = wait_path(near, "tertiary")
    assert doc["path"]["officialCluster"] == "eu", doc
    assert doc["cluster"] == "office", doc
    by_url = {x["url"]: x for x in doc["edges"]}
    assert by_url[far]["officialCluster"] == "eu", doc
    assert by_url[far]["tier"] == "tertiary", doc
    e = env()
    assert "LOSOS_EDGE_PATH_TIER=tertiary\n" in e and "LOSOS_EDGE_PATH_OFFICIAL_CLUSTER=eu\n" in e, e
    assert through(hubs[near], box_port(hubs[near], "mattbox")).strip() == "hello-losos"

    # ── 6. that hub is banned: the box moves to the other on its own ─────
    box.succeed(f"install -d -m 0755 /var/lib/losos && echo '{{\"edges\":[\"{near}\"]}}' > /var/lib/losos/edge-bans.json")
    doc = wait_path(far, "tertiary")
    assert doc["leftBanned"] == near, doc
    e = env()
    far_host = hubs[far].name
    assert f"LOSOS_EDGE_PATH_TUNNEL={far_host}:2333\n" in e, e
    pin = [line.split("=", 1)[1] for line in e.splitlines() if line.startswith("LOSOS_EDGE_PATH_TUNNEL_PIN=")][0]
    client_remote(f"{far_host}:2333")
    assert box.succeed(f"cat {pin}").strip() == hub_pin[far_host], pin
    assert through(hubs[far], box_port(hubs[far], "mattbox")).strip() == "hello-losos"

    # ── 7. gwa comes back; hub1 banned on the gateway moves its uplink ───
    gwa.succeed("install -d /var/lib/losos-registrar && echo '{\"edges\":[\"http://hub1:8443\"]}' > /var/lib/losos-registrar/bans.json")
    gwa.succeed(start_gw)
    doc = wait_path("http://gwa.local:8443", "secondary")
    assert doc["leftBanned"] is None, doc
    retry(
        lambda _: json.loads(gwa.succeed("cat /var/lib/losos-edge/uplink.json"))["registrar_url"] == "http://hub2:8443",
        timeout_seconds=120,
    )
    hub2.wait_until_succeeds("grep -F -q '\"acme.mattbox\":' /etc/losos-tunnel/server.json", timeout=180)
    registry = json.loads(hub2.succeed("cat /var/lib/losos-registrar/registry.json"))
    assert registry["tenants"]["acme.mattbox"]["via"] == "acme", registry
    gwa.wait_until_succeeds("grep -F -q '\"remote\": \"hub2:2333\"' /etc/losos-tunnel/uplink.json")
    uplink = json.loads(gwa.succeed("cat /etc/losos-tunnel/uplink.json"))
    assert uplink["pin"] == hub_pin["hub2"], uplink
    assert gwa.succeed("cat /var/lib/losos-edge/hub-tunnel.pin").strip() == hub_pin["hub2"]

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
