# nixos-test-vms config for LosOS Git's pages (admin-ui/app/git/,
# admin-ui/app/src/git/) on a real box: the front vhost
# (modules/containers.nix, modules/tls.nix), LosOS Login
# (modules/identity.nix), lososd claiming the box, and LosOS Git
# (modules/git.nix) behind them.
#
# The laptop is two people's computer on the LAN. With LosOS Git tokens,
# git over HTTPS through nginx:
#
#   * the owner makes a repository and pushes main, and pushes a branch
#     from a worktree named "release-notes" on "laptop";
#   * Eva, a collaborator, pushes the branch fix-backup from a worktree she
#     names with push options, opens pull request #1 and asks the owner for
#     a review; a check reports on her commit.
#
# Then Chromium signs in as the owner through LosOS Login and walks the
# pages (nix/tests/git-shots.mjs): the repositories, the code, a file, the
# pull requests, the pull request, its files; a comment on a line kept for
# a review and the review sent with Approve. Eva answers on the line, and
# the owner resolves the thread, looks at the worktrees, squash-merges and
# deletes the branch. After each step the test reads LosOS Git's API to see
# that the page did what it showed.
#
# Then LosOS Reviewer: Eva opens #2, a pruning script, while the
# repository's AI review is off, and the pages are pictured; the owner turns
# it on and asks the bot, which reviews #2 with the stand-in for Ollama from
# nix/tests/git-reviewer.nix, and the same pages are pictured again. Then
# who may see a repository: the owner makes a public one and shares
# backup-tool with everyone on the box, and a visitor who has not signed
# in reads the public one (with LOSOS_GIT_OLD_UI, the same steps on the
# pages before first). Last, the same addresses on the box without the
# pages, for the pull request's "before" column.
{ pkgs }:

let
  inherit (pkgs) lib;
  lososPkgs = import ../packages.nix { inherit pkgs; };

  # LosOS Reviewer's model: Ollama replaced by the stand-in that replays
  # what Qwen2.5-Coder 1.5B answered about the same prune.sh
  # (git-reviewer/ollama-stand-in.py, recorded 2026-10-21). The comment
  # comes from the box's own check; the model only describes the file.
  model = "qwen2.5-coder:1.5b";
  answers = pkgs.writeText "answers.json" (
    builtins.toJSON [
      {
        kind = "find";
        path = "prune.sh";
        answer = "LGTM";
      }
      {
        kind = "describe";
        path = "prune.sh";
        answer = "Adds a script to prune old data directories in the target folder.";
      }
    ]
  );
  pruneSh = pkgs.writeText "prune.sh" ''
    #!/bin/sh
    set -eu
    TARGET="$1"
    KEEP="''${2:-7}"
    for d in $(ls -1 "$TARGET" | sort | head -n -"$KEEP"); do
      rm -rf $TARGET/$d
    done
  '';

  # Playwright from nixpkgs' playwright-driver (playwright-core), which the
  # binary cache carries, under the Node it is built for.
  shots = pkgs.writeShellScriptBin "git-shots" ''
    export PLAYWRIGHT_CORE=${pkgs.playwright-driver}/index.mjs
    export PLAYWRIGHT_BROWSERS_PATH=${
      pkgs.playwright-driver.browsers.override {
        withFirefox = false;
        withWebkit = false;
      }
    }
    export PLAYWRIGHT_SKIP_VALIDATE_HOST_REQUIREMENTS=true
    exec ${pkgs.bun}/bin/bun ${./git-shots.mjs} "$@"
  '';
  screens = import ./screens { inherit pkgs; };
in

