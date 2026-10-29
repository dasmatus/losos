# nixos-test-vms config for LosOS Git's issue tracker (backend/src/forge/tracker/,
# admin-ui/app/src/git/tracker/) on a real box: the front vhost, LosOS
# Login, lososd claiming the box, and LosOS Git behind them.
#
# The box is one that ran LosOS Git and LosOS Cloud before the tracker:
#
#   * backup-tool has two issues carried over from Forgejo (#1 open with a
#     comment of Eva's, #2 closed), written into LosOS Git's database the
#     way the Forgejo import left them;
#   * LosOS Cloud has two boards: Garden (columns To do, Doing, Done; Eva
#     may edit) and Backup tool, linked to the repository, whose cards point
#     at #1 and #2 or stand alone.
#
# LosOS Git starts on the new build. The repository's issues become the
# project BT with the same numbers, losos-git-boards-import takes the
# boards in once (Garden a project of its own, the Backup tool cards into
# BT), and a second start changes nothing. Then people work: issues,
# a cycle, a sub-issue, a mention; the owner pushes "Fixes BT-4" to main
# over HTTPS and BT-4 is done with the commit beside it; Eva opens a pull
# request from bt-6-free-space that says "Closes BT-6", which puts BT-6 in
# review, and the owner merges it, which closes it.
#
# Then Chromium on the laptop signs in as the owner through LosOS Login and
# works through the pages (nix/tests/issue-shots.mjs): the list, an issue,
# the status menu from the keyboard, the board and a drag, the palette, a
# new issue, the inbox, the cycles, the Garden board. After it the test
# reads the tracker's API to see that each change the page showed was made.
# With LOSOS_ISSUES_OLD_UI (an admin UI from before the tracker, built
# through admin-ui-without-lab.nix) the same repository is pictured on the
# old pages for the "before" column.
{ pkgs }:

