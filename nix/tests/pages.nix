# nixos-test-vms config for the pages as MDX (admin-ui/app/src/pages/,
# backend/plugins/pages/).
#
# A box with the real lososd and its plugins, nginx, LosOS Login and the
# built admin UI, and a laptop on the LAN with a browser. What is asserted:
#
#   * a page nobody changed is not in GET /api/pages, and the Pages
#     section of the Look pane is an administrator's alone;
#   * an administrator's save is checked by losos-mdx on the box: a block
#     the page does not have is refused with its line, a good page is a
#     commit in /var/lib/losos/pages by that person, and so are the
#     stylesheet and a reset;
#   * someone who is not an administrator reads the changed pages and
#     cannot write one, nor read a page's source;
#   * the changed Overview and Storage are drawn with JavaScript on and
#     off, the stylesheet with them, and no page trips the policy;
#   * the editor docks beside the page and draws what is typed into it.
#
# The laptop's pictures are the ones the pull request shows: the
# derivation's output holds them, one folder per plan. `before`, a path to
# an earlier admin-ui/app, builds that UI instead and takes the same
# screens as they were:
#
#   nix build --impure --expr '(import ./nix/tests/pages.nix {
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

  ui = import ./admin-ui-without-lab.nix (
    lososPkgs.losos-admin-ui.overrideAttrs (_: lib.optionalAttrs (!after) { src = before; })
  );

  user = "matus";
  password = "Correct-Horse-Battery-9";

  heading = name: {
    role = "heading";
    inherit name;
  };
  overview = heading "mattbox";

  # The pages an administrator writes in this test.
  overviewPage = ''
    <BoxName />

    ## Welcome to the office box

    Files and code live here. Ask Matus before you install an app.

    <Button href="/apps" variant="secondary">Open the apps</Button>

    <Apps hideStorage />

    <Glance />
  '';
  storagePage = ''
    The reserve is for the photo archive. Ask Matus before you use it.

    <Disk />
  '';
  style = ''
    html[data-page="overview"] .mdx-page-text h2 { color: var(--accent); }
  '';

  # The sign-in form with JavaScript off, as a person fills it.
  signInPage = {
    selector = "#username";
  };
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

  pagesSection = {
    selector = "[data-testid=pages-section]";
  };

  # The three screens the change touches, as they are before anything is
  # changed: the Overview, Storage and the Look pane.
  screensNow = [
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
      name = "look";
      path = "/settings/look";
      waitFor = {
        role = "radiogroup";
        name = "Background";
      };
      steps = [ { scroll = "end"; } ];
    }
  ];

  plan =
    name: attrs:
    pkgs.writeText "plan-${name}.json" (
      builtins.toJSON (
        {
          width = 1280;
          height = 900;
          colorScheme = "light";
        }
        // attrs
      )
    );

  plans = {
    # Nothing changed yet, script on and off.
    default = plan "default" { shots = screensNow; };
    default-off = plan "default-off" {
      script = false;
      shots = [
        {
          name = "overview";
          path = "/";
          waitFor = signInPage;
          steps = signIn;
        }
      ];
    };
  }
  // lib.optionalAttrs after {
    # The Pages section, and the editor beside the Overview with a line
    # typed into it, not saved.
    editor = plan "editor" {
      shots = [
        {
          name = "pages-section";
          path = "/settings/look";
          waitFor = pagesSection;
          steps = [ { scroll = pagesSection; } ];
        }
        {
          name = "editor";
          path = "/settings/look";
          waitFor = pagesSection;
          steps = [
            {
              click = {
                role = "button";
                name = "Edit Overview";
              };
            }
            # The dock and its editor are lazy chunks, slow to arrive on an
            # emulated laptop.
            {
              waitFor = {
                role = "textbox";
                name = "Source of the Overview page";
              };
              timeout = 240000;
            }
            {
              click = {
                role = "textbox";
                name = "Source of the Overview page";
              };
            }
            { press = "Control+End"; }
            { type = "\n## A line typed just now\n"; }
            {
              waitFor = heading "A line typed just now";
              timeout = 240000;
            }
          ];
        }
      ];
    };
    # The pages the administrator saved, script on and off.
    changed = plan "changed" {
      shots = [
        {
          name = "overview";
          path = "/";
          waitFor = heading "Welcome to the office box";
        }
        {
          name = "storage";
          path = "/storage";
          waitFor = {
            text = "The reserve is for the photo archive. Ask Matus before you use it.";
          };
        }
      ];
    };
    changed-off = plan "changed-off" {
      script = false;
      shots = [
        {
          name = "overview";
          path = "/";
          waitFor = signInPage;
          steps = signIn ++ [ { waitFor = heading "Welcome to the office box"; } ];
        }
        {
          name = "storage";
          path = "/storage";
          waitFor = {
            text = "The reserve is for the photo archive. Ask Matus before you use it.";
          };
        }
      ];
    };
  };
