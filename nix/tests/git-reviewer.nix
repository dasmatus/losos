# nixos-test-vms config for LosOS Reviewer: the box's code model reviewing
# pull requests in LosOS Git (backend/src/forge/reviewer/, modules/git.nix).
#
# One box with LosOS Git as the install configuration wires it, Build with
# AI on, and Ollama replaced by a stand-in that replays what Qwen2.5-Coder
# 1.5B answered to the same questions about the same pull request
# (git-reviewer/ollama-stand-in.py): the VM cannot download the model, and
# the model under TCG would take hours. What is under test is everything
# around the model:
#
#   * the setting is off until a repository administrator turns it on;
#   * a pull request is queued, reviewed file by file with progress on the
#     head commit, and answered with one review by losos-reviewer: an
#     overview, a table of the files, and comments on the lines the box's
#     checks flagged, with a suggested line where the fix is one line;
#   * people reply to and resolve its comments, and ask it again;
#   * a push is reviewed from the last reviewed head;
#   * every question goes to loopback with the builder's context size, and
#     the service cannot reach any address but loopback;
#   * turned off, it reviews nothing and cannot be asked.
{ pkgs }:

let
  lososPkgs = import ../packages.nix { inherit pkgs; };
  model = "qwen2.5-coder:1.5b";
  context = 12288;

  stubUi = pkgs.runCommand "stub-admin-ui" { } ''
    mkdir -p $out
    echo "admin ui" > $out/index.html
  '';

  # Qwen2.5-Coder 1.5B's answers, recorded with `losos-git review-diff`
  # on this pull request (2026-10-21). It found no bug of its own here; the
  # two comments come from the box's checks.
  answers = pkgs.writeText "answers.json" (
    builtins.toJSON [
      {
        kind = "find";
        path = "BACKUP.md";
        answer = "LGTM";
      }
      {
        kind = "describe";
        path = "BACKUP.md";
        answer = "Adds a scheduled task to run the backup script every night.";
      }
      {
        kind = "find";
        path = "backup.py";
        answer = "LGTM";
      }
      {
        kind = "describe";
        path = "backup.py";
        answer = "Adds a function `backup` to the `backup.py` file that copies a source directory to a target directory, keeping the newest copies.";
      }
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
      {
        kind = "overview";
        answer = "This pull request adds a scheduled task to run a backup script every night, a function to copy a source directory to a target directory, and a script to prune old data directories in the target folder.";
      }
    ]
  );

  # The pull request: three new files.
  backupPy = pkgs.writeText "backup.py" ''
    import shutil


    def backup(source, target, keep=[]):
        """Copy source to target, keeping the newest copies."""
        keep.append(target)
        copied = shutil.copytree(source, target + ".new")
        if copied:
            shutil.rmtree(target)
        return keep
  '';
  pruneSh = pkgs.writeText "prune.sh" ''
    #!/bin/sh
    set -eu
    TARGET="$1"
    KEEP="''${2:-7}"
    for d in $(ls -1 "$TARGET" | sort | head -n -"$KEEP"); do
      rm -rf $TARGET/$d
    done
  '';
