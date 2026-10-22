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

  # Every page of every app, in the order a person meets them.
  pages = [
    {
      name = "admin-home";
      path = "/";
      settle = 1500;
      # The tiles, once the box has found its apps: both columns in the
      # same state, not one of them still loading.
      steps = [
        {
          waitFor = {
            role = "link";
            name = "Files";
          };
          timeout = 120000;
        }
      ];
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
        # What this change adds beyond the opening: the dialog leaving, and
        # a switch's thumb while it is held down (a still: the mouse stays down).
        (shot {
          name = "motion-palette-close";
          path = "/";
          strip = frames;
          steps = [
            { press = "Control+Shift+P"; }
            { waitFor.selector = "dialog[open]"; }
            { freeze = true; }
            { press = "Escape"; }
          ];
        })
        (shot {
          name = "motion-switch-hold";
          path = "/settings/security";
          steps = [
            { hold.selector = "[data-slot=switch]"; }
          ];
        })
      ];
    }
  );

  # The launch film's stills of the shared bar: the home under it, the
  # person's menu with Sign out and its keys, and the palette finding Sign out
  # as it is typed, one picture per letter. Dark, 1280x900 at two device
  # pixels per CSS pixel, with the caret hidden; film.json lists each one
  # with what is pressed next and where. The motion comes as frames at 60
  # per second (`clip`), for video.
  person = {
    selector = "[data-testid=signed-in-as]";
  };
  typed = text: {
    name = "palette-${text}";
    path = "/";
    steps = [
      { press = "Control+Shift+P"; }
      { waitFor.selector = "dialog[open] input"; }
      { type = text; }
    ];
    film = {
      title = "The palette finds Sign out, with its keys";
      action.kind = "type";
      targets = {
        field.selector = "dialog[open] input";
        signOut.selector = "dialog[open] >> text=Sign out";
      };
    };
  };
  filmClip = {
    fps = 60;
    ms = 300;
  };
  filmPlan = pkgs.writeText "film-plan.json" (
    builtins.toJSON {
      width = 1280;
      height = 900;
      variants = [
        {
          suffix = null;
          colorScheme = "dark";
          dpr = 2;
          timezoneId = "Europe/Bratislava";
          hideCaret = true;
        }
      ];
      shots = map shot [
        {
          name = "01-home";
          path = "/";
          settle = 1500;
          steps = [
            {
              waitFor = {
                role = "link";
                name = "Files";
              };
              timeout = 120000;
            }
          ];
          film = {
            title = "The box's home under the one top bar every app shares";
            action = {
              kind = "click";
              target = "person";
            };
            targets = {
              inherit person;
              bar.selector = "[data-slot=app-bar]";
            };
          };
        }
        {
          name = "02-person-menu";
          path = "/";
          steps = [
            { click = person; }
            { waitFor.selector = "[data-slot=person-menu]"; }
          ];
          film = {
            title = "The person's menu: Your account, and Sign out with Alt+Shift+Q";
            action = {
              kind = "hover";
              target = "signOut";
            };
            targets = {
              menu.selector = "[data-slot=person-menu]";
              signOut.selector = "[data-slot=person-menu] >> text=Sign out";
            };
          };
        }
        {
          name = "03-palette";
          path = "/";
          steps = [
            { press = "Control+Shift+P"; }
            { waitFor.selector = "dialog[open] input"; }
          ];
          film = {
            title = "The command palette";
            action = {
              kind = "type";
              target = "field";
            };
            targets.field.selector = "dialog[open] input";
          };
        }
        (typed "s" // { name = "04-palette-s"; })
        (typed "si" // { name = "05-palette-si"; })
        (typed "sig" // { name = "06-palette-sig"; })
        (
          typed "sign"
          // {
            name = "07-palette-sign";
          }
        )
        {
          name = "clip-person-menu";
          path = "/";
          clip = filmClip;
          steps = [
            { click = person; }
            { waitFor.selector = "[data-slot=person-menu]"; }
          ];
        }
        {
          name = "clip-palette-open";
          path = "/";
          clip = filmClip;
          steps = [
            { press = "Control+Shift+P"; }
            { waitFor.selector = "dialog[open]"; }
          ];
        }
        {
          name = "clip-palette-close";
          path = "/";
          clip = filmClip;
          steps = [
            { press = "Control+Shift+P"; }
            { waitFor.selector = "dialog[open]"; }
            { freeze = true; }
            { press = "Escape"; }
          ];
        }
        {
          name = "clip-switch";
          path = "/settings/security";
          clip = filmClip;
          steps = [
            { click.selector = "[data-slot=switch]"; }
          ];
        }
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
          forgejo.enable = false;
        };
        systemd.services.lososd.path = lib.mkForce [
          pkgs.coreutils
        ];
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
        environment.etc."losos-test/film-plan.json".source = filmPlan;
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
        print(laptop.succeed(
            f"losos-capture --base https://{BOX} --plan {plan} "
            f"--out {out} --user matus --password '{PASSWORD}'",
            timeout=3600,
        ))
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
        for name in ["motion-person-menu", "motion-palette", "motion-switch", "motion-palette-close"]:
            for ms in ["000", "080", "240"]:
                laptop.succeed(f"test -s /tmp/screens/{name}-light-desktop-{ms}ms.png")
            laptop.succeed(f"test -s /tmp/screens/{name}-light-desktop.png")

    with subtest("the launch film's stills of the shared bar"):
        try:
            capture("/etc/losos-test/film-plan.json", "/tmp/film")
        finally:
            laptop.copy_from_machine("/tmp/film", "")
        film = json.loads(laptop.succeed("cat /tmp/film/film.json"))
        assert len(film) == 7, film
        size = laptop.succeed("identify -format '%w %h' /tmp/film/01-home.png")
        assert size == "2560 1800", size
        for clip in ["person-menu", "palette-open", "palette-close", "switch"]:
            laptop.succeed(f"test -s /tmp/film/clip-{clip}-clip-018.png")
  '';
}
