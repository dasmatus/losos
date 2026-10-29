# nixos-test-vms config for Notes in LosOS Cloud and the knowledge bases
# published from them (backend/src/cloud/notes/, admin-ui/app/src/notes/,
# src/cloud/notes/, src/kb/, modules/cloud.nix), behind the real front
# vhost and LosOS Login, with lososd claiming the box.
#
# The box shares and is in a mesh, so it offers knowledge bases. The laptop
# is the owner's computer on the LAN: it copies a kitchen garden vault
# (admin-ui/app/tests/fixtures/vault/, with an .obsidian folder and a
# canvas, as Obsidian syncs one) into LosOS Cloud over WebDAV, and then
#
#   * the box lists the vault: every note with its tags, aliases and links,
#     finds the vault from a note deep inside it, and searches its text;
#   * Chromium opens Notes before there is any note, which makes the
#     Notes folder and offers the first note;
#   * Chromium opens it in Notes, reads a note with its embeds, callouts and
#     backlinks, draws its mind map and the vault's graph, types a link into
#     a note with the box saving it, searches, and publishes the folder as a
#     knowledge base from its right click (nix/tests/notes-shots.mjs);
#   * the site reads without signing in, from the LAN and through the
#     tunnel, leaves out the private note and anything outside the folder,
#     and ignores a made-up sign-in header; kept to people on the box, it
#     answers only a signed-in reader;
#   * with sharing off (the specialisation `offMesh`) the box offers no
#     knowledge base, refuses to publish one and serves none, and the
#     pictures' "before" column is taken there.
#
# nix/cluster.nix is not imported: `cluster.enable`
# is the gate's half, with no edge to join. The admin UI is built without
# LosOS Diagrams, as in nix/tests/mesh-collab.nix.
{ pkgs }:

let
  inherit (pkgs) lib;
  lososPkgs = import ../packages.nix { inherit pkgs; };

  noLab = import ./admin-ui-without-lab.nix;

  vault = ../../admin-ui/app/tests/fixtures/vault;

  # Obsidian's own files beside the notes: the settings folder that makes
  # the folder a vault, and a canvas. JSON, so made here and not committed.
  canvas = pkgs.writeText "garden-plan.canvas" (
    builtins.toJSON {
      nodes = [
        {
          id = "g";
          type = "group";
          label = "Summer";
          x = 0;
          y = 0;
          width = 900;
          height = 500;
        }
        {
          id = "a";
          type = "text";
          text = "## Beds\nRotate every year. See [[Compost]].";
          x = 20;
          y = 40;
          width = 300;
          height = 200;
        }
        {
          id = "b";
          type = "file";
          file = "Tomatoes.md";
          x = 400;
          y = 40;
          width = 300;
          height = 200;
        }
      ];
      edges = [
        {
          id = "e";
          fromNode = "a";
          toNode = "b";
        }
      ];
    }
  );

  # The vault into LosOS Cloud over WebDAV, as a file manager copies it.
  seed = pkgs.writeShellScriptBin "notes-seed" ''
    set -eu
    base=$1 user=$2 password=$3 ca=$4 folder=$5
    dav="$base/remote.php/webdav"
    enc() { ${pkgs.jq}/bin/jq -rn --arg s "$1" '$s | split("/") | map(@uri) | join("/")'; }
    put() { ${pkgs.curl}/bin/curl -sf --cacert "$ca" -u "$user:$password" -T "$2" "$dav/$(enc "$1")" >/dev/null; }
    mkcol() { ${pkgs.curl}/bin/curl -s --cacert "$ca" -u "$user:$password" -X MKCOL "$dav/$(enc "$1")" >/dev/null; }
    mkcol "$folder"
    mkcol "$folder/.obsidian"
    echo '{}' > /tmp/app.json
    put "$folder/.obsidian/app.json" /tmp/app.json
    cd ${vault}
    find . -type d ! -name . | sort | while read -r d; do mkcol "$folder/''${d#./}"; done
    find . -type f | sort | while read -r f; do put "$folder/''${f#./}" "$f"; done
    put "$folder/Garden plan.canvas" ${canvas}
  '';

  shots = pkgs.writeShellScriptBin "notes-shots" ''
    export PLAYWRIGHT_CORE=${pkgs.playwright-driver}/index.mjs
    export PLAYWRIGHT_BROWSERS_PATH=${
      pkgs.playwright-driver.browsers.override {
        withFirefox = false;
        withWebkit = false;
      }
    }
    export PLAYWRIGHT_SKIP_VALIDATE_HOST_REQUIREMENTS=true
    exec ${pkgs.nodejs}/bin/node ${./notes-shots.mjs} "$@"
  '';
