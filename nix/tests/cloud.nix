# nixos-test-vms config for LosOS Cloud (modules/cloud.nix,
# backend/src/cloud/): people's files at /cloud/, over WebDAV, and to the
# Nextcloud desktop and phone apps, behind the real front vhost
# (modules/containers.nix, modules/tls.nix) and LosOS Login
# (modules/identity.nix), with lososd claiming the box.
#
# The laptop is a person's computer on the LAN: it finds the box by name,
# trusts its certificate as the first-run page tells an owner to, and then
#
#   * finds "LosOS Cloud on mattbox" among the LAN's WebDAV shares;
#   * lists, uploads, downloads, moves, copies and deletes over WebDAV with
#     a LosOS Login name and password, the owner's and a second person's,
#     and sends a large file in chunks as the desktop client does;
#   * is refused without a password, with a wrong one and with a forged
#     sign-in header;
#   * gets a password of its own through Login Flow v2 (the desktop client)
#     and v1 (the phone apps), meeting LosOS Login on the way;
#   * syncs a folder both ways with nextcloudcmd, the desktop client's own
#     sync engine;
#   * opens the web app in Chromium and takes the pictures for the pull
#     request and the launch film (nix/tests/cloud-shots.mjs), then the same
#     addresses on the box without LosOS Cloud, for the "before" column.
#
{ pkgs }:

let
  inherit (pkgs) lib;
  lososPkgs = import ../packages.nix { inherit pkgs; };

  python = pkgs.python3.withPackages (ps: [
    ps.requests
    ps.pillow
  ]);
  script =
    name: file:
    pkgs.writeScriptBin name ''
      #!${python}/bin/python3
      ${builtins.readFile file}
    '';
  flows = script "cloud-flows" ./cloud-flows.py;
  seed = script "cloud-seed" ./cloud-seed.py;
  # Playwright from nixpkgs' playwright-driver (playwright-core), which the
  # binary cache carries, under the Node it is built for.
  shots = pkgs.writeShellScriptBin "cloud-shots" ''
    export PLAYWRIGHT_CORE=${pkgs.playwright-driver}/index.mjs
    export PLAYWRIGHT_BROWSERS_PATH=${
      pkgs.playwright-driver.browsers.override {
        withFirefox = false;
        withWebkit = false;
      }
    }
    export PLAYWRIGHT_SKIP_VALIDATE_HOST_REQUIREMENTS=true
    exec ${pkgs.bun}/bin/bun ${./cloud-shots.mjs} "$@"
  '';
  screens = import ./screens { inherit pkgs; };
in

