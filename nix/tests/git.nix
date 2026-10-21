# nixos-test-vms config for LosOS Git (modules/git.nix, backend/src/forge/).
#
# The parts the Rust tests (backend/tests/forge.rs) cannot reach, on a box
# with lososd, nginx's front vhost and LosOS Git as the install
# configuration wires them, and a laptop on the LAN running plain git:
#
#   * a box that ran Forgejo keeps its repositories: they are moved into
#     LosOS Git's state directory before it starts, owned by its user, with
#     Forgejo's hooks taken out, and served;
#   * lososd publishes /etc/nixos to the owner's private configuration
#     repository, and a commit pushed there from a clone comes back to the
#     box through the same gates an Apply goes through, or is refused;
#   * clone and push through nginx's /git/ with a token, a large push
#     streamed, and a token that is not the box's, someone else's private
#     repository and a push to a repository that does not exist all turned
#     away;
#   * private, internal and public repositories read for exactly their
#     people, through the API, the pages and git, and push for none else;
#   * the headers that name a person count only on the socket nginx opens.
{ pkgs }:

let
  lososPkgs = import ../packages.nix { inherit pkgs; };
  git = "${pkgs.gitMinimal}/bin/git";

  # The admin pages are not under test; one file stands in.
  stubUi = pkgs.runCommand "stub-admin-ui" { } ''
    mkdir -p $out
    echo "admin ui" > $out/index.html
  '';

  # What a box that ran Forgejo has on disk: one repository with a commit,
  # Forgejo's hooks in it (they call a Forgejo binary that is gone), and
  # the owner Forgejo ran as.
  forgejoLeftovers = pkgs.writeShellScript "forgejo-leftovers" ''
    set -eu
    [ -e /var/lib/losos-git ] && exit 0
    work=$(mktemp -d)
    ${git} -C "$work" init -q -b main
    echo "# Notes" > "$work/README.md"
    ${git} -C "$work" add README.md
    ${git} -C "$work" -c user.name=Matus -c user.email=m@x commit -qm "First notes"
    install -d /var/lib/forgejo/repositories/matus
    ${git} clone -q --bare "$work" /var/lib/forgejo/repositories/matus/notes.git
    hooks=/var/lib/forgejo/repositories/matus/notes.git/hooks
    for h in pre-receive update post-receive; do
      printf '#!/bin/sh\nexec /nonexistent/forgejo hook %s\n' "$h" > "$hooks/$h"
      chmod +x "$hooks/$h"
    done
    install -d "$hooks/pre-receive.d"
    printf '#!/bin/sh\nexit 1\n' > "$hooks/pre-receive.d/gitea"
    chmod +x "$hooks/pre-receive.d/gitea"
    echo sample > "$hooks/update.sample"
    chown -R 1003:1003 /var/lib/forgejo
  '';

  # /etc/nixos as the installer leaves it: a git repository with the box's
  # settings file in it.
  configTree = pkgs.writeShellScript "config-tree" ''
    set -eu
    [ -e /etc/nixos/.git ] && exit 0
    install -d /etc/nixos/modules
    cp ${../../modules/overrides.nix} /etc/nixos/modules/overrides.nix
    chmod 0644 /etc/nixos/modules/overrides.nix
    ${git} -C /etc/nixos init -q -b main
    ${git} -C /etc/nixos add -A
    ${git} -C /etc/nixos -c user.name=installer -c user.email=i@x commit -qm "Installed"
  '';