in

pkgs.testers.nixosTest {
  name = "losos-notes";

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
      ];
      networking.hostName = "mattbox";
      losos = {
        hostName = "mattbox";
        backend.package = lososPkgs.losos-ctl;
        admin.ui = noLab lososPkgs.losos-admin-ui;
        cloud.enable = true;
        git.enable = false;
        mdns.enable = true;
        # Sharing and in a mesh: knowledge bases on.
        sharingMyStorage = true;
        cluster.enable = true;
      };
      # The same box once it stops sharing: knowledge bases off.
      specialisation.offMesh.configuration.losos.sharingMyStorage = lib.mkForce false;
      environment.systemPackages = [
        pkgs.jq
        pkgs.curl
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
        seed
        shots
      ];
      fonts.packages = [ pkgs.dejavu_fonts ];
      virtualisation = {
        memorySize = 3072;
        cores = 2;
        diskSize = 3072;
        vlans = [ 1 ];
      };
    };
  };

  testScript = ''
    import json
    import re
    import urllib.parse

    HOST = "mattbox.local"
    BASE = f"https://{HOST}"
    CA = "/etc/losos-box.pem"
    PASSWORD = "Correct-Horse-Battery-9"
    VAULT = "Kitchen garden"
    Q = urllib.parse.quote

    # One machine at a time: two TCG guests booting together can miss the
    # driver's wait for a shell.
    box.succeed("true")
    laptop.succeed("true")
    box.wait_for_unit("nginx.service")
    box.wait_for_unit("lososd.service")
    box.wait_for_unit("losos-cloud.service")
    assert "--knowledge-bases" in box.succeed("systemctl show -p ExecStart losos-cloud.service")

    cert = box.succeed("cat /var/lib/losos-tls/cert.pem")
    laptop.succeed(f"cat > {CA} <<'EOF'\n{cert}EOF")

    def curl(args):
        return laptop.succeed(f"curl -s --cacert {CA} {args}")

    def status(args):
        return int(curl(f"-o /dev/null -w '%{{http_code}}' {args}"))

    def get_json(args):
        return json.loads(curl(f"-f {args}"))

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

    def sign_in(name, password):
        body = json.dumps({"username": name, "password": password})
        head = curl(
            f"-D - -o /dev/null {BASE}/auth/api/firstfactor "
            f"-H 'Content-Type: application/json' -d '{body}'"
        )
        m = re.search(r"(?im)^set-cookie: (losos_session=[^;]+)", head)
        assert m, f"no session cookie for {name}: {head}"
        return m.group(1)

    cookie = sign_in("matus", PASSWORD)
    me = f"-H 'Cookie: {cookie}'"

    with subtest("an Obsidian vault arrives over WebDAV"):
        laptop.wait_until_succeeds(
            f"curl -sf --cacert {CA} -u 'matus:{PASSWORD}' -X PROPFIND -H 'Depth: 0' {BASE}/remote.php/webdav/ -o /dev/null",
            timeout=120,
        )
        laptop.succeed(f"notes-seed {BASE} matus '{PASSWORD}' {CA} '{VAULT}'")

    with subtest("the box lists the vault, finds it from a note and searches it"):
        v = get_json(f"{me} '{BASE}/cloud/api/notes/vault?path={Q(VAULT)}'")
        notes = {n["path"]: n for n in v["notes"]}
        assert v["name"] == VAULT, v["name"]
        assert set(notes) == {
            "Home.md", "Tomatoes.md", "Compost.md", "Planting calendar.md",
            "Pests/Aphids.md", "Pests/Slugs.md", "Private diary.md", "Harvest.note",
        }, sorted(notes)
        assert [c["path"] for c in v["canvases"]] == ["Garden plan.canvas"], v["canvases"]
        assert [f["path"] for f in v["files"]] == ["attachments/tomato.svg"], v["files"]
        # .obsidian is Obsidian's, not a note or a folder of the vault.
        assert not any(".obsidian" in f for f in v["folders"]), v["folders"]
        home = notes["Home.md"]
        assert set(home["tags"]) >= {"garden", "plan", "pests"}, home["tags"]
        assert home["aliases"] == ["Start here"], home["aliases"]
        assert {"target": "Beans", "embed": False} in home["links"], home["links"]
        tomatoes = notes["Tomatoes.md"]
        assert {"target": "Planting calendar", "embed": True} in tomatoes["links"], tomatoes["links"]
        assert "vegetables/fruiting" in tomatoes["tags"], tomatoes["tags"]
        assert notes["Private diary.md"]["private"] is True
        # A .note (MDX) is read like any other note: tags inside a
        # component, a wikilink to a heading.
        harvest = notes["Harvest.note"]
        assert set(harvest["tags"]) >= {"garden", "harvest"}, harvest["tags"]
        assert {"target": "Tomatoes", "embed": False} in harvest["links"], harvest["links"]
        # Comments and code are not text to scan.
        assert not any("shade" in l["target"] for l in notes["Planting calendar.md"]["links"])
        root = get_json(f"{me} '{BASE}/cloud/api/notes/root?path={Q(VAULT + "/Pests/Aphids.md")}'")
        assert root["root"] == VAULT, root
        hits = get_json(f"{me} '{BASE}/cloud/api/notes/search?path={Q(VAULT)}&q=ladybirds'")["hits"]
        assert [h["path"] for h in hits] == ["Pests/Aphids.md"], hits
        assert get_json(f"{me} {BASE}/cloud/api/kb") == {"available": True, "sites": []}

    with subtest("Chromium reads, maps, graphs, edits and publishes the vault"):
        laptop.succeed("rm -rf /tmp/shots")
        laptop.succeed(
            f"notes-shots {BASE} matus '{PASSWORD}' /tmp/shots/after --test losos-notes"
            " >&2"
        )
        # Opening Notes made the default vault, empty, for the first note.
        assert status(f"-u 'matus:{PASSWORD}' -X PROPFIND -H 'Depth: 0' {BASE}/remote.php/webdav/Notes/") == 207
        # What was typed in the editor is in the file, written by the box.
        text = curl(f"-f -u 'matus:{PASSWORD}' '{BASE}/remote.php/webdav/{Q(VAULT)}/{Q("Planting calendar.md")}'")
        assert "Feed with [[Compost]]" in text, text
        # Convert to .note moved Compost.md, and the box reads it as a note.
        v = get_json(f"{me} '{BASE}/cloud/api/notes/vault?path={Q(VAULT)}'")
        paths = {n["path"] for n in v["notes"]}
        assert "Compost.note" in paths and "Compost.md" not in paths, sorted(paths)
        note = curl(f"-f -u 'matus:{PASSWORD}' '{BASE}/remote.php/webdav/{Q(VAULT)}/Compost.note'")
        assert note.startswith("---\ntags: soil\n---") and "\n# Compost\n" in note, note[:200]
        laptop.copy_from_machine("/tmp/shots/after", "shots")

    sites = get_json(f"{me} {BASE}/cloud/api/kb")["sites"]
    assert len(sites) == 1 and sites[0]["slug"] == "kitchen-garden", sites
    site = sites[0]
    KB = f"{BASE}/kb/api/kitchen-garden"

    with subtest("the site reads without signing in and keeps to what is published"):
        got = get_json(f"{KB}/site")
        assert got["title"] == VAULT and got["audience"] == "anyone", got
        paths = [n["path"] for n in got["vault"]["notes"]]
        assert "Private diary.md" not in paths and "Home.md" in paths, paths
        # The owner's folder is not on the site.
        assert got["vault"]["root"] == "", got["vault"]["root"]
        assert "Feed with [[Compost]]" in curl(f"-f '{KB}/note?path={Q("Planting calendar.md")}'")
        assert status(f"'{KB}/note?path={Q("Private diary.md")}'") == 404
        assert status(f"'{KB}/note?path=../todo.md'") == 404
        assert status(f"'{KB}/note?path={Q(".obsidian/app.json")}'") == 404
        assert status(f"'{KB}/file?path={Q("../" + VAULT + "/Home.md")}'") == 404
        head = curl(f"-D - -o /dev/null '{KB}/file?path={Q("attachments/tomato.svg")}'")
        assert re.search(r"(?im)^content-security-policy: sandbox", head), head
        assert status(f"-X POST '{KB}/site'") == 404
        # The page itself, from the LAN and through the tunnel (loopback).
        assert status(f"{BASE}/kb/kitchen-garden/Tomatoes") == 200
        local = f"--resolve {HOST}:443:127.0.0.1 --cacert /var/lib/losos-tls/cert.pem"
        assert box.succeed(f"curl -s -o /dev/null -w '%{{http_code}}' {local} {BASE}/kb/kitchen-garden/") == "200"
        assert box.succeed(f"curl -s -o /dev/null -w '%{{http_code}}' {local} {KB}/site") == "200"
        assert status(f"{BASE}/kb/api/no-such-site/site") == 404

    with subtest("a site kept to people on the box answers only them"):
        out = curl(
            f"-f -X PATCH {me} -H 'Content-Type: application/json'"
            f" -d '{json.dumps({"audience": "box"})}' {BASE}/cloud/api/kb/{site["id"]}"
        )
        assert json.loads(out)["audience"] == "box", out
        assert status(f"{KB}/site") == 401
        forged = "-H 'X-Losos-Cloud-User: notshared' -H 'X-Losos-Cloud-Login: matus' -H 'Remote-User: matus'"
        assert status(f"{forged} {KB}/site") == 401
        assert get_json(f"{me} {KB}/site")["audience"] == "box"
        curl(
            f"-f -X PATCH {me} -H 'Content-Type: application/json'"
            f" -d '{json.dumps({"audience": "anyone"})}' {BASE}/cloud/api/kb/{site["id"]}"
        )
        assert status(f"{KB}/site") == 200

    with subtest("off the mesh, no knowledge base"):
        box.succeed("/run/current-system/specialisation/offMesh/bin/switch-to-configuration test")
        box.wait_until_succeeds(
            "! systemctl show -p ExecStart losos-cloud.service | grep -q -- --knowledge-bases", timeout=120
        )
        box.wait_for_unit("losos-cloud.service")
        laptop.wait_until_succeeds(
            f"curl -sf --cacert {CA} -H 'Cookie: {cookie}' {BASE}/cloud/api/kb | jq -e '.available == false'",
            timeout=120,
        )
        assert status(f"{KB}/site") == 404
        assert status(f"{me} {KB}/site") == 404
        out = curl(
            f"-w '\\n%{{http_code}}' -X POST {me} -H 'Content-Type: application/json'"
            f" -d '{json.dumps({"path": VAULT + "/Pests", "title": "Pests", "audience": "anyone"})}' {BASE}/cloud/api/kb"
        )
        body, code = out.rsplit("\n", 1)
        assert code == "409" and json.loads(body)["code"] == "meshRequired", out
        # Notes stay: they are the person's files.
        assert get_json(f"{me} '{BASE}/cloud/api/notes/vault?path={Q(VAULT)}'")["name"] == VAULT
        laptop.succeed(f"notes-shots {BASE} matus '{PASSWORD}' /tmp/shots/before --before >&2")
        laptop.copy_from_machine("/tmp/shots/before", "shots")
  '';
}
