# nixos-test-vms config for LosOS Wiki on a real box: spaces kept as LosOS
# Git wikis (backend/src/forge/wiki.rs, modules/git.nix), the directory and
# editor in the admin UI at /wikis (admin-ui/app/src/wikis/), a space's
# published site at /wiki/<owner>/<space>/ (src/wiki/site.tsx, the `/wiki/`
# location of modules/containers.nix), and the knowledge bases LosOS Cloud
# lists for the directory (backend/src/cloud/notes/kb.rs), behind the real
# front vhost and LosOS Login, with lososd claiming the box.
#
# The box shares and is in a mesh, so LosOS Cloud publishes knowledge bases
# (nix/cluster.nix is not imported: `cluster.enable` is the gate's half,
# with no edge to join, as in nix/tests/notes.nix). The laptop is the
# owner's computer on the LAN, and Eva has an account in LosOS Git:
#
#   * the owner clones garden.wiki.git, writes pages in folders with a
#     sidebar, a footer and a picture, and pushes them; Eva writes the wiki
#     of her public repository through the API; the owner's plans stay
#     private and have no wiki;
#   * a Forgejo box's wiki left on disk beside its repository, on `master`
#     with HEAD at a branch it lacks, is its repository's wiki after a
#     restart, never a repository of its own;
#   * who reads and who writes: Eva reads the owner's space for the box and
#     cannot write it; a visitor reads Eva's public one alone, through nginx;
#     a save over a page that changed meanwhile is refused; a picture is
#     served as a picture, and nothing else from the wiki is;
#   * the owner publishes a folder of notes as a knowledge base;
#   * Chromium signs in through LosOS Login and pictures /wikis, a page, the
#     editor, the search, a new space, the published site and LosOS Git's
#     repository menu (nix/tests/screens/), light, dark and on a phone, then
#     saves an edit and makes a space in the browser, and the test reads
#     both back from Git.
#
# LOSOS_WIKI_OLD_UI, a losos-admin-ui from before LosOS Wiki, gives the
# pictures their "before" column: its pages over the same box, with
# /wiki/ falling through to the admin page as it did.
{ pkgs }:

