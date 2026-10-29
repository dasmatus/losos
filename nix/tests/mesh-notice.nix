# nixos-test-vms config for the mesh banner (admin-ui/app/src/components/mesh-notice.tsx).
#
# A box with the real lososd, nginx, LosOS Login and the built admin UI, and
# a laptop on the LAN with a browser. The box's saved settings are what the
# banner reads (GET /api/settings, from /etc/nixos/modules/overrides.nix), so
# the test writes that file the way an Apply would and asserts:
#
#   * a box that shares its storage but has not joined a mesh shows the
#     banner on the Overview and on the Mesh pane, in both themes and at
#     phone width, and names no feature;
#   * the Dismiss button hides it, and a page opened afterwards in the same
#     browser does not show it again;
#   * once the box has joined, a fresh browser sees no banner.
#
# The laptop's pictures are the ones the pull request shows: the
# derivation's output holds them, one folder per plan.
#
# `before`, a path to an earlier admin-ui/app, builds that UI instead and
# only takes the pictures, for the Before column:
#
#   nix build --impure --expr '(import ./nix/tests/mesh-notice.nix {
#     pkgs = (builtins.getFlake (toString ./.)).inputs.nixpkgs.legacyPackages.x86_64-linux;
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

  # The admin UI as it ships, without the Lab page, which this test never
  # opens and whose WebAssembly takes longer to build than the test to run.
  ui = import ./admin-ui-without-lab.nix (
    # The earlier UI shares this one's lock file, so its node_modules too.
    lososPkgs.losos-admin-ui.overrideAttrs (_: lib.optionalAttrs (!after) { src = before; })
  );

  # The box's settings as Apply writes them: a named box with its tunnel on,
  # sharing its storage, joined to a mesh or not.
  overrides =
    joined:
    pkgs.writeText "overrides-${if joined then "joined" else "alone"}.nix" ''
      { ... }:

      {
        losos.sharingMyStorage = true;
        losos.hostName = "kitchenbox";
        losos.gpu.enable = true;
        losos.proxy.enable = true;
        losos.cluster.enable = ${lib.boolToString joined};
        losos.cluster.shareCompute = false;
        losos.cluster.computeWindow.start = "23:00";
        losos.cluster.computeWindow.end = "07:00";
        losos.hardening.apparmor = false;
        losos.hardening.malloc = false;
        losos.hardening.nosmt = false;
        losos.hardening.usbguard = false;
      }
    '';

  banner = {
    selector = "[data-notice=\"mesh\"]";
  };
  # The Overview has drawn once the box's name is its heading.
  overviewReady = {
    role = "heading";
    name = "kitchenbox";
  };
  # Before the banner existed there is nothing to wait for but the page.
  seeBanner = lib.optional after { waitFor = banner; };

  plan =
    name: attrs:
    pkgs.writeText "plan-${name}.json" (
      builtins.toJSON (
        {
          width = 1280;
          height = 800;
        }
        // attrs
      )
    );

  plans = {
    light = plan "light" {
      shots = [
        {
          name = "overview";
          path = "/";
          waitFor = overviewReady;
          steps = seeBanner;
        }
        {
          name = "mesh";
          path = "/mesh";
          waitFor = {
            role = "switch";
            name = "Join the mesh";
          };
          steps = seeBanner;
        }
      ]
      ++ lib.optionals after [
        {
          name = "dismissed";
          path = "/";
          waitFor = overviewReady;
          steps = [
            { waitFor = banner; }
            {
              click = {
                role = "button";
                name = "Dismiss";
              };
            }
            { gone = banner; }
          ];
        }
        # A new page in the same browser: the dismissal holds.
        {
          name = "dismissed-reload";
          path = "/mesh";
          waitFor = {
            role = "switch";
            name = "Join the mesh";
          };
          steps = [
            { sleep = 3000; }
            { absent = banner; }
          ];
        }
      ];
    };
    dark = plan "dark" {
      colorScheme = "dark";
      shots = [
        {
          name = "overview";
          path = "/";
          waitFor = overviewReady;
          steps = seeBanner;
        }
      ];
    };
    phone = plan "phone" {
      width = 390;
      height = 844;
      shots = [
        {
          name = "overview";
          path = "/";
          waitFor = overviewReady;
          steps = seeBanner;
        }
      ];
    };
    joined = plan "joined" {
      shots = [
        {
          name = "overview";
          path = "/";
          waitFor = overviewReady;
          steps = [
            { sleep = 3000; }
          ]
          ++ lib.optional after { absent = banner; };
        }
      ];
    };
  };
in

pkgs.testers.nixosTest {
  name = if after then "losos-mesh-notice" else "losos-mesh-notice-before";

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
          hostName = "mattbox";
          backend.package = lososPkgs.losos-ctl;
          backend.plugins = lososPkgs.losos-plugins;
          admin.ui = ui;
          git.enable = false;
        };
        systemd.services.lososd.path = lib.mkForce [ pkgs.coreutils ];
        environment.systemPackages = [ pkgs.jq ];
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
        environment.systemPackages = [
          pkgs.curl
          pkgs.jq
        ];
        environment.etc = lib.mapAttrs' (
          name: file: lib.nameValuePair "losos-test/plan-${name}.json" { source = file; }
        ) plans;
        virtualisation.vlans = [ 1 ];
      };
  };

  testScript = ''
    import json
    import re

    BOX = "192.168.1.1"
    PASSWORD = "Correct-Horse-Battery-9"

    start_all()
    box.wait_for_unit("lososd.service")
    # lososd's unit is up before its API: under emulation loading the
    # plugins takes it a minute, and the claim would meet nginx's 502.
    box.wait_for_open_port(8082)
    box.wait_for_unit("nginx.service")
    box.wait_for_open_port(443)
    laptop.wait_for_unit("multi-user.target")

    def curl(args):
        return laptop.succeed(f"curl -sk {args}")

    def sign_in(name, password):
        body = json.dumps({"username": name, "password": password})
        head = curl(
            f"-D - -o /dev/null https://{BOX}/auth/api/firstfactor "
            f"-H 'Content-Type: application/json' -d '{body}'"
        )
        m = re.search(r"(?im)^set-cookie: (losos_session=[^;]+)", head)
        assert m, f"no session cookie for {name}: {head}"
        return m.group(1)

    def settings(cookie):
        return json.loads(curl(f"https://{BOX}/api/settings -H 'Cookie: {cookie}'"))

    def write_settings(path):
        box.succeed(f"mkdir -p /etc/nixos/modules && install -m 0644 {path} /etc/nixos/modules/overrides.nix")

    def capture(plan):
        laptop.succeed(
            f"losos-capture --base https://{BOX} --plan /etc/losos-test/plan-{plan}.json "
            f"--out /tmp/screens/{plan} --user matus --password '{PASSWORD}'",
            timeout=1800,
        )

    with subtest("claim the box and sign in"):
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
        cookie = sign_in("matus", PASSWORD)

    with subtest("a box that shares its storage but has not joined a mesh"):
        write_settings("${overrides false}")
        s = settings(cookie)
        assert s["sharingMyStorage"] is True and s["clusterEnable"] is False, s
        for plan in ["light", "dark", "phone"]:
            capture(plan)

    with subtest("once the box has joined, the banner is gone"):
        write_settings("${overrides true}")
        s = settings(cookie)
        assert s["sharingMyStorage"] is True and s["clusterEnable"] is True, s
        capture("joined")

    laptop.copy_from_machine("/tmp/screens", "")
  '';
}
