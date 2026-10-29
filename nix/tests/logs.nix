# nixos-test-vms config for Settings › Logs (modules/log-reader.nix,
# backend/src/logs/): the box's journal in the admin pages, from a reader
# of its own beside lososd.
#
# A box with the real lososd and its plugins, nginx, LosOS Login, the log
# reader and the built admin UI, and a laptop on the LAN with a browser.
# What is asserted:
#
#   * the reader runs as its own user on a socket only root and nginx open;
#   * a service's lines that carry a password, a Bearer header, a password
#     in an address, a GitHub token and the spare key itself come out with
#     each one hidden, in the lines, the download, the page without
#     JavaScript and a problem report's excerpt, and a search for a secret's
#     letters finds nothing, while the journal on the box still has them;
#   * nobody gets a line without signing in, a person who is not an
#     administrator is told so, and the excerpt route is the box's alone;
#   * with lososd stopped, the page without JavaScript still opens, with
#     LosOS Login and with the spare key, says lososd does not answer and
#     shows the lines that say it stopped;
#   * the pictures: the pane with and without JavaScript, on a desktop and a
#     phone, the hidden secrets, following, the spare-key sign-in and the
#     page with lososd down.
#
# `before`, a path to an earlier checkout, boots that tree's own modules and
# packages instead and only takes the same addresses' pictures:
#
#   nix build --impure --expr '(import ./nix/tests/logs.nix {
#     pkgs = (builtins.getFlake (toString ./.)).inputs.nixpkgs.legacyPackages.x86_64-linux;
#     before = /path/to/main; })'
{
  pkgs,
  before ? null,
}:

