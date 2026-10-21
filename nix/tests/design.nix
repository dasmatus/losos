# nixos-test-vms config for the look every page shares.
#
# A box with the real lososd, nginx, LosOS Login and the built admin UI, and
# a laptop on the LAN that photographs each page four ways: light and dark,
# at a desktop's width and at a phone's (390 px). Each app adds its own pages
# to `pages` below as it lands, so one run puts every app side by side under
# the same light, the same width and the same signed-in person. It asserts
# what a person would notice first:
#
#   * no page is wider than a phone's screen, which would make it scroll
#     sideways;
#   * every page has the shared top bar (components/app-bar.tsx);
#   * a background picture under a Dark picked on the page is veiled dark,
#     not light.
#
# It also takes the shared motion frame by frame (`motion-*-<ms>ms.png`, see
# screens/capture.mjs): the person's menu opening from its button, the
# palette's dialog coming in and a switch thrown. Each strip has a still
# beside it with the motion finished.
#
# The derivation's output holds the pictures (screens/) and report.json.
{ pkgs }:

let
  lososPkgs = import ../packages.nix { inherit pkgs; };
  screens = import ./screens { inherit pkgs; };

  stubOcc = pkgs.writeShellScriptBin "nextcloud-occ" ''
    case "$1" in
      status) echo '{"installed":true,"maintenance":false,"needsDbUpgrade":false}' ;;
      user:resetpassword) echo "Successfully reset password for $2" ;;
      *) echo "stub occ: $*" >&2; exit 1 ;;
    esac
  '';
  standIn =
    name:
    pkgs.hello
    // {
      imageName = name;
      imageTag = "test";
    };

  # Every page of every app, in the order a person meets them.
  pages = [
    {
      name = "admin-home";
      path = "/";
      settle = 1500;
    }
    {
      name = "admin-apps";
      path = "/apps";
    }
    {
      name = "admin-storage";
      path = "/storage";
    }
    {
      name = "admin-account";
      path = "/settings/account";
    }
    {
      name = "admin-look";
      path = "/settings/look";
    }
  ];

  shot =
    page:
    {
      waitFor.selector = "[data-slot=app-bar]";
      settle = 800;
    }
    // page;

  plan = pkgs.writeText "plan.json" (
    builtins.toJSON {
      width = 1280;
      height = 800;
      variants = [
        {
          suffix = "light-desktop";
          colorScheme = "light";
        }
        {
          suffix = "dark-desktop";
          colorScheme = "dark";
        }
        {
          suffix = "light-phone";
          colorScheme = "light";
          width = 390;
          height = 844;
        }
        {
          suffix = "dark-phone";
          colorScheme = "dark";
          width = 390;
          height = 844;
        }
      ];
      shots = map shot pages;
    }
  );

  # Dark picked on the page's own toggle in a light browser, over a picture:
  # the one combination the veil depends on the explicit-dark tokens for.
  veilPlan = pkgs.writeText "veil-plan.json" (
    builtins.toJSON {
      width = 1280;
      height = 800;
      variants = [
        {
          suffix = "picked-dark";
          colorScheme = "light";
          theme = "dark";
        }
      ];
      shots = [
        (shot {
          name = "veil-home";
          path = "/";
          settle = 2500;
        })
      ];
    }
  );

  # The shared motion, frame by frame, once in light at a desktop's width.
  # Every transition the shared pieces run is 100 to 200 ms long.
  frames = [
    0
    40
    80
    120
    160
    240
  ];
  motionPlan = pkgs.writeText "motion-plan.json" (
    builtins.toJSON {
      width = 1280;
      height = 800;
      variants = [
        {
          suffix = "light-desktop";
          colorScheme = "light";
        }
      ];
      shots = [
        (shot {
          name = "motion-person-menu";
          path = "/";
          strip = frames;
          steps = [
            { click.selector = "[data-testid=signed-in-as]"; }
            { waitFor.selector = "[data-slot=person-menu]"; }
          ];
        })
        (shot {
          name = "motion-palette";
          path = "/";
          strip = frames;
          steps = [
            { press = "Control+Shift+P"; }
            { waitFor.selector = "dialog[open]"; }
          ];
        })
        (shot {
          name = "motion-switch";
          path = "/settings/security";
          strip = frames;
          steps = [
            { click.selector = "[data-slot=switch]"; }
          ];
        })
      ];
    }
  );
in