pkgs.testers.nixosTest {
  name = "losos-git-pages";

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
        admin.ui = lososPkgs.losos-admin-ui;
        git.enable = true;
        mdns.enable = true;
        # What modules/git.nix reads to find the model; modules/builder.nix
        # (the real Ollama) is not imported, the stand-in takes its port.
        builder = {
          enable = true;
          inherit model;
        };
      };
      systemd.services.ollama-stand-in = {
        wantedBy = [ "multi-user.target" ];
        before = [ "losos-git.service" ];
        serviceConfig.ExecStart = "${pkgs.python3}/bin/python3 ${./git-reviewer/ollama-stand-in.py} 11434 ${model} ${answers} 45";
      };
      # The same box before the pages, for the pictures' "before" column:
      # LosOS Git answered every /git/ address itself.
      specialisation.before.configuration.services.nginx.virtualHosts."losos-front".locations."/git/" =
        lib.mkForce
          { proxyPass = "http://unix:/run/losos-git/http.sock:/"; };
      # Every page behind LosOS Login, as it was before repositories had
      # three levels: the "before" column of the visibility pictures.
      specialisation.signed-in-only.configuration.losos.git.site.requireSignIn = true;
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
      # A refused push fails instead of asking for a password.
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

    ${screens.handbookScript}

    start_all()
    box.wait_for_unit("nginx.service")
    box.wait_for_unit("lososd.service")
    box.wait_for_unit("losos-git.service")
    box.wait_for_open_port(3000)

    cert = box.succeed("cat /var/lib/losos-tls/cert.pem")
    laptop.succeed(f"cat > {CA} <<'EOF'\n{cert}EOF")
    laptop.succeed(f"git config --system http.sslCAInfo {CA}")
    git_admin = box.succeed("cat /var/lib/losos-git/admin-token").strip()

    def curl(args):
        return laptop.succeed(f"curl -s --cacert {CA} {args}")

    def forge(method, path, body=None, token=None):
        """LosOS Git's API on its loopback port: (status, JSON or text)."""
        args = f"-s -o /tmp/forge -w '%{{http_code}}' -X {method}"
        args += f" -H 'Authorization: token {token or git_admin}'"
        if body is not None:
            args += f" -H 'Content-Type: application/json' -d '{json.dumps(body)}'"
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

    MATUS = ("Matus", "matus@mattbox.local")
    EVA = ("Eva", "eva@mattbox.local")

    with subtest("the laptop finds the box by name"):
        laptop.wait_until_succeeds(f"getent hosts {HOST}", timeout=120)

    with subtest("the owner claims the box"):
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

    with subtest("two people, a repository and its main branch"):
        ok("PUT", "/admin/users/notshared", {"full_name": "matus", "email": MATUS[1]})
        ok("PUT", "/admin/users/eva", {"full_name": "Eva", "email": EVA[1]})
        owner = token_for("notshared")
        eva = token_for("eva")
        ok("POST", "/user/repos", {
            "name": "backup-tool",
            "description": "Copies the photos to the NAS every night",
            "private": True,
        }, token=owner)
        ok("PUT", f"/repos/{REPO}/collaborators/eva", {"permission": "write"}, token=owner)
        laptop.succeed(
            "mkdir -p /tmp/main && cd /tmp/main && git init -q -b main",
            "cat > /tmp/main/README.md <<'EOF'\n"
            "# Backup tool\n\n"
            "Copies the photos from this box to the NAS in the hall, every night at two.\n\n"
            "## Run it\n\n"
            "```sh\n./backup.sh /srv/photos nas.local:/backup/photos\n```\n\n"
            "- Only changed files are copied.\n"
            "- A **failed copy** is logged in `/var/log/backup.log`.\n"
            "EOF",
            "cat > /tmp/main/backup.sh <<'EOF'\n"
            "#!/bin/sh\n"
            "# Copy a folder to the NAS, keeping what is already there.\n"
            "set -eu\n\n"
            "from=$1\n"
            "to=$2\n"
            "log=/var/log/backup.log\n\n"
            "echo \"$(date -Is) copying $from\" >> \"$log\"\n"
            "rsync -a --partial \"$from/\" \"$to/\"\n"
            "echo \"$(date -Is) done\" >> \"$log\"\n"
            "EOF",
            "chmod +x /tmp/main/backup.sh",
        )
        commit("/tmp/main", MATUS, "Copy the photos to the NAS")
        laptop.succeed(
            "mkdir -p /tmp/main/systemd && cat > /tmp/main/systemd/backup.timer <<'EOF'\n"
            "[Timer]\nOnCalendar=*-*-* 02:00\nPersistent=true\n\n"
            "[Install]\nWantedBy=timers.target\nEOF"
        )
        commit("/tmp/main", MATUS, "Run it every night at two")
        laptop.succeed(f"cd /tmp/main && git push -q {url(owner)} main")
        doc = ok("GET", f"/repos/{REPO}")
        assert doc["default_branch"] == "main", doc

    with subtest("the owner pushes from a named worktree"):
        laptop.succeed(
            "cd /tmp/main && git config extensions.worktreeConfig true"
            " && git worktree add -q ../release-notes -b release-notes",
            "cd /tmp/release-notes"
            " && git config --worktree push.pushOption worktree=release-notes"
            " && git config --worktree --add push.pushOption computer=laptop",
            "echo '# Changes' > /tmp/release-notes/CHANGES.md",
        )
        commit("/tmp/release-notes", MATUS, "Start the list of changes")
        laptop.succeed(f"cd /tmp/release-notes && git push -q {url(owner)} release-notes")

    with subtest("Eva pushes fix-backup from her worktree and opens a pull request"):
        laptop.succeed(
            f"git clone -q {url(eva)} /tmp/eva",
            "cd /tmp/eva && git config extensions.worktreeConfig true"
            " && git worktree add -q ../fix-backup -b fix-backup",
            "cd /tmp/fix-backup"
            " && git config --worktree push.pushOption worktree=fix-backup"
            " && git config --worktree --add push.pushOption computer=eva-laptop",
            "cd /tmp/fix-backup && sed -i"
            " -e 's|^rsync -a --partial .*|for try in 1 2 3; do\\n"
            "  rsync -a --partial \"$from/\" \"$to/\" \\&\\& break\\n"
            "  echo \"$(date -Is) try $try failed\" >> \"$log\"\\n"
            "  sleep 60\\n"
            "done|' backup.sh",
            "grep -q 'for try in 1 2 3' /tmp/fix-backup/backup.sh",
        )
        commit("/tmp/fix-backup", EVA, "Retry a failed copy three times")
        laptop.succeed(
            "cd /tmp/fix-backup && sed -i 's|^- A \\*\\*failed copy\\*\\* is logged|"
            "- A **failed copy** is tried three times and logged|' README.md"
        )
        commit("/tmp/fix-backup", EVA, "Say so in the README")
        laptop.succeed(f"cd /tmp/fix-backup && git push -q {url(eva)} fix-backup")
        head = laptop.succeed("git -C /tmp/fix-backup rev-parse HEAD").strip()
        pr = ok("POST", f"/repos/{REPO}/pulls", {
            "title": "Retry a failed copy",
            "body": "The NAS drops off the network at night now and then, and the copy\n"
                    "stopped at the first error. It now tries **three times**, a minute\n"
                    "apart, and logs each failed try.",
            "head": "fix-backup",
            "base": "main",
        }, token=eva)
        assert pr["number"] == 1, pr
        ok("POST", f"/repos/{REPO}/pulls/1/requested_reviewers", {"reviewers": ["notshared"]}, token=eva)
        for context, description in [("CI / build (push)", "Built in 41 s"), ("CI / test (push)", "12 tests passed")]:
            ok("POST", f"/repos/{REPO}/statuses/{head}",
               {"state": "success", "context": context, "description": description}, token=owner)
        trees = {w["name"]: w for w in ok("GET", f"/repos/{REPO}/worktrees", token=owner)}
        assert trees["fix-backup"]["computer"] == "eva-laptop", trees
        assert trees["fix-backup"]["branch"] == "fix-backup", trees
        assert trees["release-notes"]["computer"] == "laptop", trees

    def git_shots(phase, folder, *flags):
        # On a failure the window as it was (failed-<phase>.png) comes out too.
        try:
            laptop.succeed(f"git-shots {BASE} matus '{PASSWORD}' /tmp/shots/{folder} {phase} {' '.join(flags)}")
        except Exception:
            laptop.copy_from_machine(f"/tmp/shots/{folder}", "failed")
            raise

    with subtest("the owner reads the code and reviews the pull request"):
        git_shots("browse", "after", "--test", "losos-git-pages")
        reviews = ok("GET", f"/repos/{REPO}/pulls/1/reviews")
        assert [r["state"] for r in reviews] == ["approved"], reviews
        assert reviews[0]["user"]["login"] == "notshared", reviews
        notes = ok("GET", f"/repos/{REPO}/pulls/1/comments")
        assert len(notes) == 1 and notes[0]["path"] == "backup.sh", notes
        assert "log line" in notes[0]["body"], notes

    with subtest("Eva answers on the line; the owner resolves it and merges"):
        ok("POST", f"/repos/{REPO}/pulls/1/comments",
           {"in_reply_to": notes[0]["id"], "body": "Logs it now: each failed try gets its own line."},
           token=eva)
        git_shots("finish", "after", "--test", "losos-git-pages")
        notes = ok("GET", f"/repos/{REPO}/pulls/1/comments")
        assert notes[0]["resolved"] is True, notes
        pr = ok("GET", f"/repos/{REPO}/pulls/1")
        assert pr["merged"] is True, pr
        branches = [b["name"] for b in ok("GET", f"/repos/{REPO}/branches")]
        assert "fix-backup" not in branches and "release-notes" in branches, branches
        # Squashed: one commit on main over the owner's two, with Eva's change.
        laptop.succeed(f"cd /tmp/main && git pull -q {url(owner)} main")
        assert laptop.succeed("git -C /tmp/main rev-list --count main").strip() == "3"
        laptop.succeed("grep -q 'for try in 1 2 3' /tmp/main/backup.sh")
        laptop.copy_from_machine("/tmp/shots/after", "shots")

    with subtest("LosOS Reviewer: pull request #2 before and after AI review is on"):
        laptop.succeed(
            "cd /tmp/eva && git fetch -q origin && git checkout -q -b prune-copies origin/main",
            "cp ${pruneSh} /tmp/eva/prune.sh && chmod +x /tmp/eva/prune.sh",
        )
        commit("/tmp/eva", EVA, "Prune old copies on the NAS")
        laptop.succeed(f"cd /tmp/eva && git push -q {url(eva)} prune-copies")
        pr = ok("POST", f"/repos/{REPO}/pulls", {
            "title": "Prune old copies",
            "body": "Keeps the newest seven copies on the NAS and deletes the rest.",
            "head": "prune-copies",
            "base": "main",
        }, token=eva)
        assert pr["number"] == 2, pr
        setting = ok("GET", f"/repos/{REPO}/ai-review", token=owner)
        assert setting["enabled"] is False and setting["available"], setting
        assert setting["model"] == "${model}", setting
        git_shots("reviewer", "reviewer-before", "--test", "losos-git-pages")
        assert ok("GET", f"/repos/{REPO}/pulls/2/reviews") == []

        ok("PUT", f"/repos/{REPO}/ai-review", {"enabled": True}, token=owner)
        ok("POST", f"/repos/{REPO}/pulls/2/requested_reviewers", {"reviewers": ["losos-reviewer"]}, token=owner)
        box.wait_until_succeeds(
            f"curl -fsS -H 'Authorization: token {git_admin}'"
            f" http://127.0.0.1:3000/api/v1/repos/{REPO}/pulls/2/reviews"
            " | jq -e 'map(select(.user.login == \"losos-reviewer\" and .user.is_bot)) | length == 1'",
            timeout=300,
        )
        lines = ok("GET", f"/repos/{REPO}/pulls/2/comments")
        assert [(c["path"], c["line"]) for c in lines] == [("prune.sh", 6)], lines
        assert '```suggestion\n  rm -rf "$TARGET/$d"\n```' in lines[0]["body"], lines[0]
        git_shots("reviewer", "reviewer-after", "--reviewed", "--test", "losos-git-pages")
        laptop.copy_from_machine("/tmp/shots/reviewer-before", "shots")
        laptop.copy_from_machine("/tmp/shots/reviewer-after", "shots")

    with subtest("who may see a repository: private, this box, public"):
        # LOSOS_GIT_OLD_UI, a losos-admin-ui from before the three levels,
        # gives the pictures their "before" column: its pages over this
        # box, every page behind LosOS Login.
        old_ui = os.environ.get("LOSOS_GIT_OLD_UI", "")
        if old_ui:
            ui = "${lososPkgs.losos-admin-ui}"
            box.succeed(
                "/run/booted-system/specialisation/signed-in-only/bin/switch-to-configuration test",
                f"mount --bind {old_ui} {ui}",
                "systemctl restart nginx",
            )
            box.wait_for_open_port(443)
            git_shots("visibility", "visibility-before", "--old", "--test", "losos-git-pages")
            ok("DELETE", "/repos/notshared/website")
            box.succeed(
                f"umount {ui}",
                "/run/booted-system/bin/switch-to-configuration test",
                "systemctl restart nginx",
            )
            box.wait_for_open_port(443)
            box.wait_for_open_port(3000)
            laptop.copy_from_machine("/tmp/shots/visibility-before", "shots")
        git_shots("visibility", "visibility-after", "--test", "losos-git-pages")
        site = ok("GET", "/repos/notshared/website")
        assert site["visibility"] == "public" and site["private"] is False, site
        tool = ok("GET", f"/repos/{REPO}")
        assert tool["visibility"] == "internal" and tool["private"] is True, tool
        # What the visitor's pages read: the public one, and nothing else.
        found = json.loads(curl(f"https://{HOST}/git/api/v1/repos/search"))
        assert [r["full_name"] for r in found["data"]] == ["notshared/website"], found
        laptop.copy_from_machine("/tmp/shots/visibility-after", "shots")

    with subtest("the handbook's pictures"):
        page = f"/git/{REPO}"
        handbook_pictures(laptop, BASE, "matus", PASSWORD, [
            {"name": "git-repos", "path": "/git/", "waitFor": {"selector": "[data-repos]"}},
            {"name": "git-code", "path": page, "waitFor": {"selector": "[data-readme] h2"}},
            {"name": "git-pull", "path": f"{page}/pulls/1",
             "waitFor": {"selector": '[data-merge-box="merged"]'}},
            {"name": "git-worktrees", "path": f"{page}/worktrees",
             "waitFor": {"selector": '[data-worktree="release-notes"]'}},
            {"name": "git-settings", "path": f"{page}/settings",
             "waitFor": {"selector": "[data-visibility-field]"},
             "steps": [{"waitFor": {"selector": '[data-ai-review-setting="on"]'}}]},
            {"name": "git-ai-review", "path": f"{page}/pulls/2/files",
             "waitFor": {"selector": "[data-thread] [data-suggestion]"},
             "steps": [{"scroll": {"selector": "[data-thread]"}}]},
        ])

    with subtest("the pictures: the same addresses before the pages"):
        box.succeed("/run/current-system/specialisation/before/bin/switch-to-configuration test")
        git_shots("before", "before")
        laptop.copy_from_machine("/tmp/shots/before", "shots")
  '';
}
