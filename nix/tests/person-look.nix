# nixos-test-vms config for each person's own look: the theme and the
# language in the person menu, and a background picture per person
# (backend/plugins/look/, admin-ui/app/src/components/preferences-menu.tsx,
# backend/plugins/page/src/shell.rs).
#
# A box with the real lososd and its plugins, nginx, LosOS Login and the
# built admin UI, and a laptop on the LAN with a browser. What is asserted:
#
#   * the owner's first look is a copy of the box's background, and from
#     then on each person's pick is theirs: Ada's choice never reaches
#     Matus's pages, nor his hers, and nobody reads another's picture;
#   * a person with no section at all opens their own Look pane, without
#     the widgets, which stay the look section's;
#   * the theme and the language sit in the person menu, with JavaScript on
#     and off, and a choice there changes the page;
#   * the background is behind the pages without JavaScript too.
#
# The laptop's pictures are the ones the pull request shows: the
# derivation's output holds them, one folder per plan. `before`, a path to
# an earlier checkout of this repository, boots that tree's lososd, plugins,
# modules and admin UI instead and only takes the pictures:
#
#   nix build --impure --expr '(import ./nix/tests/person-look.nix {
#     pkgs = (builtins.getFlake (toString ./.)).inputs.nixpkgs.legacyPackages.x86_64-linux;
#     before = /path/to/main; })'
{
  pkgs,
  before ? null,
}:

let
  inherit (pkgs) lib;
  after = before == null;
  root = if after then ../.. else before;
  lososPkgs = import (root + "/nix/packages.nix") { inherit pkgs; };
  screens = import ./screens { inherit pkgs; };

  # The admin UI as it ships, without the Diagrams page, which this test
  # never opens and whose WebAssembly takes longer to build than the test.
  ui = import ./admin-ui-without-lab.nix lososPkgs.losos-admin-ui;

  password = "Correct-Horse-Battery-9";

  sel = selector: { inherit selector; };
  personMenu = [
    { click = sel "[data-testid=signed-in-as]"; }
    { waitFor = sel "[data-slot=person-menu]"; }
  ];
  # A row of the person menu opened to its choices.
  row = slot: [
    { click = sel "[data-slot=${slot}]"; }
    { waitFor = sel "[data-slot=${slot}-choices]"; }
  ];
  # A person's own background, as the pane names it (before: the box's).
  lookPane = {
    role = "radiogroup";
    name = if after then "Your background" else "Background";
  };
  pick = name: toast: [
    {
      click = {
        role = "radio";
        inherit name;
      };
    }
    { waitText = toast; }
    # The picture fades in behind the page.
    { sleep = 4500; }
  ];
  home = name: {
    inherit name;
    path = "/";
    waitFor = sel "[data-testid=signed-in-as]";
    settle = 2000;
  };
  # The pages without JavaScript: the person menu is a disclosure.
  basicMenu = [
    { click = sel "details:has([data-slot=person-menu]) > summary"; }
    {
      waitFor = {
        role = "menuitem";
        name = "Sign out";
      };
    }
  ];
  basicHome = {
    role = "heading";
    name = "mattbox";
  };

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
    # Matus, the owner: the menu, his pick, his home under it.
    matus = plan "matus" {
      shots = [
        (home "home")
        (home "person-menu" // { steps = personMenu; })
      ]
      ++ lib.optionals after [
        (home "theme-menu" // { steps = personMenu ++ row "theme-menu"; })
        (home "language-menu" // { steps = personMenu ++ row "language-menu"; })
      ]
      ++ [
        {
          name = "look";
          path = "/settings/look";
          waitFor = lookPane;
          steps = pick "Tide" "Background set.";
        }
        (home "home-tide")
      ];
    };
    # Ada, in no section: her home stays plain under his pick, and her
    # own Look pane (before: none to open).
    ada = plan "ada" {
      shots = [
        (home "ada-home")
        (
          {
            name = "ada-look";
            path = "/settings/look";
          }
          // (
            if after then
              {
                waitFor = lookPane;
                steps = pick "Dusk" "Background set.";
              }
            else
              { settle = 4000; }
          )
        )
      ]
      ++ lib.optionals after [ (home "ada-home-dusk") ];
    };
    # Without JavaScript, as Matus.
    basic = plan "basic" {
      script = false;
      shots = [
        {
          name = "basic-menu";
          path = "/";
          waitFor = basicHome;
          steps = basicMenu;
        }
      ]
      ++ lib.optionals after [
        {
          name = "basic-theme";
          path = "/";
          waitFor = basicHome;
          steps = basicMenu ++ [
            { click = sel "[data-slot=theme-menu] > summary"; }
            { waitFor = sel "[data-slot=theme-choices]"; }
          ];
        }
        {
          name = "basic-look";
          path = "/settings/look";
          waitFor = {
            role = "heading";
            name = "Look";
          };
        }
      ];
    };
  };
in

pkgs.testers.nixosTest {
  name = if after then "losos-person-look" else "losos-person-look-before";

  nodes = {
    box =
      { lib, ... }:
      {
        imports = map (m: root + "/modules/${m}.nix") [
          "options"
          "containers"
          "tls"
          "daemon"
          "identity"
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
    import re

    BOX = "192.168.1.1"
    PASSWORD = "${password}"
    AFTER = ${if after then "True" else "False"}
    PNG = (
        "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg=="
    )

    box.start()
    box.wait_for_unit("lososd.service")
    # Loading the plugins takes lososd a minute under emulation.
    box.wait_for_open_port(8082)
    box.wait_for_unit("nginx.service")
    box.wait_for_open_port(443)
    laptop.start()
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

    def sign_in(name):
        body = json.dumps({"username": name, "password": PASSWORD})
        head = curl(
            f"-D - -o /dev/null https://{BOX}/auth/api/firstfactor "
            f"-H 'Content-Type: application/json' -d '{body}'"
        )
        m = re.search(r"(?im)^set-cookie: (losos_session=[^;]+)", head)
        assert m, f"no session cookie for {name}: {head}"
        return m.group(1)

    def capture(plan, user):
        laptop.succeed(
            f"losos-capture --base https://{BOX} --plan /etc/losos-test/plan-{plan}.json "
            f"--out /tmp/screens/{plan} --user {user} --password '{PASSWORD}'",
            timeout=2400,
        )
        seen = json.loads(laptop.succeed(f"cat /tmp/screens/{plan}/report.json"))
        for page in seen:
            assert page["pageWidth"] <= page["width"], f"{plan}: a page scrolls sideways: {page}"
            assert page["csp"] == [], f"{plan}: the policy refused something: {page}"

    with subtest("claim the box, and add Ada, who is in no section"):
        code, body = api(
            "/api/setup/claim",
            method="POST",
            data={"name": "matus", "password": PASSWORD},
            extra=f"-H 'Host: {BOX}'",
        )
        assert code == 200, (code, body)
        bearer = f"-H 'Authorization: Bearer {json.loads(body)['token']}'"
        box.wait_for_unit("authelia-losos.service")
        box.systemctl("start losos-identity-setup.service")
        box.wait_until_succeeds(f"grep -q \"domain: '{BOX}'\" /run/losos-identity/session.yml")
        box.wait_until_succeeds("systemctl is-active authelia-losos.service")
        box.wait_for_open_port(9091)
        code, body = api(
            "/api/people",
            method="POST",
            data={"name": "ada", "displayName": "Ada", "password": PASSWORD, "groups": []},
            extra=bearer,
        )
        assert code == 200, (code, body)
        matus = sign_in("matus")
        # LosOS Login reads the people file again on its own.
        laptop.wait_until_succeeds(
            f"curl -sk -o /dev/null -w '%{{http_code}}' https://{BOX}/auth/api/firstfactor "
            f"-H 'Content-Type: application/json' "
            f"-d '{json.dumps({'username': 'ada', 'password': PASSWORD})}' | grep -qx 200",
            timeout=120,
        )
        ada = sign_in("ada")

    if AFTER:
        with subtest("the owner inherits the box's background once, then each look is a person's"):
            laptop.succeed(f"echo {PNG} | base64 -d > /tmp/bg.png")
            code, body = api(
                "/api/look/background",
                method="PUT",
                extra=f"{bearer} -H 'Content-Type: image/png' --data-binary @/tmp/bg.png",
            )
            assert code == 200 and json.loads(body)["personal"] is False, (code, body)
            code, body = api("/api/look", matus)
            look = json.loads(body)
            assert look["background"]["url"] == "/api/look/background/matus?v=1", look
            code, body = api("/api/look", ada)
            assert json.loads(body)["background"] == {"kind": "none"}, body
            # Nobody reads another's picture, or the box's without the key.
            code, _ = api("/api/look/background/matus?v=1", matus, extra="-o /tmp/got.png")
            assert code == 200, code
            laptop.succeed("cmp /tmp/bg.png /tmp/got.png")
            code, _ = api("/api/look/background/matus?v=1", ada)
            assert code == 404, code
            # The widgets stay the look section's.
            code, _ = api("/api/look/widgets", ada, "POST", {"name": "x", "source": "<b>x</b>"})
            assert code == 403, code
            # Plain again for the pictures, Matus's and the box's.
            for who in [matus, None]:
                code, body = api("/api/look/background", who, "DELETE", extra="" if who else bearer)
                assert code == 200, (code, body)
            box.fail("test -e /var/lib/losos/background.img -o -e /var/lib/losos/looks/matus.img")

    with subtest("the pictures: the person menu, a pick each, and the pages without JavaScript"):
        capture("matus", "matus")
        capture("ada", "ada")
        capture("basic", "matus")

    if AFTER:
        with subtest("each pick stayed its person's"):
            code, body = api("/api/look", matus)
            assert json.loads(body)["background"] == {"kind": "shipped", "name": "tide"}, body
            code, body = api("/api/look", ada)
            assert json.loads(body)["background"] == {"kind": "shipped", "name": "dusk"}, body
            on_disk = json.loads(box.succeed("cat /var/lib/losos/looks/ada.json"))
            assert on_disk["background"] == {"kind": "shipped", "name": "dusk"}, on_disk
            box.succeed("test $(stat -c %a /var/lib/losos/looks) = 700")
            # The page without JavaScript draws Matus's picture behind it.
            page = curl(f"-H 'Cookie: {matus}' https://{BOX}/basic/")
            m = re.search(r'href="(/basic/look\.css\?[^"]+)"', page)
            assert m, page[:2000]
            href = m.group(1).replace("&amp;", "&")
            sheet = curl(f"-H 'Cookie: {matus}' 'https://{BOX}{href}'")
            assert "/backgrounds/tide.svg" in sheet, sheet

    laptop.copy_from_machine("/tmp/screens", "")
  '';
}