let
  inherit (pkgs) lib;
  lososPkgs = import ../packages.nix { inherit pkgs; };
  noLab = import ./admin-ui-without-lab.nix;
  screens = import ./screens { inherit pkgs; };

  # The garden's beds, a picture the wiki keeps: drawn here, not committed.
  beds = pkgs.runCommand "beds.png" { nativeBuildInputs = [ pkgs.imagemagick ]; } ''
    magick -size 720x240 -seed 7 plasma:'#2f6b2f'-'#cfe6a8' -blur 0x1.5 $out
  '';

  pages = {
    "Home.md" = ''
      # Garden

      How we look after the beds behind the house. Start with [[Planting]], and read [the watering guide](Guides/Watering) before July.

      ![The beds in June](beds.png)

      ## This season

      | Bed | Crop | Sown |
      | --- | --- | --- |
      | North | Tomatoes | 12 May |
      | South | Beans | 20 May |
      | Herbs | Basil and parsley | 2 June |

      ## Who does what

      - **Matus** waters on weekdays.
      - **Eva** takes the weekends and the compost.
    '';
    "Planting.md" = ''
      # Planting

      Tomatoes go in after the last frost, in the middle of May. Beans follow a week later, straight into the soil.

      Keep a metre between the rows so the hose reaches every plant. See [[Guides/Compost|compost]] for what goes under them.
    '';
    "Guides.md" = ''
      # Guides

      What we learned, one page per job. [[Guides/Watering]] and [[Guides/Compost]] so far.
    '';
    "Guides/Watering.md" = ''
      # Watering

      Water every evening in July and August, and every second evening before that.

      1. Open the tap by the shed halfway.
      2. Give each tomato a full can at its roots, not on the leaves.
      3. Close the tap and coil the hose.

      Rain over five millimetres counts as an evening. Back to [[../Planting|planting]].
    '';
    "Guides/Compost.md" = ''
      # Compost

      Green and brown in turns: kitchen scraps, then dry leaves. Turn it every two weeks.
    '';
    "_Sidebar.md" = ''
      **Garden**

      - [[Home]]
      - [[Planting]]
      - [[Guides/Watering|Watering]]
    '';
    "_Footer.md" = ''
      Every change is a commit in the space's history in LosOS Git.
    '';
  };
  wiki = pkgs.runCommand "garden-wiki" { } (
    lib.concatStrings (
      lib.mapAttrsToList (path: text: ''
        mkdir -p "$out/$(dirname '${path}')"
        cp ${pkgs.writeText "page.md" text} "$out/${path}"
      '') pages
    )
    + ''
      cp ${beds} $out/beds.png
    ''
  );

  plan =
    name: attrs:
    pkgs.writeText "plan-${name}.json" (
      builtins.toJSON (
        {
          width = 1280;
          height = 860;
          variants = [
            {
              suffix = "light";
              colorScheme = "light";
            }
            {
              suffix = "dark";
              colorScheme = "dark";
            }
            {
              suffix = "phone";
              colorScheme = "light";
              width = 390;
              height = 844;
            }
          ];
        }
        // attrs
      )
    );

  garden = {
    selector = "[data-wiki-page=\"Home\"] img[alt=\"The beds in June\"]";
  };
  plans = {
    after = plan "after" {
      shots = [
        {
          name = "space";
          path = "/wikis/notshared/garden/";
          waitFor = garden;
        }
        {
          name = "folder-page";
          path = "/wikis/notshared/garden/Guides/Watering";
          waitFor.selector = "[data-wiki-page=\"Guides/Watering\"] [data-wiki-sidebar]";
        }
        {
          name = "editor";
          path = "/wikis/notshared/garden/Planting";
          waitFor.selector = "[data-wiki-edit]";
          steps = [
            { click.selector = "[data-wiki-edit]"; }
            { waitFor.selector = "[data-wiki-editor] textarea"; }
          ];
        }
        {
          name = "search";
          path = "/wikis";
          waitFor.selector = "[data-wiki-entry=\"space:notshared/garden\"]";
          steps = [
            {
              fill = {
                label = "Search the pages of every wiki on this box";
                value = "water";
              };
            }
            { press = "Enter"; }
            { waitFor.selector = "[data-wiki-results] [data-hit=\"garden/Guides/Watering\"]"; }
            { waitFor.selector = "[data-wiki-results] [data-hit^=\"birds/\"]"; }
          ];
        }
        {
          name = "new-space";
          path = "/wikis";
          waitFor.selector = "[data-wiki-entry=\"space:notshared/garden\"]";
          steps = [
            {
              click = {
                role = "button";
                name = "New space";
              };
            }
            {
              fill = {
                label = "Name";
                value = "Team handbook";
              };
            }
            { waitText = "Repository: Team-handbook"; }
          ];
        }
        {
          name = "site";
          path = "/wiki/notshared/garden/";
          waitFor = garden;
        }
        {
          name = "site-directory";
          path = "/wiki/";
          waitFor.selector = "[data-wiki-directory] [data-wiki-space=\"eva/recipes\"]";
        }
        {
          name = "git-repository";
          path = "/git/notshared/garden";
          waitFor.selector = "[data-repo-wiki]";
        }
      ];
    };
    # The same addresses on the build before LosOS Wiki: no pages there yet.
    before = plan "before" {
      shots =
        map
          (s: {
            inherit (s) name path;
            waitFor.selector = "body";
            steps = [ { sleep = 4000; } ];
          })
          [
            {
              name = "space";
              path = "/wikis/notshared/garden/";
            }
            {
              name = "site";
              path = "/wiki/notshared/garden/";
            }
            {
              name = "git-repository";
              path = "/git/notshared/garden";
            }
          ];
    };
    # Done once, in the browser: an edit saved, a space made.
    edit = pkgs.writeText "plan-edit.json" (
      builtins.toJSON {
        width = 1280;
        height = 860;
        shots = [
          {
            name = "edit-saved";
            path = "/wikis/notshared/garden/Planting";
            waitFor.selector = "[data-wiki-edit]";
            steps = [
              { click.selector = "[data-wiki-edit]"; }
              {
                fill = {
                  label = "Text in Markdown";
                  value = "# Planting\n\nTomatoes go in after the last frost. This year the frost ended on 9 May.\n";
                };
              }
              {
                fill = {
                  label = "What changed";
                  value = "Note this year's last frost";
                };
              }
              {
                click = {
                  role = "button";
                  name = "Save";
                };
              }
              { gone.selector = "[data-wiki-editor]"; }
              { waitFor.selector = "[data-wiki-page=\"Planting\"]"; }
            ];
          }
          {
            name = "space-made";
            path = "/wikis";
            waitFor.selector = "[data-wiki-entry=\"space:notshared/garden\"]";
            steps = [
              {
                click = {
                  role = "button";
                  name = "New space";
                };
              }
              {
                fill = {
                  label = "Name";
                  value = "Team handbook";
                };
              }
              {
                click = {
                  role = "button";
                  name = "Make the space";
                };
              }
              { waitFor.selector = "[data-wiki-page=\"Home\"]"; }
            ];
          }
        ];
      }
    );
  };
