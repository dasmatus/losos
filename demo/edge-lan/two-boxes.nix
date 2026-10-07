# Two boxes and an edge on one network, without the edge first, then with it.
#
# This is the verification walkthrough (scenario A,
# "no edge proxy", then scenario B, "with the edge proxy"), as a
# nixos-test-vms config so that it is both a check and a recording:
#
#   * as `checks.losos-edge-lan-two-boxes` it asserts, on TWO boxes at once,
#     what tests/edge-lan.nix asserts on one: with no edge anywhere each box
#     says so, refuses network-dependent sharing with its own sentence, still
#     serves its owner, and keeps saying so across a reboot; the moment an
#     edge appears on the LAN both find it within a scan, the gate opens on
#     both, and when it goes away both notice; the second, unsigned edge
#     opens sharing but not trading.
#   * with LOSOS_RECORD_DIR set in the environment of a driver run on the
#     host (demo/edge-lan/record.sh), the script also pauses at each step
#     until demo/edge-lan/record.mjs has photographed the Mesh pane of both
#     boxes through their real nginx + admin UI (forwarded to the host), so the
#     recording shows the product, not a stub.
#
# What the boxes are here: the appliance's control plane (lososd, the facade,
# the front nginx with the admin SPA, Avahi), not an installed appliance —
# tests/admin-vm.nix and tests/front-vhost.nix are the same shape, and the
# install itself is demonstrated by the installer recordings. What this
# cannot show is the mesh pooling storage (B3/B4 of the checklist): the join a
# box accepts here queues a rebuild into mesh mode, and that rebuild needs the
# real system — see demo/edge-lan/CHECKLIST.md for what covers those steps.
#
# The configured edge URL on both boxes is the unsigned second edge on the
# edge VM, as in tests/edge-lan.nix, so the "no edge anywhere" steps are
# reachable and the official/unofficial distinction can be shown.
{ pkgs }:

