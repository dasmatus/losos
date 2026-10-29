# nixos-test-vms config for LosOS Mail: Stalwart as the shared user, lososd's
# mail API over JMAP, and the gate that keeps both off while the box does
# not share its storage.
#
# The box runs the real pieces: Stalwart from modules/mail.nix (its own
# unit as `shared`, mailboxes under /home/shared/data/mail), lososd with
# backend/src/mail.rs, LosOS Login (modules/identity.nix) and the front
# vhost (modules/containers.nix, modules/tls.nix). The admin pages are a
# stub index.html; tests/mail.browser.mjs covers the Mail screen.
# The shared domain's fscrypt key is off here, because no VM test has the
# TPM-less keyring set-up modules/fscrypt.nix expects; the directory is the
# same one.
#
# The laptop is someone on the LAN with a browser:
#
#   * the owner claims the box and adds Eva through the Users pane's route;
#   * both find Mail in /api/me's `shared` and get a mailbox on first use;
#   * Matus writes to Eva, Eva reads it, replies, flags, moves and deletes;
#   * a message with HTML is drawn only from its own route, under its own
#     sandboxing CSP and without X-Frame-Options;
#   * Eva makes, nests, renames, moves and deletes folders of her own
#     (backend/plugins/mail), her mail going to the bin first; LosOS Mail's
#     own folders stay put, and each refusal carries its word;
#   * Matus's filters are saved as his Sieve script, and Stalwart files,
#     flags and marks read Peter's next mail with lososd stopped;
#   * mail to another domain, a bad address, the spare key and a signed-out
#     request are refused, and nobody reads someone else's message;
#   * nothing but lososd reaches Stalwart: it listens on loopback only;
#   * sharing on but no mesh (cluster.enable off) is not the shared side;
#   * with sharing off (a specialisation, switched to live) Stalwart stops,
#     /api/me lists nothing, /api/mail answers 404 and /mail is gone; with
#     sharing back on, Eva's mail is still there.
#
# `losos-mail-screens` is the same run with the real admin pages and
# Chromium on the laptop (nix/tests/mail-screens.mjs): the screenshots of the
# sidebar with sharing off and on, the inbox, a message, a reply typed word
# by word and the reply in Sent, with a manifest of where things are. They
# land in the run's output directory under mail-shots/.
{
  pkgs,
  screens ? false,
}:

let
  lososPkgs = import ../packages.nix { inherit pkgs; };

  # Without screens the admin pages are not under test; one file stands in.
  stubUi = pkgs.runCommand "stub-admin-ui" { } ''
    mkdir -p $out
    echo "admin ui" > $out/index.html
  '';

  shots = pkgs.writeShellScriptBin "mail-screens" ''
    export PLAYWRIGHT_CORE=${pkgs.playwright-driver}
    exec ${pkgs.bun}/bin/bun ${./mail-screens.mjs} "$@"
  '';
  pictures = import ./screens { inherit pkgs; };
in

