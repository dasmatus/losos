# A box that ran Forgejo switches to LosOS Git (modules/git.nix,
# backend/src/forge/import.rs).
#
# The first generation runs the real Forgejo on Postgres, as a
# container-mode box did, and the script fills it the way people do: two
# accounts, a private repository with a branch, a label, issues (one
# closed), a comment, an assignee, a file attached to an issue, a pull
# request and an access token. Then the box switches to a generation with
# Forgejo gone and Postgres gone too, and the test checks that:
#
#   * the import ran once, before LosOS Git, and says complete;
#   * the old token still clones, pushes and reads the API;
#   * every issue, pull request, comment, label, assignee and attachment is
#     there, with its author and state;
#   * what was not carried over is named in the report, and the full
#     database dump is on the box;
#   * nothing of Forgejo's was deleted, and a second run changes nothing.
{ pkgs }:

let
  lososPkgs = import ../packages.nix { inherit pkgs; };
  password = "Correct-Horse-Battery-9";
in

pkgs.testers.nixosTest {
  name = "losos-git-forgejo";

  nodes.box =
    { config, lib, ... }:
    {
      imports = [
        ../../modules/options.nix
        ../../modules/git.nix
      ];
      losos.backend.package = lososPkgs.losos-ctl;
      losos.git.enable = false;

      # The first generation: Forgejo on Postgres, with the uid it had.
      users.users.forgejo = lib.mkIf config.services.forgejo.enable { uid = 1003; };
      users.groups.forgejo = lib.mkIf config.services.forgejo.enable { gid = 1003; };
      services.forgejo = {
        enable = true;
        database.type = "postgres";
        settings = {
          server = {
            HTTP_ADDR = "127.0.0.1";
            HTTP_PORT = 3000;
            ROOT_URL = "http://127.0.0.1:3000/";
          };
          service.DISABLE_REGISTRATION = true;
        };
      };

      specialisation.losos-git.configuration = {
        services.forgejo.enable = lib.mkForce false;
        services.postgresql.enable = lib.mkForce false;
        losos.git.enable = lib.mkForce true;
      };

      environment.systemPackages = [
        pkgs.git
        pkgs.curl
        pkgs.jq
      ];
      virtualisation = {
        memorySize = 2048;
        cores = 2;
      };
    };

  testScript =
    { nodes, ... }:
    ''
      import json

      PASSWORD = "${password}"
      FORGEJO = "${pkgs.lib.getExe nodes.box.services.forgejo.package}"
      API = "http://127.0.0.1:3000/api/v1"

      def api(method, path, auth, data=None, extra=""):
          args = f"-sS -X {method} '{API}{path}' {auth} {extra}"
          if data is not None:
              args += f" -H 'Content-Type: application/json' -d '{json.dumps(data)}'"
          out = box.succeed(f"curl -w '\\n%{{http_code}}' {args}")
          body, code = out.rsplit("\n", 1)
          return int(code), (json.loads(body) if body.strip() else None)

      def ok(code_doc, want=(200, 201, 204)):
          code, doc = code_doc
          assert code in want, (code, doc)
          return doc

      start_all()
      box.wait_for_unit("forgejo.service")
      box.wait_for_open_port(3000)

      with subtest("Forgejo, used the way people use it"):
          for name, admin in (("matus", "--admin"), ("eva", "")):
              box.succeed(
                  f"runuser -u forgejo -- env GITEA_WORK_DIR=/var/lib/forgejo {FORGEJO}"
                  " admin user create"
                  f" --username {name} --password '{PASSWORD}' --email {name}@mattbox.local"
                  f" --must-change-password=false {admin}"
              )
          matus = f"-u matus:'{PASSWORD}'"
          eva = f"-u eva:'{PASSWORD}'"
          token = ok(api("POST", "/users/matus/tokens", matus,
                         {"name": "laptop", "scopes": ["write:repository", "write:issue", "read:user"]}))["sha1"]
          ok(api("POST", "/user/repos", matus,
                 {"name": "notes", "private": True, "auto_init": True, "default_branch": "main",
                  "readme": "Default"}))
          ok(api("PUT", "/repos/matus/notes/collaborators/eva", matus, {"permission": "write"}))
          label = ok(api("POST", "/repos/matus/notes/labels", matus,
                         {"name": "bug", "color": "#ee0701"}))["id"]
          ok(api("POST", "/repos/matus/notes/issues", matus,
                 {"title": "Fix the backup script", "body": "It stops at 3 a.m.",
                  "labels": [label], "assignees": ["eva"]}))
          ok(api("POST", "/repos/matus/notes/issues/1/comments", eva, {"body": "Seen it too."}))
          box.succeed("printf 'backup: stopped' > /tmp/backup.log")
          asset = ok(api("POST", "/repos/matus/notes/issues/1/assets", matus, None,
                         "-F attachment=@/tmp/backup.log"))
          box.succeed(
              f"git clone -q http://matus:{token}@127.0.0.1:3000/matus/notes.git /tmp/notes"
              " && cd /tmp/notes && git checkout -qb fast"
              " && git -c user.name=Matus -c user.email=matus@mattbox.local"
              "    commit -q --allow-empty -m 'Back up in parallel'"
              " && git push -q origin fast"
          )
          ok(api("POST", "/repos/matus/notes/pulls", matus,
                 {"head": "fast", "base": "main", "title": "Faster backups"}))
          ok(api("POST", "/repos/matus/notes/issues", eva, {"title": "Old idea"}))
          ok(api("PATCH", "/repos/matus/notes/issues/3", matus, {"state": "closed"}))
          issues = ok(api("GET", "/repos/matus/notes/issues?state=all&type=issues", matus))
          assert len(issues) == 2, issues

      with subtest("the switch: Forgejo and Postgres go, LosOS Git comes"):
          box.succeed(
              "${nodes.box.system.build.toplevel}/specialisation/losos-git/bin/switch-to-configuration test"
          )
          box.wait_for_unit("losos-git.service")
          box.wait_for_open_port(3000)
          assert box.succeed(
              "systemctl show -P Result losos-git-forgejo-import.service"
          ).strip() == "success"
          box.fail("systemctl is-active postgresql.service")

      report = json.loads(box.succeed("cat /var/lib/losos-git/forgejo/import.json"))
      print(json.dumps(report, indent=2))

      with subtest("the old token still works, for git and the API"):
          auth = f"-H 'Authorization: token {token}'"
          me = ok(api("GET", "/user", auth))
          assert me["login"] == "matus", me
          box.succeed(
              f"git clone -q http://matus:{token}@127.0.0.1:3000/matus/notes.git /tmp/again"
              " && cd /tmp/again && git rev-parse -q --verify origin/fast"
              " && echo more >> README.md && git -c user.name=Matus"
              "    -c user.email=matus@mattbox.local commit -qam 'After the move'"
              " && git push -q origin main"
          )

      with subtest("issues, pull requests, comments, labels, assignees, attachments"):
          one = ok(api("GET", "/repos/matus/notes/issues/1", auth))
          assert one["title"] == "Fix the backup script" and one["state"] == "open", one
          assert one["body"] == "It stops at 3 a.m.", one
          assert one["user"]["login"] == "matus", one
          assert [l["name"] for l in one["labels"]] == ["bug"], one
          assert [a["login"] for a in one["assignees"]] == ["eva"], one
          assert [a["name"] for a in one["assets"]] == ["backup.log"], one
          comments = ok(api("GET", "/repos/matus/notes/issues/1/comments", auth))
          assert [(c["user"]["login"], c["body"]) for c in comments] == [("eva", "Seen it too.")], comments
          pulls = ok(api("GET", "/repos/matus/notes/pulls?state=all", auth))
          assert [(p["number"], p["title"], p["pull_request"]["head"], p["pull_request"]["base"])
                  for p in pulls] == [(2, "Faster backups", "fast", "main")], pulls
          closed = ok(api("GET", "/repos/matus/notes/issues?state=closed", auth))
          assert [(i["number"], i["user"]["login"]) for i in closed] == [(3, "eva")], closed
          got = box.succeed(
              f"curl -sSf {auth} {API}/attachments/{asset['uuid']}"
          )
          assert got == "backup: stopped", got
          assert report["forgejo"]["issue"] == report["imported"]["issue"] == 3, report
          assert report["imported"]["access_token"] == 1, report

      with subtest("what did not come over is named, and kept"):
          assert report["complete"] is True, report
          assert any("timeline entries" in n for n in report["notImported"]), report
          box.succeed("grep -q 'Fix the backup script' /var/lib/losos-git/forgejo/forgejo.sql")
          box.succeed("test -d /var/lib/forgejo/data")
          box.succeed("ls /var/lib/postgresql/*/PG_VERSION")
          box.fail("ls -d /var/lib/losos-git/.postgres.*")

      with subtest("a second import changes nothing"):
          # Once complete it does nothing; without the report it runs again.
          box.succeed("rm /var/lib/losos-git/forgejo/import.json")
          box.succeed("systemctl restart losos-git-forgejo-import.service losos-git.service")
          again = json.loads(box.succeed("cat /var/lib/losos-git/forgejo/import.json"))
          assert again["imported"] == report["imported"], again
          box.wait_for_open_port(3000)
          issues = ok(api("GET", "/repos/matus/notes/issues?state=all", auth))
          assert len(issues) == 3, issues
          comments = ok(api("GET", "/repos/matus/notes/issues/1/comments", auth))
          assert len(comments) == 1, comments
    '';
}
