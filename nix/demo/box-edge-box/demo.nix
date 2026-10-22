# Box, edge, box: two LosOS boxes and an edge gateway on one network, as
# small VMs. This is the light mode of the demo that
# nix/demo/box-edge-box/bootstrap.sh sets up (its full mode installs two real
# boxes from the ISO instead and needs KVM), and it is a check:
# `checks.losos-box-edge-box`.
#
#   * `boxa`, `boxb`: the appliance's control plane: lososd, the front nginx
#                      with the admin UI, mDNS, and the edge path
#                      (modules/proxy.nix) switched on, with nothing
#                      provisioned: lososd mints the box's tokens, the way a
#                      box installed from the ISO does.
#   * `edge`: the edge gateway (losos.edge.gateway.enable, what the
#                      losos-disk-edge-qcow2 image runs): LAN advert, open
#                      enrolment, the manage page. It starts switched off.
#
# The demo, in order:
#   1. no edge anywhere: each box finds the other by mDNS (its .local name
#      and its _losos._tcp advert) but no edge, refuses to share its disk,
#      and the other box cannot reach it by its public name;
#   2. the edge is switched on: both boxes find it on the LAN by mDNS;
#   3. it enrols both on first contact and publishes them: two tunnels;
#   4. box A reaches box B through the edge by B's public name, and back;
#      the admin pages stay LAN-only on that road;
#   5. both owners let the edge manage their box; its page lists both;
#   6. the gate is open: both boxes accept sharing;
#   7. the edge goes away: both notice, the tunnels stop, sharing is refused
#      again, and each box keeps serving its own LAN.
#
# What is smaller than a real box: LosOS Cloud does not run in these VMs (it
# needs gigabytes and, without KVM, most of an hour to start). A one-file
# stand-in answers on the loopback port the cloud pod would, behind the real
# front nginx, so a request through the edge ends at box B's own nginx and
# its /nextcloud location. The full mode reaches the real LosOS Cloud.
#
# With LOSOS_RECORD_DIR set on the host, the script pauses at each step
# until a recorder has photographed the admin pages (two-boxes.nix in
# nix/demo/edge-lan/ does the same); without it the hooks only print. A step
# goes on by itself after LOSOS_RECORD_WAIT seconds (180), or never with 0,
# which is how --pause waits for the presenter.
{ pkgs }:

let
  lososPkgs = import ../../packages.nix { inherit pkgs; };

  # The VMs' network: 192.168.77.0/24, boxa .1, boxb .2, edge .3. Number 77
  # rather than the default 1 keeps it off the 192.168.1.0/24 so many home
  # networks use, because bootstrap.sh --host-mdns puts the host on it at
  # .254: the test driver plugs a tap named vde-tap77 into this network
  # when one exists.
  vlans = [ 77 ];

  # Where the cloud pod would answer (losos.nextcloud.apachePort's default),
  # and what the stand-in says there.
  cloudPort = 11000;
  standIn =
    name:
    pkgs.writeTextDir "nextcloud/status.php" (
      builtins.toJSON {
        installed = true;
        productname = "LosOS Cloud";
        box = name;
        note = "light demo stand-in; the full demo runs the real LosOS Cloud";
      }
    );

  box =
    name:
    { pkgs, ... }:
    {
      imports = [
        ../../../modules/options.nix
        ../../../modules/daemon.nix
        ../../../modules/containers.nix
        ../../../modules/mdns.nix
        ../../../modules/proxy.nix
      ];
      networking.hostName = name;
      losos = {
        hostName = name;
        backend.package = lososPkgs.losos-ctl;
        admin.ui = lososPkgs.losos-admin-ui;
        # `<name>.local` and the `_losos._tcp` advert a real box announces
        # (modules/configuration.nix, which these VMs leave out).
        mdns = {
          enable = true;
          services.losos = ''
            Name=%H
            Type=_losos._tcp
            Port=80
          '';
        };
        # LosOS Login (modules/identity.nix) is not in these VMs; the admin
        # pages are opened with the box's spare key, as its sign-in allows.
        identity.enable = false;
        # The stock options: the configured edge is the public one, which
        # this network cannot reach, and the tokens are minted by lososd.
        # Only the heartbeat is quicker than a real box's 30 s, so the demo
        # does not wait on it.
        proxy = {
          enable = true;
          heartbeatInterval = "5s";
          registrar.package = lososPkgs.losos-registrar;
        };
      };
      systemd.services.losos-cloud-stand-in = {
        description = "stand-in for LosOS Cloud on its loopback port (light demo only)";
        wantedBy = [ "multi-user.target" ];
        serviceConfig = {
          ExecStart = "${pkgs.python3}/bin/python3 -m http.server ${toString cloudPort} --bind 127.0.0.1 --directory ${standIn name}";
          DynamicUser = true;
        };
      };
      environment.systemPackages = [ pkgs.jq ];
      virtualisation = {
        memorySize = 1024;
        cores = 2;
        inherit vlans;
      };
    };