let
  lososPkgs = import ../../flake/packages.nix { inherit pkgs; };
  proxyTokenValue = "demo-proxy-token-0123456789abcdef";
  bootstrapTokenValue = "demo-bootstrap-0123456789abcdef0123";
  proxyToken = "/var/secrets/losos-proxy-token";
  bootstrapToken = "/var/secrets/losos-rathole-bootstrap";
  # The same shape as tests/edge-lan.nix: a root key and one edge certificate
  # made at build time, so the recording's edge is "official" to these two
  # boxes without touching the committed (empty) keys/ file.
  identityFixture =
    pkgs.runCommand "losos-edge-identity-demo-fixture"
      {
        nativeBuildInputs = [ lososPkgs.losos-registrar ];
      }
      ''
        mkdir -p $out
        losos-registrar identity keygen --out $out/root.key > $out/root.pub
        losos-registrar identity keygen --out $out/edge.key > $out/edge.pub
        losos-registrar identity sign \
          --root-key $out/root.key --public-key "$(cat $out/edge.pub)" \
          --name "LosOS edge (demo)" --url http://edge.local:8443 --days 30 \
          > $out/edge.cert.json
      '';
  identityKey = "/var/secrets/losos-edge-identity.key";
  plainEdgeRoot = pkgs.runCommand "plain-edge-root" { } ''
    mkdir -p $out
    echo ok > $out/health
  '';

  # One box of the two. The admin UI is the real package on the real front
  # vhost (modules/containers.nix), reached from the host through a QEMU port
  # forward; the LAN guard sees the forward's 10.0.2.2 as a LAN address.
  box =
    name:
    { pkgs, ... }:
    {
      imports = [
        ../../modules/options.nix
        ../../modules/daemon.nix
        ../../modules/containers.nix
      ];
      networking.hostName = name;
      losos = {
        hostName = name;
        backend.package = lososPkgs.losos-ctl;
        admin.ui = lososPkgs.losos-admin-ui;
        # The unsigned second edge: see the header.
        proxy.registrarUrl = "http://edge.local:8444";
        proxy.officialRootKeyFile = "${identityFixture}/root.pub";
      };
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
in

pkgs.testers.nixosTest {
  name = "losos-edge-lan-two-boxes";

  nodes = {
    edge =
      { ... }:
      {
        imports = [
          ../../modules/options.nix
          ../../modules/edge.nix
        ];
        _module.args.self = {
          packages.x86_64-linux.losos-registrar = lososPkgs.losos-registrar;
        };
        networking.hostName = "edge";
        losos.edge = {
          enable = true;
          acmeEmail = "demo@losos.cfd";
          registrarApiPort = 8443;
          ratholePortRange = "50000-50010";
          reconcileInterval = "2s";
          bootstrapTokenFile = bootstrapToken;
          tenants.box1 = {
            hostname = "box1.losos.cfd";
            tokenFile = proxyToken;
          };
          tenants.box2 = {
            hostname = "box2.losos.cfd";
            tokenFile = proxyToken;
          };
          lan.advertise = true;
          identity = {
            keyFile = identityKey;
            certFile = "${identityFixture}/edge.cert.json";
          };
        };
        systemd.tmpfiles.rules = [
          "d /var/secrets 0700 root root - -"
          "f ${proxyToken} 0600 root root - ${proxyTokenValue}"
          "f ${bootstrapToken} 0600 root root - ${bootstrapTokenValue}"
          "C ${identityKey} 0600 root root - ${identityFixture}/edge.key"
        ];
        systemd.services.losos-plain-edge = {
          description = "a bare /health responder standing in for an unsigned edge";
          wantedBy = [ "multi-user.target" ];
          after = [ "network.target" ];
          serviceConfig = {
            ExecStart = "${pkgs.python3}/bin/python3 -m http.server 8444 --bind :: --directory ${plainEdgeRoot}";
            DynamicUser = true;
          };
        };
        networking.firewall.allowedTCPPorts = [ 8444 ];
        virtualisation = {
          memorySize = 1024;
          cores = 2;
        };
      };

    box1 = box "box1";
    box2 = box "box2";
  };

  testScript = ''
    import json
    import os
    import shlex
    import time

    # ── Recording hooks ─────────────────────────────────────────────────────
    # With LOSOS_RECORD_DIR set, each step is written to a timeline and the
    # script waits (up to two minutes) for the recorder on the host to
    # photograph both boxes in that state and acknowledge. Without it, the
    # hooks only print.
    rec = os.environ.get("LOSOS_RECORD_DIR")
    seq = 0

    def note(path, text):
        if rec:
            with open(os.path.join(rec, path), "w") as f:
                f.write(text)

    def event(step, caption, **extra):
        global seq
        seq += 1
        print(f"=== {step}: {caption}")
        if not rec:
            return
        with open(os.path.join(rec, "timeline.jsonl"), "a") as f:
            f.write(json.dumps({"n": seq, "step": step, "caption": caption, "t": time.time(), **extra}) + "\n")
        ack = os.path.join(rec, f"ack-{seq}")
        for _ in range(120):
            if os.path.exists(ack):
                return
            time.sleep(1)
        print(f"recorder did not acknowledge step {seq}; carrying on")

    boxes = {"box1": box1, "box2": box2}
    ports = {"box1": 18081, "box2": 18082}

    # ── Scenario A: no edge proxy anywhere ─────────────────────────────────
    # The edge VM is deliberately not started. The boxes come up alone.
    box1.start()
    box2.start(allow_reboot=True)
    for name, m in boxes.items():
        m.wait_for_unit("avahi-daemon.service")
        m.wait_for_unit("nginx.service")
        m.wait_until_succeeds("losos-ctl state --json")
        m.wait_until_succeeds("curl -fsS 127.0.0.1:8082/api/health")
        # The owner claimed this box in the first-run wizard (the installer
        # recordings show that step); here the claim is stated, so the admin
        # UI opens on the sign-in rather than the wizard.
        m.succeed(
          "systemctl stop lososd.service; install -d -m 0700 /var/lib/losos;"
          " printf '%s' '{\"mode\":\"local\",\"sharing\":false,\"rebuild\":null,\"claimed\":true}'"
          " > /var/lib/losos/state.json; systemctl start lososd.service"
        )
        m.wait_until_succeeds("curl -fsS 127.0.0.1:8082/api/health")
        m.forward_port(host_port=ports[name], guest_port=80)
        note(f"{name}.token", m.succeed("cat /var/secrets/losos-admin-token").strip())
        note(f"{name}.port", str(ports[name]))

    tokens = {n: m.succeed("cat /var/secrets/losos-admin-token").strip() for n, m in boxes.items()}

    def api(m, name, method, path, data=None, ctype="application/json"):
        extra = f"-H 'Content-Type: {ctype}' --data-binary @- " if data is not None else ""
        cmd = (
            f"curl -s -o /tmp/body -w '%{{http_code}}' -X {method}"
            f" -H 'Authorization: Bearer {tokens[name]}' {extra}127.0.0.1:8082{path}"
        )
        if data is not None:
            payload = json.dumps(data) if ctype == "application/json" else data
            cmd = f"printf %s {shlex.quote(payload)} | " + cmd
        code = m.succeed(cmd).strip()
        return code, m.succeed("cat /tmp/body")

    def edge_doc(m, name):
        code, body = api(m, name, "GET", "/api/edge")
        assert code == "200", f"{name}: /api/edge answered {code}: {body!r}"
        return json.loads(body)

    def wait_edge(reachable, official=None, timeout=120):
        for name, m in boxes.items():
            def probe(_):
                d = edge_doc(m, name)
                return d["reachable"] == reachable and (official is None or d["official"] == official)
            with m.nested(f"{name}: waiting for /api/edge reachable={reachable} official={official}"):
                retry(probe, timeout_seconds=timeout)

    def refuses_mesh(m, name):
        code, body = api(m, name, "POST", "/api/change", {"mode": "mesh"})
        assert code == "409", f"{name}: change to mesh with no edge answered {code}: {body!r}"
        refused = json.loads(body)
        assert refused["edgeRequired"] is True, refused
        assert "no edge proxy is reachable" in refused["error"], refused
        return refused["error"]

    def accepts_mesh(m, name):
        code, body = api(m, name, "POST", "/api/change", {"mode": "mesh"})
        assert code == "200", f"{name}: change to mesh with an edge in reach answered {code}: {body!r}"
        state = json.loads(m.succeed("losos-ctl state --json"))
        assert state["mode"] == "mesh" and state["sharing"] is True, state
        # The rebuild this queued needs the real system (no flake in this VM);
        # put the posture back so the recording does not carry a failed job.
        code, body = api(m, name, "POST", "/api/change", {"mode": "local"})
        assert code == "200", f"{name}: change to local answered {code}: {body!r}"

    # A1. Both boxes searched and found nothing; their own services answer.
    wait_edge(False)
    for name, m in boxes.items():
        d = edge_doc(m, name)
        assert d["edges"] == [] and d["lanSearched"] is True, d
        assert d["configuredUrl"] == "http://edge.local:8444", d
    # Reaching the admin UI from the other box over the LAN: the front door is
    # up with no edge (loopback is 403 by design, so ask from the neighbour).
    box2.succeed("curl -fsS http://box1/ | grep -q '<div id=\"root\">'")
    box1.succeed("curl -fsS http://box2/ | grep -q '<div id=\"root\">'")
    event("A1", "Both boxes up with no edge proxy anywhere: each searched its network and nothing answered; both admin UIs are served.")

    # A2. Sharing is refused, in the daemon's words, and nothing is written.
    before = {n: m.succeed("cat /var/lib/losos/state.json") for n, m in boxes.items()}
    sentences = {n: refuses_mesh(m, n) for n, m in boxes.items()}
    for name, m in boxes.items():
        joining = (
          "{ ... }:\n{\n"
          "  losos.sharingMyStorage = false;\n"
          "  losos.cluster.enable = true;\n"
          f"  losos.hostName = \"{name}\";\n"
          "}\n"
        )
        code, body = api(m, name, "POST", "/api/apply", joining, ctype="text/plain")
        assert code == "409", f"{name}: apply joining the mesh with no edge answered {code}: {body!r}"
        assert json.loads(body)["setting"] == "losos.cluster.enable", body
        out = m.fail("losos-ctl change --mode mesh 2>&1")
        assert "no edge proxy is reachable" in out, out
        assert m.succeed("cat /var/lib/losos/state.json") == before[name], f"{name}: state.json moved on a refused request"
    event("A2", "Turning sharing on is refused on both boxes (409) with the reason; nothing was written.", sentence=sentences["box1"])

    # A3. Time passes: the boxes keep looking, nothing changes, the rest works.
    time.sleep(45)
    wait_edge(False)
    for name, m in boxes.items():
        code, body = api(m, name, "GET", "/api/settings")
        assert code == "200" and json.loads(body)["clusterEnable"] is False, body
        assert m.succeed("cat /var/lib/losos/state.json") == before[name]
    event("A3", "Two scan periods later: still no edge, state unchanged, settings read back fine on both.")

    # A4. One box reboots: it comes back in the same state and still refuses.
    box2.reboot()
    box2.wait_for_unit("multi-user.target")
    box2.wait_until_succeeds("curl -fsS 127.0.0.1:8082/api/health")
    box2.forward_port(host_port=ports["box2"], guest_port=80)
    tokens["box2"] = box2.succeed("cat /var/secrets/losos-admin-token").strip()
    note("box2.token", tokens["box2"])
    with box2.nested("box2 after reboot: no edge, still refusing"):
        retry(lambda _: edge_doc(box2, "box2")["reachable"] is False, timeout_seconds=120)
    refuses_mesh(box2, "box2")
    event("A4", "box2 rebooted: same state after the restart, local services up, sharing still refused.")

    # ── Scenario B: the edge appears ───────────────────────────────────────
    edge.start()
    edge.wait_for_unit("losos-registrar.service")
    edge.wait_for_unit("avahi-daemon.service")
    edge.wait_until_succeeds("curl -fsS http://localhost:8443/health")
    edge.wait_for_unit("losos-plain-edge.service")
    edge.wait_until_succeeds("curl -fsS http://localhost:8444/health")
    t0 = time.time()
    wait_edge(True, official=True)
    found_in = int(time.time() - t0)
    for name, m in boxes.items():
        d = edge_doc(m, name)
        assert [e["url"] for e in d["edges"]] == ["http://edge.local:8443", "http://edge.local:8444"], d
        assert d["edges"][0]["source"] == "lan" and d["edges"][0]["official"] is True, d
        assert d["edges"][1]["official"] is False, d
    event("B1", f"The edge is switched on: both boxes found it on the LAN by mDNS within {found_in} s; the registrar proved it is official.", seconds=found_in)

    # B2. The gate is open on both: the join is accepted.
    for name, m in boxes.items():
        accepts_mesh(m, name)
    event("B2", "Join the mesh accepted on both boxes (200; the rebuild into mesh mode runs on a real box, not in this VM).")

    # B5. The edge goes away: both notice within two scans, refuse again, keep
    #     serving their owners.
    edge.succeed("systemctl stop losos-registrar.service losos-plain-edge.service")
    t0 = time.time()
    wait_edge(False)
    gone_in = int(time.time() - t0)
    for name, m in boxes.items():
        refuses_mesh(m, name)
    box2.succeed("curl -fsS http://box1/ | grep -q '<div id=\"root\">'")
    box1.succeed("curl -fsS http://box2/ | grep -q '<div id=\"root\">'")
    event("B5", f"The edge is switched off: both boxes noticed within {gone_in} s, refuse sharing again, and keep serving locally.", seconds=gone_in)

    # B6. It comes back: the gate reopens with no hand on either box.
    edge.succeed("systemctl start losos-registrar.service losos-plain-edge.service")
    edge.wait_until_succeeds("curl -fsS http://localhost:8443/health")
    wait_edge(True, official=True)
    for name, m in boxes.items():
        accepts_mesh(m, name)
    event("B6", "The edge is back: found again on both, the gate reopened by itself, the join is accepted again.")

    # B8. Only the unsigned edge answers: sharing yes, trading no.
    edge.succeed("systemctl stop losos-registrar.service")
    for name, m in boxes.items():
        with m.nested(f"{name}: waiting for the unsigned edge alone"):
            retry(lambda _: [e["url"] for e in edge_doc(m, name)["edges"]] == ["http://edge.local:8444"], timeout_seconds=120)
        d = edge_doc(m, name)
        assert d["reachable"] is True and d["official"] is False, d
        code, body = api(m, name, "GET", "/api/market")
        assert json.loads(body) == {"available": False, "reason": "noOfficialEdge"}, body
        code, body = api(m, name, "POST", "/api/market/orders", {"listing_id": "lst_1", "quantity": 1})
        assert code == "409" and json.loads(body)["officialEdgeRequired"] is True, body
        accepts_mesh(m, name)
    event("B8", "Only an edge nobody signed is left: sharing through it is allowed on both, the market says noOfficialEdge and an order is refused (409).")

    edge.succeed("systemctl start losos-registrar.service")
    wait_edge(True, official=True)
    event("end", "The official edge is back; both boxes show the check sign again.")
  '';
}