let
  inherit (pkgs) lib;
  after = before == null;
  tree = if after then ../.. else before;
  lososPkgs = import (tree + "/nix/packages.nix") { inherit pkgs; };
  screens = import ./screens { inherit pkgs; };

  # The admin UI as it ships, without the Diagrams page, which this test
  # never opens and whose WebAssembly takes longer to build than the test.
  ui = import (tree + "/nix/tests/admin-ui-without-lab.nix") lososPkgs.losos-admin-ui;

  user = "matus";
  password = "Correct-Horse-Battery-9";

  heading = name: {
    role = "heading";
    inherit name;
  };
  line = {
    selector = "[data-slot=log-line]";
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
  phone = {
    width = 390;
    height = 844;
    colorScheme = "dark";
  };

  # The same addresses with and without JavaScript.
  pages = [
    {
      name = "lososd";
      path = "/settings/logs";
      waitFor = line;
    }
    {
      name = "secrets-hidden";
      path = "/settings/logs?unit=rebuilds&priority=7&span=boot";
      waitFor = {
        text = "visible-marker-42 done";
      };
    }
  ];
  basicPages = map (
    p:
    p
    // {
      path = "/basic" + p.path;
    }
  ) pages;

  plans =
    if after then
      {
        on-desktop = plan "on-desktop" {
          shots = pages ++ [
            {
              name = "following";
              path = "/settings/logs?unit=everything&priority=6";
              waitFor = line;
              steps = [
                {
                  click = {
                    role = "button";
                    name = "Follow";
                  };
                }
                {
                  waitFor = {
                    selector = "[data-slot=log-following]";
                  };
                }
                { scroll = "end"; }
              ];
            }
          ];
        };
        on-phone = plan "on-phone" (phone // { shots = pages; });
        off-desktop = plan "off-desktop" {
          script = false;
          shots = basicPages;
        };
        off-phone = plan "off-phone" (
          phone
          // {
            script = false;
            shots = basicPages;
          }
        );
        off-denied = plan "off-denied" {
          script = false;
          shots = [
            {
              name = "not-an-administrator";
              path = "/basic/settings/logs";
              waitFor = heading "For administrators";
            }
          ];
        };
        lososd-down = plan "lososd-down" {
          script = false;
          shots = [
            {
              name = "lososd-down";
              path = "/basic/settings/logs";
              waitFor = {
                text = "The control service does not answer";
              };
            }
            {
              name = "lososd-down-newest";
              path = "/basic/settings/logs";
              waitFor = {
                text = "The control service does not answer";
              };
              steps = [ { scroll = "end"; } ];
            }
          ];
        };
      }
    else
      # Before: whatever the same addresses were, with nothing to wait for.
      {
        on-desktop = plan "on-desktop" {
          shots = map (p: removeAttrs p [ "waitFor" ] // { settle = 5000; }) pages;
        };
        off-desktop = plan "off-desktop" {
          script = false;
          shots = map (p: removeAttrs p [ "waitFor" ] // { settle = 3000; }) basicPages;
        };
      };
in

pkgs.testers.nixosTest {
  name = if after then "losos-logs" else "losos-logs-before";

  nodes = {
    box =
      { ... }:
      {
        imports =
          map (m: tree + "/modules/${m}.nix") [
            "options"
            "containers"
            "tls"
            "daemon"
            "identity"
            "config-repo"
          ]
          ++ lib.optional after ../../modules/log-reader.nix;
        losos = {
          hostName = "mattbox";
          backend.package = lososPkgs.losos-ctl;
          backend.plugins = lososPkgs.losos-plugins;
          admin.ui = ui;
          git.enable = false;
        };
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
    import shlex

    BOX = "192.168.1.1"
    PASSWORD = "${password}"
    AFTER = ${if after then "True" else "False"}
    SOCKET = "/run/losos-logs/http.sock"

    start_all()
    box.wait_for_unit("lososd.service")
    box.wait_for_open_port(8082)
    box.wait_for_unit("nginx.service")
    box.wait_for_open_port(443)
    laptop.wait_for_unit("multi-user.target")

    def curl(args):
        return laptop.succeed(f"curl -sk {args}")

    def call(path, extra=""):
        out = curl(f"-w '\\n%{{http_code}}' {extra} 'https://{BOX}{path}'")
        body, code = out.rsplit("\n", 1)
        return int(code), body

    def capture(plan, who=None, write=None):
        if write is not None:
            laptop.succeed(
                f"mkdir -p /tmp/plans && echo {shlex.quote(json.dumps(write))} > /tmp/plans/{plan}.json"
            )
            where = f"/tmp/plans/{plan}.json"
        else:
            where = f"/etc/losos-test/plan-{plan}.json"
        sign = f"--user {who[0]} --password '{who[1]}'" if who else ""
        laptop.succeed(
            f"losos-capture --base https://{BOX} --plan {where} --out /tmp/screens/{plan} {sign}",
            timeout=2400,
        )
        seen = json.loads(laptop.succeed(f"cat /tmp/screens/{plan}/report.json"))
        for page in seen:
            assert page["pageWidth"] <= page["width"], f"{plan}: a page scrolls sideways: {page}"
            assert page["csp"] == [], f"{plan}: the policy refused something: {page}"
        return seen

    def sign_in(name, password):
        body = json.dumps({"username": name, "password": password})
        head = curl(
            f"-D - -o /dev/null https://{BOX}/auth/api/firstfactor "
            f"-H 'Content-Type: application/json' -d '{body}'"
        )
        m = re.search(r"(?im)^set-cookie: (losos_session=[^;]+)", head)
        assert m, f"no session cookie for {name}: {head}"
        return m.group(1)

    with subtest("claim the box, and a second person who is not an administrator"):
        claim = json.dumps({"name": "${user}", "password": PASSWORD})
        code, body = call("/api/setup/claim", f"-X POST -H 'Content-Type: application/json' -d '{claim}'")
        assert code == 200, (code, body)
        box.wait_for_unit("authelia-losos.service")
        box.systemctl("start losos-identity-setup.service")
        box.wait_until_succeeds(f"grep -q \"domain: '{BOX}'\" /run/losos-identity/session.yml")
        box.wait_until_succeeds("systemctl is-active authelia-losos.service")
        box.wait_for_open_port(9091)
        MATUS = sign_in("${user}", PASSWORD)
        eva = json.dumps({
            "name": "eva", "displayName": "Eva", "email": "eva@mattbox.local",
            "password": "Eva-Long-Password-3", "groups": ["losos-backup"],
        })
        code, body = call(
            "/api/people",
            f"-X POST -H 'Cookie: {MATUS}' -H 'Content-Type: application/json' -d '{eva}'",
        )
        assert code == 200, (code, body)
        box.wait_until_succeeds("grep -q eva /var/lib/losos-identity/users.yml")
        EVA = None
        for _ in range(30):
            try:
                EVA = sign_in("eva", "Eva-Long-Password-3")
                break
            except AssertionError:
                box.sleep(2)
        assert EVA, "eva cannot sign in"
        KEY = box.succeed(
            "systemd-creds decrypt --name=admin-token /etc/credstore.encrypted/admin-token -"
        ).strip()

    if not AFTER:
        with subtest("before: what the same addresses showed"):
            capture("on-desktop", who=("${user}", PASSWORD))
            capture("off-desktop", who=("${user}", PASSWORD))
        laptop.copy_from_machine("/tmp/screens", "")
    else:
        SECRETS = {
            "password": "hunter2-Correct-Horse",
            "bearer": "ZXhhbXBsZS1hY2Nlc3MtdG9rZW4tZm9yLXRlc3Q",
            "userinfo": "s3cret-pass",
            "github": "ghp_" + "A1b2C3d4E5f6G7h8I9j0K1l2M3n4O5p6Q7r8",
            "spare": KEY,
        }

        with subtest("the reader is its own user, on a socket only root and nginx open"):
            box.wait_for_unit("losos-logs.service")
            box.wait_for_file(SOCKET)
            assert box.succeed("systemctl show -p User --value losos-logs.service").strip() == "losos-logs"
            assert box.succeed(f"stat -c '%a %U %G' {SOCKET}").strip() == "660 losos-logs losos-logs"
            assert box.succeed("stat -c '%a' /run/losos-logs").strip() == "750"
            groups = box.succeed("id -nG nginx").split()
            assert "losos-logs" in groups, groups
            box.fail(f"runuser -u nobody -- curl -sf --unix-socket {SOCKET} http://logs/health")
            box.succeed(f"curl -sf --unix-socket {SOCKET} http://logs/health")

        with subtest("a service writes secrets to the journal"):
            script = "; ".join([
                f"echo 'signing in with password={SECRETS['password']} now'",
                f"echo 'Authorization: Bearer {SECRETS['bearer']}'",
                f"echo 'cloning https://matus:{SECRETS['userinfo']}@example.org/repo.git'",
                f"echo 'pushing with {SECRETS['github']}'",
                f"echo 'the spare key is {SECRETS['spare']}'",
                "echo 'visible-marker-42 done'",
            ])
            box.succeed(
                f"systemd-run --unit=losos-rebuild-secrets --wait --collect sh -c {shlex.quote(script)}"
            )
            box.wait_until_succeeds("journalctl -u losos-rebuild-secrets.service | grep -q visible-marker-42")
            # The journal itself keeps them: the reader is what hides them.
            raw = box.succeed("journalctl -u losos-rebuild-secrets.service -o cat")
            for name, value in SECRETS.items():
                assert value in raw, (name, raw)

        def no_secrets(where, text):
            for name, value in SECRETS.items():
                assert value not in text, f"{where} shows the {name}: {text[:2000]}"

        with subtest("the lines, the download, the page and the excerpt hide every one"):
            spare = f"-H 'Authorization: Bearer {KEY}'"
            q = "unit=rebuilds&priority=7&span=boot"
            code, body = call(f"/api/logs?{q}", spare)
            assert code == 200, (code, body)
            got = json.loads(body)
            assert got["error"] is None, got
            # Ours, without systemd's own lines about the unit (whose
            # description is the command line, secrets and all: hidden too).
            mine = ("signing in", "Authorization", "cloning", "pushing", "the spare key", "visible-marker")
            messages = [l["message"] for l in got["lines"] if l["message"].startswith(mine)]
            assert len(messages) == 6, messages
            no_secrets("/api/logs", body)
            assert messages[-1] == "visible-marker-42 done", messages
            assert all("[hidden]" in m for m in messages[:5]), messages
            assert "lososd" in got["sources"] and "everything" in got["sources"], got["sources"]

            code, text = call(f"/api/logs/download?{q}", spare)
            assert code == 200 and "visible-marker-42" in text, (code, text[:500])
            no_secrets("the download", text)
            head = curl(f"-D - -o /dev/null {spare} 'https://{BOX}/api/logs/download?{q}'")
            assert re.search(r'(?i)content-disposition: attachment; filename="losos-rebuilds-\d{8}-\d{4}\.log"', head), head

            code, page = call(f"/basic/settings/logs?{q}", f"-b 'losos_spare={KEY}'")
            assert code == 200 and "visible-marker-42" in page, (code, page[:500])
            no_secrets("the page without JavaScript", page)

            excerpt = box.succeed(
                f"curl -sf --unix-socket {SOCKET} "
                "'http://logs/internal/logs/excerpt?unit=losos-rebuild-secrets.service&lines=20&priority=7'"
            )
            assert "visible-marker-42" in excerpt, excerpt
            no_secrets("the excerpt", excerpt)

        with subtest("a search cannot find a secret by its letters"):
            for value in ["hunter2", SECRETS["bearer"][:12], SECRETS["spare"][:16], "ghp_A1b2"]:
                code, body = call(f"/api/logs?{q}&q={value}", spare)
                assert code == 200 and json.loads(body)["lines"] == [], (value, body[:500])
            code, body = call(f"/api/logs?{q}&q=VISIBLE-marker", spare)
            assert "visible-marker-42 done" in [l["message"] for l in json.loads(body)["lines"]], body

        with subtest("nobody gets a line without signing in, and only an administrator gets one"):
            assert call("/api/logs")[0] == 401
            assert call("/api/logs/download")[0] == 401
            assert call("/api/logs", "-H 'Authorization: Bearer nope'")[0] == 401
            code, body = call("/api/logs", f"-H 'Cookie: {EVA}'")
            assert code == 403 and json.loads(body)["error"] == "adminOnly", (code, body)
            assert call("/api/logs/download", f"-H 'Cookie: {EVA}'")[0] == 403
            code, body = call("/api/logs", f"-H 'Cookie: {MATUS}'")
            assert code == 200 and json.loads(body)["lines"], (code, body[:500])
            # Who LosOS Login says is asking is LosOS Login's to say.
            assert call("/api/logs", "-H 'Remote-User: matus' -H 'Remote-Groups: losos-admins'")[0] == 401

            code, page = call("/basic/settings/logs")
            assert code == 401 and 'name="key"' in page and "<script" not in page, (code, page[:800])
            code, page = call("/basic/settings/logs", f"-H 'Cookie: {EVA}'")
            assert code == 403 and "For administrators" in page, code
            assert 'data-slot="log-line"' not in page
            assert call("/basic/settings/logs/download", f"-H 'Cookie: {EVA}'")[0] == 403
            out = curl(f"-o /dev/null -w '%{{http_code}} %{{redirect_url}}' http://{BOX}/basic/settings/logs")
            assert out == f"301 https://{BOX}/basic/settings/logs", out
            # The excerpt is the box's own: nginx answers 404, and the reader
            # refuses whatever nginx forwards.
            assert call("/internal/logs/excerpt?unit=lososd.service", spare)[0] == 404
            box.fail(
                f"curl -sf --unix-socket {SOCKET} -H 'X-Losos-Front: 1' "
                "'http://logs/internal/logs/excerpt?unit=lososd.service'"
            )

        with subtest("the spare key signs in on the page without JavaScript"):
            head = curl(
                f"-D - -o /dev/null -X POST -d 'key=nope&next=/basic/settings/logs' "
                f"https://{BOX}/basic/settings/logs/key"
            )
            assert "losos_spare=" not in head, head
            head = curl(
                f"-D - -o /dev/null -X POST --data-urlencode 'key={KEY}' "
                f"--data-urlencode 'next=https://elsewhere.example/' https://{BOX}/basic/settings/logs/key"
            )
            assert re.search(r"(?im)^location: /basic/settings/logs\r?$", head), head
            assert "losos_spare=" in head and "HttpOnly" in head and "Secure" in head, head
            out = curl(
                f"-o /dev/null -w '%{{http_code}}' -X POST -H 'Sec-Fetch-Site: cross-site' "
                f"--data-urlencode 'key={KEY}' https://{BOX}/basic/settings/logs/key"
            )
            assert out == "403", out

        with subtest("pictures with lososd up"):
            capture("on-desktop", who=("${user}", PASSWORD))
            capture("on-phone", who=("${user}", PASSWORD))
            capture("off-desktop", who=("${user}", PASSWORD))
            capture("off-phone", who=("${user}", PASSWORD))
            capture("off-denied", who=("eva", "Eva-Long-Password-3"))
            capture("spare-key", write={
                "width": 1280, "height": 800, "script": False,
                "shots": [
                    {"name": "sign-in", "path": "/basic/settings/logs",
                     "waitFor": {"role": "heading", "name": "Sign in to read the logs"}},
                    {"name": "signed-in-with-the-key", "path": "/basic/settings/logs",
                     "waitFor": {"role": "heading", "name": "Sign in to read the logs"},
                     "steps": [
                         {"fill": {"label": "Spare key", "value": KEY}},
                         {"click": {"role": "button", "name": "Open with the spare key"}},
                         {"waitFor": {"selector": "[data-slot=log-line]"}},
                     ]},
                ],
            })

        with subtest("with lososd stopped the page without JavaScript still answers"):
            box.systemctl("stop lososd.service")
            box.fail("systemctl is-active lososd.service")
            for who in [f"-b 'losos_spare={KEY}'", f"-H 'Cookie: {MATUS}'"]:
                code, page = call("/basic/settings/logs", who)
                assert code == 200, (who, code, page[:800])
                assert "The control service does not answer" in page, page[:2000]
                assert "Stopped" in page or "Deactivated successfully" in page, page[:4000]
            code, page = call("/basic/settings/logs", f"-H 'Cookie: {EVA}'")
            assert code == 403, code
            code, page = call("/basic/settings/logs")
            assert code == 401, code
            code, body = call("/api/logs?unit=lososd&span=boot", spare)
            assert code == 200 and json.loads(body)["lines"], (code, body[:500])
            capture("lososd-down", who=("${user}", PASSWORD))
            box.systemctl("start lososd.service")
            box.wait_for_open_port(8082)

        laptop.copy_from_machine("/tmp/screens", "")
  '';
}
