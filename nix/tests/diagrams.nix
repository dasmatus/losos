# nixos-test-vms config for LosOS Diagrams (admin-ui/app/diagrams/,
# src/lab/canvas/, admin-ui/lab/core/src/canvas/) on a real box: lososd,
# nginx, LosOS Login and LosOS Cloud, with a laptop on the LAN whose
# browser takes the pull request's pictures.
#
# The test asserts:
#
#   * `/lab/` answers 301 to `/diagrams/`, query kept, and `/diagrams/`
#     is the page, LAN-only like the rest of the admin surface;
#   * a `.canvas` file put in LosOS Cloud over WebDAV opens in Diagrams from
#     `?file=`, takes a new card, and Ctrl+S writes it back over the version
#     it read: the file in LosOS Cloud then has the card, and the keys from
#     other tools it came with;
#   * LosOS Cloud's own page lists the file, and the Diagrams dialog lists
#     the folder, both through the box's real API;
#   * the tray's Machines says why a box that shares no disk has none.
#
# The GPU canvas (admin-ui/lab/render) is not built here: the VMs have no
# GPU, every shot pins `?canvas=svg`, and the Diagram view never uses it.
# Its two Bevy modules take longer to build than the test takes to run.
#
# `before`, a path to an earlier checkout of the repository, builds that
# checkout's admin UI and serves it with that checkout's modules (its nginx
# knows the old paths), and only takes the pictures of the old Lab, for the
# Before column:
#
#   nix build --impure --expr '(import ./nix/tests/diagrams.nix {
#     pkgs = (builtins.getFlake (toString ./.)).inputs.nixpkgs.legacyPackages.x86_64-linux;
#     before = /path/to/main; })'
{
  pkgs,
  before ? null,
}:

