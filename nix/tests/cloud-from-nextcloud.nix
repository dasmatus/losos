# nixos-test-vms config for the move from Nextcloud to LosOS Cloud
# (modules/cloud.nix, `losos-cloud-from-nextcloud`; backend/src/cloud/import.rs).
#
# The box starts as every LosOS box with Nextcloud did: Nextcloud 34 with
# Deck, its database in the box's Postgres, its data under
# /var/lib/nextcloud/data. Two people fill it the way they would: files,
# a deleted file, events and a task over CalDAV, a contact over CardDAV, a
# Deck board with columns, labels, a shared member, an assignee and a
# comment, and an app password like the one a phone holds.
#
# Then it switches to its next configuration, as the 03:00 rebuild does:
# Nextcloud and Postgres gone, LosOS Cloud on. Before LosOS Cloud starts,
# the move reads Nextcloud's database from a private copy (Postgres is off
# by then) and brings everything across. The test checks every item came
# over intact, that Nextcloud's own files and database are still on disk,
# that a second start moves nothing twice, and that the old address and the
# old app password still sync with the desktop client's engine.
{ pkgs }:

let
  inherit (pkgs) lib;
  lososPkgs = import ../packages.nix { inherit pkgs; };
  adminPass = "Nextcloud-Admin-Pass-1";
  # Pictures of the move for the pull request: Nextcloud's pages before,
  # LosOS Cloud's after, from a browser on the box itself.
  shots = pkgs.writeShellScriptBin "cloud-move-shots" ''
    export PLAYWRIGHT_CORE=${pkgs.playwright-driver}/index.mjs
    export PLAYWRIGHT_BROWSERS_PATH=${
      pkgs.playwright-driver.browsers.override {
        withFirefox = false;
        withWebkit = false;
      }
    }
    export PLAYWRIGHT_SKIP_VALIDATE_HOST_REQUIREMENTS=true
    exec ${pkgs.bun}/bin/bun ${./cloud-move-shots.mjs} "$@"
  '';
in

