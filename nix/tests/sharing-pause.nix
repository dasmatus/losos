# nixos-test-vms pictures of the Mesh page while the box has paused sharing
# because no edge answered (backend/src/edge.rs, `sharing_step`; the pause
# and the resume themselves run against a real edge in edge-lan.nix).
#
# A box with the real lososd, nginx, LosOS Login and the built admin UI, and
# a laptop on the LAN with a browser. The box is in the state the pause
# leaves it in: sharing off in overrides.nix, and `sharing_paused_at` in
# lososd's state.json, written while lososd is stopped as lososd itself
# writes it. No edge is on the LAN, so the row says the box is waiting.
#
# The derivation's output holds the pictures, one folder per plan.
# `before`, a path to an earlier admin-ui/app, builds that UI instead:
#
#   nix build --impure --expr '(builtins.getFlake (toString ./.)).inputs.nixpkgs.legacyPackages.x86_64-linux.callPackage ./nix/tests/sharing-pause.nix {
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

  # What the pause leaves: sharing off, the tunnel still on.
  overrides = pkgs.writeText "overrides-paused.nix" ''
    { ... }:

    {
      losos.sharingMyStorage = false;
      losos.hostName = "kitchenbox";
      losos.gpu.enable = false;
      losos.proxy.enable = true;
      losos.cluster.enable = false;
      losos.cluster.shareCompute = false;
      losos.cluster.computeWindow.start = "23:00";
      losos.cluster.computeWindow.end = "07:00";
      losos.hardening.apparmor = false;
      losos.hardening.malloc = false;
      losos.hardening.nosmt = false;
      losos.hardening.usbguard = false;
    }
  '';

  # 2026-10-23 06:12 UTC: when the box paused sharing.
  pausedAt = 1792735920;

  ready =
    if after then
      { text = "Waiting for an edge"; }
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
  name = if after then "losos-sharing-pause" else "losos-sharing-pause-before";

  nodes = {
    box =
      { lib, ... }:
      {
        imports = [
          ../../modules/options.nix
          ../../modules/containers.nix
          ../../modules/tls.nix
          ../../modules/daemon.nix
          ../../modules/identity.nix
        ];
        losos = {
          hostName = "kitchenbox";
          backend.package = lososPkgs.losos-ctl;
          backend.plugins = lososPkgs.losos-plugins;
          admin.ui = ui;
          git.enable = false;
        };
        systemd.services.lososd.path = lib.mkForce [ pkgs.coreutils ];
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

    BOX = "192.168.1.1"
    PASSWORD = "Correct-Horse-Battery-9"

    start_all()
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

    with subtest("the box paused sharing when no edge answered"):
        box.succeed("mkdir -p /etc/nixos/modules && install -m 0644 ${overrides} /etc/nixos/modules/overrides.nix")
        box.systemctl("stop lososd.service")
        box.succeed(
            "jq '.sharing = false | .sharing_paused_at = ${toString pausedAt} | .sharing_edge_seen = true' "
            "/var/lib/losos/state.json > /tmp/state.json && install -m 0600 /tmp/state.json /var/lib/losos/state.json"
        )
        box.systemctl("start lososd.service")
        box.wait_for_open_port(8082)
        token = box.succeed(
            "systemd-creds decrypt --name=admin-token /etc/credstore.encrypted/admin-token -"
        ).strip()
        edge = json.loads(box.succeed(f"curl -fsS -H 'Authorization: Bearer {token}' http://127.0.0.1:8082/api/edge"))
        if ${lib.boolToString after}:
            assert edge["sharingPausedAt"] == ${toString pausedAt}, edge
        # No edge on this LAN: the pause holds.
        assert edge["reachable"] is False, edge

    with subtest("the Mesh page says so"):
        for plan in ["light", "dark", "phone"]:
            laptop.succeed(
                f"losos-capture --base https://{BOX} --plan /etc/losos-test/plan-{plan}.json "
                f"--out /tmp/screens/{plan} --user matus --password '{PASSWORD}'",
                timeout=1800,
            )
        state = json.loads(box.succeed("cat /var/lib/losos/state.json"))
        assert state.get("sharing_paused_at") == ${toString pausedAt}, state

    laptop.copy_from_machine("/tmp/screens", "")
  '';
}