pkgs.testers.nixosTest {
  name = "losos-cloud";
  # The handbook's pictures, nineteen pages in three languages, take most
  # of an hour on their own under TCG.
  globalTimeout = 3 * 3600;

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
        # The option document, so the Apps pane draws LosOS Cloud's fold
        # for the pictures.
        ../../modules/config-repo.nix
      ];
      networking.hostName = "mattbox";
      losos = {
        hostName = "mattbox";
        backend.package = lososPkgs.losos-ctl;
        # The look and the pins, which the admin home asks for.
        backend.plugins = lososPkgs.losos-plugins;
        admin.ui = lososPkgs.losos-admin-ui;
        cloud.enable = true;
        # Faces are nix/tests/cloud-photos.nix's; here the pass reads only
        # the camera's details, which takes seconds, so Photos is not still
        # reading (a turning spinner) when the pictures are taken.
        cloud.photos.faces = false;
        git.enable = false;
        configRepo.enable = false;
        mdns.enable = true;
      };
      # The same box before LosOS Cloud, for the pictures' "before" column.
      specialisation.before.configuration.losos.cloud.enable = lib.mkForce false;
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
        pkgs.avahi
        pkgs.openssl
        pkgs.nextcloud-client
        flows
        seed
        shots
        screens.capture
      ];
      fonts.packages = [ pkgs.dejavu_fonts ];
      virtualisation = {
        memorySize = 3072;
        cores = 2;
        # The desktop client keeps 1 GB free before it downloads anything.
        diskSize = 4096;
        vlans = [ 1 ];
      };
    };
  };

  testScript = ''
    import json
    import re

    HOST = "mattbox.local"
    BASE = f"https://{HOST}"
    DAV = f"{BASE}/remote.php/webdav/"
    CA = "/etc/losos-box.pem"
    PASSWORD = "Correct-Horse-Battery-9"
    EVA = "Eva-Long-Password-3"

    ${screens.handbookScript}

    # One after the other: two VMs booting at once under TCG can miss the
    # driver's wait for a shell.
    box.start()
    box.succeed("true")
    laptop.start()
    laptop.succeed("true")
    box.wait_for_unit("nginx.service")
    box.wait_for_unit("lososd.service")
    box.wait_for_unit("losos-cloud.service")

    cert = box.succeed("cat /var/lib/losos-tls/cert.pem")
    laptop.succeed(f"cat > {CA} <<'EOF'\n{cert}EOF")

    def curl(args):
        return laptop.succeed(f"curl -s --cacert {CA} {args}")

    def code(args):
        return int(curl(f"-o /dev/null -w '%{{http_code}}' {args}"))

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
        body, status = out.rsplit("\n", 1)
        assert status == "200", out
        box.wait_for_unit("authelia-losos.service")
        box.wait_for_open_port(9091)

    def sign_in(name, password):
        """Sign in at /auth/ the way the portal does; return the cookie."""
        body = json.dumps({"username": name, "password": password})
        head = curl(
            f"-D - -o /dev/null {BASE}/auth/api/firstfactor "
            f"-H 'Content-Type: application/json' -d '{body}'"
        )
        m = re.search(r"(?im)^set-cookie: (losos_session=[^;]+)", head)
        assert m, f"no session cookie for {name}: {head}"
        return m.group(1)

    with subtest("a second person, added on the Users pane"):
        cookie = sign_in("matus", PASSWORD)
        mark = box.succeed(
            "journalctl -u authelia-losos -n 0 --show-cursor | sed -n 's/^-- cursor: //p'"
        ).strip()
        person = {"name": "eva", "displayName": "Eva", "email": "eva@mattbox.local", "password": EVA, "groups": []}
        out = curl(
            f"-w '\\n%{{http_code}}' -X POST {BASE}/api/people -H 'Cookie: {cookie}'"
            f" -H 'Content-Type: application/json' -d '{json.dumps(person)}'"
        )
        assert out.rsplit("\n", 1)[1] == "200", out
        box.wait_until_succeeds(
            f"journalctl -u authelia-losos --after-cursor='{mark}' | grep -q 'Reloaded successfully'",
            timeout=60,
        )

    with subtest("the apps find a server where they look for one"):
        status = json.loads(curl(f"-f {BASE}/status.php"))
        assert status["installed"] and status["productname"] == "LosOS Cloud", status
        caps = json.loads(curl(f"-f '{BASE}/ocs/v1.php/cloud/capabilities?format=json'"))
        dav = caps["ocs"]["data"]["capabilities"]["dav"]
        assert dav["chunking"] == "1.0", caps

    with subtest("the Network view lists LosOS Cloud"):
        found = laptop.wait_until_succeeds(
            "avahi-browse -rpt _webdavs._tcp | grep '^='", timeout=120
        )
        fields = found.splitlines()[0].split(";")
        name, host, port, txt = fields[3], fields[6], fields[8], fields[9]
        assert name == r"LosOS\032Cloud\032on\032mattbox", name
        assert host == HOST and port == "443", fields
        m = re.search(r'"path=([^"]+)"', txt)
        assert m and m.group(1) == "/remote.php/webdav/", txt

    def dav(user, password, method, path, extra=""):
        return curl(
            f"-u '{user}:{password}' -X {method} -w '\\n%{{http_code}}' {extra}"
            f" '{DAV}{path}'"
        ).rsplit("\n", 1)

    def files_round_trip(user, password, tag):
        body, status = dav(user, password, "PROPFIND", "", "-H 'Depth: 1'")
        assert status == "207", (status, body[:400])
        laptop.succeed(f"head -c 3000000 /dev/urandom > /tmp/{tag}.bin")
        _, status = dav(user, password, "PUT", f"{tag}.bin", f"-T /tmp/{tag}.bin")
        assert status in ("201", "204"), status
        laptop.succeed(
            f"curl -sf --cacert {CA} -u '{user}:{password}' -o /tmp/{tag}.back '{DAV}{tag}.bin'"
            f" && cmp /tmp/{tag}.bin /tmp/{tag}.back"
        )
        _, status = dav(user, password, "MKCOL", f"{tag}-dir")
        assert status == "201", status
        _, status = dav(
            user, password, "MOVE", f"{tag}.bin", f"-H 'Destination: {DAV}{tag}-dir/moved.bin'"
        )
        assert status == "201", status
        _, status = dav(
            user, password, "COPY", f"{tag}-dir/moved.bin",
            f"-H 'Destination: {DAV}{tag}-copy.bin'",
        )
        assert status == "201", status
        body, status = dav(user, password, "PROPFIND", f"{tag}-dir/", "-H 'Depth: 1'")
        assert status == "207" and "moved.bin" in body, (status, body[:400])
        _, status = dav(user, password, "DELETE", f"{tag}-copy.bin")
        assert status == "204", status
        _, status = dav(user, password, "GET", f"{tag}-copy.bin", "-o /dev/null")
        assert status == "404", status

    with subtest("a LosOS Login name and password open the share"):
        files_round_trip("matus", PASSWORD, "owner")
        files_round_trip("eva", EVA, "eva")
        # Each person's files are their own: the owner's are notshared's.
        assert code(f"-u 'eva:{EVA}' -X PROPFIND '{BASE}/remote.php/dav/files/notshared/'") in (403, 404)
        box.succeed("test -f /var/lib/losos-cloud/notshared/files/owner-dir/moved.bin")
        box.succeed("test -f /var/lib/losos-cloud/eva/files/eva-dir/moved.bin")

    with subtest("no password, a wrong one, or a made-up header is refused"):
        assert code(f"-X PROPFIND '{DAV}'") == 401
        assert code(f"-u matus:not-the-password -X PROPFIND '{DAV}'") == 401
        forged = "-H 'X-Losos-Cloud-User: notshared' -H 'Remote-User: matus'"
        assert code(f"{forged} -X PROPFIND '{DAV}'") == 401
        assert code(f"{forged} '{BASE}/cloud/api/me'") == 401
        # Signed out, the page sends the browser to LosOS Login and back.
        where = curl(f"-o /dev/null -w '%{{redirect_url}}' {BASE}/cloud/files")
        assert where.startswith(f"{BASE}/auth/?rd=") and "/cloud/files" in where, where

    with subtest("a large file goes up in chunks, as the desktop client sends it"):
        up = f"{BASE}/remote.php/dav/uploads/notshared/web-test-1"
        dest = f"{BASE}/remote.php/dav/files/notshared/big.bin"
        auth = f"-u 'matus:{PASSWORD}'"
        laptop.succeed("head -c 25000000 /dev/urandom > /tmp/big.bin && split -b 10000000 -d -a 5 /tmp/big.bin /tmp/big.part.")
        assert code(f"{auth} -X MKCOL -H 'Destination: {dest}' '{up}'") == 201
        parts = sorted(laptop.succeed("ls /tmp/big.part.*").split())
        for i, part in enumerate(parts, 1):
            assert code(
                f"{auth} -X PUT -H 'Destination: {dest}' -H 'OC-Total-Length: 25000000'"
                f" -T {part} '{up}/{i:05d}'"
            ) == 201, part
        assert code(
            f"{auth} -X MOVE -H 'Destination: {dest}' -H 'OC-Total-Length: 25000000' '{up}/.file'"
        ) in (201, 204)
        laptop.succeed(
            f"curl -sf --cacert {CA} {auth} -o /tmp/big.back '{dest}' && cmp /tmp/big.bin /tmp/big.back"
        )

    def flow(kind, name, password):
        out = json.loads(laptop.succeed(f"cloud-flows {kind} {BASE} {name} '{password}'"))
        print(json.dumps(out, indent=1))
        return out

    with subtest("the desktop client signs the owner in through LosOS Login"):
        got = flow("flow2", "matus", PASSWORD)
        assert got["init"]["login"].startswith(f"{BASE}/index.php/login/v2/flow/"), got
        assert got["init"]["poll"]["endpoint"] == f"{BASE}/index.php/login/v2/poll", got
        assert got["server"] == BASE and got["loginName"] == "notshared", got
        assert got["asked"]["client"] == "Nextcloud desktop on Linux", got
        assert any(u.startswith(f"{BASE}/auth/") for u in got["seen"]), got["seen"]
        # The password is handed over once.
        assert got["secondPoll"] == 404, got
        owner_app = got["appPassword"]
        files_round_trip("notshared", owner_app, "owner-app")

    with subtest("the phone app signs a second person in"):
        got = flow("flow1", "eva", EVA)
        assert got["server"] == BASE and got["loginName"] == "eva", got
        eva_app = got["appPassword"]
        files_round_trip("eva", eva_app, "eva-app")
        assert code(f"-u 'eva:{eva_app}' -X PROPFIND '{BASE}/remote.php/dav/files/notshared/'") in (403, 404)
        devices = json.loads(curl(f"-f -u 'eva:{eva_app}' {BASE}/cloud/api/devices"))["devices"]
        assert [d["name"] for d in devices] == ["Nextcloud on Android"], devices

    with subtest("an app password works off the LAN too, a LosOS Login password does not"):
        # From the box itself, as the tunnel delivers: only the app's own
        # password counts there (modules/identity.nix, losos_lan).
        local = f"--resolve {HOST}:443:127.0.0.1 --cacert /var/lib/losos-tls/cert.pem"
        assert box.succeed(
            f"curl -s -o /dev/null -w '%{{http_code}}' {local} -u 'notshared:{owner_app}' -X PROPFIND '{DAV}'"
        ) == "207"
        assert box.succeed(
            f"curl -s -o /dev/null -w '%{{http_code}}' {local} -u 'matus:{PASSWORD}' -X PROPFIND '{DAV}'"
        ) == "401"

    with subtest("the desktop client's sync engine syncs a folder both ways"):
        laptop.succeed("mkdir -p /tmp/sync && echo 'from the laptop' > /tmp/sync/laptop.txt")
        sync = (
            # --trust is the desktop client's "Trust this certificate"
            # prompt, which it shows for the box's self-signed certificate:
            # Qt does not take a leaf certificate as its own trust anchor
            # the way curl (partial chains) does.
            f"nextcloudcmd --non-interactive --trust -u notshared -p '{owner_app}'"
            f" /tmp/sync {BASE}"
        )
        laptop.succeed(sync)
        assert curl(f"-f -u 'notshared:{owner_app}' '{DAV}laptop.txt'") == "from the laptop\n"
        laptop.succeed("cmp /tmp/sync/big.bin /tmp/big.bin")
        dav("notshared", owner_app, "PUT", "from-box.txt", "--data-binary 'from the box'")
        laptop.succeed("rm /tmp/sync/laptop.txt")
        laptop.succeed(sync)
        assert laptop.succeed("cat /tmp/sync/from-box.txt") == "from the box"
        # The file the laptop deleted waits in the trash.
        trash = json.loads(curl(f"-f -u 'notshared:{owner_app}' {BASE}/cloud/api/trash"))["entries"]
        assert "laptop.txt" in [t["name"] for t in trash], trash

    with subtest("the pictures: the owner's cloud in Chromium"):
        # A clean set of files for the pictures, as a person would have them.
        for path in ["owner-dir", "owner-app-dir", "owner-name-dir", "big.bin", "from-box.txt", "owner-app.bin"]:
            dav("notshared", owner_app, "DELETE", path)
        # The trash holds what this test deleted; empty it for ordinary files.
        curl(f"-f -X POST -u 'notshared:{owner_app}' {BASE}/cloud/api/trash/empty")
        laptop.succeed(f"cloud-seed {BASE} notshared '{owner_app}' {CA}")
        for path in ["Documents/old%20notes.txt", "Documents/budget%20draft.ods", "Photos/IMG_2043%20(2).jpg"]:
            _, status = dav("notshared", owner_app, "DELETE", path)
            assert status == "204", (path, status)
        dav("notshared", owner_app, "PROPPATCH", "Documents/budget.ods",
            "-H 'Content-Type: application/xml' --data '<d:propertyupdate xmlns:d=\"DAV:\" xmlns:oc=\"http://owncloud.org/ns\"><d:set><d:prop><oc:favorite>1</oc:favorite></d:prop></d:set></d:propertyupdate>'")
        # Until a pass finds nothing new. The timer's pass may already be
        # running on a list from before the seed (start then waits for it),
        # so only a pass from the second round on counts.
        for round in range(10):
            box.succeed("systemctl start losos-cloud-photos.service")
            last = box.succeed("journalctl -u losos-cloud-photos.service -o cat | grep 'photos read' | tail -n 1")
            if round > 0 and last.startswith("0 photos read"):
                break
        else:
            raise AssertionError(f"the photos pass kept finding new photos: {last}")
        # A sign-in the desktop client has started, for the connect page.
        init = json.loads(curl(f"-f -X POST -A 'Mozilla/5.0 (Linux) mirall/3.16.0 (Nextcloud)' {BASE}/index.php/login/v2"))
        laptop.succeed(
            f"cloud-shots {BASE} matus '{PASSWORD}' /tmp/shots/after"
            f" --connect {init['login']} --test losos-cloud"
        )
        got = json.loads(curl(f"-f -X POST {BASE}/index.php/login/v2/poll -d token={init['poll']['token']}"))
        assert got["loginName"] == "notshared", got
        laptop.copy_from_machine("/tmp/shots/after", "shots")

    with subtest("the handbook's pictures"):
        pending = json.loads(curl(f"-f -X POST -A 'Mozilla/5.0 (Linux) mirall/3.16.0 (Nextcloud)' {BASE}/index.php/login/v2"))
        handbook_pictures(laptop, BASE, "matus", PASSWORD, [
            {"name": "cloud-files", "path": "/cloud/files/Documents",
             "waitFor": {"selector": '[data-entry="budget.ods"]'}},
            {"name": "cloud-photos", "path": "/cloud/photos",
             "waitFor": {"selector": "[data-photos] img"}, "settle": 1500},
            {"name": "cloud-devices", "path": "/cloud/devices",
             "waitFor": {"selector": "[data-devices]"}},
            {"name": "cloud-connect", "path": pending["login"].removeprefix(BASE),
             "waitFor": {"selector": "[data-connect]"}},
            # The admin pages around it, on a box with LosOS Cloud.
            {"name": "overview", "path": "/", "waitFor": {"role": "heading", "name": "mattbox"},
             "storage": {"losos-widgets": json.dumps({"version": 1, "widgets": [
                 {"id": f"w{i}", "source": {"kind": "builtin", "id": w}}
                 for i, w in enumerate(["disk", "apps", "rebuilds"])]})},
             "settle": 1500},
            {"name": "apps", "path": "/apps", "waitFor": {"selector": "[data-testid=accordion-cloud]"},
             "settle": 1500},
        ] + [
            {"name": f"{name}", "path": path, "waitFor": {"selector": "main h1"}, "settle": 1500}
            for name, path in [
                ("storage", "/storage"),
                ("settings-about", "/settings/about"),
                ("settings-account", "/settings/account"),
                ("settings-users", "/settings/users"),
                ("settings-security", "/settings/security"),
                ("settings-network", "/settings/network"),
                ("settings-look", "/settings/look"),
                ("settings-backup", "/settings/backup"),
                ("settings-history", "/settings/history"),
                ("settings-advanced", "/settings/advanced"),
                ("settings-reset", "/settings/reset"),
                ("settings-hardware", "/settings/hardware"),
                # No edge in this test, so the pane says none was found.
                ("mesh-none", "/mesh"),
            ]
        ])

    with subtest("the pictures: the same addresses before LosOS Cloud"):
        box.succeed("/run/current-system/specialisation/before/bin/switch-to-configuration test")
        box.wait_until_fails("systemctl is-active losos-cloud.service")
        laptop.succeed(f"cloud-shots {BASE} matus '{PASSWORD}' /tmp/shots/before --before")
        laptop.copy_from_machine("/tmp/shots/before", "shots")
  '';
}
