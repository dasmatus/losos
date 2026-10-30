# nixos-test-vms config for kindashared: sharing with the local gateways of
# the box's own network and nothing further (losos.sharingLocally,
# backend/src/edge.rs `check_local_gate`, backend/src/losos.rs
# `local_sharing_tick`).
#
# Three VMs on one LAN:
#   * `gateway` — a LosOS edge gateway (losos.edge.gateway.enable, the shape
#                 of the losos-disk-edge-qcow2 image): it advertises itself
#                 over mDNS and answers GET /cluster/secondary with role
#                 `gateway`, so the box's link to it is secondary. Beside it,
#                 a bare /health responder on 8444 stands in for an edge the
#                 box only reaches by its configured URL, the way it reaches
#                 an official edge over the internet: a tertiary link.
#   * `box`     — the real lososd, nginx, LosOS Login and the built admin UI,
#                 with mDNS, configured for the bare edge.
#   * `laptop`  — someone on the LAN with a browser, for the pictures.
#
# What is asserted, in order:
#   1. with only the bare edge in reach the box has an edge but no local
#      gateway, and an apply that turns losos.sharingLocally on is refused
#      with 409 `localGatewayRequired`, nothing written;
#   2. the gateway answers: /api/edge says localGateway, the same apply is
#      accepted and overrides.nix carries the switch, and a market listing
#      is refused: nothing shared only with this network is sold (this box
#      runs no apps; backend/src/apps.rs tests that kindashared is offered);
#   3. the gateway goes away for longer than losos.proxy.pauseSharingAfter
#      (60 s here): the box turns it off by itself and /api/edge says since
#      when, while the bare edge, which is not a local gateway, stays;
#   4. the gateway comes back and a minute later the box shares again.
#
# The pictures are the Mesh page in each state. `before`, a path to an
# earlier admin-ui/app, builds that UI instead:
#
#   nix build --impure --expr '(builtins.getFlake (toString ./.)).inputs.nixpkgs.legacyPackages.x86_64-linux.callPackage ./nix/tests/kindashared.nix {
#     before = /path/to/main/admin-ui/app; })'
{
  pkgs,
  before ? null,
}:

let
  inherit (pkgs) lib;
  lososPkgs = import ../packages.nix { inherit pkgs; };
  screens = import ./screens { inherit pkgs; };
  after = before == null;

  ui = import ./admin-ui-without-lab.nix (
    lososPkgs.losos-admin-ui.overrideAttrs (_: lib.optionalAttrs (!after) { src = before; })
  );

  # What lososd's rebuild unit runs: a quick success, so the pages show the
  # applied setting and not a rebuild that could not find a flake.
  stubRebuild = pkgs.writeShellScriptBin "nixos-rebuild" ''
    echo "building the system for $*"
    sleep 5
    echo "activating the new system"
  '';

  # The bare edge: GET /health is 200, everything else 404.
  plainEdgeRoot = pkgs.runCommand "plain-edge-root" { } ''
    mkdir -p $out
    echo ok > $out/health
  '';

  overrides = pkgs.writeText "overrides-local.nix" ''
    { ... }:

    {
      losos.sharingMyStorage = false;
      losos.sharingLocally = false;
      losos.hostName = "kitchenbox";
      losos.cluster.enable = false;
    }
  '';

  ready =
    if after then
      {
        role = "switch";
        name = "Share with this network's gateways";
      }
    else
      {
        role = "switch";
        name = "Join the mesh";
      };

  plan =
    name: attrs:
    pkgs.writeText "plan-${name}.json" (
      builtins.toJSON (
        {
          width = 1280;
          height = 800;
          shots = [
            {
              name = "mesh";
              path = "/mesh";
              waitFor = ready;
              steps = [ { sleep = 2000; } ];
              fullPage = true;
            }
          ];
        }
        // attrs
      )
    );

  plans = {
    light = plan "light" { };
    dark = plan "dark" { colorScheme = "dark"; };
    phone = plan "phone" {
      width = 390;
      height = 844;
    };
  };
in

