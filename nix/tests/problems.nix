# nixos-test-vms config for problem reports on the box (backend/plugins/problems,
# modules/problems.nix, handbook/docs/manual/problem-reports.md).
#
# A box with the real lososd, its plugins from the store, nginx, LosOS Login
# and the built admin UI; a stand-in for GitHub on the box's loopback
# (github-stand-in.py); and a laptop with a browser. Three services go wrong
# the ways the box watches for, and the test asserts what only a booted box
# shows:
#
#   * the GitHub App's client ID arrives the way the installer hands it
#     over, as a plaintext file the first boot seals into the credential
#     store and deletes, and lososd reads it from its credential directory;
#   * the problems plugin finds a service that failed, one that crashed with
#     SIGSEGV and one whose failure the handbook explains, from the journal,
#     with no core file;
#   * a report holds the unit's journal with the box's name, its addresses
#     and its people taken out, and nothing reaches GitHub before the owner
#     sends it;
#   * sending runs the device flow: the code is shown, the issue is filed
#     once the code is entered, and the same problem sent again is a comment
#     on that issue; a documented problem cannot be sent;
#   * the pane in a browser, and the same pane without JavaScript.
#
# The laptop's pictures are the ones the pull request shows: the
# derivation's output holds them (screens/, basic/).
{ pkgs }:

let
  lososPkgs = import ../packages.nix { inherit pkgs; };
  screens = import ./screens { inherit pkgs; };
  github = 8765;
in