let
  inherit (pkgs) lib;
  lososPkgs = import ../packages.nix { inherit pkgs; };
  screens = import ./screens { inherit pkgs; };
  after = before == null;
  modules = if after then ../../modules else before + "/modules";

  shipped =
    if after then
      lososPkgs.losos-admin-ui
    else
      (import (before + "/nix/packages.nix") { inherit pkgs; }).losos-admin-ui;
  # The admin UI with Diagrams on SVG only, and the diagram from elsewhere.
  common = import ./diagrams-common.nix { inherit pkgs shipped; };
  inherit (common) ui launch;

  stubOcc = pkgs.writeShellScriptBin "nextcloud-occ" ''
    case "$1" in
      status) echo '{"installed":true,"maintenance":false,"needsDbUpgrade":false}' ;;
      user:resetpassword) echo "Successfully reset password for $2" ;;
      *) echo "stub occ: $*" >&2; exit 1 ;;
    esac
  '';

  # The page has drawn once its splash is gone and the canvas is there.
  ready = [
    { gone.selector = "[data-testid=lab-splash]"; }
    { waitFor.selector = "[data-testid=diagram]"; }
  ];
  card = label: { waitFor.selector = "[data-testid=diagram] [data-node][aria-label=\"${label}\"]"; };
  fresh = [
    { click.selector = "[data-testid=file-menu]"; }
    { click.selector = "[data-testid=file-new]"; }
    { waitFor.selector = "[data-testid=diagram-empty]"; }
  ];
  opened = "/diagrams/?canvas=svg&file=Diagrams%2Flaunch.canvas";
  # The overview has heard from the box and listed its apps.
  overviewLoaded = [
    {
      waitFor = {
        role = "heading";
        name = "mattbox";
      };
      timeout = 180000;
    }
    {
      gone.text = "Looking for the apps on this box.";
      timeout = 180000;
    }
  ];

  plan =
    name: attrs:
    pkgs.writeText "plan-${name}.json" (
      builtins.toJSON (
        {
          width = 1440;
          height = 900;
        }
        // attrs
      )
    );

  # The Before column: the Lab as it was, at its old address, and the
  # sidebar that linked to it.
  beforePlans = {
    light = plan "light" {
      shots = [
        {
          name = "network";
          path = "/lab/?canvas=svg";
          waitFor.selector = "[data-testid=lab-canvas-wrap]";
          steps = [ { gone.selector = "[data-testid=lab-splash]"; } ];
          settle = 1500;
        }
        {
          name = "sidebar";
          path = "/";
          waitFor = {
            role = "link";
            name = "Lab";
          };
          steps = overviewLoaded;
        }
      ];
    };
  };

  afterPlans = {
    light = plan "light" {
      shots = [
        {
          name = "network";
          path = "/diagrams/?canvas=svg&view=logical";
          waitFor.selector = "[data-testid=lab-canvas-wrap]";
          steps = [ { gone.selector = "[data-testid=lab-splash]"; } ];
          settle = 1500;
        }
        {
          name = "sidebar";
          path = "/";
          waitFor = {
            role = "link";
            name = "Diagrams";
          };
          steps = overviewLoaded;
        }
        {
          name = "this-box";
          path = "/diagrams/?canvas=svg";
          steps = ready ++ [
            { waitFor.selector = "[data-testid=diagram] [data-node][aria-label^=\"mattbox,\"]"; }
            # The fallback setup has acme-gw; the box's own drawing does not.
            { absent.selector = "[data-testid=diagram] [data-node][aria-label^=\"acme-gw,\"]"; }
            { absent.text = "Sign in on the admin page first, then open Diagrams from its sidebar: Diagrams reads this box with the same key."; }
          ];
          settle = 1500;
        }
        {
          name = "from-cloud";
          path = opened;
          steps = ready ++ [
            (card "Back up the old NAS")
            { absent.text = "Sign in on the admin page first, then open Diagrams from its sidebar: Diagrams reads this box with the same key."; }
          ];
          settle = 1500;
        }
        {
          name = "saved-to-cloud";
          path = opened;
          steps = ready ++ [
            (card "Back up the old NAS")
            { click.selector = "[data-testid=tray-items] [data-card=note]"; }
            { click.selector = "[data-testid=diagram-wrap]"; }
            {
              fill = {
                label = "Card text, in Markdown";
                value = "Ask the team about the printer";
              };
            }
            { press = "Escape"; }
            { press = "Control+s"; }
            { waitText = "Saved launch in LosOS Cloud (Diagrams/launch.canvas)."; }
          ];
        }
        {
          name = "cloud-dialog";
          path = "/diagrams/?canvas=svg";
          steps = ready ++ [
            { click.selector = "[data-testid=file-menu]"; }
            { click.selector = "[data-testid=cloud-open]"; }
            {
              click = {
                role = "button";
                name = "Diagrams";
              };
            }
            {
              waitFor = {
                role = "button";
                name = "launch.canvas";
              };
            }
          ];
        }
        {
          name = "template";
          path = "/diagrams/?canvas=svg";
          steps =
            ready
            ++ fresh
            ++ [
              {
                click = {
                  role = "button";
                  name = "Templates";
                };
              }
              { click.selector = "[data-testid=tray-items] [data-template=architecture]"; }
              { click.selector = "[data-testid=diagram-wrap]"; }
              { gone.selector = "[data-testid=diagram-empty]"; }
              { press = "Shift+1"; }
            ];
          settle = 1000;
        }
        {
          name = "markdown";
          path = "/diagrams/?canvas=svg";
          steps =
            ready
            ++ fresh
            ++ [
              { click.selector = "[data-testid=tray-items] [data-card=text]"; }
              { click.selector = "[data-testid=diagram-wrap]"; }
              {
                fill = {
                  label = "Card text, in Markdown";
                  value = "# Release notes\n\n- [x] the diagram editor\n- [x] files in LosOS Cloud\n- [ ] the film\n\n**Friday**, see [the handbook](https://losos.dasmat.us)";
                };
              }
              { press = "Escape"; }
              (card "Release notes")
            ];
        }
        {
          name = "machines";
          path = "/diagrams/?canvas=svg";
          steps = ready ++ [
            {
              click = {
                role = "button";
                name = "Machines";
              };
            }
            { waitFor.selector = "[data-testid=machines-unavailable]"; }
          ];
        }
        {
          name = "cloud-folder";
          path = "/cloud/files/Diagrams";
          waitFor.text = "launch.canvas";
        }
      ];
    };
    dark = plan "dark" {
      colorScheme = "dark";
      shots = [
        {
          name = "from-cloud";
          path = opened;
          steps = ready ++ [
            (card "Back up the old NAS")
            { absent.text = "Sign in on the admin page first, then open Diagrams from its sidebar: Diagrams reads this box with the same key."; }
          ];
          settle = 1500;
        }
      ];
    };
    phone = plan "phone" {
      width = 390;
      height = 844;
      shots = [
        {
          name = "from-cloud";
          path = opened;
          steps = ready ++ [
            (card "Back up the old NAS")
            { absent.text = "Sign in on the admin page first, then open Diagrams from its sidebar: Diagrams reads this box with the same key."; }
          ];
          settle = 1500;
        }
      ];
    };
    sk = plan "sk" {
      locale = "sk-SK";
      shots = [
        {
          name = "from-cloud";
          path = opened;
          steps = ready ++ [
            (card "Back up the old NAS")
            { absent.text = "Sign in on the admin page first, then open Diagrams from its sidebar: Diagrams reads this box with the same key."; }
          ];
          settle = 1500;
        }
      ];
    };
  };
  plans = if after then afterPlans else beforePlans;
in

