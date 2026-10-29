# nixos-test-vms config for the admin pages without JavaScript
# (backend/plugins/basic/, served at /basic/ by modules/containers.nix).
#
# A box with the real lososd and its plugins, nginx, LosOS Login and the
# built admin UI, and a laptop on the LAN with a browser. What is asserted:
#
#   * every address of the admin UI sends a browser with JavaScript off to
#     the same page under /basic/, and nothing else rides along in the path;
#   * /basic/ is LosOS Login's: a visitor is sent to its sign-in page, plain
#     http goes to https, and the pages carry their own strict policy;
#   * with JavaScript off, a person signs in with their name and password,
#     opens the overview, storage, network, hardware and security, turns a
#     switch, sees the Apply bar count it, applies it, watches the box
#     rebuild and is told when it is done, and signs out;
#   * no page carries a script, and nothing trips the policy;
#   * the same pages with JavaScript on, for the pictures side by side.
#
# The rebuild is lososd's own `losos-rebuild-<job>` unit; a stand-in for
# `nixos-rebuild` makes it last long enough to be seen, then succeed.
#
# The laptop's pictures are the ones the pull request shows: the
# derivation's output holds them, one folder per plan. `before`, a path to
# an earlier admin-ui/app, builds that UI instead and only takes what a
# browser with JavaScript off saw there:
#
#   nix build --impure --expr '(import ./nix/tests/no-js.nix {
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

  # What lososd's rebuild unit runs: long enough for the page to show the
  # wait, then a success.
  stubRebuild = pkgs.writeShellScriptBin "nixos-rebuild" ''
    echo "building the system for $*"
    sleep 90
    echo "activating the new system"
  '';

  user = "matus";
  password = "Correct-Horse-Battery-9";

  heading = name: {
    role = "heading";
    inherit name;
  };
  # The sign-in form, in any language.
  signInPage = {
    selector = "#username";
  };
  overview = heading "mattbox";
  # The sign-in form, as a person fills it: Enter in the password sends it.
  signIn = [
    {
      click = {
        selector = "#username";
      };
    }
    { type = user; }
    {
      click = {
        selector = "#password";
      };
    }
    { type = password; }
    { press = "Enter"; }
    { waitFor = overview; }
  ];

  # The pages both kinds of browser open, by the script page's address.
  pages = [
    {
      name = "overview";
      path = "/";
      waitFor = overview;
    }
    {
      name = "storage";
      path = "/storage";
      waitFor = heading "Storage";
    }
    {
      name = "network";
      path = "/settings/network";
      waitFor = heading "Network";
    }
    {
      name = "hardware";
      path = "/settings/hardware";
      waitFor = heading "Hardware";
    }
    {
      name = "security";
      path = "/settings/security";
      waitFor = heading "Security";
    }
  ];

  gpu = {
    role = "switch";
    name = "Let apps use the graphics chip";
  };
  # The checkbox under a switch is visually hidden and its label takes
  # the pointer, as a person's click does.
  gpuSwitch = {
    selector = "label[data-slot=switch]:has(#f-gpuEnable)";
  };
  personMenu = {
    selector = "details:has([data-slot=person-menu]) > summary";
  };

  # JavaScript off: from the first address to signing out.
  withoutScript = [
    {
      name = "sign-in";
      path = "/";
      waitFor = signInPage;
    }
    {
      name = "signed-in";
      path = "/";
      waitFor = signInPage;
      steps = signIn;
    }
  ]
  ++ pages
  ++ [
    {
      name = "apply-bar";
      path = "/settings/hardware";
      waitFor = gpu;
      steps = [
        { click = gpuSwitch; }
        # The count is the stylesheet's own text (a counter), which
        # no locator reads; the bar showing is what the change did.
        {
          waitFor = {
            selector = "[data-basic-apply]";
          };
        }
      ];
    }
    {
      name = "menu";
      path = "/";
      waitFor = overview;
      steps = [
        { click = personMenu; }
        {
          waitFor = {
            role = "menuitem";
            name = "Sign out";
          };
        }
      ];
    }
  ];

  # Applying a change, its rebuild and its end, then signing out.
  applying = [
    {
      name = "applying";
      path = "/settings/hardware";
      waitFor = gpu;
      steps = [
        { click = gpuSwitch; }
        {
          click = {
            role = "button";
            name = "Apply";
          };
        }
        { waitText = "Applying your changes"; }
        # The page reloads itself every few seconds while the rebuild
        # runs; under emulation a screenshot takes longer than that.
        { stay = true; }
      ];
    }
    {
      name = "applied";
      path = "/settings/hardware";
      waitFor = heading "Hardware";
      steps = [
        {
          waitFor = {
            text = "Changes applied";
          };
          timeout = 600000;
        }
      ];
    }
    {
      name = "signed-out";
      path = "/";
      waitFor = overview;
      steps = [
        { click = personMenu; }
        {
          click = {
            role = "menuitem";
            name = "Sign out";
          };
        }
        { waitFor = signInPage; }
      ];
    }
  ];

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
  phone = {
    width = 390;
    height = 844;
    colorScheme = "dark";
  };

  plans =
    if after then
      {
        off-desktop = plan "off-desktop" {
          script = false;
          shots = withoutScript;
        };
        off-phone = plan "off-phone" (
          phone
          // {
            script = false;
            shots = withoutScript;
          }
        );
        off-apply = plan "off-apply" {
          script = false;
          shots = [
            {
              name = "signed-in";
              path = "/";
              waitFor = signInPage;
              steps = signIn;
            }
          ]
          ++ applying;
        };
        # The handbook's picture, in each of its languages.
        handbook = plan "handbook" {
          script = false;
          variants =
            map
              (l: {
                inherit (l) suffix;
                inherit (l) locale;
              })
              [
                {
                  suffix = "en";
                  locale = "en-US";
                }
                {
                  suffix = "sk";
                  locale = "sk-SK";
                }
                {
                  suffix = "de";
                  locale = "de-DE";
                }
              ];
          shots = [
            {
              name = "overview";
              path = "/";
              waitFor = signInPage;
              steps = signIn;
            }
          ];
        };
        on-desktop = plan "on-desktop" { shots = pages; };
        on-phone = plan "on-phone" (phone // { shots = pages; });
      }
    else
      # Before: the script page's document, and nothing to wait for.
      lib.genAttrs [ "off-desktop" "off-phone" ] (
        name:
        plan name (
          lib.optionalAttrs (name == "off-phone") phone
          // {
            script = false;
            shots = map (p: removeAttrs p [ "waitFor" ] // { settle = 3000; }) pages;
          }
        )
      );