in

pkgs.testers.nixosTest {
  name = "losos-wiki";

  nodes = {
    box = _: {
      imports = [
        ../../modules/options.nix
        ../../modules/mdns.nix
        ../../modules/containers.nix
        ../../modules/tls.nix
        ../../modules/daemon.nix
        ../../modules/identity.nix
        ../../modules/cloud.nix
        ../../modules/git.nix
      ];
      networking.hostName = "mattbox";
      losos = {
        hostName = "mattbox";
        backend.package = lososPkgs.losos-ctl;
        admin.ui = noLab lososPkgs.losos-admin-ui;
        cloud.enable = true;
        git.enable = true;
        mdns.enable = true;
        # Sharing and in a mesh: knowledge bases on.
        sharingMyStorage = true;
        cluster.enable = true;
      };
      # The box before LosOS Wiki: /wiki/ was the admin page's, like any
      # other address it did not know.
      specialisation.before.configuration.services.nginx.virtualHosts."losos-front".locations."/wiki/" =
        lib.mkForce
          { tryFiles = "$uri /index.html"; };
      environment.systemPackages = [
        pkgs.jq
        pkgs.curl
        pkgs.gitMinimal
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
      services.avahi = {
        enable = true;
        nssmdns4 = true;
      };
      environment.systemPackages = [
        pkgs.curl
        pkgs.jq
        pkgs.gitMinimal
      ];
      environment.etc = lib.mapAttrs' (
        name: file: lib.nameValuePair "losos-test/plan-${name}.json" { source = file; }
      ) plans;
      # A refused push fails instead of asking for a password.
      environment.variables.GIT_TERMINAL_PROMPT = "0";
      virtualisation = {
        # Chromium and three picture variants under TCG.
        memorySize = lib.mkForce 3072;
        diskSize = 3072;
        vlans = [ 1 ];
      };
    };
  };

  testScript = ''
    import json
    import os
    import re

    HOST = "mattbox.local"
    BASE = f"https://{HOST}"
    CA = "/etc/losos-box.pem"
    PASSWORD = "Correct-Horse-Battery-9"

    # One machine at a time: two TCG guests booting together can miss the
    # driver's wait for a shell.
    box.succeed("true")
    laptop.succeed("true")
    box.wait_for_unit("nginx.service")
    box.wait_for_unit("lososd.service")
    box.wait_for_unit("losos-git.service")
    box.wait_for_unit("losos-cloud.service")
    box.wait_for_open_port(3000)

    cert = box.succeed("cat /var/lib/losos-tls/cert.pem")
    laptop.succeed(f"cat > {CA} <<'EOF'\n{cert}EOF")
    laptop.succeed(f"git config --system http.sslCAInfo {CA}")
    git_admin = box.succeed("cat /var/lib/losos-git/admin-token").strip()

    def curl(args):
        return laptop.succeed(f"curl -s --cacert {CA} {args}")

    def status(args):
        return int(curl(f"-o /dev/null -w '%{{http_code}}' {args}"))

    def forge(method, path, body=None, token=None):
        """LosOS Git's API on its loopback port: (status, JSON or text)."""
        args = f"-s -o /tmp/forge -w '%{{http_code}}' -X {method}"
        args += f" -H 'Authorization: token {token or git_admin}'"
        if body is not None:
            box.succeed(f"cat > /tmp/body.json <<'EOF'\n{json.dumps(body)}\nEOF")
            args += " -H 'Content-Type: application/json' --data-binary @/tmp/body.json"
        code = int(box.succeed(f"curl {args} http://127.0.0.1:3000/api/v1{path}"))
        out = box.succeed("cat /tmp/forge")
        try:
            return code, json.loads(out)
        except ValueError:
            return code, out

    def ok(method, path, body=None, token=None, want=(200, 201, 204)):
        code, doc = forge(method, path, body, token)
        assert code in want, (method, path, code, doc)
        return doc

    def token_for(login):
        doc = ok("POST", f"/admin/users/{login}/tokens",
                 {"name": "laptop", "scopes": ["write:repository", "read:user"], "replace": True})
        return doc["token"]

    with subtest("the owner claims the box"):
        laptop.wait_until_succeeds(f"getent hosts {HOST}", timeout=120)
        laptop.wait_until_succeeds(
            f"curl -sf --cacert {CA} {BASE}/api/setup/claim | jq -e .ready", timeout=600
        )
        out = curl(
            f"-w '\\n%{{http_code}}' -X POST {BASE}/api/setup/claim"
            " -H 'Content-Type: application/json'"
            f" -d '{json.dumps({"name": "matus", "password": PASSWORD})}'"
        )
        assert out.rsplit("\n", 1)[1] == "200", out
        box.wait_for_unit("authelia-losos.service")
        box.wait_for_open_port(9091)

    with subtest("two people and three repositories"):
        ok("PUT", "/admin/users/notshared", {"full_name": "Matus", "email": "matus@mattbox.local"})
        ok("PUT", "/admin/users/eva", {"full_name": "Eva", "email": "eva@mattbox.local"})
        owner = token_for("notshared")
        eva = token_for("eva")
        ok("POST", "/user/repos", {"name": "garden", "description": "How we look after the garden", "visibility": "internal"}, token=owner)
        ok("POST", "/user/repos", {"name": "plans", "description": "", "visibility": "private"}, token=owner)
        ok("POST", "/user/repos", {"name": "recipes", "description": "Soups and bread", "visibility": "public"}, token=eva)

    with subtest("the owner pushes the garden's wiki with git"):
        url = f"https://x:{owner}@{HOST}/git/notshared/garden.wiki.git"
        # No wiki yet: nothing to clone, and the first push makes it.
        laptop.fail(f"git clone -q {url} /tmp/nothing")
        laptop.succeed(
            "mkdir -p /tmp/garden && cd /tmp/garden && git init -q -b main",
            "cp -r ${wiki}/. /tmp/garden/ && chmod -R u+w /tmp/garden",
            "cd /tmp/garden && git add -A"
            " && git -c user.name=Matus -c user.email=matus@mattbox.local commit -qm 'Start the garden'",
            f"cd /tmp/garden && git push -q {url} main",
        )
        w = ok("GET", "/repos/notshared/garden/wiki", token=owner)
        assert w["exists"] and w["writable"] and w["home"] == "Home", w
        names = [p["name"] for p in w["pages"]]
        assert names == ["Guides", "Guides/Compost", "Guides/Watering", "Home", "Planting"], names
        assert w["clone_url"].endswith("/git/notshared/garden.wiki.git"), w
        page = ok("GET", "/repos/notshared/garden/wiki/pages/Guides/Watering", token=owner)
        assert page["title"] == "Watering" and page["sidebar_page"] == "_Sidebar", page
        assert "Every change is a commit" in page["footer"], page
        assert page["updated"]["author"] == "Matus", page
        # The wiki is no repository of its own.
        names = [r["name"] for r in ok("GET", "/user/repos", token=owner)]
        assert sorted(names) == ["garden", "plans"], names

    with subtest("Eva writes her public wiki in the browser's way, page by page"):
        home = ok("PUT", "/repos/eva/recipes/wiki/pages/Home",
                  {"text": "# Recipes\n\n[[Bean soup]] and rye bread.\n", "sha": None}, token=eva)
        ok("PUT", "/repos/eva/recipes/wiki/pages/Bean-soup",
           {"text": "# Bean soup\n\nSoak the beans overnight, then simmer them with thyme for an hour.\n", "sha": None, "message": "Write down the soup"},
           token=eva)
        soup = ok("GET", "/repos/eva/recipes/wiki/pages/Bean-soup", token=eva)
        assert soup["history"][0]["subject"] == "Write down the soup", soup
        # A save over a page that changed meanwhile is refused.
        code, _ = forge("PUT", "/repos/eva/recipes/wiki/pages/Home",
                        {"text": "mine", "sha": "0" * 40}, token=eva)
        assert code == 409, code
        code, _ = forge("PUT", "/repos/eva/recipes/wiki/pages/Bean-soup", {"text": "new", "sha": None}, token=eva)
        assert code == 409, code
        assert ok("GET", "/repos/eva/recipes/wiki/pages/Home", token=eva)["sha"] == home["sha"]

    with subtest("who reads and who writes"):
        mine = {w["full_name"]: w for w in ok("GET", "/wikis", token=owner)["wikis"]}
        assert sorted(mine) == ["eva/recipes", "notshared/garden"], mine
        assert mine["notshared/garden"]["writable"] and not mine["eva/recipes"]["writable"], mine
        hers = {w["full_name"]: w for w in ok("GET", "/wikis", token=eva)["wikis"]}
        assert sorted(hers) == ["eva/recipes", "notshared/garden"], hers
        assert not hers["notshared/garden"]["writable"], hers
        code, _ = forge("PUT", "/repos/notshared/garden/wiki/pages/Planting", {"text": "x", "sha": None}, token=eva)
        assert code in (403, 404), code
        laptop.fail(f"git ls-remote https://x:{eva}@{HOST}/git/notshared/plans.wiki.git")
        laptop.succeed(f"git clone -q https://x:{eva}@{HOST}/git/notshared/garden.wiki.git /tmp/eva-garden")
        laptop.succeed("echo more >> /tmp/eva-garden/Home.md && cd /tmp/eva-garden"
                       " && git -c user.name=Eva -c user.email=eva@mattbox.local commit -qam more")
        laptop.fail("cd /tmp/eva-garden && git push -q origin main")
        # A visitor, through nginx: the public space alone.
        seen = json.loads(curl(f"{BASE}/git/api/v1/wikis?q=soup"))
        assert [w["full_name"] for w in seen["wikis"]] == ["eva/recipes"], seen
        # The page called so first, then the lines that say it.
        assert [h["page"] for h in seen["hits"]] == ["Bean-soup", "Home"], seen
        assert status(f"{BASE}/git/api/v1/repos/notshared/garden/wiki") == 404
        assert status(f"{BASE}/git/api/v1/repos/notshared/garden/wiki/pages/Home") == 404
        # The search finds titles first, then lines.
        hits = ok("GET", "/repos/notshared/garden/wiki/search?q=water", token=owner)["hits"]
        assert hits[0]["page"] == "Guides/Watering" and hits[0]["line"] is None, hits
        assert {h["page"] for h in hits} == {"Guides/Watering", "Home", "Guides"}, hits
        # A picture is served as one; a page's file is not.
        head = curl(f"-D - -o /tmp/beds.png -H 'Authorization: token {owner}' {BASE}/git/api/v1/repos/notshared/garden/wiki/raw/beds.png")
        assert re.search(r"(?im)^content-type: image/png", head), head
        assert re.search(r"(?im)^x-content-type-options: nosniff", head), head
        laptop.succeed("cmp /tmp/beds.png ${beds}")
        assert status(f"-H 'Authorization: token {owner}' {BASE}/git/api/v1/repos/notshared/garden/wiki/raw/Home.md") == 404
        assert status(f"{BASE}/git/api/v1/repos/notshared/garden/wiki/raw/beds.png") == 404
        # Published sites: the page is there for anyone, the data for whom it is.
        assert status(f"{BASE}/wiki/eva/recipes/Bean-soup") == 200
        assert status(f"{BASE}/wiki/") == 200

    with subtest("a wiki Forgejo left on disk is its repository's"):
        box.succeed(
            "cd /var/lib/losos-git/repositories/notshared"
            " && runuser -u losos-git -- git init -q --bare -b main family.git"
            " && runuser -u losos-git -- git init -q --bare -b main family.wiki.git",
            "rm -rf /tmp/family && mkdir /tmp/family && cd /tmp/family && git init -q -b master"
            " && printf '# Family\\n\\nBirthdays and phone numbers.\\n' > Home.md && git add -A"
            " && git -c user.name=Matus -c user.email=matus@mattbox.local commit -qm 'Home'"
            " && chmod -R a+rX /tmp/family"
            " && runuser -u losos-git -- git -c safe.directory='*'"
            "    -C /var/lib/losos-git/repositories/notshared/family.wiki.git fetch -q /tmp/family master:master",
            "systemctl restart losos-git.service",
        )
        box.wait_for_open_port(3000)
        names = sorted(r["name"] for r in ok("GET", "/user/repos", token=owner))
        assert names == ["family", "garden", "plans"], names
        w = ok("GET", "/repos/notshared/family/wiki", token=owner)
        assert w["exists"] and w["branch"] == "master" and w["home"] == "Home", w
        assert "Birthdays" in ok("GET", "/repos/notshared/family/wiki/pages/Home", token=owner)["text"]

    with subtest("a folder of notes published as a knowledge base"):
        head = curl(
            f"-D - -o /dev/null {BASE}/auth/api/firstfactor -H 'Content-Type: application/json'"
            f" -d '{json.dumps({"username": "matus", "password": PASSWORD})}'"
        )
        m = re.search(r"(?im)^set-cookie: (losos_session=[^;]+)", head)
        assert m, head
        me = f"-H 'Cookie: {m.group(1)}'"
        dav = f"{BASE}/remote.php/webdav"
        laptop.wait_until_succeeds(
            f"curl -sf --cacert {CA} -u 'matus:{PASSWORD}' -X PROPFIND -H 'Depth: 0' {dav}/ -o /dev/null",
            timeout=120,
        )
        curl(f"-f -u 'matus:{PASSWORD}' -X MKCOL {dav}/Birds")
        laptop.succeed(
            "printf '# Ducks\\n\\nDucks like the water by the shed.\\n' > /tmp/Ducks.md",
            "printf '# Robins\\n\\nA robin follows whoever digs.\\n' > /tmp/Robins.md",
        )
        curl(f"-f -u 'matus:{PASSWORD}' -T /tmp/Ducks.md {dav}/Birds/Ducks.md")
        curl(f"-f -u 'matus:{PASSWORD}' -T /tmp/Robins.md {dav}/Birds/Robins.md")
        out = curl(
            f"-f -X POST {me} -H 'Content-Type: application/json'"
            f" -d '{json.dumps({"path": "Birds", "title": "Bird notes", "audience": "anyone"})}' {BASE}/cloud/api/kb"
        )
        assert json.loads(out)["slug"] == "bird-notes", out
        index = json.loads(curl(f"'{BASE}/kb/api/?q=water'"))
        assert [s["title"] for s in index["sites"]] == ["Bird notes"], index
        assert [h["path"] for h in index["hits"]] == ["Ducks.md"], index

    def capture(name, out):
        try:
            laptop.succeed(
                f"losos-capture --base {BASE} --plan /etc/losos-test/plan-{name}.json"
                f" --out /tmp/shots/{out} --user matus --password '{PASSWORD}'",
                timeout=3600,
            )
        finally:
            laptop.copy_from_machine(f"/tmp/shots/{out}", "shots")
        report = json.loads(laptop.succeed(f"cat /tmp/shots/{out}/report.json"))
        return report

    with subtest("Chromium pictures LosOS Wiki"):
        report = capture("after", "after")
        print(json.dumps(report, indent=1))

    with subtest("an edit and a new space, made in the browser"):
        capture("edit", "edit")
        planting = ok("GET", "/repos/notshared/garden/wiki/pages/Planting", token=owner)
        assert "frost ended on 9 May" in planting["text"], planting
        assert planting["history"][0]["subject"] == "Note this year's last frost", planting
        assert planting["history"][0]["author"] == "Matus", planting
        laptop.succeed(f"cd /tmp/garden && git pull -q https://x:{owner}@{HOST}/git/notshared/garden.wiki.git main")
        laptop.succeed("grep -q 'frost ended on 9 May' /tmp/garden/Planting.md")
        repo = ok("GET", "/repos/notshared/Team-handbook", token=owner)
        assert repo["visibility"] == "internal", repo
        assert ok("GET", "/repos/notshared/Team-handbook/wiki", token=owner)["home"] == "Home"

    with subtest("the pictures: the same addresses before LosOS Wiki"):
        old_ui = os.environ.get("LOSOS_WIKI_OLD_UI", "")
        if old_ui:
            ui = "${noLab lososPkgs.losos-admin-ui}"
            box.succeed(
                "/run/booted-system/specialisation/before/bin/switch-to-configuration test",
                f"mount --bind {old_ui} {ui}",
                "systemctl restart nginx",
            )
            box.wait_for_open_port(443)
            capture("before", "before")
  '';
}