pkgs.testers.nixosTest {
  name = if after then "losos-diagrams" else "losos-diagrams-before";

  nodes = {
    box = _: {
      imports = [
        (modules + "/options.nix")
        (modules + "/containers.nix")
        (modules + "/tls.nix")
        (modules + "/daemon.nix")
        (modules + "/identity.nix")
      ]
      ++ lib.optional after (modules + "/cloud.nix");
      networking.hostName = "mattbox";
      losos = {
        hostName = "mattbox";
        backend.package = lososPkgs.losos-ctl;
        backend.plugins = lososPkgs.losos-plugins;
        admin.ui = ui;
        nextcloud.mode = "native";
        forgejo.enable = false;
      }
      // lib.optionalAttrs after { cloud.enable = true; };
      systemd.services.lososd.path = lib.mkBefore [ stubOcc ];
      systemd.services.lososd.environment.LOSOS_NEXTCLOUD_MODE = lib.mkForce "native";
      environment.systemPackages = [
        pkgs.jq
        pkgs.curl
      ];
      virtualisation = {
        memorySize = 2048;
        cores = 2;
        diskSize = 4096;
        vlans = [ 1 ];
      };
    };

    laptop = _: {
      imports = [ screens.browser ];
      environment.systemPackages = [
        pkgs.curl
        pkgs.jq
      ];
      environment.etc =
        lib.mapAttrs' (
          name: file: lib.nameValuePair "losos-test/plan-${name}.json" { source = file; }
        ) plans
        // {
          "losos-test/launch.canvas".source = launch;
        };
      virtualisation.vlans = [ 1 ];
    };
  };

  testScript = ''
    import json
    import re

    BOX = "192.168.1.1"
    PASSWORD = "Correct-Horse-Battery-9"
    AFTER = ${if after then "True" else "False"}

    start_all()
    box.wait_for_unit("lososd.service")
    box.wait_for_unit("nginx.service")
    box.wait_for_open_port(443)
    if AFTER:
        box.wait_for_unit("losos-cloud.service")
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

    def capture(plan):
        laptop.succeed(
            f"losos-capture --base https://{BOX} --plan /etc/losos-test/plan-{plan}.json "
            f"--out /tmp/screens/{plan} --user matus --password '{PASSWORD}'",
            timeout=2400,
        )

    with subtest("claim the box and sign in"):
        claim = json.dumps({"name": "matus", "password": PASSWORD})
        laptop.wait_until_succeeds(
            f"curl -sfk https://{BOX}/api/setup/claim -H 'Host: {BOX}' | jq -e .ready", timeout=600
        )
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

    if AFTER:
        with subtest("/lab/ moves to /diagrams/, and /diagrams/ is the admin page"):
            moved = curl(f"-o /dev/null -w '%{{http_code}} %{{redirect_url}}' 'https://{BOX}/lab/?canvas=svg'")
            assert moved == f"301 https://{BOX}/diagrams/?canvas=svg", moved
            page = curl(f"-f https://{BOX}/diagrams/")
            assert '<div id="root">' in page, page[:300]
            policy = curl(f"-D - -o /dev/null https://{BOX}/diagrams/")
            assert "wasm-unsafe-eval" in policy, policy

        DAV = f"https://{BOX}/remote.php/webdav/"
        with subtest("a diagram from another tool goes into LosOS Cloud"):
            code = curl(f"-u 'matus:{PASSWORD}' -X MKCOL -o /dev/null -w '%{{http_code}}' '{DAV}Diagrams'")
            assert code == "201", code
            code = curl(
                f"-u 'matus:{PASSWORD}' -T /etc/losos-test/launch.canvas -o /dev/null -w '%{{http_code}}' "
                f"'{DAV}Diagrams/launch.canvas'"
            )
            assert code in ("201", "204"), code

    with subtest("the pictures"):
        for plan in ${builtins.toJSON (builtins.attrNames plans)}:
            capture(plan)

    if AFTER:
        with subtest("the save from the page is in LosOS Cloud, other tools' keys kept"):
            doc = json.loads(curl(f"-f -u 'matus:{PASSWORD}' '{DAV}Diagrams/launch.canvas'"))
            texts = [n.get("text", "") for n in doc["nodes"]]
            assert "Ask the team about the printer" in texts, texts
            assert doc["x-board-plugin"] == {"lane": "this week"}, doc.get("x-board-plugin")
            nas = next(n for n in doc["nodes"] if n["id"] == "nas")
            assert nas["x-tasks-plugin"] == {"due": "2026-10-30"}, nas
            copied = next(n for n in doc["nodes"] if n["id"] == "copied")
            assert copied["styleAttributes"]["shape"] == "diamond", copied

    laptop.copy_from_machine("/tmp/screens", "")
  '';
}