in
pkgs.testers.nixosTest {
  name = "losos-git";

  nodes = {
    box = _: {
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
        # No LosOS Login here (nix/tests/identity.nix boots it): nginx then
        # clears the sign-in headers and every call carries a token.
        identity.enable = false;
      };
      systemd.services.forgejo-leftovers = {
        wantedBy = [ "losos-git.service" ];
        before = [
          "losos-git-forgejo-import.service"
          "losos-git.service"
        ];
        serviceConfig = {
          Type = "oneshot";
          ExecStart = forgejoLeftovers;
        };
      };
      systemd.services.config-tree = {
        wantedBy = [ "lososd.service" ];
        before = [ "lososd.service" ];
        serviceConfig = {
          Type = "oneshot";
          ExecStart = configTree;
        };
      };
      environment.systemPackages = [
        pkgs.gitMinimal
        pkgs.jq
      ];
      virtualisation = {
        memorySize = 1536;
        cores = 2;
        vlans = [ 1 ];
      };
    };

    laptop = _: {
      environment.systemPackages = [
        pkgs.gitMinimal
        pkgs.curl
        pkgs.jq
      ];
      # A refused clone fails instead of asking for a password.
      environment.variables.GIT_TERMINAL_PROMPT = "0";
      virtualisation.vlans = [ 1 ];
    };
  };

  testScript = ''
    import json

    BOX = "192.168.1.1"
    REPOS = "/var/lib/losos-git/repositories"

    start_all()
    box.wait_for_unit("losos-git.service")
    box.wait_for_unit("lososd.service")
    box.wait_for_unit("nginx.service")
    box.wait_for_open_port(3000)
    box.wait_until_succeeds("curl -fsS 127.0.0.1:8082/api/health")
    laptop.wait_for_unit("multi-user.target")

    git_admin = box.succeed("cat /var/lib/losos-git/admin-token").strip()
    losos_admin = box.succeed("cat /var/secrets/losos-admin-token").strip()

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

    def lososd(method, path):
        out = box.succeed(
            f"curl -fsS -X {method} -H 'Authorization: Bearer {losos_admin}' 127.0.0.1:8082{path}"
        )
        return json.loads(out)

    def token_for(login, name):
        code, body = forge("POST", f"/admin/users/{login}/tokens",
                           {"name": name, "scopes": ["write:repository", "read:user"], "replace": True})
        assert code == 201, (code, body)
        return body["token"]

    def url(token, path):
        return f"http://x:{token}@{BOX}/git/{path}"

    def rebuild_done():
        box.wait_until_succeeds(
            f"curl -fsS -H 'Authorization: Bearer {losos_admin}' 127.0.0.1:8082/api/status"
            " | jq -e '.state != \"building\"'",
            timeout=300,
        )

    with subtest("Forgejo's repositories move into LosOS Git, without its hooks"):
        box.fail("test -e /var/lib/forgejo/repositories")
        box.succeed(f"test $(stat -c %U {REPOS}/matus/notes.git/HEAD) = losos-git")
        hooks = box.succeed(f"ls -A {REPOS}/matus/notes.git/hooks").split()
        # git's own samples stay; Forgejo's scripts and its .d folders go.
        assert "update.sample" in hooks, hooks
        assert all(h.endswith(".sample") for h in hooks), hooks
        code, repo = forge("GET", "/repos/matus/notes")
        assert code == 200 and repo["default_branch"] == "main", (code, repo)
        # And it is served: the owner clones it through nginx.
        matus = token_for("matus", "laptop")
        laptop.succeed(f"git clone -q {url(matus, 'matus/notes.git')} /tmp/notes")
        assert laptop.succeed("cat /tmp/notes/README.md").strip() == "# Notes"
        # A push works too: none of Forgejo's hooks is left to refuse it.
        laptop.succeed(
            "cd /tmp/notes && echo more >> README.md"
            " && git -c user.name=Matus -c user.email=m@x commit -qam 'More notes'"
            " && git push -q origin main"
        )
        assert box.succeed(f"git -c safe.directory='*' -C {REPOS}/matus/notes.git log -1 --format=%s main").strip() == "More notes"

    with subtest("lososd publishes /etc/nixos to the owner's private repository"):
        doc = lososd("POST", "/api/config/sync")
        assert doc["sync"]["state"] == "ok", doc["sync"]
        assert doc["repository"]["viaServer"] is True, doc["repository"]
        code, repo = forge("GET", "/repos/notshared/losos-config")
        assert code == 200 and repo["private"] is True, (code, repo)
        local = box.succeed("git -C /etc/nixos rev-parse HEAD").strip()
        remote = box.succeed(f"git -c safe.directory='*' -C {REPOS}/notshared/losos-config.git rev-parse main").strip()
        assert local == remote, (local, remote)
        # A second pass has nothing to do.
        assert lososd("POST", "/api/config/sync")["sync"]["detail"] == "LosOS Git has this box's configuration."

    owner = token_for("notshared", "laptop")

    with subtest("a commit pushed from a clone comes back to the box"):
        laptop.succeed(
            f"git clone -q {url(owner, 'notshared/losos-config.git')} /tmp/config",
            "cd /tmp/config && sed -i 's/losos.hostName = \"mattbox\";/losos.hostName = \"otherbox\";/'"
            " modules/overrides.nix && grep -q otherbox modules/overrides.nix",
            "cd /tmp/config && git -c user.name=Matus -c user.email=m@x commit -qam 'Rename the box'"
            " && git push -q origin main",
        )
        pushed = laptop.succeed("git -C /tmp/config rev-parse HEAD").strip()
        # lososd's own 30 s pass may take the push before this call does; then
        # this pass finds the box in step. Either way the box ends on the push.
        doc = lososd("POST", "/api/config/sync")
        assert doc["sync"]["state"] == "ok", doc["sync"]
        assert doc["sync"]["remoteHead"] == pushed, doc["sync"]
        assert box.succeed("git -C /etc/nixos rev-parse HEAD").strip() == pushed
        box.wait_until_succeeds("journalctl -b -u 'losos-rebuild-*' | grep -q Started", timeout=60)
        box.succeed("grep -q 'losos.hostName = \"otherbox\";' /etc/nixos/modules/overrides.nix")
        # There is no flake in this /etc/nixos, so the rebuild it queued
        # fails; that it was tried is what matters here.
        rebuild_done()

    with subtest("a pushed setting the box does not declare is refused"):
        laptop.succeed(
            "cd /tmp/config && sed -i 's/^}$/  losos.noSuch.option = true;\\n}/' modules/overrides.nix"
            " && git -c user.name=Matus -c user.email=m@x commit -qam 'A typo'"
            " && git push -q origin main"
        )
        before = box.succeed("git -C /etc/nixos rev-parse HEAD").strip()
        doc = lososd("POST", "/api/config/sync")
        assert doc["sync"]["state"] == "refused", doc["sync"]
        assert "noSuch.option" in doc["sync"]["detail"], doc["sync"]
        assert box.succeed("git -C /etc/nixos rev-parse HEAD").strip() == before

    with subtest("a large push streams through nginx"):
        code, repo = forge("POST", "/user/repos", {"name": "photos", "private": True}, token=owner)
        assert code == 201, (code, repo)
        laptop.succeed(
            "mkdir /tmp/photos && cd /tmp/photos && git init -q -b main"
            " && head -c 48M /dev/urandom > big.bin && git add big.bin"
            " && git -c user.name=Matus -c user.email=m@x commit -qm 'A big file'",
            f"cd /tmp/photos && git push -q {url(owner, 'notshared/photos.git')} main",
        )
        laptop.succeed(f"git clone -q {url(owner, 'notshared/photos.git')} /tmp/photos2")
        laptop.succeed("cmp /tmp/photos/big.bin /tmp/photos2/big.bin")

    with subtest("tokens, private repositories and missing ones are turned away"):
        # No token, and a token this box never made.
        laptop.fail(f"git ls-remote http://{BOX}/git/notshared/losos-config.git 2>/dev/null")
        laptop.fail(f"git ls-remote {url('0' * 40, 'notshared/losos-config.git')}")
        # Someone else's private repository, read or written.
        laptop.fail(f"git ls-remote {url(matus, 'notshared/losos-config.git')}")
        laptop.fail(f"cd /tmp/notes && git push -q {url(matus, 'notshared/photos.git')} main")
        # A push to a repository that does not exist makes none.
        laptop.fail(f"cd /tmp/notes && git push -q {url(owner, 'notshared/nothing.git')} main")
        assert forge("GET", "/repos/notshared/nothing")[0] == 404

    with subtest("each visibility reads for its people, and only its people push"):
        names = ("closed", "ours", "open")
        for name, vis in zip(names, ("private", "internal", "public")):
            code, repo = forge("POST", "/user/repos",
                               {"name": name, "visibility": vis, "auto_init": True}, token=owner)
            assert code == 201 and repo["visibility"] == vis, (code, repo)
            assert repo["private"] == (vis != "public"), repo

        def reads(token, name):
            where = url(token, f"notshared/{name}.git") if token else f"http://{BOX}/git/notshared/{name}.git"
            return laptop.execute(f"git ls-remote {where} >/dev/null 2>&1")[0] == 0

        # Nobody signed in; another account on the box; the owner.
        assert [reads(None, n) for n in names] == [False, False, True]
        assert [reads(matus, n) for n in names] == [False, True, True]
        assert [reads(owner, n) for n in names] == [True, True, True]
        # Reading is all the others get: a push from a clone is refused.
        laptop.succeed(f"git clone -q {url(matus, 'notshared/open.git')} /tmp/open")
        laptop.fail(
            "cd /tmp/open && echo hi > hi.txt && git add hi.txt"
            " && git -c user.name=Matus -c user.email=m@x commit -qm hi && git push -q origin main"
        )
        # The API agrees: a visitor's list holds the public one only, an
        # account's list the two the box shares and nothing private.
        found = json.loads(laptop.succeed(f"curl -fsS http://{BOX}/git/api/v1/repos/search"))
        assert [r["full_name"] for r in found["data"]] == ["notshared/open"], found
        code, mine = forge("GET", "/user/repos", token=matus)
        theirs = sorted(r["name"] for r in mine if r["owner"]["login"] == "notshared")
        assert theirs == ["open", "ours"], mine
        # A visitor reads the public one's code, and of the others not even
        # that they exist (nix/tests/git-pages.nix has the pages).
        laptop.succeed(f"curl -fsS http://{BOX}/git/api/v1/repos/notshared/open/readme | grep -q open")
        code = laptop.succeed(
            f"curl -s -o /dev/null -w '%{{http_code}}' http://{BOX}/git/api/v1/repos/notshared/ours"
        )
        assert code == "404", code
        # Changed back: the public one closes to visitors at once.
        code, repo = forge("PATCH", "/repos/notshared/open", {"visibility": "private"}, token=owner)
        assert code == 200 and repo["visibility"] == "private", (code, repo)
        assert not reads(None, "open") and not reads(matus, "open")
        # Forgejo's flag still works, and the two saying different things is refused.
        code, repo = forge("PATCH", "/repos/notshared/open", {"private": False}, token=owner)
        assert code == 200 and repo["visibility"] == "public", (code, repo)
        code, why = forge("PATCH", "/repos/notshared/ours", {"visibility": "public", "private": True}, token=owner)
        assert code == 422, (code, why)

    with subtest("only nginx's socket believes the sign-in headers"):
        named = "-H 'X-Losos-Cloud-User: notshared'"
        # From the LAN, nginx clears them: nobody.
        code = laptop.succeed(
            f"curl -s -o /dev/null -w '%{{http_code}}' {named} http://{BOX}/git/api/v1/user"
        )
        assert code == "401", code
        # On the loopback port, whoever sends them: nobody.
        code = box.succeed(
            f"curl -s -o /dev/null -w '%{{http_code}}' {named} http://127.0.0.1:3000/api/v1/user"
        )
        assert code == "401", code
        # The socket opens for nginx's group and root, nobody else.
        sock = "--unix-socket /run/losos-git/http.sock http://git/api/v1/user"
        me = json.loads(box.succeed(f"runuser -u nginx -- curl -fsS {named} {sock}"))
        assert me["login"] == "notshared", me
        box.fail(f"runuser -u nobody -- curl -fsS {named} {sock}")
        box.succeed("test $(stat -c %a /run/losos-git) = 750")
  '';
}