let
  lososPkgs = import ../packages.nix { inherit pkgs; };
  # The pages never open Diagrams, so its WebAssembly is left out.
  ui = import ./admin-ui-without-lab.nix lososPkgs.losos-admin-ui;
  screens = import ./screens { inherit pkgs; };

  shots = pkgs.writeShellScriptBin "issue-shots" ''
    export PLAYWRIGHT_CORE=${pkgs.playwright-driver}/index.mjs
    export PLAYWRIGHT_BROWSERS_PATH=${
      pkgs.playwright-driver.browsers.override {
        withFirefox = false;
        withWebkit = false;
      }
    }
    export PLAYWRIGHT_SKIP_VALIDATE_HOST_REQUIREMENTS=true
    exec ${pkgs.bun}/bin/bun ${./issue-shots.mjs} "$@"
  '';

  # LosOS Cloud's boards as backend/src/cloud/boards.rs keeps them, its
  # SCHEMA read from the source so the two cannot drift apart, with what a
  # person had on them.
  cloudSchema =
    let
      after = builtins.elemAt (builtins.split ''pub const SCHEMA: &str = r#"'' (builtins.readFile ../../backend/src/cloud/boards.rs)) 2;
    in
    builtins.head (builtins.split ''"#;'' after);
  cloudBoards = pkgs.writeText "cloud-boards.sql" ''
    ${cloudSchema}
    INSERT INTO board VALUES
      (1, 'Garden', '31cc7c', 'notshared', NULL, 0, 1760000000, 1760000000),
      (2, 'Backup tool', '0f6e7a', 'notshared', 'notshared/backup-tool', 0, 1760000000, 1760000000);
    INSERT INTO board_member VALUES (1, 'eva', 1, 0, 0);
    INSERT INTO board_column VALUES
      (10, 1, 'To do', 0, NULL), (11, 1, 'Doing', 1, NULL), (12, 1, 'Done', 2, 'closed'),
      (20, 2, 'Ideas', 0, NULL), (21, 2, 'Doing', 1, NULL), (22, 2, 'Done', 2, 'closed');
    INSERT INTO board_label VALUES (30, 1, 'outside', 'ff9900'), (31, 1, 'shopping', '2f7de1');
    INSERT INTO card VALUES
      (100, 1, 11, 0, 'Plant tomatoes', 'Six plants by the south wall.', 1793404800, NULL, NULL, 0, 1760000000, 1760000100, NULL),
      (101, 1, 10, 0, 'Fix the rain barrel tap', ''', NULL, NULL, NULL, 0, 1760000000, 1760000000, NULL),
      (102, 1, 12, 0, 'Buy seeds', ''', NULL, NULL, NULL, 0, 1760000000, 1760000000, 1760000000),
      (103, 1, 10, 1, 'Order compost', ''', NULL, NULL, NULL, 0, 1760000000, 1760000000, NULL),
      (200, 2, 21, 0, 'Wake the NAS first', ''', NULL, 'notshared/backup-tool', 1, 0, 1760000000, 1760000000, NULL),
      (201, 2, 22, 0, 'Retry log', ''', NULL, 'notshared/backup-tool', 2, 0, 1760000000, 1760000000, 1760000000),
      (202, 2, 20, 0, 'Restore one folder from the NAS', 'Bring one folder back without the whole set.', NULL, NULL, NULL, 0, 1760000000, 1760000000, NULL);
    INSERT INTO card_label VALUES (100, 30), (103, 30), (102, 31);
    INSERT INTO card_assignee VALUES (100, 'eva'), (101, 'notshared');
    INSERT INTO card_comment VALUES (1, 100, 'eva', 'I have the pots.', 1760000150);
  '';
in

pkgs.testers.nixosTest {
  name = "losos-git-issues";

  nodes = {
    box = _: {
      imports = [
        ../../modules/options.nix
        ../../modules/mdns.nix
        ../../modules/containers.nix
        ../../modules/tls.nix
        ../../modules/daemon.nix
        ../../modules/identity.nix
        ../../modules/git.nix
      ];
      networking.hostName = "mattbox";
      losos = {
        hostName = "mattbox";
        backend.package = lososPkgs.losos-ctl;
        # The pins and the look are plugins; without them the page sees 404s.
        backend.plugins = lososPkgs.losos-plugins;
        admin.ui = ui;
        git.enable = true;
        mdns.enable = true;
      };
      environment.systemPackages = [
        pkgs.jq
        pkgs.curl
        pkgs.sqlite
      ];
      virtualisation = {
        memorySize = 2048;
        cores = 2;
        diskSize = 4096;
        vlans = [ 1 ];
      };
    };

    laptop = _: {
      services.avahi = {
        enable = true;
        nssmdns4 = true;
      };
      environment.systemPackages = [
        pkgs.curl
        pkgs.jq
        pkgs.gitMinimal
        shots
        screens.capture
      ];
      environment.variables.GIT_TERMINAL_PROMPT = "0";
      fonts.packages = [ pkgs.dejavu_fonts ];
      virtualisation = {
        memorySize = 3072;
        cores = 2;
        vlans = [ 1 ];
      };
    };
  };

  testScript = ''
    import json
    import os

    HOST = "mattbox.local"
    BASE = f"https://{HOST}"
    CA = "/etc/losos-box.pem"
    PASSWORD = "Correct-Horse-Battery-9"
    REPO = "notshared/backup-tool"
    DB = "/var/lib/losos-git/git.db"

    ${screens.handbookScript}

    # One VM after the other: two TCG guests started together can miss the
    # driver's wait for a shell.
    box.start()
    box.wait_for_unit("nginx.service")
    box.wait_for_unit("lososd.service")
    box.wait_for_unit("losos-git.service")
    box.wait_for_open_port(3000)
    laptop.start()
    laptop.succeed("true")

    cert = box.succeed("cat /var/lib/losos-tls/cert.pem")
    laptop.succeed(f"cat > {CA} <<'EOF'\n{cert}EOF")
    laptop.succeed(f"git config --system http.sslCAInfo {CA}")
    git_admin = box.succeed("cat /var/lib/losos-git/admin-token").strip()

    def forge(method, path, body=None, token=None):
        args = f"-s -o /tmp/forge -w '%{{http_code}}' -X {method}"
        args += f" -H 'Authorization: token {token or git_admin}'"
        if body is not None:
            box.succeed(f"cat > /tmp/forge-body <<'EOF'\n{json.dumps(body)}\nEOF")
            args += " -H 'Content-Type: application/json' --data-binary @/tmp/forge-body"
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
                 {"name": "laptop", "scopes": ["write:repository", "write:issue", "read:user"], "replace": True})
        return doc["token"]

    def url(token):
        return f"https://x:{token}@{HOST}/git/{REPO}.git"

    def commit(where, who, message):
        name, email = who
        laptop.succeed(
            f"cd {where} && git add -A"
            f" && git -c user.name='{name}' -c user.email={email} commit -qm '{message}'"
        )

    def issue(key, token=None):
        return ok("GET", f"/tracker/issues/{key}", token=token)

    def sql(query):
        return box.succeed(f"sqlite3 {DB} \"{query}\"").strip()

    MATUS = ("Matus", "matus@mattbox.local")
    EVA = ("Eva", "eva@mattbox.local")

    with subtest("the laptop finds the box and the owner claims it"):
        laptop.wait_until_succeeds(f"getent hosts {HOST}", timeout=120)
        laptop.wait_until_succeeds(
            f"curl -sf --cacert {CA} {BASE}/api/setup/claim | jq -e .ready", timeout=600
        )
        out = laptop.succeed(
            f"curl -s --cacert {CA} -w '\\n%{{http_code}}' -X POST {BASE}/api/setup/claim"
            " -H 'Content-Type: application/json'"
            f" -d '{json.dumps({"name": "matus", "password": PASSWORD})}'"
        )
        assert out.rsplit("\n", 1)[1] == "200", out
        box.wait_for_unit("authelia-losos.service")
        box.wait_for_open_port(9091)

    with subtest("three people and the repository, with main pushed"):
        ok("PUT", "/admin/users/notshared", {"full_name": "Matus", "email": MATUS[1]})
        ok("PUT", "/admin/users/eva", {"full_name": "Eva", "email": EVA[1]})
        ok("PUT", "/admin/users/jonas", {"full_name": "Jonas", "email": "jonas@mattbox.local"})
        owner = token_for("notshared")
        eva = token_for("eva")
        ok("POST", "/user/repos", {
            "name": "backup-tool",
            "description": "Copies the photos to the NAS every night",
            "private": True,
        }, token=owner)
        for who in ("eva", "jonas"):
            ok("PUT", f"/repos/{REPO}/collaborators/{who}", {"permission": "write"}, token=owner)
        laptop.succeed(
            "mkdir -p /tmp/main && cd /tmp/main && git init -q -b main",
            "printf '# Backup tool\\n\\nCopies the photos from this box to the NAS every night at two.\\n'"
            " > /tmp/main/README.md",
            "printf '#!/bin/sh\\nset -eu\\nrsync -a --partial \"$1/\" \"$2/\"\\n' > /tmp/main/backup.sh",
        )
        commit("/tmp/main", MATUS, "Copy the photos to the NAS")
        laptop.succeed(f"cd /tmp/main && git push -q {url(owner)} main")

    with subtest("issues carried over from Forgejo and LosOS Cloud's boards are there"):
        box.succeed("systemctl stop losos-git.service")
        repo_id = sql("SELECT r.id FROM repos r JOIN users u ON u.id = r.owner_id WHERE u.login = 'notshared' AND r.name = 'backup-tool'")
        uid = {who: sql(f"SELECT id FROM users WHERE login = '{who}'") for who in ("notshared", "eva")}
        sql(
            "INSERT INTO issues (repo_id, number, poster_id, title, body, closed, created, updated, closed_at) VALUES"
            f" ({repo_id}, 1, {uid['notshared']}, 'Backups stop when the NAS sleeps',"
            " 'The copy fails at two when the NAS is asleep. It should wake it first.', 0,"
            " '2026-09-30T19:02:00Z', '2026-10-01T07:40:00Z', NULL),"
            f" ({repo_id}, 2, {uid['eva']}, 'Log every failed try', ''', 1,"
            " '2026-10-02T10:00:00Z', '2026-10-03T09:00:00Z', '2026-10-03T09:00:00Z');"
            "INSERT INTO issue_comments (issue_id, poster_id, body, created, updated)"
            f" SELECT id, {uid['eva']}, 'The NAS answers wake-on-LAN, I tried it from the laptop.',"
            " '2026-10-01T07:40:00Z', '2026-10-01T07:40:00Z' FROM issues WHERE number = 1;"
        )
        box.succeed(
            "install -d /var/lib/losos-cloud"
            " && sqlite3 /var/lib/losos-cloud/cloud.db < ${cloudBoards}"
        )
        # The box boots with the import before LosOS Git; here the boards
        # arrive later, so the unit runs by hand in the same order.
        box.succeed("systemctl start losos-git-boards-import.service", "systemctl start losos-git.service")
        box.wait_for_open_port(3000)
        report = json.loads(box.succeed("cat /var/lib/losos-git/boards/import.json"))
        assert report["complete"] is True, report
        project = ok("GET", f"/tracker/repos/{REPO}", token=owner)
        assert project["key"] == "BT", project
        one = issue("BT-1", owner)
        assert one["title"] == "Backups stop when the NAS sleeps", one
        assert one["repo"]["number"] == 1, one
        # The Backup tool board had it in Doing.
        assert one["state"]["category"] == "started", one
        assert [c["body"] for c in one["comments"]] == ["The NAS answers wake-on-LAN, I tried it from the laptop."], one
        assert issue("BT-2", owner)["state"]["category"] == "done"
        garden = ok("GET", "/tracker/projects/GAR", token=owner)
        assert [s["name"] for s in garden["states"]] == ["To do", "Doing", "Done", "Canceled"], garden
        found = ok("GET", "/tracker/issues?project=GAR", token=owner)["issues"]
        plant = next(i for i in found if i["title"] == "Plant tomatoes")
        assert plant["assignee"] == "eva" and plant["state"]["name"] == "Doing", plant
        assert [l["name"] for l in plant["labels"]] == ["outside"], plant
        restore = next(i for i in ok("GET", "/tracker/issues?project=BT", token=owner)["issues"]
                       if i["title"] == "Restore one folder from the NAS")
        assert restore["state"]["category"] == "backlog", restore

    with subtest("a second start takes nothing in twice"):
        before = sql("SELECT COUNT(*) FROM tickets")
        box.succeed("systemctl restart losos-git.service")
        box.wait_for_open_port(3000)
        assert sql("SELECT COUNT(*) FROM tickets") == before
        box.succeed("systemctl start losos-git-boards-import.service")
        assert sql("SELECT COUNT(*) FROM tickets") == before
        # Nothing of LosOS Cloud's was changed.
        assert box.succeed("sqlite3 /var/lib/losos-cloud/cloud.db 'SELECT COUNT(*) FROM card'").strip() == "7"

    with subtest("people work on the issues"):
        today = box.succeed("date +%F").strip()
        later = box.succeed("date -d '+9 days' +%F").strip()
        earlier = box.succeed("date -d '-4 days' +%F").strip()
        ok("POST", "/tracker/projects/BT/cycles", {"name": "", "starts": earlier, "ends": later}, token=owner)
        cycle = ok("GET", "/tracker/projects/BT", token=owner)["cycles"][0]["id"]
        soon = box.succeed("date -d '+2 days' +%F").strip()
        def new(**fields):
            return ok("POST", "/tracker/issues", {"project": "BT", **fields}, token=owner)
        four = new(title="Log a failed copy to /var/log/backup.log", priority=3, assignee="notshared",
                    labels=["feature"], cycle=cycle, estimate=1, category="unstarted")
        assert four["key"] == "BT-4", four
        new(title="Backup runs twice after a reboot at midnight", priority=1, assignee="notshared",
            labels=["bug"], cycle=cycle, estimate=2, category="started", due=soon)
        six = new(title="Check free space on the NAS before copying", priority=2, assignee="eva",
                   labels=["bug"], cycle=cycle, estimate=2, category="unstarted")
        assert six["key"] == "BT-6", six
        new(title="Copy only the photos that changed", priority=2, assignee="eva",
            labels=["feature"], cycle=cycle, estimate=3, category="unstarted")
        new(title="Write the README section on restoring", priority=4, assignee="jonas",
            labels=["docs"], category="unstarted")
        new(title="Send a mail when a backup fails", priority=3, labels=["feature"], category="backlog")
        new(title="Wake the NAS with a magic packet", parent="BT-1", assignee="notshared", priority=2)
        ok("POST", "/tracker/issues/BT-5/relations", {"kind": "blocks", "other": "BT-1"}, token=owner)
        ok("POST", "/tracker/issues/BT-1/comments",
           {"body": "@notshared BT-5 has to land first or it copies twice."}, token=eva)
        mine = ok("GET", "/tracker/issues?assignee=me", token=owner)["issues"]
        assert {i["key"] for i in mine} >= {"BT-4", "BT-5", "BT-10"}, mine
        inbox = ok("GET", "/tracker/inbox", token=owner)
        assert any(i["kind"] == "mention" and i["issue"]["key"] == "BT-1" for i in inbox["items"]), inbox

    with subtest("a commit that says Fixes BT-4 closes it"):
        laptop.succeed("printf 'log=/var/log/backup.log\\n' >> /tmp/main/backup.sh")
        commit("/tmp/main", MATUS, "Log a failed copy\n\nFixes BT-4.")
        laptop.succeed(f"cd /tmp/main && git push -q {url(owner)} main")
        box.wait_until_succeeds(
            f"curl -fsS -H 'Authorization: token {git_admin}' http://127.0.0.1:3000/api/v1/tracker/issues/BT-4"
            " | jq -e '.state.category == \"done\"'",
            timeout=60,
        )
        four = issue("BT-4", owner)
        assert [(l["kind"], l["closes"]) for l in four["links"]] == [("commit", True)], four

    with subtest("Eva's pull request puts BT-6 in review and the merge closes it"):
        laptop.succeed(
            f"git clone -q {url(eva)} /tmp/eva",
            "cd /tmp/eva && git checkout -q -b bt-6-free-space",
            "printf 'df -h \"$2\"\\n' >> /tmp/eva/backup.sh",
        )
        commit("/tmp/eva", EVA, "Check free space first")
        laptop.succeed(f"cd /tmp/eva && git push -q {url(eva)} bt-6-free-space")
        pr = ok("POST", f"/repos/{REPO}/pulls", {
            "title": "Check free space first",
            "body": "Stops before copying when the NAS is nearly full.\n\nCloses BT-6.",
            "head": "bt-6-free-space",
            "base": "main",
        }, token=eva)
        box.wait_until_succeeds(
            f"curl -fsS -H 'Authorization: token {git_admin}' http://127.0.0.1:3000/api/v1/tracker/issues/BT-6"
            " | jq -e '.state.name == \"In Review\"'",
            timeout=60,
        )
        ok("POST", f"/repos/{REPO}/pulls/{pr['number']}/merge", {"style": "merge"}, token=owner)
        box.wait_until_succeeds(
            f"curl -fsS -H 'Authorization: token {git_admin}' http://127.0.0.1:3000/api/v1/tracker/issues/BT-6"
            " | jq -e '.state.category == \"done\"'",
            timeout=60,
        )
        six = issue("BT-6", owner)
        assert any(l["kind"] == "pull" and l["closes"] for l in six["links"]), six

    def issue_shots(phase, folder):
        try:
            laptop.succeed(f"issue-shots {BASE} matus '{PASSWORD}' /tmp/shots/{folder} {phase} --test losos-git-issues")
        except Exception:
            laptop.copy_from_machine(f"/tmp/shots/{folder}", "failed")
            raise

    with subtest("the pictures: the same repository before the tracker"):
        old_ui = os.environ.get("LOSOS_ISSUES_OLD_UI", "")
        if old_ui:
            ui = "${ui}"
            box.succeed(f"mount --bind {old_ui} {ui}", "systemctl restart nginx")
            box.wait_for_open_port(443)
            issue_shots("before", "before")
            box.succeed(f"umount {ui}", "systemctl restart nginx")
            box.wait_for_open_port(443)
            laptop.copy_from_machine("/tmp/shots/before", "shots")

    with subtest("the owner works through the pages"):
        issue_shots("tracker", "after")
        laptop.copy_from_machine("/tmp/shots/after", "shots")
        # What the pages did, as LosOS Git has it.
        six = issue("BT-7", owner)
        assert six["state"]["name"] == "In Progress", six
        assert issue("BT-8", owner)["state"]["name"] == "In Progress"
        made = [i for i in ok("GET", "/tracker/issues?project=BT", token=owner)["issues"]
                if i["title"] == "Rotate the backup key every year"]
        assert len(made) == 1, made
        assert made[0]["state"]["category"] in ("unstarted", "started"), made

    with subtest("the handbook's pictures"):
        bt7 = {"selector": '[data-issue-key="BT-7"]'}
        handbook_pictures(laptop, BASE, "matus", PASSWORD, [
            {"name": "git-issues", "path": "/git/issues/p/BT", "waitFor": bt7, "settle": 1500},
            {"name": "git-issue", "path": "/git/issues/p/BT", "waitFor": bt7,
             "steps": [{"click": bt7}, {"waitFor": {"selector": "[data-timeline]"}}],
             "settle": 1500},
            {"name": "git-issues-board", "path": "/git/issues/p/BT?layout=board",
             "waitFor": {"selector": "[data-board]"}, "settle": 1500},
        ])
  '';
}
