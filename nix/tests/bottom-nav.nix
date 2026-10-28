# nixos-test-vms config for the phone's bottom bar
# (admin-ui/app/src/components/ui/bottom-nav.tsx).
#
# A box with the real lososd, nginx, LosOS Login and the built admin UI, and
# a laptop on the LAN with a browser at a phone's width (390 px). It asserts
# what a person holding the phone would notice:
#
#   * every page has the bar along the bottom edge, with the page's own tab
#     marked, and a page the bar does not name marks More;
#   * More raises the whole sidebar as a sheet from the bottom, and a place
#     chosen in it closes the sheet;
#   * the foot of a long page scrolls clear of the bar;
#   * no page is wider than the phone (report.json);
#   * a desktop's width has the sidebar and no bar.
#
# The laptop's pictures are the ones the pull request shows: the
# derivation's output holds them, one folder per plan.
#
# `before`, a path to an earlier admin-ui/app, builds that UI instead and
# only takes the pictures, for the Before column:
#
#   nix build --impure --expr '(import ./nix/tests/bottom-nav.nix {
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

  # The admin UI as it ships, without the Diagrams page, which this test
  # never opens and whose WebAssembly takes longer to build than the test.
  ui = import ./admin-ui-without-lab.nix (
    lososPkgs.losos-admin-ui.overrideAttrs (_: lib.optionalAttrs (!after) { src = before; })
  );

  bar = {
    selector = "[data-slot=bottom-nav]";
  };
  tab = name: {
    selector = "[data-slot=bottom-nav] a[aria-current=page]:has-text('${name}')";
  };
  moreLit = {
    selector = "[data-slot=bottom-nav] button[aria-haspopup=dialog][data-active]";
  };
  more = {
    role = "button";
    name = "More";
  };
  sheet = {
    role = "dialog";
    name = "Sections";
  };
  # The Overview has drawn once the box's name is its heading.
  overviewReady = {
    role = "heading";
    name = "mattbox";
  };
  # Before this change a phone opened the sidebar from a row at the top.
  opener =
    if after then
      more
    else
      {
        role = "button";
        name = "Expand the sidebar";
      };
  # What only the bar can show; nothing before it to wait for.
  sees = steps: lib.optionals after steps;

  phoneShots = [
    {
      name = "overview";
      path = "/";
      waitFor = overviewReady;
      steps = sees [ { waitFor = tab "Overview"; } ];
    }
    {
      name = "storage";
      path = "/storage";
      waitFor = {
        role = "heading";
        name = "Storage";
      };
      steps = sees [ { waitFor = tab "Storage"; } ];
    }
    {
      name = "mesh";
      path = "/mesh";
      waitFor = {
        role = "switch";
        name = "Join the mesh";
      };
      steps = sees [ { waitFor = tab "Mesh"; } ];
    }
    # A page the bar does not name: More is lit, and the foot of the page
    # clears the bar.
    {
      name = "about";
      path = "/settings/about";
      waitFor = {
        role = "heading";
        name = "About";
      };
      steps = sees [ { waitFor = moreLit; } ];
    }
    {
      name = "about-end";
      path = "/settings/about";
      waitFor = {
        role = "heading";
        name = "About";
      };
      steps = [
        { scroll = "end"; }
        { sleep = 500; }
      ];
    }
    {
      name = "more";
      path = "/";
      waitFor = overviewReady;
      steps = [
        { click = opener; }
        { waitFor = sheet; }
        { sleep = 800; }
      ];
    }
  ];

  plan =
    name: attrs:
    pkgs.writeText "plan-${name}.json" (
      builtins.toJSON (
        {
          width = 390;
          height = 844;
        }
        // attrs
      )
    );

  plans = {
    phone = plan "phone" { shots = phoneShots; };
    phone-dark = plan "phone-dark" {
      colorScheme = "dark";
      shots = phoneShots;
    };
    # A place chosen in the sheet goes there and puts the sheet away.
    choose = plan "choose" {
      shots = lib.optionals after [
        {
          name = "chosen";
          path = "/";
          waitFor = overviewReady;
          steps = [
            { click = more; }
            { waitFor = sheet; }
            {
              click = {
                role = "button";
                name = "Settings";
              };
            }
            {
              click = {
                role = "link";
                name = "Security";
              };
            }
            { gone = sheet; }
            {
              waitFor = {
                role = "heading";
                name = "Security";
              };
            }
            { waitFor = moreLit; }
          ];
        }
      ];
    };
    desktop = plan "desktop" {
      width = 1280;
      height = 800;
      shots = [
        {
          name = "overview";
          path = "/";
          waitFor = overviewReady;
          steps = [
            {
              waitFor = {
                role = "navigation";
                name = "Sections";
              };
            }
            { absent = bar; }
          ];
        }
      ];
    };
  };
in

pkgs.testers.nixosTest {
  name = if after then "losos-bottom-nav" else "losos-bottom-nav-before";

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

    BOX = "192.168.1.1"
    PASSWORD = "Correct-Horse-Battery-9"
    AFTER = ${if after then "True" else "False"}

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

    def capture(plan):
        laptop.succeed(
            f"losos-capture --base https://{BOX} --plan /etc/losos-test/plan-{plan}.json "
            f"--out /tmp/screens/{plan} --user matus --password '{PASSWORD}'",
            timeout=1800,
        )
        return json.loads(laptop.succeed(f"cat /tmp/screens/{plan}/report.json"))

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

    with subtest("on a phone: the bar, the marked tab, More and its sheet"):
        for plan in ["phone", "phone-dark"]:
            for seen in capture(plan):
                assert seen["pageWidth"] <= seen["width"], f"{plan}: a page scrolls sideways: {seen}"

    if AFTER:
        with subtest("a place chosen in the sheet goes there and closes it"):
            capture("choose")

        with subtest("a desktop keeps the sidebar and has no bar"):
            capture("desktop")

    laptop.copy_from_machine("/tmp/screens", "")
  '';
}