pkgs.testers.nixosTest {
  name = if after then "losos-kindashared" else "losos-kindashared-before";
  # Five captures and a pause and a resume under TCG take longer than the
  # default hour.
  globalTimeout = 4 * 3600;

  nodes = {
    gateway =
      { lib, ... }:
      {
        imports = [
          ../../modules/options.nix
          ../../modules/edge.nix
          ../../modules/edge-gateway.nix
        ];
        _module.args.self = {
          packages.x86_64-linux = {
            inherit (lososPkgs) losos-registrar;
          };
        };
        networking.hostName = "gateway";
        users.users.root.initialPassword = lib.mkForce null;
        losos.edge = {
          gateway.enable = true;
          manage.enable = true;
          manage.ui = null;
          registrarApiPort = 8443;
          tunnelPortRange = "50000-50010";
          reconcileInterval = "2s";
        };
        systemd.services.losos-plain-edge = {
          description = "a bare /health responder standing in for an edge reached over the internet";
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
          vlans = [ 1 ];
        };
      };

    box =
      { lib, ... }:
      {
        imports = [
          ../../modules/options.nix
          ../../modules/containers.nix
          ../../modules/tls.nix
          ../../modules/daemon.nix
          ../../modules/identity.nix
          ../../modules/mdns.nix
        ];
        losos = {
          hostName = "kitchenbox";
          backend.package = lososPkgs.losos-ctl;
          backend.plugins = lososPkgs.losos-plugins;
          admin.ui = ui;
          git.enable = false;
          # The bare edge: configured, so a tertiary link.
          proxy.registrarUrl = "http://gateway.local:8444";
          # Pause after a minute without a local gateway rather than ten.
          proxy.pauseSharingAfter = 60;
          mdns.enable = true;
        };
        # The stand-in first, so it shadows the real nixos-rebuild; git and
        # the rest of lososd's path stay for the settings commit.
        systemd.services.lososd.path = lib.mkBefore [ stubRebuild ];
        environment.systemPackages = [ pkgs.jq ];
        time.timeZone = "UTC";
        virtualisation = {
          memorySize = 1536;
          cores = 2;
          vlans = [ 1 ];
        };
      };

    laptop =
      { ... }:
      {
        imports = [ screens.browser ];
        environment.systemPackages = [ pkgs.curl ];
        environment.etc = lib.mapAttrs' (
          name: file: lib.nameValuePair "losos-test/plan-${name}.json" { source = file; }
        ) plans;
        time.timeZone = "UTC";
        virtualisation.vlans = [ 1 ];
      };
  };

  testScript = ''
    import json
    import shlex

    BOX = "192.168.1.1"
    PASSWORD = "Correct-Horse-Battery-9"

    # One after another: three VMs started at once under TCG can miss the
    # driver's wait for a shell.
    for m in (gateway, box, laptop):
        m.start()
        m.succeed("true")
    gateway.wait_for_unit("losos-registrar.service")
    gateway.wait_until_succeeds("curl -fsS http://localhost:8443/health")
    gateway.wait_for_unit("losos-plain-edge.service")
    gateway.wait_until_succeeds("curl -fsS http://localhost:8444/health")
    secondary = json.loads(gateway.succeed("curl -fsS http://localhost:8443/cluster/secondary"))
    assert secondary["role"] == "gateway" and secondary["tier"] == "secondary", secondary
    # Step 1 is without it.
    gateway.succeed("systemctl stop losos-registrar.service")

    box.wait_for_unit("lososd.service")
    box.wait_for_open_port(8082)
    box.wait_for_unit("nginx.service")
    box.wait_for_open_port(443)
    laptop.wait_for_unit("multi-user.target")

    def curl(args):
        return laptop.succeed(f"curl -sk {args}")

    with subtest("claim the box"):
        claim = json.dumps({"name": "matus", "password": PASSWORD})
        out = curl(
            f"-w '\\n%{{http_code}}' -X POST https://{BOX}/api/setup/claim "
            f"-H 'Host: {BOX}' -H 'Content-Type: application/json' -d '{claim}'"
        )
        body, code = out.rsplit("\n", 1)
        assert code == "200", (code, body)
        box.wait_for_unit("authelia-losos.service")
        box.systemctl("start losos-identity-setup.service")
        box.wait_until_succeeds(f"grep -q \"domain: '{BOX}'\" /run/losos-identity/session.yml")
        box.wait_until_succeeds("systemctl is-active authelia-losos.service")
        box.wait_for_open_port(9091)
        box.succeed("mkdir -p /etc/nixos/modules && install -m 0644 ${overrides} /etc/nixos/modules/overrides.nix")

    token = box.succeed(
        "systemd-creds decrypt --name=admin-token /etc/credstore.encrypted/admin-token -"
    ).strip()
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
        return code, box.succeed("cat /tmp/body")

    def edge_doc():
        code, body = api("GET", "/api/edge")
        assert code == "200", f"/api/edge answered {code}: {body!r}"
        return json.loads(body)

    def wait_for(what, probe, timeout=300):
        with box.nested(f"waiting for {what}"):
            retry(lambda _: probe(edge_doc()), timeout_seconds=timeout)

    def overrides_now():
        return box.succeed("cat /etc/nixos/modules/overrides.nix")

    def settled():
        box.wait_until_succeeds(
            "losos-ctl status --json | jq -e '.state != \"building\"'", timeout=300
        )

    def capture(state, plans):
        for plan in plans:
            laptop.succeed(
                f"losos-capture --base https://{BOX} --plan /etc/losos-test/plan-{plan}.json "
                f"--out /tmp/screens/{state}/{plan} --user matus --password '{PASSWORD}'",
                timeout=1800,
            )

    locally = (
        "{ ... }:\n{\n"
        "  losos.sharingMyStorage = false;\n"
        "  losos.sharingLocally = true;\n"
        "  losos.hostName = \"kitchenbox\";\n"
        "  losos.cluster.enable = false;\n"
        "}\n"
    )

    with subtest("an edge reached only by its configured URL is not a local gateway"):
        wait_for("the bare edge", lambda d: d["reachable"] and d["localGateway"] is False)
        doc = edge_doc()
        assert [e["tier"] for e in doc["edges"]] == ["tertiary"], doc
        before = box.succeed("cat /var/lib/losos/state.json")
        code, body = api("POST", "/api/apply", locally, ctype="text/plain")
        assert code == "409", f"sharing locally with no local gateway answered {code}: {body!r}"
        refused = json.loads(body)
        assert refused["localGatewayRequired"] is True, refused
        assert refused["setting"] == "losos.sharingLocally", refused
        assert "no local gateway is reachable" in refused["error"], refused
        assert "losos.sharingLocally = true" not in overrides_now()
        assert box.succeed("cat /var/lib/losos/state.json") == before
        capture("refused", ["light"])

    with subtest("a local gateway in reach opens the switch"):
        gateway.succeed("systemctl start losos-registrar.service")
        gateway.wait_until_succeeds("curl -fsS http://localhost:8443/health")
        wait_for("the local gateway", lambda d: d["localGateway"] is True)
        doc = edge_doc()
        assert "secondary" in [e["tier"] for e in doc["edges"]], doc
        code, body = api("POST", "/api/apply", locally, ctype="text/plain")
        if code != "200":
            print(box.execute("journalctl -u lososd --no-pager -n 40")[1])
        assert code == "200", f"sharing locally through a gateway answered {code}: {body!r}"
        assert "losos.sharingLocally = true;" in overrides_now(), overrides_now()
        settings = json.loads(box.succeed("losos-ctl settings --json"))
        assert settings["sharingLocally"] is True, settings
        # No market: nothing shared only with this network is sold, and the
        # refusal is the box's own, before any edge is asked.
        code, body = api("POST", "/api/market/listings", {"kind": "storage", "unit_price": 100, "capacity": 4})
        assert code == "409", f"a listing on a box sharing only locally answered {code}: {body!r}"
        assert "nothing shared that way is sold" in json.loads(body)["error"], body
        # Nothing is paused or resumed over a running rebuild.
        settled()
        assert edge_doc()["localSharingPausedAt"] is None
        capture("on", ["light", "dark", "phone"])

    with subtest("no local gateway for a while pauses it"):
        gateway.succeed("systemctl stop losos-registrar.service")
        wait_for("the gateway to go", lambda d: d["localGateway"] is False)
        # The bare edge is still there, and it does not count.
        assert edge_doc()["reachable"] is True
        wait_for("the pause", lambda d: d["localSharingPausedAt"] is not None)
        assert "losos.sharingLocally = false;" in overrides_now(), overrides_now()
        box.succeed(
            "journalctl -u lososd --no-pager "
            "| grep -F 'sharing with this network is paused until one does'"
        )
        settled()
        capture("paused", ["light"])

    with subtest("the gateway comes back and so does the sharing"):
        gateway.succeed("systemctl start losos-registrar.service")
        gateway.wait_until_succeeds("curl -fsS http://localhost:8443/health")
        wait_for("the resume", lambda d: d["localSharingPausedAt"] is None)
        assert "losos.sharingLocally = true;" in overrides_now(), overrides_now()
        box.succeed(
            "journalctl -u lososd --no-pager "
            "| grep -F 'a local gateway answers again: sharing with this network is back on'"
        )
        state = json.loads(box.succeed("cat /var/lib/losos/state.json"))
        assert state.get("local_sharing_paused_at") is None, state

    laptop.copy_from_machine("/tmp/screens", "")
  '';
}
