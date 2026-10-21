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
# checkout's admin UI instead and only takes the pictures of the old Lab,
# for the Before column:
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

  shipped =
    if after then
      lososPkgs.losos-admin-ui
    else
      (import (before + "/nix/packages.nix") { inherit pkgs; }).losos-admin-ui;
  # The page imports the GPU canvas lazily; a module that refuses to start
  # keeps it on SVG, as a browser without WebGPU or WebGL2 does.
  noGpu = ''
    for api in webgpu webgl2; do
      mkdir -p src/lab/render-pkg/$api
      echo 'export default async function () { throw new Error("no GPU canvas in this test"); }' \
        > src/lab/render-pkg/$api/losos_lab_render.js
      printf '\0asm\1\0\0\0' > src/lab/render-pkg/$api/losos_lab_render_bg.wasm
    done
  '';
  ui = shipped.overrideAttrs (o: {
    pname = "losos-admin-ui-svg-only";
    preBuild = o.passthru.labCorePkg + o.passthru.labVirtPkg + noGpu;
    # vite alone: the type check needs the real module's declarations.
    buildPhase = ''
      runHook preBuild
      npx vite build
      runHook postBuild
    '';
  });

  stubOcc = pkgs.writeShellScriptBin "nextcloud-occ" ''
    case "$1" in
      status) echo '{"installed":true,"maintenance":false,"needsDbUpgrade":false}' ;;
      user:resetpassword) echo "Successfully reset password for $2" ;;
      *) echo "stub occ: $*" >&2; exit 1 ;;
    esac
  '';

  # A diagram made elsewhere, as Obsidian Canvas writes one: a group, cards
  # in Markdown, shapes and colours with Advanced Canvas's names, labelled
  # arrows, and one key from another plugin on the document and a card.
  launch = pkgs.writeText "launch.canvas" (
    builtins.toJSON {
      nodes = [
        {
          id = "week";
          type = "group";
          label = "Moving the office to LosOS";
          x = -620;
          y = -300;
          width = 1240;
          height = 620;
          color = "5";
        }
        {
          id = "nas";
          type = "text";
          text = "## Back up the old NAS\n- [x] photos\n- [x] shared drive\n- [ ] invoices from 2024";
          x = -580;
          y = -240;
          width = 300;
          height = 180;
          "x-tasks-plugin" = {
            due = "2026-10-30";
          };
        }
        {
          id = "copied";
          type = "text";
          text = "Everything copied?";
          x = -200;
          y = -220;
          width = 240;
          height = 140;
          color = "3";
          styleAttributes = {
            shape = "diamond";
          };
        }
        {
          id = "cloud";
          type = "text";
          text = "**LosOS Cloud**\nfiles for the whole team";
          x = 140;
          y = -230;
          width = 260;
          height = 120;
          color = "4";
          styleAttributes = {
            shape = "database";
          };
        }
        {
          id = "dns";
          type = "text";
          text = "Point `office.example` at the box";
          x = 140;
          y = 0;
          width = 260;
          height = 90;
          color = "6";
          styleAttributes = {
            shape = "pill";
          };
        }
        {
          id = "fix";
          type = "text";
          text = "> [!warning] Not yet\n> copy what is missing, then check again";
          x = -580;
          y = 40;
          width = 320;
          height = 140;
          color = "1";
          styleAttributes = {
            border = "dashed";
          };
        }
        {
          id = "guide";
          type = "link";
          url = "https://losos.dasmat.us";
          x = 440;
          y = -230;
          width = 190;
          height = 120;
        }
      ];
      edges = [
        {
          id = "e1";
          fromNode = "nas";
          fromSide = "right";
          toNode = "copied";
          toSide = "left";
          toEnd = "arrow";
        }
        {
          id = "e2";
          fromNode = "copied";
          fromSide = "right";
          toNode = "cloud";
          toSide = "left";
          toEnd = "arrow";
          label = "yes";
        }
        {
          id = "e3";
          fromNode = "copied";
          fromSide = "bottom";
          toNode = "fix";
          toSide = "top";
          toEnd = "arrow";
          label = "no";
          styleAttributes = {
            path = "short-dashed";
          };
        }
        {
          id = "e4";
          fromNode = "cloud";
          fromSide = "bottom";
          toNode = "dns";
          toSide = "top";
          toEnd = "arrow";
        }
      ];
      "x-board-plugin" = {
        lane = "this week";
      };
    }
  );

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
        ../../modules/options.nix
        ../../modules/containers.nix
        ../../modules/tls.nix
        ../../modules/daemon.nix
        ../../modules/identity.nix
      ]
      ++ lib.optional after ../../modules/cloud.nix;
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
        with subtest("/lab/ moves to /diagrams/, and /diagrams/ is the page"):
            moved = curl(f"-o /dev/null -w '%{{http_code}} %{{redirect_url}}' 'https://{BOX}/lab/?canvas=svg'")
            assert moved == f"301 https://{BOX}/diagrams/?canvas=svg", moved
            page = curl(f"-f https://{BOX}/diagrams/")
            assert "<title>LosOS Diagrams</title>" in page, page[:300]
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