in

pkgs.testers.nixosTest {
  name = if after then "losos-pages" else "losos-pages-before";

  nodes = {
    box =
      { ... }:
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
        environment.systemPackages = [ pkgs.git ];
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
        environment.etc =
          lib.mapAttrs' (
            name: file: lib.nameValuePair "losos-test/plan-${name}.json" { source = file; }
          ) plans
          // {
            "losos-test/overview.mdx".text = overviewPage;
            "losos-test/storage.mdx".text = storagePage;
            "losos-test/style.css".text = style;
          };
        virtualisation.vlans = [ 1 ];
      };
  };

  testScript = ''
    import json
    import re

    BOX = "192.168.1.1"
    PASSWORD = "${password}"
    AFTER = ${if after then "True" else "False"}
    REPO = "/var/lib/losos/pages"

    start_all()
    box.wait_for_unit("lososd.service")
    box.wait_for_open_port(8082)
    box.wait_for_unit("nginx.service")
    box.wait_for_open_port(443)
    laptop.wait_for_unit("multi-user.target")

    def curl(args):
        return laptop.succeed(f"curl -sk {args}")

    def api(path, cookie=None, method="GET", data=None, extra=""):
        args = f"-X {method} https://{BOX}{path} {extra}"
        if cookie:
            args += f" -H 'Cookie: {cookie}'"
        if data is not None:
            laptop.succeed(f"cat > /tmp/body.json <<'EOF'\n{json.dumps(data)}\nEOF")
            args += " -H 'Content-Type: application/json' --data-binary @/tmp/body.json"
        out = curl(f"-w '\\n%{{http_code}}' {args}")
        body, code = out.rsplit("\n", 1)
        return int(code), body

    def sign_in(name, password):
        body = json.dumps({"username": name, "password": password})
        head = curl(
            f"-D - -o /dev/null https://{BOX}/auth/api/firstfactor "
            f"-H 'Content-Type: application/json' -d '{body}'"
        )
        m = re.search(r"(?im)^set-cookie: (losos_session=[^;]+)", head)
        assert m, f"no session cookie for {name}: {head}"
        return m.group(1)

    def capture(plan, signed_in=True):
        who = f"--user ${user} --password '{PASSWORD}'" if signed_in else ""
        laptop.succeed(
            f"losos-capture --base https://{BOX} --plan /etc/losos-test/plan-{plan}.json "
            f"--out /tmp/screens/{plan} {who}",
            timeout=2400,
        )
        seen = json.loads(laptop.succeed(f"cat /tmp/screens/{plan}/report.json"))
        for page in seen:
            assert page["pageWidth"] <= page["width"], f"{plan}: a page scrolls sideways: {page}"
            assert page["csp"] == [], f"{plan}: the policy refused something: {page}"
        return seen

    def commits_made():
        return box.succeed(f"git -C {REPO} log --format='%an|%ae|%s'").strip().splitlines()

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

    with subtest("nothing changed: the screens as they ship, script on and off"):
        capture("default")
        capture("default-off", signed_in=False)

    if AFTER:
        cookie = sign_in("${user}", PASSWORD)

        with subtest("a box nobody customized has no changed pages and no stylesheet"):
            code, body = api("/api/pages", cookie)
            assert code == 200, (code, body)
            assert json.loads(body) == {"pages": {}, "style": None}, body
            code, _ = api("/api/pages/style.css", cookie)
            assert code == 404, code

        with subtest("the editor docks beside the page and draws what is typed"):
            capture("editor")
            # The preview wrote nothing.
            assert json.loads(api("/api/pages", cookie)[1])["pages"] == {}

        with subtest("the box reads a save as the editor did, and refuses one that does not read"):
            code, body = api("/api/pages/overview", cookie, "PUT", {"source": "# Hi\n\n<Nope />\n"})
            assert code == 400, (code, body)
            assert "line 3, column 1" in body and "<Nope> is not a block" in body, body
            code, body = api("/api/pages/nowhere", cookie, "PUT", {"source": "# Hi\n"})
            assert code == 404, (code, body)

        with subtest("each save is a commit by the administrator in the pages' own repository"):
            overview = laptop.succeed("cat /etc/losos-test/overview.mdx")
            storage = laptop.succeed("cat /etc/losos-test/storage.mdx")
            style = laptop.succeed("cat /etc/losos-test/style.css")
            code, body = api("/api/pages/overview", cookie, "PUT", {"source": overview})
            assert code == 200, (code, body)
            listed = json.loads(body)
            assert "Welcome to the office box" in listed["pages"]["overview"]["html"], listed
            assert listed["pages"]["overview"]["by"] == "${user}", listed
            assert api("/api/pages/settings.storage", cookie, "PUT", {"source": storage})[0] == 200
            assert api("/api/pages/style.css", cookie, "PUT", {"source": style})[0] == 200
            commits = commits_made()
            assert commits[:3] == [
                "${user}|${user}@losos.invalid|Add a stylesheet for the pages",
                "${user}|${user}@losos.invalid|Change the Storage page",
                "${user}|${user}@losos.invalid|Change the Overview page",
            ], commits
            box.succeed(f"test -z \"$(git -C {REPO} status --porcelain)\"")
            assert box.succeed(f"cat {REPO}/overview.mdx") == overview
            box.succeed(f"test -f {REPO}/defaults/overview.mdx")
            assert box.succeed(f"stat -c %a {REPO}").strip() == "700"
            code, body = api("/api/pages/overview", cookie)
            source = json.loads(body)
            assert source["custom"] is True and source["defaultChanged"] is False, source
            assert source["history"][0]["message"] == "Change the Overview page", source

        with subtest("someone who is not an administrator reads the pages and cannot change one"):
            code, body = api(
                "/api/people", cookie, "POST",
                {
                    "name": "eva",
                    "displayName": "Eva",
                    "email": "eva@mattbox.local",
                    "password": "Eva-Long-Password-3",
                    "groups": ["losos-look"],
                },
            )
            assert code == 200, (code, body)
            # LosOS Login reads the people file again a moment later.
            laptop.wait_until_succeeds(
                f"curl -sk -o /dev/null -w '%{{http_code}}' https://{BOX}/auth/api/firstfactor "
                "-H 'Content-Type: application/json' "
                "-d '{\"username\": \"eva\", \"password\": \"Eva-Long-Password-3\"}' | grep -q 200",
                timeout=120,
            )
            eva = sign_in("eva", "Eva-Long-Password-3")
            code, body = api("/api/pages", eva)
            assert code == 200 and "overview" in json.loads(body)["pages"], (code, body)
            assert api("/api/pages/style.css", eva)[0] == 200
            for path, method in [
                ("/api/pages/overview", "GET"),
                ("/api/pages/overview", "PUT"),
                ("/api/pages/overview", "DELETE"),
                ("/api/pages/style.css", "PUT"),
                ("/api/pages/style.css", "DELETE"),
            ]:
                data = {"source": "# Mine\n"} if method == "PUT" else None
                code, body = api(path, eva, method, data)
                assert code == 403, (path, method, code, body)
            assert len(commits_made()) == len(commits), commits_made()

        with subtest("the changed pages and the stylesheet, script on and off"):
            capture("changed")
            capture("changed-off", signed_in=False)
            page = curl(f"-H 'Cookie: {cookie}' https://{BOX}/basic/")
            assert "Welcome to the office box" in page, page[:800]
            assert "/api/pages/style.css?v=" in page, page[:800]
            assert "losos-block" not in page, page[:800]
            # Named as the script page names it, so the stylesheet's page
            # selectors hold with JavaScript off too.
            assert 'data-page="overview"' in page, page[:800]

        with subtest("a reset is a commit too, and the page is LosOS's again"):
            code, body = api("/api/pages/settings.storage", cookie, "DELETE")
            assert code == 200 and "settings.storage" not in json.loads(body)["pages"], (code, body)
            assert commits_made()[0] == "${user}|${user}@losos.invalid|Put the Storage page back to LosOS's default", commits_made()
            box.fail(f"test -e {REPO}/settings.storage.mdx")

    laptop.copy_from_machine("/tmp/screens", "")
  '';
}