in

pkgs.testers.nixosTest {
  name = if after then "losos-no-js" else "losos-no-js-before";

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
          ../../modules/config-repo.nix
        ];
        losos = {
          hostName = "mattbox";
          backend.package = lososPkgs.losos-ctl;
          backend.plugins = lososPkgs.losos-plugins;
          admin.ui = ui;
          git.enable = false;
        };
        # The stand-in first, so it shadows the real nixos-rebuild; git and
        # the rest of lososd's path stay for the settings commit.
        systemd.services.lososd.path = lib.mkBefore [ stubRebuild ];
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
    PASSWORD = "${password}"
    AFTER = ${if after then "True" else "False"}
    BASIC_CSP = (
        "default-src 'none'; style-src 'self'; img-src 'self' data:; font-src 'self'; "
        "form-action 'self'; frame-ancestors 'none'; base-uri 'none'"
    )

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

    def capture(plan, signed_in=False, strict=True):
        who = f"--user ${user} --password '{PASSWORD}'" if signed_in else ""
        laptop.succeed(
            f"losos-capture --base https://{BOX} --plan /etc/losos-test/plan-{plan}.json "
            f"--out /tmp/screens/{plan} {who}",
            timeout=2400,
        )
        seen = json.loads(laptop.succeed(f"cat /tmp/screens/{plan}/report.json"))
        for page in seen:
            assert page["pageWidth"] <= page["width"], f"{plan}: a page scrolls sideways: {page}"
            # The SPA's own policy is the admin-ui browser check's; here
            # it is only reported.
            if strict:
                assert page["csp"] == [], f"{plan}: the policy refused something: {page}"
            elif page["csp"]:
                print(f"{plan}: the policy refused something: {page}")
        return seen

    with subtest("claim the box"):
        claim = json.dumps({"name": "${user}", "password": PASSWORD})
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

    if AFTER:
        with subtest("every address sends a browser without JavaScript to its page under /basic/"):
            for path, there in [
                ("/", "/basic/"),
                ("/storage", "/basic/storage"),
                ("/settings/network", "/basic/settings/network"),
                ("/settings/network?tab=1", "/basic/settings/network"),
                # Anything but letters, digits and /_- is the overview.
                ("/x%22%3E%3Cb%3E", "/basic/"),
                ("/a.b", "/basic/"),
            ]:
                page = curl(f"https://{BOX}{path}")
                want = f'<meta http-equiv="refresh" content="0; url={there}" />'
                assert want in page, (path, page[:600])

        with subtest("/basic/ is behind LosOS Login, https only, under its own policy"):
            out = curl(f"-o /dev/null -w '%{{http_code}} %{{redirect_url}}' https://{BOX}/basic/storage")
            assert out == f"303 https://{BOX}/basic/sign-in?next=/basic/storage", out
            out = curl(f"-o /dev/null -w '%{{http_code}} %{{redirect_url}}' http://{BOX}/basic/sign-in")
            assert out == f"301 https://{BOX}/basic/sign-in", out
            head = curl(f"-D - -o /tmp/sign-in.html https://{BOX}/basic/sign-in")
            assert f"content-security-policy: {BASIC_CSP}".lower() in head.lower(), head
            page = laptop.succeed("cat /tmp/sign-in.html")
            assert "<script" not in page and " style=" not in page, page[:600]
            # A wrong spare key in the cookie opens nothing.
            out = curl(
                f"-o /dev/null -w '%{{http_code}} %{{redirect_url}}' -b 'losos_spare=nope' https://{BOX}/basic/"
            )
            assert out == f"303 https://{BOX}/basic/sign-in?next=/basic/", out
            # A form from another site changes nothing.
            out = curl(
                f"-o /dev/null -w '%{{http_code}}' -X POST -H 'Origin: https://elsewhere.example' "
                f"-d 'theme=dark' https://{BOX}/basic/prefs"
            )
            assert out == "403", out

        with subtest("without JavaScript: sign in, every page, the Apply bar, the menu"):
            capture("off-desktop")
            capture("off-phone")

        with subtest("without JavaScript: apply a change, watch the rebuild, sign out"):
            capture("off-apply")
            box.succeed("grep -q 'losos.gpu.enable = false' /etc/nixos/modules/overrides.nix")

        with subtest("the handbook's picture in English, Slovak and German"):
            capture("handbook")

        with subtest("the same pages with JavaScript on"):
            capture("on-desktop", signed_in=True, strict=False)
            capture("on-phone", signed_in=True, strict=False)
    else:
        with subtest("before: what a browser without JavaScript was served"):
            capture("off-desktop")
            capture("off-phone")

    laptop.copy_from_machine("/tmp/screens", "")
  '';
}