pkgs.testers.nixosTest {
  name = if screens then "losos-mail-screens" else "losos-mail";
  # The handbook's pictures in three languages take most of an hour on
  # their own under TCG.
  globalTimeout = 3 * 3600;

  nodes = {
    box =
      { lib, ... }:
      {
        imports = [
          ../../modules/options.nix
          ../../modules/containers.nix
          ../../modules/tls.nix
          ../../modules/daemon.nix
          ../../modules/identity.nix
          ../../modules/mail.nix
        ];
        losos = {
          hostName = "mattbox";
          backend.package = lososPkgs.losos-ctl;
          # The mail plugin answers the folder and filter routes.
          backend.plugins = lososPkgs.losos-plugins;
          admin.ui = if screens then lososPkgs.losos-admin-ui else stubUi;
          git.enable = false;
          sharingMyStorage = true;
          # In a mesh, as far as the gate goes; modules/cluster.nix (rke2)
          # is not imported, so nothing joins anything.
          cluster.enable = true;
          shared.fscrypt.enable = false;
        };
        specialisation.closed.configuration.losos.sharingMyStorage = lib.mkForce false;
        # Sharing on without a mesh is not the shared side either.
        specialisation.alone.configuration.losos.cluster.enable = lib.mkForce false;

        # The shared user as modules/configuration.nix makes it, and the
        # data domain modules/impermanence.nix binds in.
        users.groups.shared.gid = 1001;
        users.users.shared = {
          uid = 1001;
          isNormalUser = true;
          group = "shared";
          homeMode = "700";
        };
        systemd.tmpfiles.rules = [ "d /home/shared/data 0700 shared shared -" ];

        systemd.services.lososd.path = lib.mkForce [
          pkgs.coreutils
          # The mail plugin's calls to Stalwart go through the host's curl,
          # as on a real box (modules/daemon.nix).
          pkgs.curl
        ];
        services.authelia.instances.losos.settings.server.endpoints.rate_limits.session_elevation_start.enable =
          false;
        environment.systemPackages = [
          pkgs.jq
          pkgs.curl
          pkgs.iproute2
        ];
        virtualisation = {
          memorySize = 2048;
          cores = 2;
          vlans = [ 1 ];
        };
      };

    laptop =
      { lib, ... }:
      {
        environment.systemPackages = [
          pkgs.curl
          pkgs.jq
        ]
        ++ lib.optionals screens [
          shots
          pictures.capture
        ];
        # What a laptop's mDNS would answer.
        networking.extraHosts = "192.168.1.1 mattbox.local";
        environment.variables = lib.mkIf screens {
          PLAYWRIGHT_BROWSERS_PATH = "${pkgs.playwright-driver.browsers.override {
            withFirefox = false;
            withWebkit = false;
          }}";
          PLAYWRIGHT_SKIP_VALIDATE_HOST_REQUIREMENTS = "true";
        };
        fonts = lib.mkIf screens {
          packages = [
            pkgs.roboto
            pkgs.dejavu_fonts
          ];
          fontconfig.defaultFonts.sansSerif = [ "Roboto" ];
        };
        virtualisation = {
          memorySize = if screens then 3072 else 1024;
          cores = 2;
          vlans = [ 1 ];
        };
      };
  };

  testScript = ''
    import json
    import re
    import time

    SCREENS = ${if screens then "True" else "False"}
    BOX = "192.168.1.1"
    PASSWORD = "Correct-Horse-Battery-9"
    EVA_PASSWORD = "Eva-Long-Password-3"

    ${pictures.handbookScript}

    DOMAIN = "mattbox.home.arpa"
    CSP = (
        "sandbox allow-popups allow-popups-to-escape-sandbox; default-src 'none'; "
        "style-src 'unsafe-inline'; img-src data:; font-src data:; frame-ancestors 'self'; "
        "base-uri 'none'; form-action 'none'"
    )
    # The same, while the message's pictures are hidden (backend/src/pictures.rs).
    CSP_NO_PICTURES = CSP.replace("img-src data:", "img-src 'none'")

    start_all()
    box.wait_for_unit("lososd.service")
    box.wait_for_unit("nginx.service")
    box.wait_for_unit("losos-mail.service")
    box.wait_for_open_port(8096)
    box.wait_for_open_port(443)
    laptop.wait_for_unit("multi-user.target")

    def curl(args):
        return laptop.succeed(f"curl -sk {args}")

    def box_settings(sharing, mesh):
        # What Apply would have written for this state: lososd's settings
        # (and the pages that read them, like the mesh banner) come from
        # overrides.nix, while the gate itself is the evaluated config.
        lines = [
            "{ ... }:",
            "{",
            f"  losos.sharingMyStorage = {str(sharing).lower()};",
            f"  losos.cluster.enable = {str(mesh).lower()};",
            "}",
        ]
        body = "\n".join(lines)
        # Written beside it and moved in, so lososd never reads half a file.
        box.succeed(
            "mkdir -p /etc/nixos/modules"
            f" && cat > /etc/nixos/modules/overrides.nix.new <<'EOF'\n{body}\nEOF\n"
            "mv /etc/nixos/modules/overrides.nix.new /etc/nixos/modules/overrides.nix"
        )

    def mail_page():
        return [curl(f"-o /dev/null -w '%{{http_code}}' https://{BOX}{p}") for p in ["/mail", "/mail/"]]

    def api(path, cookie=None, method="GET", data=None, extra=""):
        args = f"-X {method} 'https://{BOX}{path}' {extra}"
        if cookie:
            args += f" -H 'Cookie: {cookie}'"
        if data is not None:
            body = json.dumps(data).replace("'", "'\\'''")
            args += f" -H 'Content-Type: application/json' -d '{body}'"
        out = curl(f"-w '\\n%{{http_code}}' {args}")
        body, code = out.rsplit("\n", 1)
        return int(code), body

    def ok(path, cookie, **kw):
        code, body = api(path, cookie, **kw)
        assert code == 200, (path, code, body)
        return json.loads(body)

    def sign_in(name, password):
        body = json.dumps({"username": name, "password": password})
        head = curl(
            f"-D - -o /dev/null https://{BOX}/auth/api/firstfactor "
            f"-H 'Content-Type: application/json' -d '{body}'"
        )
        m = re.search(r"(?im)^set-cookie: (losos_session=[^;]+)", head)
        assert m, f"no session cookie for {name}: {head}"
        return m.group(1)

    def folder(home, role):
        return next(m["id"] for m in home["mailboxes"] if m["role"] == role)

    def wait_for(cookie, mailbox, subject, timeout=120):
        """Delivery is queued; poll the list until the message is there."""
        deadline = time.time() + timeout
        while time.time() < deadline:
            page = ok(f"/api/mail/messages?mailbox={mailbox}", cookie)
            found = [m for m in page["messages"] if m["subject"] == subject]
            if found:
                return found[0]
            time.sleep(2)
        raise AssertionError(f"{subject!r} never arrived in {mailbox}: {page}")

    box_settings(True, True)

    with subtest("Stalwart runs as the shared user, on loopback only"):
        pid = box.succeed("systemctl show -p MainPID --value losos-mail.service").strip()
        uid = box.succeed(f"stat -c %u /proc/{pid}").strip()
        assert uid == "1001", uid
        assert box.succeed("stat -c '%U %a' /home/shared/data/mail").strip() == "shared 700"
        box.succeed("test -d /home/shared/data/mail/db")
        local = box.succeed("ss -Hltn 'sport = :8096' | awk '{print $4}'").split()
        assert local == ["127.0.0.1:8096"], local
        laptop.fail(f"curl -s --max-time 5 http://{BOX}:8096/")
        # The secret it shares with lososd is sealed in the credential store,
        # and no plaintext copy is left where an older build kept it.
        assert box.succeed("stat -c '%U %a' /etc/credstore.encrypted/mail-master").strip() == "root 600"
        box.fail("test -e /var/lib/losos-mail/master-secret")
        # Its own mail listeners are not configured: nothing on 25, 143, 993.
        for port in (25, 143, 465, 587, 993):
            box.fail(f"ss -Hltn 'sport = :{port}' | grep -q LISTEN")

    with subtest("the owner claims the box and adds Eva"):
        laptop.wait_until_succeeds(f"curl -sfk https://{BOX}/api/setup/claim | jq -e .ready", timeout=300)
        code, body = api(
            "/api/setup/claim",
            method="POST",
            data={"name": "matus", "password": PASSWORD},
            extra=f"-H 'Host: {BOX}'",
        )
        assert code == 200, (code, body)
        spare = json.loads(body)["token"]
        box.wait_for_unit("authelia-losos.service")
        box.wait_for_open_port(9091)
        box.systemctl("start losos-identity-setup.service")
        box.wait_until_succeeds(f"grep -q \"domain: '{BOX}'\" /run/losos-identity/session.yml")
        box.wait_until_succeeds("systemctl is-active authelia-losos.service")
        box.wait_for_open_port(9091)
        matus = sign_in("matus", PASSWORD)
        code, body = api(
            "/api/people",
            matus,
            method="POST",
            data={"name": "eva", "displayName": "Eva", "password": EVA_PASSWORD, "groups": []},
        )
        assert code == 200, (code, body)
        laptop.wait_until_succeeds(
            f"curl -sk https://{BOX}/auth/api/firstfactor -H 'Content-Type: application/json'"
            f" -d '{json.dumps({"username": "eva", "password": EVA_PASSWORD})}' | grep -q OK",
            timeout=60,
        )
        eva = sign_in("eva", EVA_PASSWORD)

    with subtest("Mail is listed among the shared apps, and its screen is served"):
        assert ok("/api/me", matus)["shared"] == ["mail"]
        assert ok("/api/me", eva)["shared"] == ["mail"]
        # A screen of the admin pages: /mail falls through to their index.
        assert mail_page() == ["200", "200"]
        # The saved settings agree with the gate, so no page (the mesh
        # banner, the About pane) says this box is out of a mesh.
        settings = ok("/api/settings", matus)
        assert settings.get("sharingMyStorage") is True and settings.get("clusterEnable") is True, settings

    with subtest("each person gets a mailbox on first use"):
        mhome = ok("/api/mail", matus)
        ehome = ok("/api/mail", eva)
        assert mhome["address"] == f"matus@{DOMAIN}", mhome
        assert ehome["address"] == f"eva@{DOMAIN}", ehome
        roles = [m["role"] for m in ehome["mailboxes"]]
        assert roles[0] == "inbox" and "sent" in roles and "trash" in roles, roles
        people = {p["address"] for p in mhome["people"]}
        assert f"eva@{DOMAIN}" in people, mhome["people"]

    with subtest("Matus writes to Eva and Eva reads it"):
        ok("/api/mail/send", matus, method="POST", data={
            "to": [f"eva@{DOMAIN}"],
            "subject": "Dinner on Friday",
            "text": "Salmon, of course.\nBring the good knife.",
        })
        got = wait_for(eva, folder(ehome, "inbox"), "Dinner on Friday")
        assert got["from"][0]["email"] == f"matus@{DOMAIN}", got
        assert got["seen"] is False, got
        msg = ok(f"/api/mail/messages/{got['id']}", eva)
        assert msg["text"].startswith("Salmon, of course."), msg
        assert msg["hasHtml"] is False, msg
        # Opening it marks it read.
        again = wait_for(eva, folder(ehome, "inbox"), "Dinner on Friday")
        assert again["seen"] is True, again
        sent = wait_for(matus, folder(mhome, "sent"), "Dinner on Friday")
        assert sent["to"][0]["email"] == f"eva@{DOMAIN}", sent

    with subtest("Eva replies, and the reply threads under the first message"):
        ok("/api/mail/send", eva, method="POST", data={
            "to": f"matus@{DOMAIN}",
            "subject": "Re: Dinner on Friday",
            "text": "Deal.",
            "inReplyTo": got["id"],
        })
        reply = wait_for(matus, folder(mhome, "inbox"), "Re: Dinner on Friday")
        assert reply["threadId"] == sent["threadId"], (reply, sent)

    with subtest("search, flag, unread, move and delete"):
        # The full-text index is written in the background; wait for it.
        deadline = time.time() + 120
        while True:
            page = ok(f"/api/mail/messages?mailbox={folder(ehome, 'inbox')}&q=knife", eva)
            if page["messages"] or time.time() > deadline:
                break
            time.sleep(2)
        assert [m["subject"] for m in page["messages"]] == ["Dinner on Friday"], page
        page = ok(f"/api/mail/messages?mailbox={folder(ehome, 'inbox')}&q=nowhere", eva)
        assert page["messages"] == [], page
        mid = got["id"]
        ok(f"/api/mail/messages/{mid}", eva, method="POST", data={"flagged": True, "seen": False})
        m = wait_for(eva, folder(ehome, "inbox"), "Dinner on Friday")
        assert m["flagged"] is True and m["seen"] is False, m
        ok(f"/api/mail/messages/{mid}", eva, method="POST", data={"mailbox": folder(ehome, "archive")})
        wait_for(eva, folder(ehome, "archive"), "Dinner on Friday")
        ok(f"/api/mail/messages/{mid}", eva, method="DELETE")
        wait_for(eva, folder(ehome, "trash"), "Dinner on Friday")
        # Deleting from the bin deletes for good.
        ok(f"/api/mail/messages/{mid}", eva, method="DELETE")
        code, _ = api(f"/api/mail/messages/{mid}", eva)
        assert code == 404, code
        code, _ = api(f"/api/mail/messages/{mid}", eva, method="POST", data={"nothing": 1})
        assert code == 400, code

    with subtest("formatted mail is drawn only from its own sandboxed route"):
        # A sender's HTML, put in Eva's inbox the way delivery would: through
        # Stalwart's JMAP as Eva, from the box itself.
        secret = box.succeed(
            "systemd-creds decrypt --name=mail-master /etc/credstore.encrypted/mail-master -"
        ).strip()
        jmap = f"curl -sf -u 'eva%losos:{secret}' http://127.0.0.1:8096"
        session = json.loads(box.succeed(f"{jmap}/jmap/session"))
        account = session["primaryAccounts"]["urn:ietf:params:jmap:mail"]
        inbox = folder(ehome, "inbox")
        html = "<p style='color:red'>Hello <b>Eva</b></p><script>parent.alert(1)</script><img src='https://tracker.example/p.gif'>"
        call = {
            "using": ["urn:ietf:params:jmap:core", "urn:ietf:params:jmap:mail"],
            "methodCalls": [["Email/set", {"accountId": account, "create": {"h": {
                "mailboxIds": {inbox: True},
                "from": [{"name": "Shop", "email": "news@shop.example"}],
                "to": [{"email": f"eva@{DOMAIN}"}],
                "subject": "Autumn sale",
                "bodyStructure": {"type": "text/html", "partId": "1"},
                "bodyValues": {"1": {"value": html}},
            }}}, "s"]],
        }
        box.succeed(f"cat > /tmp/html.json <<'EOF'\n{json.dumps(call)}\nEOF")
        out = json.loads(box.succeed(f"{jmap}/jmap/ -H 'Content-Type: application/json' -d @/tmp/html.json"))
        assert "h" in out["methodResponses"][0][1]["created"], out
        sale = wait_for(eva, inbox, "Autumn sale")
        msg = ok(f"/api/mail/messages/{sale['id']}", eva)
        assert msg["hasHtml"] is True, msg
        # Pictures are hidden until the person shows them: lososd's policy
        # then lets no picture load, and the browser keeps to the stricter of
        # its and nginx's.
        head = curl(f"-D - -o /tmp/frame.html -H 'Cookie: {eva}' https://{BOX}/api/mail/messages/{sale['id']}/html")
        csp = re.findall(r"(?im)^content-security-policy: (.*?)\r?$", head)
        assert CSP_NO_PICTURES in csp and set(csp) <= {CSP, CSP_NO_PICTURES}, (csp, head)
        assert "img{color:transparent" in laptop.succeed("cat /tmp/frame.html")
        head = curl(f"-D - -o /tmp/frame.html -H 'Cookie: {eva}' 'https://{BOX}/api/mail/messages/{sale['id']}/html?pictures=1'")
        csp = re.findall(r"(?im)^content-security-policy: (.*?)\r?$", head)
        # lososd and nginx both send it, the same text: either alone holds.
        assert csp and all(c == CSP for c in csp), (csp, head)
        assert not re.search(r"(?im)^x-frame-options:", head), head
        assert re.search(r"(?im)^x-content-type-options: nosniff", head), head
        frame = laptop.succeed("cat /tmp/frame.html")
        assert frame.startswith('<!doctype html><meta charset="utf-8"><base target="_blank">'), frame
        # Every other route keeps the admin page's own policy and frame rule.
        head = curl(f"-D - -o /dev/null -H 'Cookie: {eva}' https://{BOX}/api/mail")
        assert "sandbox" not in head and re.search(r"(?im)^x-frame-options: DENY", head), head

    with subtest("nobody reads someone else's mail"):
        # Ids are per account, so the same id may name one of Matus's own
        # messages; what he gets is never Eva's.
        code, body = api(f"/api/mail/messages/{sale['id']}", matus)
        assert code == 404 or json.loads(body)["subject"] != "Autumn sale", (code, body)
        code, body = api(f"/api/mail/messages/{sale['id']}/html", matus)
        assert code == 404 or "Hello" not in body, (code, body)
        page = ok(f"/api/mail/messages?mailbox={folder(mhome, 'inbox')}&q=sale", matus)
        assert page["messages"] == [], page
        assert wait_for(eva, inbox, "Autumn sale")["id"] == sale["id"]

    with subtest("refusals: another domain, a bad address, no one to send to"):
        for data, want in [
            ({"to": ["someone@example.com"], "subject": "x", "text": "x"}, 422),
            ({"to": ["not an address"], "subject": "x", "text": "x"}, 400),
            ({"to": [], "subject": "x", "text": "x"}, 400),
            ({"to": [f"eva@{DOMAIN}"], "subject": "two\nlines", "text": "x"}, 400),
        ]:
            code, body = api("/api/mail/send", matus, method="POST", data=data)
            assert code == want, (data, code, body)
        code, _ = api("/api/mail/messages/..%2Fetc", eva)
        assert code in (400, 404), code

    with subtest("the spare key has no mailbox, and signed out is signed out"):
        code, body = api("/api/mail", extra=f"-H 'Authorization: Bearer {spare}'")
        assert code == 409, (code, body)
        assert ok("/api/me", None, extra=f"-H 'Authorization: Bearer {spare}'").get("shared", []) == []
        code, _ = api("/api/mail")
        assert code == 401, code
        code, _ = api("/api/mail", extra="-H 'Remote-User: eva'")
        assert code == 401, code

    with subtest("the owner's inbox: mail from Eva and Peter"):
        code, body = api(
            "/api/people",
            matus,
            method="POST",
            data={"name": "peter", "displayName": "Peter", "password": "Peter-Long-Password-5", "groups": []},
        )
        assert code == 200, (code, body)
        laptop.wait_until_succeeds(
            f"curl -sk https://{BOX}/auth/api/firstfactor -H 'Content-Type: application/json'"
            f" -d '{json.dumps({"username": "peter", "password": "Peter-Long-Password-5"})}' | grep -q OK",
            timeout=60,
        )
        peter = sign_in("peter", "Peter-Long-Password-5")
        ok("/api/mail/send", peter, method="POST", data={
            "to": [f"matus@{DOMAIN}"],
            "subject": "Fix the backup script",
            "text": "The Sunday backup stopped at the photos folder again. Can you have a look tonight?",
        })
        ok("/api/mail/send", eva, method="POST", data={
            "to": [f"matus@{DOMAIN}"],
            "subject": "Photos from the lake",
            "text": "Hi Matus,\n\nI put the photos from Saturday in the shared folder. The one of the boat at sunset is my favourite.\n\nEva",
        })
        wait_for(matus, folder(mhome, "inbox"), "Fix the backup script")
        wait_for(matus, folder(mhome, "inbox"), "Photos from the lake")

    secret = box.succeed(
        "systemd-creds decrypt --name=mail-master /etc/credstore.encrypted/mail-master -"
    ).strip()

    def jmap_as(name, calls, using=("core", "mail")):
        """A JMAP request straight to Stalwart as `name`, from the box: no lososd."""
        body = {"using": [f"urn:ietf:params:jmap:{u}" for u in using], "methodCalls": calls}
        box.succeed(f"cat > /tmp/jmap.json <<'EOF'\n{json.dumps(body)}\nEOF")
        out = box.succeed(
            f"curl -sf -u '{name}%losos:{secret}' http://127.0.0.1:8096/jmap/"
            " -H 'Content-Type: application/json' -d @/tmp/jmap.json"
        )
        return [r[1] for r in json.loads(out)["methodResponses"]]

    def account_of(name):
        session = json.loads(box.succeed(f"curl -sf -u '{name}%losos:{secret}' http://127.0.0.1:8096/jmap/session"))
        return session["primaryAccounts"]["urn:ietf:params:jmap:mail"]

    def named(cookie, name):
        return next(m for m in ok("/api/mail", cookie)["mailboxes"] if m["name"] == name)

    with subtest("a person's own folders: made, nested, renamed, moved and deleted"):
        shop = ok("/api/mail/folders", eva, method="POST", data={"name": "Shop", "parentId": None})["id"]
        receipts = ok("/api/mail/folders", eva, method="POST", data={"name": "Receipts", "parentId": shop})["id"]
        assert named(eva, "Receipts")["parentId"] == shop
        assert named(eva, "Shop")["parentId"] is None
        ok(f"/api/mail/folders/{receipts}", eva, method="PUT", data={"name": "Receipts 2026"})
        assert named(eva, "Receipts 2026")["id"] == receipts
        # Each refusal carries the word the page says it by.
        for path, method, data, want, word in [
            ("/api/mail/folders", "POST", {"name": "Shop", "parentId": None}, 409, "exists"),
            ("/api/mail/folders", "POST", {"name": "a/b", "parentId": None}, 400, "badName"),
            (f"/api/mail/folders/{shop}", "PUT", {"parentId": receipts}, 409, "loop"),
            (f"/api/mail/folders/{shop}", "DELETE", None, 409, "hasFolders"),
            (f"/api/mail/folders/{folder(ehome, 'inbox')}", "PUT", {"name": "Mine"}, 403, "own"),
            (f"/api/mail/folders/{folder(ehome, 'trash')}", "DELETE", None, 403, "own"),
        ]:
            code, body = api(path, eva, method=method, data=data)
            assert code == want and json.loads(body)["code"] == word, (path, method, code, body)
        ok(f"/api/mail/folders/{receipts}", eva, method="PUT", data={"parentId": None})
        assert named(eva, "Receipts 2026")["parentId"] is None
        # A folder's mail goes to the bin before the folder goes, and a
        # filter that filed into it is turned off.
        ok("/api/mail/send", matus, method="POST", data={
            "to": [f"eva@{DOMAIN}"], "subject": "Receipt for the knife", "text": "34 EUR, paid.",
        })
        knife = wait_for(eva, folder(ehome, "inbox"), "Receipt for the knife")
        ok(f"/api/mail/messages/{knife['id']}", eva, method="POST", data={"mailbox": shop})
        wait_for(eva, shop, "Receipt for the knife")
        ok("/api/mail/filters", eva, method="PUT", data={"filters": [{
            "name": "Shop", "enabled": True, "match": "all",
            "conditions": [{"field": "from", "value": "shop.example"}],
            "moveTo": shop, "markRead": False, "flag": False,
        }]})
        ok(f"/api/mail/folders/{receipts}", eva, method="DELETE")
        assert ok(f"/api/mail/folders/{shop}", eva, method="DELETE") == {"moved": 1}
        wait_for(eva, folder(ehome, "trash"), "Receipt for the knife")
        names = [m["name"] for m in ok("/api/mail", eva)["mailboxes"]]
        assert "Shop" not in names and "Receipts 2026" not in names, names
        [left] = ok("/api/mail/filters", eva)["filters"]
        assert left["enabled"] is False and left["moveTo"] is None, left
        # Folders are each person's own: Matus has none of Eva's.
        assert all(m["role"] is not None for m in ok("/api/mail", matus)["mailboxes"])

    with subtest("filters are kept by the mail server and run with Mail closed"):
        projects = ok("/api/mail/folders", matus, method="POST", data={"name": "Projects", "parentId": None})["id"]
        losos_box = ok("/api/mail/folders", matus, method="POST", data={"name": "LosOS box", "parentId": projects})["id"]
        filters = [
            {
                "name": "Backup reports", "enabled": True, "match": "all",
                "conditions": [{"field": "from", "value": "peter@"}, {"field": "subject", "value": "backup"}],
                "moveTo": losos_box, "markRead": True, "flag": True,
            },
            {
                "name": "Newsletters", "enabled": False, "match": "any",
                "conditions": [{"field": "subject", "value": "newsletter"}],
                "moveTo": folder(mhome, "trash"), "markRead": False, "flag": False,
            },
        ]
        assert ok("/api/mail/filters", matus, method="PUT", data={"filters": filters})["filters"] == filters
        assert ok("/api/mail/filters", matus)["filters"] == filters
        code, body = api("/api/mail/filters", matus, method="PUT", data={"filters": [{**filters[0], "moveTo": "zz9"}]})
        assert code == 400, (code, body)
        code, body = api("/api/mail/filters", matus, method="PUT", data={"filters": [{**filters[0], "conditions": []}]})
        assert code == 400, (code, body)
        assert ok("/api/mail/filters", matus)["filters"] == filters, "a refused save changed the filters"

        # What Stalwart runs, read from Stalwart: one active script, the
        # filter that is on as Sieve, the one that is off only in the comment.
        maccount = account_of("matus")
        [got] = jmap_as("matus", [["SieveScript/get", {"accountId": maccount}, "g"]], ("core", "sieve"))
        [script] = got["list"]
        assert script["name"] == "losos" and script["isActive"] is True, script
        sieve = box.succeed(
            f"curl -sf -u 'matus%losos:{secret}' "
            f"'http://127.0.0.1:8096/jmap/download/{maccount}/{script['blobId']}/losos.sieve?accept=application/sieve'"
        )
        assert f'fileinto :mailboxid "{losos_box}" "LosOS box";' in sieve, sieve
        assert 'addflag ["\\\\Seen", "\\\\Flagged"];' in sieve, sieve
        assert "newsletter" not in sieve.split("\n", 1)[1], sieve

        # Mail is closed: lososd is not even running. Peter writes through
        # Stalwart itself, and the server files the message on its own.
        box.succeed("systemctl stop lososd.service")
        paccount = account_of("peter")
        boxes, idents = jmap_as("peter", [
            ["Mailbox/get", {"accountId": paccount, "properties": ["role"]}, "m"],
            ["Identity/get", {"accountId": paccount}, "i"],
        ], ("core", "mail", "submission"))
        drafts = next(b["id"] for b in boxes["list"] if b["role"] == "drafts")
        # onSuccessDestroyEmail adds an implicit Email/set answer at the end.
        made, submitted, *_ = jmap_as("peter", [
            ["Email/set", {"accountId": paccount, "create": {"d": {
                "mailboxIds": {drafts: True},
                "keywords": {"$draft": True},
                "from": [{"name": "Peter", "email": f"peter@{DOMAIN}"}],
                "to": [{"email": f"matus@{DOMAIN}"}],
                "subject": "Backup log: Sunday",
                "bodyValues": {"b": {"value": "All 4 folders copied, 38 GiB, no errors."}},
                "textBody": [{"partId": "b", "type": "text/plain"}],
            }}}, "e"],
            ["EmailSubmission/set", {
                "accountId": paccount,
                "create": {"s": {"identityId": idents["list"][0]["id"], "emailId": "#d"}},
                "onSuccessDestroyEmail": ["#s"],
            }, "s"],
        ], ("core", "mail", "submission"))
        assert "s" in submitted.get("created", {}), (made, submitted)
        deadline = time.time() + 120
        while True:
            query, found = jmap_as("matus", [
                ["Email/query", {"accountId": maccount, "filter": {"subject": "Backup log: Sunday"}}, "q"],
                ["Email/get", {
                    "accountId": maccount,
                    "#ids": {"resultOf": "q", "name": "Email/query", "path": "/ids"},
                    "properties": ["mailboxIds", "keywords"],
                }, "g"],
            ])
            if found["list"] or time.time() > deadline:
                break
            time.sleep(2)
        [delivered] = found["list"]
        assert delivered["mailboxIds"] == {losos_box: True}, delivered
        assert delivered["keywords"].get("$seen") and delivered["keywords"].get("$flagged"), delivered
        box.succeed("systemctl start lososd.service")
        box.wait_for_open_port(8082)
        laptop.wait_until_succeeds(f"curl -sk -o /dev/null -w '%{{http_code}}' -H 'Cookie: {matus}' https://{BOX}/api/me | grep -q 200", timeout=60)

        # In Mail it is in LosOS box, read and flagged; what came before the
        # filter stays where it was.
        m = wait_for(matus, losos_box, "Backup log: Sunday")
        assert m["seen"] is True and m["flagged"] is True, m
        wait_for(matus, folder(mhome, "inbox"), "Fix the backup script")
        tree = {f["name"]: f["parentId"] for f in ok("/api/mail", matus)["mailboxes"]}
        assert tree["LosOS box"] == projects and tree["Projects"] is None, tree

    def screens(*args):
        if SCREENS:
            quoted = " ".join("'" + a + "'" for a in args)
            print(laptop.succeed(f"MAIL_SCREENS_PASSWORD='{PASSWORD}' mail-screens {quoted}", timeout=3600))

    with subtest("sharing on but in no mesh is not the shared side"):
        box_settings(True, False)
        box.succeed("/run/current-system/specialisation/alone/bin/switch-to-configuration test")
        box.wait_until_fails("systemctl is-active losos-mail.service", timeout=60)
        box.wait_for_unit("lososd.service")
        laptop.wait_until_succeeds(f"curl -sk -o /dev/null -w '%{{http_code}}' -H 'Cookie: {eva}' https://{BOX}/api/me | grep -q 200", timeout=60)
        assert ok("/api/me", eva)["shared"] == [], "shared should be empty without a mesh"
        code, _ = api("/api/mail", eva)
        assert code == 404, code
        assert mail_page() == ["404", "404"]
        box_settings(True, True)
        box.succeed("/run/booted-system/bin/switch-to-configuration test")
        box.wait_for_unit("losos-mail.service")
        box.wait_for_open_port(8096)
        laptop.wait_until_succeeds(f"curl -sk -H 'Cookie: {eva}' https://{BOX}/api/me | jq -e '.shared == [\"mail\"]'", timeout=60)

    with subtest("with sharing off there is no mail at all"):
        box_settings(False, True)
        box.succeed("/run/current-system/specialisation/closed/bin/switch-to-configuration test")
        box.wait_until_fails("systemctl is-active losos-mail.service", timeout=60)
        box.wait_for_unit("lososd.service")
        laptop.wait_until_succeeds(f"curl -sk -o /dev/null -w '%{{http_code}}' -H 'Cookie: {eva}' https://{BOX}/api/me | grep -q 200", timeout=60)
        assert ok("/api/me", eva)["shared"] == [], "shared should be empty"
        for path in [
            "/api/mail",
            f"/api/mail/messages?mailbox={inbox}",
            f"/api/mail/messages/{sale['id']}",
            # The mail plugin's routes close with the rest.
            "/api/mail/filters",
        ]:
            code, body = api(path, eva)
            assert code == 404, (path, code, body)
        code, _ = api("/api/mail/send", matus, method="POST", data={"to": [f"eva@{DOMAIN}"], "text": "x"})
        assert code == 404, code
        assert mail_page() == ["404", "404"]
        # The mailboxes stay where they were, in the shared domain.
        box.succeed("test -d /home/shared/data/mail/db")
        screens("sidebar", "01", "sidebar-sharing-off", "the sidebar while the box does not share", "none")

    with subtest("with sharing back on, the mail is still there"):
        box_settings(True, True)
        box.succeed("/run/booted-system/bin/switch-to-configuration test")
        box.wait_for_unit("losos-mail.service")
        box.wait_for_open_port(8096)
        box.wait_for_unit("lososd.service")
        laptop.wait_until_succeeds(f"curl -sk -o /dev/null -w '%{{http_code}}' -H 'Cookie: {eva}' https://{BOX}/api/me | grep -q 200", timeout=60)
        assert ok("/api/me", eva)["shared"] == ["mail"]
        assert wait_for(eva, inbox, "Autumn sale")["id"] == sale["id"]
        assert mail_page() == ["200", "200"]
        screens("sidebar", "02", "sidebar-sharing-on", "the sidebar with sharing on: Mail is there", "mail")

    with subtest("Matus reads and answers his mail in the browser"):
        screens("mail", "3")

    with subtest("Matus's folders and filters in the browser"):
        screens("folders", "20")

    if SCREENS:
        with subtest("the handbook's pictures"):
            lake = {"text": "Photos from the lake"}
            handbook_pictures(laptop, "https://mattbox.local", "matus", PASSWORD, [
                {"name": "mail-inbox", "path": "/mail", "waitFor": lake},
                {"name": "mail-message", "path": "/mail", "waitFor": lake,
                 "steps": [{"click": lake}, {"sleep": 3000}]},
            ])

    if SCREENS:
        laptop.succeed("mail-screens manifest losos-mail-screens")
        # copy_from_machine goes through the driver's shared directory, which
        # outlives a run; a second run would land in mail-shots/shots/shots.
        laptop.succeed("rm -rf /tmp/shared/mail-shots")
        laptop.copy_from_machine("/tmp/shots", "mail-shots")
  '';
}