pkgs.testers.nixosTest {
  name = "losos-problems";

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
          ../../modules/credentials.nix
          ../../modules/problems.nix
        ];
        losos = {
          hostName = "mattbox";
          backend.package = lososPkgs.losos-ctl;
          backend.plugins = lososPkgs.losos-plugins;
          admin.ui = lososPkgs.losos-admin-ui;
          git.enable = false;
          # No clientId: the value comes from the credential below.
          report = {
            login = "http://127.0.0.1:${toString github}";
            api = "http://127.0.0.1:${toString github}";
          };
        };

        systemd.services = {
          github-stand-in = {
            wantedBy = [ "multi-user.target" ];
            serviceConfig.ExecStart = "${pkgs.python3}/bin/python3 ${./github-stand-in.py} ${toString github} /var/lib/github";
          };
          # Three ways for a service to go wrong. None starts by itself.
          eva-sync = {
            # oneshot, so that `systemctl start` waits and reports the exit.
            serviceConfig.Type = "oneshot";
            # Its log names the box, an address and the owner, which a
            # report must not carry.
            script = ''
              echo "eva-sync on $(${pkgs.hostname}/bin/hostname): matus asked 192.168.1.20 for the calendar"
              echo "eva-sync: the calendar answered with something it cannot read"
              exit 3
            '';
          };
          crasher.serviceConfig.ExecStart = "${pkgs.coreutils}/bin/sleep 600";
          filler.serviceConfig.Type = "oneshot";
          filler.script = ''
            echo "filler: write /var/lib/filler/blob: No space left on device"
            exit 1
          '';
        };

        # What the installer leaves on /persist (installer.rs
        # REPORT_CLIENT_ID_HANDOVER), there before the first boot's
        # losos-credentials runs.
        systemd.tmpfiles.rules = [ "f /var/lib/losos/report-client-id 0600 root root - Iv23test" ];

        environment.systemPackages = [ pkgs.jq ];
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
    # lososd loads its plugins before it listens; under TCG that is minutes.
    box.wait_for_open_port(8082, timeout=1200)
    box.wait_for_unit("nginx.service")
    box.wait_for_unit("github-stand-in.service")
    box.wait_for_open_port(443)
    box.wait_for_open_port(${toString github})
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

    def sign_in(name, password):
        body = json.dumps({"username": name, "password": password})
        head = curl(
            f"-D - -o /dev/null https://{BOX}/auth/api/firstfactor "
            f"-H 'Content-Type: application/json' -d '{body}'"
        )
        m = re.search(r"(?im)^set-cookie: (losos_session=[^;]+)", head)
        assert m, f"no session cookie for {name}: {head}"
        return m.group(1)

    def requests():
        out = box.succeed("cat /var/lib/github/requests.jsonl 2>/dev/null || true")
        return [json.loads(line) for line in out.splitlines() if line.strip()]

    def capture(plan, out):
        laptop.succeed("cat > /tmp/plan.json <<'EOF'\n" + json.dumps(plan) + "\nEOF")
        laptop.succeed(
            f"losos-capture --base https://{BOX} --plan /tmp/plan.json "
            f"--out {out} --user matus --password '{PASSWORD}'",
            timeout=1800,
        )

    def filed():
        return json.loads(box.succeed("cat /var/lib/github/issues.json 2>/dev/null || echo '[]'"))

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
        cookie = sign_in("matus", PASSWORD)
        box.wait_until_succeeds("journalctl -u lososd | grep -q 'plugin loaded.*problems'")

    with subtest("the client ID the installer left is sealed, and only lososd opens it"):
        box.wait_for_unit("losos-credentials.service")
        box.succeed("test -s /etc/credstore.encrypted/report-client-id")
        box.fail("grep -q Iv23test /etc/credstore.encrypted/report-client-id")
        box.fail("test -e /var/lib/losos/report-client-id")
        assert box.succeed(
            "systemd-creds decrypt --name=report-client-id /etc/credstore.encrypted/report-client-id -"
        ).strip() == "Iv23test"
        box.succeed("grep -qx Iv23test /run/credentials/lososd.service/report-client-id")
        # Not in lososd's environment, nor in any unit file.
        box.fail("tr '\\0' '\\n' < /proc/$(systemctl show -P MainPID lososd)/environ | grep -q Iv23test")
        box.fail("grep -Rq Iv23test /etc/systemd/system/")

    with subtest("nothing has gone wrong, and the box says where reports would go"):
        code, body = api("/api/problems", cookie)
        assert code == 200, (code, body)
        view = json.loads(body)
        assert view["reporting"] == {"repository": "dasmatus/losos"}, view
        assert all(p["unit"] not in ("eva-sync.service", "crasher.service") for p in view["problems"]), view
        code, _ = api("/api/problems")
        assert code == 401, code

    with subtest("a failure, a crash and a full disk land on the list"):
        box.fail("systemctl start eva-sync.service")
        box.systemctl("start crasher.service")
        box.wait_for_unit("crasher.service")
        box.succeed("systemctl kill -s SEGV crasher.service")
        box.wait_until_succeeds("systemctl is-failed crasher.service")
        box.fail("systemctl start filler.service")

        def listed(_):
            code, body = api("/api/problems", cookie)
            units = {p["unit"]: p for p in json.loads(body)["problems"]}
            return all(u in units for u in ("eva-sync.service", "crasher.service", "filler.service"))

        # The scan runs once a minute.
        retry(listed, timeout_seconds=240)
        code, body = api("/api/problems", cookie)
        units = {p["unit"]: p for p in json.loads(body)["problems"]}
        eva, crash, full = units["eva-sync.service"], units["crasher.service"], units["filler.service"]
        print(json.dumps(units, indent=2))
        assert eva["kind"] == "failed" and eva["result"] == "exit-code", eva
        assert crash["kind"] == "crashed" and crash["signal"] == "SEGV", crash
        assert eva["documented"] is None and crash["documented"] is None
        assert full["documented"]["page"] == "troubleshooting/out-of-room", full
        assert full["documented"]["title"]["de"] == "Kein Platz mehr", full
        # Nothing has gone to GitHub.
        assert requests() == [], requests()

    with subtest("a report is the unit's journal, with the box and its people taken out"):
        code, body = api(f"/api/problems/{eva['id']}", cookie)
        assert code == 200, (code, body)
        report = json.loads(body)
        text = report["body"]
        print(text)
        assert report["title"].endswith(f"[{eva['id']}]"), report["title"]
        assert "the calendar answered with something it cannot read" in text, text
        for secret in ["on box", "192.168.1.20", "matus asked"]:
            assert secret not in text, f"{secret!r} in the report"
        assert "eva-sync on [name]: [name] asked [IP] for the calendar" in text, text
        # The box is named `box` here: the words around its log keep theirs.
        assert "Sent from a LosOS box." in text, text
        code, body = api(f"/api/problems/{crash['id']}", cookie)
        assert "SIGSEGV" in json.loads(body)["title"], body
        assert requests() == [], requests()

    with subtest("a documented problem is not sent"):
        code, body = api(f"/api/problems/{full['id']}/send", cookie, "POST", {"note": ""})
        assert code == 409, (code, body)

    def sent(problem):
        def done(_):
            code, body = api(f"/api/problems/{problem}/send", cookie)
            state = json.loads(body)
            if state["state"] == "failed":
                raise Exception(f"sending failed: {state}")
            return state["state"] == "sent"
        retry(done, timeout_seconds=120)
        _, body = api(f"/api/problems/{problem}/send", cookie)
        return json.loads(body)

    with subtest("in a browser: the list, a report, the code, and the issue"):
        plan = {
            "width": 1280,
            "height": 860,
            "shots": [
                {
                    "name": "problems",
                    "path": "/settings/problems",
                    "waitFor": {"selector": f"[data-id='{crash['id']}']"},
                },
                {
                    "name": "problems-report",
                    "path": "/settings/problems",
                    "waitFor": {"selector": f"[data-id='{eva['id']}'] [data-testid=problem-read]"},
                    "steps": [
                        {"click": {"selector": f"[data-id='{eva['id']}'] [data-testid=problem-read]"}},
                        {"waitFor": {"selector": "[data-testid=problem-report-body]"}},
                        {"fill": {"label": "What were you doing when it happened? (optional, sent with the report)",
                                  "value": "I had just added a calendar from my phone."}},
                    ],
                },
                {
                    "name": "problems-code",
                    "path": "/settings/problems",
                    "waitFor": {"selector": f"[data-id='{eva['id']}'] [data-testid=problem-read]"},
                    "steps": [
                        {"click": {"selector": f"[data-id='{eva['id']}'] [data-testid=problem-read]"}},
                        {"waitFor": {"selector": "[data-testid=problem-report-body]"}},
                        {"fill": {"label": "What were you doing when it happened? (optional, sent with the report)",
                                  "value": "I had just added a calendar from my phone."}},
                        {"click": {"selector": "[data-testid=problem-send]"}},
                        {"waitFor": {"selector": "[data-testid=problem-code]"}},
                        {"waitText": "WDJB-MJHT"},
                    ],
                },
            ],
        }
        capture(plan, "/tmp/screens")
        # The code is on screen and nothing is filed yet.
        assert filed() == [], filed()
        # The owner enters the code on GitHub.
        box.succeed("touch /var/lib/github/approved")
        state = sent(eva["id"])
        assert state == {"state": "sent", "url": "https://github.com/dasmatus/losos/issues/1", "existing": False}, state
        issue = filed()[0]
        assert issue["title"] == report["title"], issue
        assert issue["body"].startswith("**From the box's owner:**\n\n> I had just added a calendar from my phone."), issue
        assert "the calendar answered with something it cannot read" in issue["body"], issue
        for secret in ["on box", "192.168.1.20"]:
            assert secret not in issue["body"], secret
        # Only the device flow went out before the token, and the token only
        # with the search and the issue.
        paths = [r["path"].split("?")[0] for r in requests()]
        assert paths[0] == "/login/device/code", paths
        assert "/repos/dasmatus/losos/issues" in paths, paths
        code, body = api("/api/problems", cookie)
        listed_eva = next(p for p in json.loads(body)["problems"] if p["id"] == eva["id"])
        assert listed_eva["sent"]["url"] == state["url"], listed_eva

        plan = {
            "width": 1280,
            "height": 860,
            "shots": [
                {
                    "name": "problems-sent",
                    "path": "/settings/problems",
                    "waitFor": {"selector": f"[data-id='{eva['id']}'] [data-testid=problem-read]"},
                    "steps": [
                        {"click": {"selector": f"[data-id='{eva['id']}'] [data-testid=problem-read]"}},
                        {"waitFor": {"selector": "[data-testid=problem-sent]"}},
                    ],
                },
            ],
        }
        capture(plan, "/tmp/screens")

    with subtest("the same problem sent again is a comment on its issue"):
        code, body = api(f"/api/problems/{eva['id']}/send", cookie, "POST", {"note": "Again today."})
        assert code == 202, (code, body)
        state = sent(eva["id"])
        assert state["existing"] is True, state
        assert state["url"].startswith("https://github.com/dasmatus/losos/issues/1#issuecomment-"), state
        issues = filed()
        assert len(issues) == 1 and len(issues[0]["comments"]) == 1, issues
        assert "Again today." in issues[0]["comments"][0], issues

    with subtest("dismissed, and back when it happens again"):
        code, _ = api(f"/api/problems/{crash['id']}/dismiss", cookie, "POST")
        assert code == 200, code
        code, body = api("/api/problems", cookie)
        assert next(p for p in json.loads(body)["problems"] if p["id"] == crash["id"])["dismissed"] is True

    with subtest("without JavaScript: the same pane, a report, and its forms"):
        plan = {
            "width": 1280,
            "height": 860,
            "script": False,
            "shots": [
                {
                    "name": "basic-problems",
                    "path": "/basic/settings/problems",
                    "waitFor": {"selector": "[data-basic-problems]"},
                },
                {
                    "name": "basic-problems-report",
                    "path": f"/basic/settings/problems?read={crash['id']}",
                    "waitFor": {"selector": "[data-testid=problem-report-body]"},
                },
            ],
        }
        capture(plan, "/tmp/basic")
        box.systemctl("start crasher.service")
        box.wait_for_unit("crasher.service")
        box.succeed("systemctl kill -s SEGV crasher.service")

        def back(_):
            code, body = api("/api/problems", cookie)
            p = next(p for p in json.loads(body)["problems"] if p["id"] == crash["id"])
            return p["count"] >= 2 and p["dismissed"] is False

        retry(back, timeout_seconds=240)
        laptop.copy_from_machine("/tmp/screens", "")
        laptop.copy_from_machine("/tmp/basic", "")
  '';
}
