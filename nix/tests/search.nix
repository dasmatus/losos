# nixos-test-vms config for the text search behind the admin palette
# (modules/search.nix, backend/src/textsearch/).
#
# The parts a unit test cannot reach: that the index unit can read the
# owner's files as `losos-cloud` through its sandbox and write its index, that
# a second pass picks up a changed file, that PDFs go through pdftotext, that
# the two socket-activated searches answer as their apps' users, and that the
# sockets are root's alone. lososd's relay is covered by
# backend/tests/text_search.rs.
{ pkgs }:

let
  lososPkgs = import ../packages.nix { inherit pkgs; };

  files = "/var/lib/losos-cloud/notshared/files";
  repos = "/var/lib/forgejo/repositories";

  # A one-page PDF with a known sentence in it.
  invoice = pkgs.runCommand "invoice.pdf" { } ''
    echo "Quarterly invoice for the allotment" \
      | ${pkgs.enscript}/bin/enscript -q -B -o - \
      | ${pkgs.ghostscript}/bin/ps2pdf - $out
  '';
in
pkgs.testers.nixosTest {
  name = "losos-search";

  nodes.machine =
    { pkgs, ... }:
    {
      imports = [
        ../../modules/options.nix
        ../../modules/search.nix
      ];
      losos.backend.package = lososPkgs.losos-ctl;
      losos.cloud.enable = true;
      losos.forgejo.enable = true;
      # The two app users, as modules/cloud.nix and
      # modules/configuration.nix declare them.
      users.groups.losos-cloud = { };
      users.users.losos-cloud = {
        group = "losos-cloud";
        isSystemUser = true;
      };
      users.groups.forgejo.gid = 1003;
      users.users.forgejo = {
        uid = 1003;
        group = "forgejo";
        isSystemUser = true;
      };
      environment.systemPackages = [
        pkgs.socat
        pkgs.gitMinimal
      ];
    };

  testScript = ''
    import json

    def ask(socket, q, user="root"):
        line = json.dumps({"q": q, "limit": 10})
        out = machine.succeed(
            f"echo '{line}' | runuser -u {user} -- socat -t 10 - UNIX-CONNECT:/run/losos-search/{socket}.sock"
        )
        return json.loads(out)

    def index():
        # A oneshot: start returns once the pass is over, and fails with it.
        machine.succeed("systemctl start losos-search-index.service")

    machine.wait_for_unit("sockets.target")

    with subtest("before the owner's folder exists, the index pass succeeds and finds nothing"):
        index()
        answer = ask("cloud", "tax")
        assert answer["hits"] == [], answer

    with subtest("the owner's files, as LosOS Cloud leaves them: 0700 and 0600, owned by losos-cloud"):
        machine.succeed(
            "install -d -m 0700 -o losos-cloud -g losos-cloud /var/lib/losos-cloud /var/lib/losos-cloud/notshared ${files} ${files}/Documents",
            "echo 'hidden-database-word' > /var/lib/losos-cloud/cloud.db",
            "echo 'The daňové priznanie and the tax return are due in March.' > ${files}/Documents/tax.txt",
            "install -m 0600 ${invoice} ${files}/Documents/invoice.pdf",
            "head -c 4096 /dev/urandom > ${files}/photo.jpg",
            "chown -R losos-cloud:losos-cloud /var/lib/losos-cloud",
            "chmod 0600 /var/lib/losos-cloud/cloud.db ${files}/Documents/tax.txt ${files}/Documents/invoice.pdf ${files}/photo.jpg",
        )

    with subtest("a pass reads them into the index, PDFs included"):
        index()
        answer = ask("cloud", "danove prizn")
        assert [h["path"] for h in answer["hits"]] == ["Documents/tax.txt"], answer
        assert answer["partial"] is False, answer
        answer = ask("cloud", "allotment")
        assert [h["path"] for h in answer["hits"]] == ["Documents/invoice.pdf"], answer
        # Only the owner's files are in reach, never LosOS Cloud's database.
        assert ask("cloud", "hidden-database-word")["hits"] == []

    with subtest("the index belongs to losos-cloud alone"):
        machine.succeed("test $(stat -c %U:%a /var/lib/losos-search) = losos-cloud:700")

    with subtest("a second pass picks up a changed file and a removed one"):
        machine.succeed(
            "echo 'Now about gooseberries.' > ${files}/Documents/tax.txt",
            "rm ${files}/Documents/invoice.pdf",
        )
        index()
        assert [h["path"] for h in ask("cloud", "gooseberr")["hits"]] == ["Documents/tax.txt"]
        assert ask("cloud", "march")["hits"] == []
        assert ask("cloud", "allotment")["hits"] == []
        last = machine.succeed("journalctl -u losos-search-index.service -o cat | grep 'index up to date' | tail -n 1")
        assert "read=1 " in last and "removed=1 " in last, last

    with subtest("Git: the default branch of each repository, as forgejo"):
        machine.succeed(
            "install -d -m 0750 -o forgejo -g forgejo /var/lib/forgejo ${repos} ${repos}/matus",
            "mkdir -p /tmp/work && cd /tmp/work && git init -q -b main && echo 'def tax_due(x):' > taxes.py"
            " && git add . && git -c user.name=t -c user.email=t@t commit -qm first",
            "git clone -q --bare /tmp/work ${repos}/matus/accounts.git",
            "chown -R forgejo:forgejo /var/lib/forgejo",
        )
        answer = ask("git", "TAX_due")
        hits = answer["hits"]
        assert len(hits) == 1, answer
        assert (hits[0]["owner"], hits[0]["repo"], hits[0]["branch"], hits[0]["path"], hits[0]["line"]) == (
            "matus", "accounts", "main", "taxes.py", 1
        ), hits

    with subtest("the sockets are root's alone"):
        for socket in ["cloud", "git"]:
            machine.succeed(f"test $(stat -c %U:%a /run/losos-search/{socket}.sock) = root:600")
        machine.fail("echo '{\"q\":\"tax\",\"limit\":1}' | runuser -u losos-cloud -- socat -t 5 - UNIX-CONNECT:/run/losos-search/cloud.sock")

    with subtest("lososd is told where both searches are"):
        env = machine.succeed("systemctl show -p Environment --value lososd.service")
        assert "LOSOS_SEARCH_CLOUD_SOCKET=/run/losos-search/cloud.sock" in env, env
        assert "LOSOS_SEARCH_GIT_SOCKET=/run/losos-search/git.sock" in env, env
  '';
}