pkgs.testers.nixosTest {
  name = "losos-design";

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
          ../../modules/workloads.nix
        ];
        losos = {
          hostName = "mattbox";
          backend.package = lososPkgs.losos-ctl;
          backend.plugins = lososPkgs.losos-plugins;
          admin.ui = lososPkgs.losos-admin-ui;
          nextcloud.mode = "container";
          forgejo.enable = false;
          workloads.nextcloudImage = standIn "losos-nextcloud-stand-in";
        };
        systemd.services.lososd.path = lib.mkForce [
          stubOcc
          pkgs.coreutils
        ];
        systemd.services.lososd.environment.LOSOS_NEXTCLOUD_MODE = lib.mkForce "native";
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
          pkgs.imagemagick
        ];
        environment.etc."losos-test/plan.json".source = plan;
        environment.etc."losos-test/veil-plan.json".source = veilPlan;
        environment.etc."losos-test/motion-plan.json".source = motionPlan;
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
    box.wait_for_unit("nginx.service")
    box.wait_for_open_port(443)
    # lososd is active before its plugins are loaded and its port is open.
    box.wait_for_open_port(8082)
    laptop.wait_for_unit("multi-user.target")

    def curl(args):
        return laptop.succeed(f"curl -sk {args}")

    def api(path, cookie=None, method="GET", data=None, extra=""):
        args = f"-X {method} https://{BOX}{path} {extra}"
        if cookie:
            args += f" -H 'Cookie: {cookie}'"
        if data is not None:
            args += f" -H 'Content-Type: application/json' -d '{json.dumps(data)}'"
        out = curl(f"-w '\\n%{{http_code}}' {args}")
        body, code = out.rsplit("\n", 1)
        return int(code), body

    def capture(plan, out):
        laptop.succeed(
            f"losos-capture --base https://{BOX} --plan {plan} "
            f"--out {out} --user matus --password '{PASSWORD}'",
            timeout=3600,
        )
        return json.loads(laptop.succeed(f"cat {out}/report.json"))

    with subtest("claim the box and sign in"):
        code, body = api(
            "/api/setup/claim",
            method="POST",
            data={"name": "matus", "password": PASSWORD},
            extra=f"-H 'Host: {BOX}'",
        )
        assert code == 200, (code, body)
        box.wait_for_unit("authelia-losos.service")
        box.systemctl("start losos-identity-setup.service")
        box.wait_until_succeeds(f"grep -q \"domain: '{BOX}'\" /run/losos-identity/session.yml")
        box.wait_until_succeeds("systemctl is-active authelia-losos.service")
        box.wait_for_open_port(9091)
        login = json.dumps({"username": "matus", "password": PASSWORD})
        head = curl(
            f"-D - -o /dev/null https://{BOX}/auth/api/firstfactor "
            f"-H 'Content-Type: application/json' -d '{login}'"
        )
        m = re.search(r"(?im)^set-cookie: (losos_session=[^;]+)", head)
        assert m, head
        cookie = m.group(1)

    with subtest("every page, light and dark, desktop and phone"):
        report = capture("/etc/losos-test/plan.json", "/tmp/screens")
        print(json.dumps(report, indent=2))
        wide = [r for r in report if r["pageWidth"] > r["width"]]
        assert not wide, f"pages wider than the window: {wide}"

    with subtest("a picked Dark veils the background picture dark"):
        code, body = api("/api/look", cookie, "POST", {"background": {"kind": "shipped", "name": "tide"}})
        assert code == 200, (code, body)
        capture("/etc/losos-test/veil-plan.json", "/tmp/screens")
        # The top-left of the page under the bar: the veiled picture. Dark
        # ground is rgb(10,14,18); a light veil there would read near white.
        rgb = laptop.succeed(
            "convert /tmp/screens/veil-home-picked-dark.png -crop 40x40+20+120 -resize 1x1! "
            "-format '%[fx:int(255*r)] %[fx:int(255*g)] %[fx:int(255*b)]' info:"
        )
        r, g, b = (int(x) for x in rgb.split())
        print(f"veil at the top left: {r} {g} {b}")
        assert max(r, g, b) < 128, f"the veil is light under a picked Dark: {rgb}"

    with subtest("the shared motion, frame by frame"):
        # On the box's own look, without the picture the veil check left.
        code, body = api("/api/look", cookie, "POST", {"background": {"kind": "none"}})
        assert code == 200, (code, body)
        try:
            capture("/etc/losos-test/motion-plan.json", "/tmp/screens")
        finally:
            laptop.copy_from_machine("/tmp/screens", "")
        for name in ["motion-person-menu", "motion-palette", "motion-switch"]:
            for ms in ["000", "080", "240"]:
                laptop.succeed(f"test -s /tmp/screens/{name}-light-desktop-{ms}ms.png")
            laptop.succeed(f"test -s /tmp/screens/{name}-light-desktop.png")
  '';
}