pkgs.testers.nixosTest {
  name = "losos-cloud-from-nextcloud";

  nodes.box = _: {
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
      admin.ui = lososPkgs.losos-admin-ui;
      nextcloud.mode = "native";
      cloud.enable = false;
      git.enable = false;
    };

    # Nextcloud as the boxes ran it: the same folders, the same database.
    services.nextcloud = {
      enable = true;
      package = pkgs.nextcloud34;
      hostName = "nextcloud.test";
      home = "/var/lib/nextcloud";
      datadir = "/var/lib/nextcloud/data";
      database.createLocally = true;
      config = {
        dbtype = "pgsql";
        adminuser = "notshared";
        adminpassFile = "${pkgs.writeText "nextcloud-admin-pass" adminPass}";
      };
      settings.trusted_domains = [
        "nextcloud.test"
        "127.0.0.1"
      ];
      extraApps = {
        inherit (pkgs.nextcloud34Packages.apps) deck;
      };
      extraAppsEnable = true;
    };

    # The pod kept its config and data one level down, in data/config and
    # data/data. nixpkgs creates only the two inner folders, so tmpfiles
    # would make data/ itself root's and Nextcloud's setup would refuse it.
    systemd.tmpfiles.rules = [ "d /var/lib/nextcloud/data 0750 nextcloud nextcloud -" ];

    # The next configuration, as the nightly rebuild brings it.
    specialisation.after.configuration = {
      services.nextcloud.enable = lib.mkForce false;
      services.postgresql.enable = lib.mkForce false;
      losos.cloud.enable = lib.mkForce true;
    };

    environment.systemPackages = [
      pkgs.jq
      pkgs.curl
      pkgs.nextcloud-client
      shots
    ];
    fonts.packages = [ pkgs.dejavu_fonts ];
    virtualisation = {
      # Nextcloud, Postgres and Chromium at once.
      memorySize = 4096;
      cores = 2;
      diskSize = 6144;
    };
  };

  testScript = ''
    import base64
    import json

    NC = "http://127.0.0.1"
    HOST = "-H 'Host: nextcloud.test'"
    ADMIN = ("notshared", "${adminPass}")
    EVA = ("eva", "Eva-Long-Password-3")

    EVENT = (
        "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//DAVx5//EN\r\nBEGIN:VEVENT\r\n"
        "UID:dentist-2026\r\nDTSTART:20261023T090000Z\r\nDTEND:20261023T100000Z\r\n"
        "SUMMARY:Dentist\r\nLOCATION:Hlavná 12\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n"
    )
    TASK = (
        "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//Tasks//EN\r\nBEGIN:VTODO\r\n"
        "UID:renew-insurance\r\nSUMMARY:Renew the car insurance\r\nEND:VTODO\r\nEND:VCALENDAR\r\n"
    )
    WORK = (
        "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:standup\r\n"
        "DTSTART:20261026T080000Z\r\nDURATION:PT15M\r\nRRULE:FREQ=DAILY\r\n"
        "SUMMARY:Stand-up\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n"
    )
    EVA_EVENT = (
        "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:eva-yoga\r\n"
        "DTSTART:20261024T170000Z\r\nDTEND:20261024T180000Z\r\nSUMMARY:Yoga\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n"
    )
    CARD = (
        "BEGIN:VCARD\r\nVERSION:3.0\r\nUID:eva-kovac\r\nFN:Eva Kováč\r\n"
        "TEL;TYPE=CELL:+421900123456\r\nEMAIL:eva@example.org\r\nEND:VCARD\r\n"
    )

    def nc(user, method, path, extra="", ok=(200, 201, 204, 207)):
        out = box.succeed(
            f"curl -s {HOST} -u '{user[0]}:{user[1]}' -X {method} -w '\\n%{{http_code}}' "
            f"{extra} '{NC}{path}'"
        )
        body, status = out.rsplit("\n", 1)
        assert int(status) in ok, (method, path, status, body[:600])
        return body

    def put_text(user, path, text, ctype):
        # Through base64, so the CRLFs and the last newline arrive as written.
        box.succeed(f"echo {base64.b64encode(text.encode()).decode()} | base64 -d > /tmp/put")
        return nc(user, "PUT", path, f"-H 'Content-Type: {ctype}' --data-binary @/tmp/put")

    def deck(user, method, path, data=None):
        extra = "-H 'OCS-APIRequest: true' -H 'Content-Type: application/json' -H 'Accept: application/json'"
        if data is not None:
            extra += f" -d '{json.dumps(data)}'"
        return json.loads(nc(user, method, f"/index.php/apps/deck/api/v1.0{path}", extra))

    box.start()
    # nextcloud-setup is a oneshot that ends inactive; wait for the boot to
    # settle, then read how it ended. Under TCG the install takes minutes.
    box.wait_for_unit("multi-user.target", timeout=1500)
    box.succeed("test \"$(systemctl show -P Result nextcloud-setup.service)\" = success")
    box.fail("test \"$(systemctl show -P ExecMainExitTimestampMonotonic nextcloud-setup.service)\" = 0")
    box.wait_for_unit("phpfpm-nextcloud.service")
    box.wait_for_unit("nginx.service")
    occ = "nextcloud-occ"

    with subtest("two people fill Nextcloud"):
        box.succeed(f"OC_PASS='{EVA[1]}' {occ} user:add --password-from-env --display-name='Eva Kováč' eva")
        box.succeed(f"{occ} app:list | grep -q 'deck'")
        dav = "/remote.php/dav/files/notshared"
        for folder in ["Documents", "Photos", "Notes"]:
            nc(ADMIN, "MKCOL", f"{dav}/{folder}")
        box.succeed("head -c 5000000 /dev/urandom > /tmp/photo.jpg")
        box.succeed("printf 'Budget 2026\\nRent 650\\n' > /tmp/budget.ods")
        box.succeed("printf '# Shopping\\n- milk\\n- bread\\n' > /tmp/shopping.md")
        box.succeed("printf 'old\\n' > /tmp/old.txt")
        for src, dst in [
            ("photo.jpg", "Photos/IMG_2041.jpg"),
            ("budget.ods", "Documents/budget.ods"),
            ("shopping.md", "Notes/Shopping.md"),
            ("old.txt", "Documents/old notes.txt"),
        ]:
            nc(ADMIN, "PUT", f"{dav}/{dst.replace(' ', '%20')}", f"--data-binary @/tmp/{src}")
        # A deleted file, for the trash bin.
        nc(ADMIN, "DELETE", f"{dav}/Documents/old%20notes.txt")
        nc(EVA, "PUT", "/remote.php/dav/files/eva/hello.txt", "--data-binary @/tmp/budget.ods")

        cal = "/remote.php/dav/calendars/notshared"
        put_text(ADMIN, f"{cal}/personal/dentist.ics", EVENT, "text/calendar")
        # Nextcloud's own calendars take events only; the Tasks app keeps
        # tasks in a calendar of their own.
        nc(ADMIN, "MKCALENDAR", f"{cal}/tasks/",
           "--data '<c:mkcalendar xmlns:d=\"DAV:\" xmlns:c=\"urn:ietf:params:xml:ns:caldav\"><d:set><d:prop><d:displayname>Tasks</d:displayname>"
           "<c:supported-calendar-component-set><c:comp name=\"VTODO\"/></c:supported-calendar-component-set></d:prop></d:set></c:mkcalendar>'")
        put_text(ADMIN, f"{cal}/tasks/insurance.ics", TASK, "text/calendar")
        nc(ADMIN, "MKCALENDAR", f"{cal}/work/",
           "--data '<c:mkcalendar xmlns:d=\"DAV:\" xmlns:c=\"urn:ietf:params:xml:ns:caldav\"><d:set><d:prop><d:displayname>Work</d:displayname></d:prop></d:set></c:mkcalendar>'")
        put_text(ADMIN, f"{cal}/work/standup.ics", WORK, "text/calendar")
        put_text(EVA, "/remote.php/dav/calendars/eva/personal/yoga.ics", EVA_EVENT, "text/calendar")
        put_text(ADMIN, "/remote.php/dav/addressbooks/users/notshared/contacts/eva.vcf", CARD, "text/vcard")

        board = deck(ADMIN, "POST", "/boards", {"title": "Garden shed", "color": "31cc7c"})
        b = board["id"]
        todo = deck(ADMIN, "POST", f"/boards/{b}/stacks", {"title": "To do", "order": 0})["id"]
        done = deck(ADMIN, "POST", f"/boards/{b}/stacks", {"title": "Done", "order": 1})["id"]
        card = deck(ADMIN, "POST", f"/boards/{b}/stacks/{todo}/cards",
                    {"title": "Buy larch boards", "type": "plain", "order": 0, "description": "48 boards, 2.4 m"})
        deck(ADMIN, "POST", f"/boards/{b}/stacks/{done}/cards", {"title": "Pour the base", "type": "plain", "order": 0})
        label = board["labels"][0]
        deck(ADMIN, "PUT", f"/boards/{b}/stacks/{todo}/cards/{card['id']}/assignLabel", {"labelId": label["id"]})
        deck(ADMIN, "POST", f"/boards/{b}/acl",
             {"type": 0, "participant": "eva", "permissionEdit": True, "permissionShare": False, "permissionManage": False})
        deck(ADMIN, "PUT", f"/boards/{b}/stacks/{todo}/cards/{card['id']}/assignUser", {"userId": "eva"})
        nc(ADMIN, "POST", f"/ocs/v2.php/apps/deck/api/v1.0/cards/{card['id']}/comments?format=json",
           "-H 'OCS-APIRequest: true' -H 'Content-Type: application/json' -d '{\"message\": \"I will pick them up on Saturday\"}'")

        app_password = box.succeed(f"OC_PASS='${adminPass}' {occ} user:auth-tokens:add --password-from-env --name 'Pixel 8' notshared").strip().splitlines()[-1].strip()
        assert len(app_password) == 72, app_password
        nc(("notshared", app_password), "PROPFIND", f"{dav}/", "-H 'Depth: 0'")

    with subtest("the pictures: Nextcloud as the owner left it"):
        # The welcome slides would cover the first picture.
        box.succeed(f"{occ} app:disable firstrunwizard || true")
        status, out = box.execute("cloud-move-shots nextcloud /tmp/shots/before notshared '${adminPass}' 2>&1", timeout=3000)
        print(out)
        box.copy_from_machine("/tmp/shots/before", "shots")
        assert status == 0, "the Nextcloud pictures failed; shots/before/failed.png shows where"

    with subtest("the nightly switch: Nextcloud and Postgres go, LosOS Cloud comes"):
        # What Nextcloud itself serves for each entry, which LosOS Cloud
        # must serve byte for byte after the move.
        nc_served = {}
        for who, path in [
            (ADMIN, "calendars/notshared/personal/dentist.ics"),
            (ADMIN, "calendars/notshared/tasks/insurance.ics"),
            (ADMIN, "calendars/notshared/work/standup.ics"),
            (EVA, "calendars/eva/personal/yoga.ics"),
            (ADMIN, "addressbooks/users/notshared/contacts/eva.vcf"),
        ]:
            nc_served[(who[0], path)] = nc(who, "GET", f"/remote.php/dav/{path}")
            assert "BEGIN:" in nc_served[(who[0], path)], (path, nc_served[(who[0], path)])
        # Taken last: signing in to Nextcloud's page for the pictures adds
        # its sample files, which move like the rest.
        before = box.succeed(
            "cd /var/lib/nextcloud/data/data/notshared/files && find . -type f -print0 | sort -z | xargs -0 sha256sum"
        )
        box.succeed("/run/current-system/specialisation/after/bin/switch-to-configuration test")
        box.wait_for_unit("losos-cloud.service")
        box.succeed("! systemctl is-active postgresql.service")
        box.succeed("! systemctl is-active phpfpm-nextcloud.service")
        journal = box.succeed("journalctl -u losos-cloud-from-nextcloud --no-pager")
        print(journal)
        report = json.loads(box.succeed("cat /var/lib/losos-cloud/from-nextcloud.json"))
        me, eva = report["people"]["notshared"], report["people"]["eva"]
        # Nextcloud seeds every new account with an example event and an
        # example contact, and Deck gives its first user a welcome board of
        # five cards. Those move too, so each count is ours plus theirs.
        assert me["calendars"] == 3 and me["events"] == 3 and me["tasks"] == 1, me
        assert me["contacts"] == 2 and me["boards"] == 2 and me["cards"] == 7, me
        assert me["trash"] == 1 and me["appPasswords"] == 1, me
        assert me["notMoved"] == [], me
        assert eva["events"] == 2 and eva["files"] >= 1, eva

    def cloud(user, method, path, extra=""):
        return box.succeed(
            f"curl -sf --unix-socket /run/losos-cloud/http.sock -H 'X-Losos-Cloud-User: {user}'"
            f" -X {method} {extra} 'http://localhost{path}'"
        )

    with subtest("every file came across byte for byte, and the trash with it"):
        after = box.succeed(
            "cd /var/lib/losos-cloud/notshared/files && find . -type f -print0 | sort -z | xargs -0 sha256sum"
        )
        assert before == after, (before, after)
        box.succeed("test -f /var/lib/losos-cloud/eva/files/hello.txt")
        trash = json.loads(cloud("notshared", "GET", "/cloud/api/trash"))["entries"]
        assert any(t["original"] == "Documents/old notes.txt" for t in trash), trash
        listing = cloud("notshared", "PROPFIND", "/remote.php/dav/files/notshared/Notes/", "-H 'Depth: 1'")
        assert "Shopping.md" in listing, listing

    with subtest("events, the task and the contact, exactly as Nextcloud served them"):
        for (login, path), served in nc_served.items():
            got = cloud(login, "GET", f"/remote.php/dav/{path}")
            assert got == served, (path, served, got)
        cols = json.loads(cloud("notshared", "GET", "/cloud/api/collections"))["collections"]
        names = {c["name"] for c in cols}
        assert {"Work", "Tasks"} <= names, cols
        assert not any(c["uri"] == "contact_birthdays" for c in cols), cols

    with subtest("the Deck board, with its columns, label, member, assignee and comment"):
        boards = json.loads(cloud("eva", "GET", "/cloud/api/boards"))["boards"]
        assert [x["title"] for x in boards] == ["Garden shed"], boards
        full = json.loads(cloud("notshared", "GET", f"/cloud/api/boards/{boards[0]['id']}"))
        assert [c["title"] for c in full["columns"]] == ["To do", "Done"], full
        c = full["columns"][0]["cards"][0]
        assert c["title"] == "Buy larch boards" and c["description"] == "48 boards, 2.4 m", c
        assert c["labels"] == [label["title"]] and c["assignees"] == ["eva"], c
        assert c["comments"][0]["body"] == "I will pick them up on Saturday", c
        assert full["members"] == ["eva"], full

    with subtest("Nextcloud's own data is still on disk, and a second start moves nothing"):
        box.succeed("test -f /var/lib/nextcloud/data/config/config.php")
        box.succeed("ls /var/lib/postgresql/*/PG_VERSION")
        box.succeed("test ! -e /var/lib/nextcloud/data/data/notshared/files/Photos/IMG_2041.jpg")
        first = json.loads(cloud("notshared", "GET", "/cloud/api/boards"))["boards"]
        box.succeed("systemctl restart losos-cloud-from-nextcloud losos-cloud")
        box.wait_for_unit("losos-cloud.service")
        # Active is not listening yet: the service is a plain process.
        box.wait_for_open_unix_socket("/run/losos-cloud/http.sock")
        boards = json.loads(cloud("notshared", "GET", "/cloud/api/boards"))["boards"]
        assert len(boards) == len(first) == 2, (first, boards)

    with subtest("the phone's old address and app password keep working"):
        cert = "/var/lib/losos-tls/cert.pem"
        out = box.succeed(
            f"curl -s --cacert {cert} -u 'notshared:{app_password}' -X PROPFIND -H 'Depth: 1'"
            " -w '\\n%{http_code}' https://127.0.0.1/nextcloud/remote.php/dav/files/notshared/Photos/"
        )
        body, status = out.rsplit("\n", 1)
        assert status == "207", out[:600]
        assert "<d:href>/nextcloud/remote.php/dav/files/notshared/Photos/IMG_2041.jpg</d:href>" in body, body[:800]
        out = box.succeed(
            f"curl -s --cacert {cert} -u 'notshared:{app_password}' -X REPORT"
            " -H 'Content-Type: application/xml' -w '\\n%{http_code}'"
            " --data '<d:sync-collection xmlns:d=\"DAV:\"><d:sync-token/><d:prop><d:getetag/></d:prop></d:sync-collection>'"
            " https://127.0.0.1/nextcloud/remote.php/dav/calendars/notshared/personal/"
        )
        assert out.endswith("207") and "dentist.ics" in out, out[:800]
        assert box.succeed("curl -s -o /dev/null -w '%{http_code}' https://127.0.0.1/.well-known/caldav -k").strip() == "301"

    with subtest("the desktop client's engine syncs from the old address with the old password"):
        box.succeed("mkdir -p /tmp/sync")
        box.succeed(
            f"nextcloudcmd --non-interactive --trust -u notshared -p '{app_password}'"
            " /tmp/sync https://127.0.0.1/nextcloud >/tmp/sync.log 2>&1 || (cat /tmp/sync.log; false)"
        )
        box.succeed("cmp /tmp/sync/Photos/IMG_2041.jpg /tmp/photo.jpg")
        box.succeed("test -f /tmp/sync/Notes/Shopping.md")

    with subtest("the pictures: the same files and trash in LosOS Cloud"):
        # The owner claims the box, and LosOS Login opens LosOS Cloud for
        # them as `notshared`, the account Nextcloud's files moved into.
        # The claim goes to lososd itself: nginx's admin routes refuse a
        # request from the box to its own address, and this box has no
        # other machine on its network. LosOS Login and LosOS Cloud answer
        # on loopback, as they do for the tunnel.
        box.wait_until_succeeds("curl -sf 127.0.0.1:8082/api/setup/claim | jq -e .ready", timeout=600)
        box.succeed(
            "curl -sf -X POST 127.0.0.1:8082/api/setup/claim"
            " -H 'Content-Type: application/json'"
            " -d '{\"name\": \"matus\", \"password\": \"Correct-Horse-Battery-9\"}'"
        )
        box.wait_for_unit("authelia-losos.service")
        box.wait_for_open_port(9091)
        status, out = box.execute("cloud-move-shots cloud /tmp/shots/after matus 'Correct-Horse-Battery-9' 127.0.0.1 2>&1", timeout=3000)
        print(out)
        box.copy_from_machine("/tmp/shots/after", "shots")
        assert status == 0, "the LosOS Cloud pictures failed; shots/after/failed.png shows where"
  '';
}