in

pkgs.testers.nixosTest {
  name = "losos-box-edge-box";

  nodes = {
    boxa = box "boxa";
    boxb = box "boxb";

    edge =
      { lib, ... }:
      {
        imports = [
          ../../../modules/options.nix
          ../../../modules/edge.nix
          ../../../modules/edge-gateway.nix
        ];
        _module.args.self = {
          packages.x86_64-linux = {
            inherit (lososPkgs) losos-registrar losos-admin-ui;
          };
        };
        networking.hostName = "edge";
        # The test framework gives root a password of its own.
        users.users.root.initialPassword = lib.mkForce null;
        losos.edge = {
          gateway.enable = true;
          registrarApiPort = 8443;
          ratholePortRange = "50000-50010";
          reconcileInterval = "2s";
          heartbeatTtl = "30s";
        };
        virtualisation = {
          memorySize = 1024;
          cores = 2;
          inherit vlans;
        };
      };
  };

  testScript = ''
    import json
    import os
    import shlex
    import time

    # ── Recording hooks (see the header) ─────────────────────────────────
    rec = os.environ.get("LOSOS_RECORD_DIR")
    wait = int(os.environ.get("LOSOS_RECORD_WAIT", "180"))
    seq = 0

    def note(path, text):
        if rec:
            with open(os.path.join(rec, path), "w") as f:
                f.write(text)

    def event(step, caption, **extra):
        global seq
        seq += 1
        print(f"\n=== {step}: {caption}\n")
        if not rec:
            return
        with open(os.path.join(rec, "timeline.jsonl"), "a") as f:
            f.write(json.dumps({"n": seq, "step": step, "caption": caption, "t": time.time(), **extra}) + "\n")
        ack = os.path.join(rec, f"ack-{seq}")
        waited = 0
        while not os.path.exists(ack):
            if 0 < wait <= waited:
                print(f"recorder did not acknowledge step {seq}; carrying on")
                return
            time.sleep(1)
            waited += 1

    boxes = {"boxa": boxa, "boxb": boxb}
    other = {"boxa": "boxb", "boxb": "boxa"}
    ports = {"boxa": 18081, "boxb": 18082}
    tokens = {}

    def api(name, method, path, data=None):
        m = boxes[name]
        extra = "-H 'Content-Type: application/json' --data-binary @- " if data is not None else ""
        cmd = (
            f"curl -s -o /tmp/body -w '%{{http_code}}' -X {method}"
            f" -H 'Authorization: Bearer {tokens[name]}' {extra}127.0.0.1:8082{path}"
        )
        if data is not None:
            cmd = f"printf %s {shlex.quote(json.dumps(data))} | " + cmd
        code = m.succeed(cmd).strip()
        return code, m.succeed("cat /tmp/body")

    def edge_doc(name):
        code, body = api(name, "GET", "/api/edge")
        assert code == "200", f"{name}: /api/edge answered {code}: {body!r}"
        return json.loads(body)

    def wait_edge(reachable, timeout=240):
        for name, m in boxes.items():
            with m.nested(f"{name}: waiting for reachable={reachable}"):
                retry(lambda _: edge_doc(name)["reachable"] == reachable, timeout_seconds=timeout)

    def sharing(name):
        # Asking the box to share its disk into the mesh. The rebuild an
        # accepted request queues needs a real box; the posture is put back.
        code, body = api(name, "POST", "/api/change", {"mode": "mesh"})
        if code == "200":
            back, body2 = api(name, "POST", "/api/change", {"mode": "local"})
            assert back == "200", f"{name}: change back to local answered {back}: {body2!r}"
        return code, body

    def mdns_ipv4(frm, name):
        # Over the wire by mDNS only: no /etc/hosts, no DNS, no cache.
        got = boxes[frm].wait_until_succeeds(
            f"resolvectl query -4 -p mdns --synthesize=no --cache=no {name}.local"
        )
        assert "protocol mDNS/IPv4" in got, got
        return got.split()[1]

    def through_edge(frm, to, path):
        # The way a browser on box `frm`'s side reaches box `to` by its public
        # name: the name points at the edge (the company's DNS does that on a
        # real site) and the edge's Traefik routes it down `to`'s tunnel.
        # Traefik has no certificate for a name this network cannot prove,
        # so it presents its own; -k for that, and only for that.
        ip = mdns_ipv4(frm, "edge")
        return boxes[frm].succeed(
            f"curl -sk --max-time 10 -o /tmp/through -w '%{{http_code}}' "
            f"--resolve {to}.losos.cfd:443:{ip} https://{to}.losos.cfd{path}; echo; cat /tmp/through || true"
        )

    # ── 1. Two boxes, no edge anywhere ────────────────────────────────────
    boxa.start()
    boxb.start()
    for name, m in boxes.items():
        m.wait_for_unit("systemd-resolved.service")
        m.wait_for_unit("nginx.service")
        m.wait_until_succeeds("curl -fsS 127.0.0.1:8082/api/health")
        # The owner claimed the box in the first-run wizard (the installer
        # recordings show that); here the claim is stated, so the admin UI
        # opens on its sign-in.
        m.succeed(
          "systemctl stop lososd.service; install -d -m 0700 /var/lib/losos;"
          " printf '%s' '{\"mode\":\"local\",\"sharing\":false,\"rebuild\":null,\"claimed\":true}'"
          " > /var/lib/losos/state.json; systemctl start lososd.service"
        )
        m.wait_until_succeeds("curl -fsS 127.0.0.1:8082/api/health")
        m.forward_port(host_port=ports[name], guest_port=80)
        tokens[name] = m.succeed("systemd-creds decrypt --name=admin-token /etc/credstore.encrypted/admin-token -").strip()
        note(f"{name}.token", tokens[name])
        note(f"{name}.port", str(ports[name]))

    wait_edge(False)
    for name, m in boxes.items():
        d = edge_doc(name)
        assert d["edges"] == [] and d["lanSearched"] is True and d["path"] is None, d
        code, body = sharing(name)
        assert code == "409" and json.loads(body)["edgeRequired"] is True, (code, body)
        # No tunnel runs on a box with no edge in reach.
        m.wait_until_succeeds("test -e /run/losos/edge-none")
        m.wait_until_succeeds("test \"$(systemctl is-active losos-rathole-client.service)\" = inactive")
        # Each finds the other by mDNS: its .local name and its _losos._tcp
        # advert, as LosOS Desktop lists boxes.
        print(f"{name} found {other[name]}.local by mDNS at {mdns_ipv4(name, other[name])}")
        advert = m.wait_until_succeeds(f"resolvectl service {other[name]} _losos._tcp local")
        assert f"{other[name]}.local:80" in advert, advert
        # The neighbour's own pages answer on the LAN at that name; its
        # public name does not.
        m.succeed(f"curl -fsS http://{other[name]}.local/ | grep -q '<div id=\"root\">'")
        m.fail(f"curl -fsS --max-time 5 https://{other[name]}.losos.cfd/")
    event("1", "Two boxes, no edge anywhere: each finds the other by mDNS (boxa.local, boxb.local) but no edge, sharing is refused (409), and neither can reach the other by its public name.")

    # ── 2. The edge is switched on and found ─────────────────────────────
    edge.start()
    edge.wait_for_unit("losos-registrar.service")
    edge.wait_for_unit("losos-rathole.service")
    edge.wait_until_succeeds("curl -fsS http://localhost:8443/health")
    edge.forward_port(host_port=18443, guest_port=8443)
    key = edge.succeed("cat /var/lib/losos-registrar/manage-key").strip()
    note("edge.key", key)
    note("edge.port", "18443")
    t0 = time.time()
    wait_edge(True)
    found_in = int(time.time() - t0)
    for name in boxes:
        d = edge_doc(name)
        assert d["path"]["source"] == "lan" and d["path"]["url"] == "http://edge.local:8443", d
        # The road it took: the edge's DNS-SD advert and its .local name.
        advert = boxes[name].wait_until_succeeds("resolvectl service 'losos edge on edge' _losos-edge._tcp local")
        assert "url=http://edge.local:8443" in advert, advert
        print(f"{name} found edge.local by mDNS at {mdns_ipv4(name, 'edge')}")
        # A company edge: nobody signed it, so sharing yes, market no.
        assert d["official"] is False, d
    print(edge.succeed("losos-edge status"))
    event("2", f"The edge is switched on: both boxes found it on the LAN by mDNS within {found_in} s. It is a company edge, not an official one.", seconds=found_in)

    # ── 3. Enrolled on first contact, two tunnels ────────────────────────
    for name, m in boxes.items():
        m.wait_until_succeeds("systemctl is-active losos-rathole-client.service", timeout=180)
        m.wait_until_succeeds("systemctl is-active losos-registrar-announce.service", timeout=180)
        edge.wait_until_succeeds(f"grep -F -q '[server.services.{name}]' /etc/rathole/server.toml", timeout=240)
    listed = edge.succeed("losos-edge boxes")
    print(listed)
    assert "boxa\tboxa.losos.cfd\tport " in listed and "boxb\tboxb.losos.cfd\tport " in listed, listed
    event("3", "The edge enrolled both boxes the first time they called and publishes them: boxa.losos.cfd and boxb.losos.cfd, one tunnel each.")

    # ── 4. Box to box, through the edge ──────────────────────────────────
    for frm, to in [("boxa", "boxb"), ("boxb", "boxa")]:
        def answered(_):
            got = through_edge(frm, to, "/nextcloud/status.php")
            return got.startswith("200") and f'"box":"{to}"' in got
        with boxes[frm].nested(f"{frm} -> edge -> {to}"):
            retry(answered, timeout_seconds=180)
        got = through_edge(frm, to, "/nextcloud/status.php")
        print(f"{frm} asked https://{to}.losos.cfd/nextcloud/status.php through the edge: {got}")
        # The admin pages stay on the LAN, also through the edge.
        admin = through_edge(frm, to, "/")
        assert admin.startswith("403"), admin
    event("4", "Box A reached box B through the edge by B's public name, and B reached A. The admin pages answer 403 on that road. They stay on the LAN.")

    # ── 5. The edge may manage both ──────────────────────────────────────
    for name in boxes:
        code, body = api(name, "POST", "/api/edge/manage", {"enabled": True})
        assert code == "200", (name, code, body)
        assert json.loads(body)["manage"]["active"] is True, body
    edge.wait_until_succeeds(
      f"curl -fsS -H 'Authorization: Bearer {key}' 127.0.0.1:8443/manage/api/boxes"
      " | jq -e '[.boxes[] | select(.managed)] | length == 2'",
      timeout=120,
    )
    print(edge.succeed(f"curl -fsS -H 'Authorization: Bearer {key}' 127.0.0.1:8443/manage/api/boxes | jq -c '.boxes[] | {{id, managed}}'"))
    event("5", "Both owners let the company edge manage their box. The edge's page lists both as managed here.")

    # ── 6. Sharing is allowed ─────────────────────────────────────────────
    for name in boxes:
        code, body = sharing(name)
        assert code == "200", f"{name}: sharing with the edge in reach answered {code}: {body!r}"
    event("6", "With an edge in reach both boxes accept sharing their disk (200). Joining the mesh itself rebuilds the box, which takes an installed one.")

    # ── 7. The edge goes away ────────────────────────────────────────────
    edge.shutdown()
    t0 = time.time()
    wait_edge(False)
    gone_in = int(time.time() - t0)
    for name, m in boxes.items():
        m.wait_until_succeeds("test -e /run/losos/edge-none")
        m.wait_until_succeeds("test \"$(systemctl is-active losos-rathole-client.service)\" = inactive")
        code, body = sharing(name)
        assert code == "409", (name, code, body)
        m.succeed(f"curl -fsS http://{other[name]}.local/ | grep -q '<div id=\"root\">'")
    event("7", f"The edge is switched off: both boxes noticed within {gone_in} s, stopped their tunnels and refuse sharing again. Each still serves its own LAN.", seconds=gone_in)
  '';
}