in
pkgs.testers.nixosTest {
  name = "losos-git-reviewer";

  nodes.box = _: {
    imports = [
      ../../modules/options.nix
      ../../modules/containers.nix
      ../../modules/daemon.nix
      ../../modules/git.nix
      ../../modules/config-repo.nix
    ];
    losos = {
      hostName = "mattbox";
      backend.package = lososPkgs.losos-ctl;
      admin.ui = stubUi;
      git.enable = true;
      identity.enable = false;
      # What modules/git.nix reads to find the model. modules/builder.nix
      # (the real Ollama) is not imported: the stand-in takes its port.
      builder = {
        enable = true;
        inherit model;
        contextTokens = context;
      };
    };
    systemd.services.ollama-stand-in = {
      wantedBy = [ "multi-user.target" ];
      before = [ "losos-git.service" ];
      serviceConfig.ExecStart = "${pkgs.python3}/bin/python3 ${./git-reviewer/ollama-stand-in.py} 11434 ${model} ${answers} 45";
    };
    environment.systemPackages = [
      pkgs.gitMinimal
      pkgs.jq
    ];
    environment.variables.GIT_TERMINAL_PROMPT = "0";
    virtualisation = {
      memorySize = 1536;
      cores = 2;
    };
  };

  testScript = ''
    import json

    start_all()
    box.wait_for_unit("losos-git.service")
    box.wait_for_unit("ollama-stand-in.service")
    box.wait_for_open_port(3000)
    box.wait_for_open_port(11434)

    git_admin = box.succeed("cat /var/lib/losos-git/admin-token").strip()

    def forge(method, path, body=None, token=None):
        args = f"-s -o /tmp/forge -w '%{{http_code}}' -X {method}"
        args += f" -H 'Authorization: token {token or git_admin}'"
        if body is not None:
            box.succeed(f"cat > /tmp/body <<'EOF'\n{json.dumps(body)}\nEOF")
            args += " -H 'Content-Type: application/json' --data-binary @/tmp/body"
        code = int(box.succeed(f"curl {args} http://127.0.0.1:3000/api/v1{path}"))
        out = box.succeed("cat /tmp/forge")
        try:
            return code, json.loads(out)
        except ValueError:
            return code, out

    def account(login, admin):
        code, body = forge("PUT", f"/admin/users/{login}",
                           {"full_name": login.title(), "email": f"{login}@example.org", "admin": admin})
        assert code == 200, (code, body)
        code, body = forge("POST", f"/admin/users/{login}/tokens",
                           {"name": "laptop", "scopes": ["all"], "replace": True})
        assert code == 201, (code, body)
        return body["token"]

    matus = account("matus", True)
    jana = account("jana", False)
    REPO = "/repos/matus/backup"

    def bot_reviews(n):
        code, all = forge("GET", f"{REPO}/pulls/{n}/reviews", token=jana)
        assert code == 200, (code, all)
        return [r for r in all if r["user"]["login"] == "losos-reviewer"]

    def wait_reviews(n, count):
        box.wait_until_succeeds(
            f"curl -fsS -H 'Authorization: token {jana}' "
            f"http://127.0.0.1:3000/api/v1{REPO}/pulls/{n}/reviews"
            f" | jq -e '[.[] | select(.user.login == \"losos-reviewer\")] | length >= {count}'",
            timeout=300,
        )
        return bot_reviews(n)

    with subtest("AI review is off until a repository administrator turns it on"):
        code, repo = forge("POST", "/user/repos",
                           {"name": "backup", "private": True, "auto_init": True}, token=matus)
        assert code == 201, (code, repo)
        assert repo["ai_review"] is False, repo
        code, _ = forge("PUT", f"{REPO}/collaborators/jana", {"permission": "write"}, token=matus)
        assert code in (200, 204), code
        code, _ = forge("PUT", f"{REPO}/ai-review", {"enabled": True}, token=jana)
        assert code == 403, code
        code, setting = forge("PUT", f"{REPO}/ai-review", {"enabled": True}, token=matus)
        assert code == 200, (code, setting)
        assert setting["enabled"] and setting["available"], setting
        assert setting["model"] == "${model}", setting

    with subtest("A pull request gets one review from LosOS Reviewer"):
        box.succeed(
            f"git clone -q http://jana:{jana}@127.0.0.1:3000/matus/backup.git /tmp/work"
            " && cd /tmp/work && git checkout -q -b nightly"
            " && cp ${backupPy} backup.py && cp ${pruneSh} prune.sh"
            " && echo 'Runs every night.' > BACKUP.md"
            " && git add . && git -c user.name=Jana -c user.email=jana@x commit -qm 'Back up the data folder'"
            " && git push -q -u origin nightly"
        )
        code, pr = forge("POST", f"{REPO}/pulls",
                         {"title": "Back up the data folder", "head": "nightly", "base": "main"},
                         token=jana)
        assert code == 201, (code, pr)
        n = pr["number"]
        head = pr["head"]["sha"]

        # Progress shows while it works: the stand-in reads at the real
        # model's pace on two cores.
        box.wait_until_succeeds(
            f"curl -fsS -H 'Authorization: token {jana}' "
            f"http://127.0.0.1:3000/api/v1{REPO}/pulls/{n}/ai-review"
            " | jq -e '.state == \"reviewing\" or .state == \"done\"'",
            timeout=120,
        )

        review = wait_reviews(n, 1)[0]
        assert review["state"] == "commented", review
        body = review["body"]
        assert body.startswith("## Pull request overview"), body
        assert "adds a scheduled task to run a backup script every night" in body, body
        assert "looked at 3 of 3 changed files and left 2 comments" in body, body
        assert "| `prune.sh` | Adds a script to prune old data directories" in body, body
        assert "Reviewed on this box by ${model}. The code did not leave it." in body, body

        code, lines = forge("GET", f"{REPO}/pulls/{n}/comments", token=jana)
        at = [(c["path"], c["line"]) for c in lines]
        assert at == [("backup.py", 4), ("prune.sh", 6)], at
        assert all(c["user"]["login"] == "losos-reviewer" for c in lines), lines
        assert "Default to `None`" in lines[0]["body"], lines[0]
        assert '```suggestion\n  rm -rf "$TARGET/$d"\n```' in lines[1]["body"], lines[1]

        code, statuses = forge("GET", f"{REPO}/commits/{head}/statuses", token=jana)
        mine = [s for s in statuses["statuses"] if s["context"] == "LosOS Reviewer"]
        assert mine and mine[0]["state"] == "success", statuses
        assert mine[0]["description"] == "Left 2 comments", mine
        assert mine[0]["targetUrl"] == f"/git/matus/backup/pulls/{n}/files", mine
        code, progress = forge("GET", f"{REPO}/pulls/{n}/ai-review", token=jana)
        assert progress["state"] == "done" and progress["filesTotal"] == 3, progress

    with subtest("Every question stays on loopback with the builder's context size"):
        asked = [json.loads(l) for l in box.succeed("cat /tmp/ollama-requests.jsonl").splitlines()]
        assert len(asked) == 7, len(asked)
        assert all(a["model"] == "${model}" for a in asked), asked
        assert all(a["options"]["num_ctx"] == ${toString context} for a in asked), asked
        assert all(a["stream"] is False for a in asked)
        deny = box.succeed("systemctl show losos-git -p IPAddressDeny --value").strip()
        allow = box.succeed("systemctl show losos-git -p IPAddressAllow --value").strip()
        assert deny == "any" or set(deny.split()) == {"0.0.0.0/0", "::/0"}, deny
        assert "127.0.0.0/8" in allow, allow

    with subtest("People reply to its comments, resolve them, and ask it again"):
        first = lines[0]["id"]
        code, reply = forge("POST", f"{REPO}/pulls/{n}/comments",
                            {"in_reply_to": first, "body": "Fixed, thanks."}, token=jana)
        assert code == 201, (code, reply)
        code, resolved = forge("PATCH", f"{REPO}/pulls/comments/{first}", {"resolved": True}, token=jana)
        assert code == 200 and resolved["resolved"], (code, resolved)
        code, doc = forge("POST", f"{REPO}/pulls/{n}/requested_reviewers",
                          {"reviewers": ["losos-reviewer"]}, token=jana)
        assert code == 200, (code, doc)
        reviews = wait_reviews(n, 2)
        assert "since the last review" not in reviews[1]["body"], reviews[1]

    with subtest("A push is reviewed from the last reviewed head"):
        box.succeed(
            "cd /tmp/work && echo 'Runs every night at two.' > BACKUP.md"
            " && git -c user.name=Jana -c user.email=jana@x commit -qam 'Say when'"
            " && git push -q"
        )
        reviews = wait_reviews(n, 3)
        assert "These are the commits pushed since the last review." in reviews[2]["body"], reviews[2]
        assert "looked at 1 of 1 changed file" in reviews[2]["body"], reviews[2]

    with subtest("Turned off, it reviews nothing and cannot be asked"):
        code, _ = forge("PUT", f"{REPO}/ai-review", {"enabled": False}, token=matus)
        assert code == 200
        before = len(box.succeed("cat /tmp/ollama-requests.jsonl").splitlines())
        box.succeed(
            "cd /tmp/work && echo 'Runs every night at three.' > BACKUP.md"
            " && git -c user.name=Jana -c user.email=jana@x commit -qam 'Later'"
            " && git push -q"
        )
        box.sleep(5)
        assert len(box.succeed("cat /tmp/ollama-requests.jsonl").splitlines()) == before
        assert len(bot_reviews(n)) == 3
        code, doc = forge("POST", f"{REPO}/pulls/{n}/requested_reviewers",
                          {"reviewers": ["losos-reviewer"]}, token=jana)
        assert code == 422, (code, doc)
  '';
}
