# nixos-test-vms config for sign-in (modules/identity.nix).
#
# Authelia in front of the admin API, lososd behind it, nginx between them,
# and a laptop on the LAN that does what a browser does: claims the box,
# signs in at /auth/, and calls the API with the session cookie. Every part
# of this fails closed in a way nothing at build time can see, so it is a
# booted test:
#
#   * Authelia must not start before the claim (it would write an example
#     account of its own) and must start after it without a reboot;
#   * the people file lososd writes must be one Authelia reads, hash
#     included;
#   * nginx must pass the person on and drop a `Remote-User` a client made
#     up, and must let the spare key past Authelia;
#   * the sections in backend/sections.json must hold: a person in
#     losos-backup reaches the backup routes and nothing an administrator
#     owns;
#   * the Users pane's routes add, change and remove people, refuse
#     anyone but an administrator, and never touch the owner;
#   * a password change and a removed account must take effect at once,
#     in the admin pages and in the apps.
#
# LosOS cloud is a stub: lososd runs in native mode with a `nextcloud-occ`
# that says "installed" and takes any password, which is all the claim asks
# of it. Booting Nextcloud would test Nextcloud.
#
# The front vhost, though, is in its container shape, with both apps'
# routes, and on the two loopback ports a stand-in answers with the path
# and headers it was sent. That is where signing in to LosOS cloud and LosOS
# Git with LosOS Login (modules/app-sign-in.nix) is decided: who nginx names
# to each app, on which pages, and where it sends a person LosOS Login
# does not know. nix/tests/app-sign-in.nix boots the two real apps on those
# headers.
{ pkgs }:

let
  lososPkgs = import ../packages.nix { inherit pkgs; };

  stubOcc = pkgs.writeShellScriptBin "nextcloud-occ" ''
    case "$1" in
      status) echo '{"installed":true,"maintenance":false,"needsDbUpgrade":false}' ;;
      user:resetpassword) echo "Successfully reset password for $2" ;;
      *) echo "stub occ: $*" >&2; exit 1 ;;
    esac
  '';

  # Each app's port answers with what it was sent.
  echo = pkgs.writeText "echo.py" ''
    import http.server, json, sys

    class Echo(http.server.BaseHTTPRequestHandler):
        def answer(self):
            if self.path.endswith("/logout"):
                self.send_response(303)
                host = self.headers["Host"]
                self.send_header("Location", f"http://{host}/nextcloud/login?clear=1")
                self.send_header("Content-Length", "0")
                self.end_headers()
                return
            body = json.dumps({
                "path": self.path,
                "headers": {k.lower(): v for k, v in self.headers.items()},
            }).encode()
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
        do_GET = do_POST = answer

    http.server.ThreadingHTTPServer(("127.0.0.1", int(sys.argv[1])), Echo).serve_forever()
  '';
  echoService = port: {
    wantedBy = [ "multi-user.target" ];
    serviceConfig = {
      ExecStart = "${pkgs.python3}/bin/python3 ${echo} ${toString port}";
      DynamicUser = true;
    };
  };
  # Any derivation with the two attributes imageRef reads: no cluster runs,
  # so the image is never pulled (nix/tests/forgejo-federation.nix).
  standIn =
    name:
    pkgs.hello
    // {
      imageName = name;
      imageTag = "test";
    };

  # The admin pages themselves are not under test; one file stands in.
  stubUi = pkgs.runCommand "stub-admin-ui" { } ''
    mkdir -p $out
    echo "admin ui" > $out/index.html
  '';
in

pkgs.testers.nixosTest {
  name = "losos-identity";

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
          ../../modules/workloads.nix
        ];
        losos = {
          hostName = "mattbox";
          backend.package = lososPkgs.losos-ctl;
          admin.ui = stubUi;
          nextcloud.mode = "container";
          forgejo.enable = true;
          forgejo.mode = "container";
          workloads.nextcloudImage = standIn "losos-nextcloud-stand-in";
          workloads.forgejoImage = standIn "losos-forgejo-stand-in";
        };
        systemd.services.lososd.path = lib.mkForce [
          stubOcc
          pkgs.coreutils
        ];
        # The stub above, not a pod.
        systemd.services.lososd.environment.LOSOS_NEXTCLOUD_MODE = lib.mkForce "native";
        systemd.services.nextcloud-stand-in = echoService 11000;
        systemd.services.forgejo-stand-in = echoService 3000;
        environment.systemPackages = [ pkgs.jq ];
        virtualisation = {
          memorySize = 1536;
          cores = 2;
          vlans = [ 1 ];
        };
      };

    laptop =
      { ... }:
      {
        environment.systemPackages = [
          pkgs.curl
          pkgs.jq
        ];
        virtualisation.vlans = [ 1 ];
      };
  };

  testScript = ''
    import json
    import re

    BOX = "192.168.1.1"
    PASSWORD = "Correct-Horse-Battery-9"
    PEOPLE = "/var/lib/losos-identity/users.yml"

    start_all()
    box.wait_for_unit("lososd.service")
    box.wait_for_unit("nginx.service")
    box.wait_for_open_port(443)
    laptop.wait_for_unit("multi-user.target")

    def curl(args):
        return laptop.succeed(f"curl -sk {args}")

    def status(args):
        return curl(f"-o /dev/null -w '%{{http_code}}' {args}").strip()

    def sign_in(name, password):
        """Sign in at /auth/ the way the portal does; return the cookie."""
        body = json.dumps({"username": name, "password": password})
        head = curl(
            f"-D - -o /dev/null https://{BOX}/auth/api/firstfactor "
            f"-H 'Content-Type: application/json' -d '{body}'"
        )
        m = re.search(r"(?im)^set-cookie: (losos_session=[^;]+)", head)
        assert m, f"no session cookie for {name}: {head}"
        return m.group(1)

    def journal_mark():
        """Where Authelia's journal ends now; see reloaded_since."""
        return box.succeed(
            "journalctl -u authelia-losos -n 0 --show-cursor | sed -n 's/^-- cursor: //p'"
        ).strip()

    def reloaded_since(mark):
        """Wait until Authelia has read the people file again after `mark`."""
        box.wait_until_succeeds(
            f"journalctl -u authelia-losos --after-cursor='{mark}'"
            " | grep -q 'Reloaded successfully'",
            timeout=60,
        )

    def api(path, cookie=None, method="GET", data=None, extra=""):
        args = f"-X {method} https://{BOX}{path} {extra}"
        if cookie:
            args += f" -H 'Cookie: {cookie}'"
        if data is not None:
            args += f" -H 'Content-Type: application/json' -d '{json.dumps(data)}'"
        out = curl(f"-w '\\n%{{http_code}}' {args}")
        body, code = out.rsplit("\n", 1)
        return int(code), body

    with subtest("before the claim nobody can sign in and Authelia stays off"):
        box.fail(f"test -e {PEOPLE}")
        box.succeed("systemctl is-active losos-identity.path")
        box.fail("systemctl is-active authelia-losos.service")
        state = json.loads(curl(f"https://{BOX}/api/setup/claim"))
        assert state["claimed"] is False and state["people"] is False, state
        code, _ = api("/api/state")
        assert code != 200, code

    with subtest("the claim makes the first person, an administrator"):
        code, body = api(
            "/api/setup/claim",
            method="POST",
            data={"name": "matus", "password": PASSWORD},
            extra=f"-H 'Host: {BOX}'",
        )
        assert code == 200, (code, body)
        claim = json.loads(body)
        assert claim["name"] == "matus", claim
        spare = claim["token"]
        assert re.fullmatch(r"[0-9a-f]{64}", spare), claim
        people = json.loads(box.succeed(f"cat {PEOPLE}"))
        matus = people["users"]["matus"]
        assert matus["groups"] == ["losos-admins", "losos-owner"], matus
        assert matus["password"].startswith("$argon2id$v=19$m=65536,t=3,p=4$"), matus
        assert box.succeed(f"stat -c '%a %U %G' {PEOPLE}").strip() == "640 root authelia-losos"
        assert json.loads(box.succeed("cat /var/lib/losos/state.json"))["owner"] == "matus"
        box.wait_for_unit("authelia-losos.service")
        box.wait_for_open_port(9091)
        state = json.loads(curl(f"https://{BOX}/api/setup/claim"))
        assert state["claimed"] is True and state["people"] is True, state

    with subtest("the session cookie works for the box's address"):
        box.systemctl("start losos-identity-setup.service")
        box.wait_until_succeeds(f"grep -q \"domain: '{BOX}'\" /run/losos-identity/session.yml")
        box.wait_until_succeeds("systemctl is-active authelia-losos.service")
        box.wait_for_open_port(9091)

    with subtest("signed out, the API sends the browser to Authelia"):
        code, _ = api("/api/state")
        assert code == 401, code
        # A made-up person in a header is not a sign-in.
        code, _ = api("/api/me", extra="-H 'Remote-User: matus' -H 'X-Losos-As: matus'")
        assert code == 401, code
        # The portal is https only.
        loc = curl(f"-o /dev/null -w '%{{redirect_url}}' http://{BOX}/auth/")
        assert loc == f"https://{BOX}/auth/", loc

    with subtest("a wrong password is refused"):
        body = json.dumps({"username": "matus", "password": "not-the-password-1A"})
        code = status(
            f"https://{BOX}/auth/api/firstfactor -H 'Content-Type: application/json' -d '{body}'"
        )
        assert code == "401", code

    with subtest("signed in, the administrator reaches everything"):
        cookie = sign_in("matus", PASSWORD)
        code, body = api("/api/me", cookie)
        assert code == 200, (code, body)
        me = json.loads(body)
        assert me["name"] == "matus" and me["admin"] is True and me["spareKey"] is False, me
        assert "backup" in me["sections"] and "storage" in me["sections"], me
        assert api("/api/state", cookie)[0] == 200
        assert api("/api/options", cookie)[0] != 403

    with subtest("the spare key opens the box without Authelia"):
        code, body = api("/api/me", extra=f"-H 'Authorization: Bearer {spare}'")
        assert code == 200 and json.loads(body)["spareKey"] is True, (code, body)
        assert status(f"http://{BOX}/auth/api/health") == "200"
        box.systemctl("stop authelia-losos.service")
        code, _ = api("/api/state", extra=f"-H 'Authorization: Bearer {spare}'")
        assert code == 200, code
        # The admin pages ask this, over plain http too, and wait while it
        # is not 200 rather than send anyone to an error page.
        assert status(f"http://{BOX}/auth/api/health") == "502"
        # Without the key, Authelia being down reads as signed out, not 500.
        code, _ = api("/api/me")
        assert code == 401, code
        box.systemctl("start authelia-losos.service")
        box.wait_for_open_port(9091)
        laptop.wait_until_succeeds(f"curl -sf http://{BOX}/auth/api/health", timeout=60)
        # The spare key cannot make a second first administrator.
        code, _ = api(
            "/api/setup/claim",
            method="POST",
            data={"name": "mallory", "password": PASSWORD},
            extra=f"-H 'Host: {BOX}' -H 'Authorization: Bearer {spare}'",
        )
        assert code == 409, code
        # Nor can anyone claim the box again.
        code, _ = api(
            "/api/setup/claim",
            method="POST",
            data={"name": "mallory", "password": "Another-Long-Pass-7"},
            extra=f"-H 'Host: {BOX}'",
        )
        assert code == 409, code

    with subtest("the Users pane lists people and adds one, and only for administrators"):
        cookie = sign_in("matus", PASSWORD)
        me = json.loads(api("/api/me", cookie)[1])
        assert me["standalone"] is True, me
        code, body = api("/api/people", cookie)
        assert code == 200, (code, body)
        listed = json.loads(body)
        assert listed["standalone"] is True and listed["you"] == "matus", listed
        assert [p["name"] for p in listed["people"]] == ["matus"], listed
        assert "backup" in listed["sections"], listed
        assert "argon2" not in body and "password" not in body, body
        # Groups the pane does not hand out, a taken name and a short
        # password are refused before anything is written.
        before = box.succeed(f"sha256sum {PEOPLE}")
        for data, want in [
            ({"name": "eva", "password": "Eva-Long-Password-3", "groups": ["losos-owner"]}, 400),
            ({"name": "eva", "password": "Eva-Long-Password-3", "groups": ["wheel"]}, 400),
            ({"name": "matus", "password": "Eva-Long-Password-3", "groups": []}, 409),
            ({"name": "notshared", "password": "Eva-Long-Password-3", "groups": []}, 400),
            ({"name": "eva", "password": "short", "groups": []}, 400),
        ]:
            code, body = api("/api/people", cookie, method="POST", data=data)
            assert code == want, (data, code, body)
        assert box.succeed(f"sha256sum {PEOPLE}") == before
        # Eva arrives through the Users pane's own route.
        mark = journal_mark()
        code, body = api(
            "/api/people",
            cookie,
            method="POST",
            data={
                "name": "eva",
                "displayName": "Eva",
                "email": "eva@mattbox.local",
                "password": "Eva-Long-Password-3",
                "groups": ["losos-backup"],
            },
        )
        assert code == 200, (code, body)
        reloaded_since(mark)
        file = json.loads(box.succeed(f"cat {PEOPLE}"))
        assert file["users"]["eva"]["groups"] == ["losos-backup"], file
        assert box.succeed(f"stat -c '%a %U %G' {PEOPLE}").strip() == "640 root authelia-losos"
        audit = box.succeed("grep '/api/people' /var/lib/losos/audit.log")
        assert '"who":"matus"' in audit and '"outcome":"ok"' in audit, audit

    with subtest("a person in one section gets that section and no more"):
        eva = sign_in("eva", "Eva-Long-Password-3")
        code, body = api("/api/me", eva)
        me = json.loads(body)
        assert me["admin"] is False and me["sections"] == ["backup"], me
        assert api("/api/state", eva)[0] == 200
        assert api("/api/backup", eva)[0] != 403
        code, body = api("/api/grow", eva, method="POST")
        assert code == 403 and json.loads(body)["section"] == "storage", (code, body)
        code, body = api("/api/recovery", eva)
        assert code == 403 and json.loads(body)["section"] == "admin", (code, body)
        code, _ = api("/api/apply", eva, method="POST", data="{}")
        assert code == 403, code
        for path, method in [("/api/people", "GET"), ("/api/people", "POST"),
                             ("/api/people/remove", "POST")]:
            code, body = api(path, eva, method=method, data={"name": "matus"})
            assert code == 403 and json.loads(body)["section"] == "admin", (path, code, body)
        audit = box.succeed("grep '\"who\":\"eva\"' /var/lib/losos/audit.log")
        assert '"outcome":"forbidden"' in audit, audit

    def app(path, cookie=None, extra="", host=BOX, via=None):
        """What an app's port was sent for `path`, or the redirect nginx gave."""
        args = f"-o /tmp/app -w '%{{http_code}} %{{redirect_url}}' {extra} https://{host}{path}"
        if cookie:
            args += f" -H 'Cookie: {cookie}'"
        node = via or laptop
        code, _, where = node.succeed(f"curl -sk {args}").partition(" ")
        if code != "200":
            return int(code), where
        return 200, json.loads(node.succeed("cat /tmp/app"))

    def person(answer):
        code, sent = answer
        assert code == 200, answer
        h = sent["headers"]
        return (
            h.get("x-losos-cloud-user"),
            h.get("x-losos-cloud-name"),
            h.get("x-losos-cloud-email"),
        )

    with subtest("LosOS Git hears who LosOS Login vouches for, and nobody else"):
        # Authelia was restarted above, and its sessions live in memory.
        cookie = sign_in("matus", PASSWORD)
        got = person(app("/forgejo/", cookie))
        # The claim's address is <name>@<the box's hostname>.local.
        assert got[:2] == ("notshared", "matus") and got[2].startswith("matus@"), got
        got = person(app("/forgejo/explore/repos", eva))
        assert got == ("eva", "Eva", "eva@mattbox.local"), got
        assert person(app("/forgejo/")) == (None, None, None)
        forged = "-H 'X-Losos-Cloud-User: notshared' -H 'X-Losos-Cloud-Name: x'"
        assert person(app("/forgejo/", extra=forged)) == (None, None, None)
        assert person(app("/forgejo/", eva, extra=forged))[0] == "eva"
        # git and the API bring credentials of their own.
        assert person(app("/forgejo/x.git/info/refs", cookie, extra="-u matus:pw"))[0] is None
        # The prefix is stripped, as before.
        assert app("/forgejo/explore/repos", eva)[1]["path"] == "/explore/repos"

    with subtest("each app's sign-in page goes to LosOS Login, or past it"):
        signin = f"https://{BOX}/auth/?rd=https://{BOX}"
        for page, back in [("/nextcloud/login", "/nextcloud/login"),
                           ("/nextcloud/index.php/login", "/nextcloud/login"),
                           ("/nextcloud/apps/user_saml/saml/login", "/nextcloud/login"),
                           ("/forgejo/user/login", "/forgejo/user/login")]:
            code, where = app(page)
            assert code == 302 and where == signin + back, (page, code, where)
        code, sent = app("/nextcloud/login?redirect_url=/nextcloud/apps/files/", cookie)
        assert person((code, sent))[0] == "notshared", sent
        assert sent["path"] == "/nextcloud/login?redirect_url=/nextcloud/apps/files/", sent
        assert person(app("/nextcloud/index.php/apps/user_saml/saml/login", eva))[0] == "eva"
        code, sent = app("/forgejo/user/login?redirect_to=%2F", eva)
        assert person((code, sent))[0] == "eva" and sent["path"] == "/user/login?redirect_to=%2F", sent
        # Every other Nextcloud page, WebDAV included, never hears a name.
        for page in ["/nextcloud/apps/files/", "/nextcloud/remote.php/dav/files/notshared/"]:
            assert person(app(page, cookie, extra=forged)) == (None, None, None), page

    with subtest("logging out of LosOS cloud logs out of LosOS Login too"):
        code, where = app("/nextcloud/logout", eva)
        assert code == 303, (code, where)
        assert where == f"https://{BOX}/auth/logout?rd=https://{BOX}/nextcloud/login", where
        # Signed out of LosOS Login already, or off the LAN: Nextcloud's own.
        code, where = app("/nextcloud/index.php/logout")
        assert where == f"https://{BOX}/nextcloud/login?clear=1", (code, where)

    with subtest("Nextcloud's own form where LosOS Login cannot help"):
        own = "/nextcloud/index.php/login?direct=1"
        code, sent = app("/nextcloud/login?direct=1")
        assert sent["path"].startswith(own) and person((code, sent))[0] is None, sent
        # A posted form keeps its fields.
        code, sent = app("/nextcloud/login", extra="-d user=notshared -d direct=1")
        assert sent["path"] == own, sent
        # Off the LAN (the tunnel arrives from 127.0.0.1): never LosOS Login.
        code, sent = app("/nextcloud/login?redirect_url=/x", host="127.0.0.1", via=box,
                         extra=f"-H 'Host: {BOX}'")
        assert sent["path"] == own + "&redirect_url=/x", sent
        code, sent = app("/forgejo/user/login", host="127.0.0.1", via=box)
        assert sent["path"] == "/user/login" and person((code, sent))[0] is None, sent
        # LosOS Login down: the apps' own pages, nobody named.
        box.systemctl("stop losos-identity.path authelia-losos.service")
        code, sent = app("/nextcloud/login", cookie)
        assert sent["path"] == own and person((code, sent))[0] is None, sent
        assert person(app("/forgejo/", cookie)) == (None, None, None)
        box.systemctl("start authelia-losos.service losos-identity.path")
        box.wait_for_open_port(9091)
        laptop.wait_until_succeeds(f"curl -sf http://{BOX}/auth/api/health", timeout=60)
        cookie = sign_in("matus", PASSWORD)
        eva = sign_in("eva", "Eva-Long-Password-3")
        assert person(app("/forgejo/", cookie))[0] == "notshared"

    with subtest("only root, nginx and the app itself reach the apps' ports"):
        for port in [11000, 3000]:
            box.succeed(f"curl -sf http://127.0.0.1:{port}/")
            box.fail(f"runuser -u nobody -- curl -sf --max-time 5 http://127.0.0.1:{port}/")
        box.succeed("runuser -u nginx -- curl -sf http://127.0.0.1:3000/")

    with subtest("a box claimed before losos-owner existed gets it on start"):
        mark = journal_mark()
        box.succeed(
            "jq '.users.matus.groups = [\"losos-admins\"]' /var/lib/losos-identity/users.yml > /tmp/u.yml"
            " && install -m 0640 -g authelia-losos /tmp/u.yml /var/lib/losos-identity/.users.tmp"
            " && mv /var/lib/losos-identity/.users.tmp /var/lib/losos-identity/users.yml"
        )
        reloaded_since(mark)
        box.systemctl("restart lososd.service")
        box.wait_until_succeeds(
            "jq -e '.users.matus.groups == [\"losos-admins\", \"losos-owner\"]'"
            " /var/lib/losos-identity/users.yml"
        )

    with subtest("a person changes their own password and it takes at once"):
        mark = journal_mark()
        code, body = api(
            "/api/set-password", eva, method="POST", data={"password": "Eva-New-Password-44"}
        )
        assert code == 200, (code, body)
        # Eva does not own LosOS cloud, so it was not touched.
        assert json.loads(body)["cloud"] is None, body
        reloaded_since(mark)
        eva = sign_in("eva", "Eva-New-Password-44")
        assert api("/api/me", eva)[0] == 200

    with subtest("an administrator changes a person's access and password"):
        code, body = api(
            "/api/people/access", cookie, method="POST",
            data={"name": "eva", "groups": ["losos-storage", "losos-viewers"]},
        )
        assert code == 200, (code, body)
        # lososd reads the file on every call: no new sign-in needed.
        me = json.loads(api("/api/me", eva)[1])
        assert me["groups"] == ["losos-storage", "losos-viewers"], me
        assert api("/api/grow", eva, method="POST")[0] != 403
        mark = journal_mark()
        code, body = api(
            "/api/people/password", cookie, method="POST",
            data={"name": "eva", "password": "Eva-Reset-Password-55"},
        )
        assert code == 200, (code, body)
        reloaded_since(mark)
        eva = sign_in("eva", "Eva-Reset-Password-55")
        # The owner keeps their account, access and password, and nobody
        # changes or removes themselves.
        for path, data in [
            ("/api/people/access", {"name": "matus", "groups": []}),
            ("/api/people/password", {"name": "matus", "password": "Another-Long-Pass-7"}),
            ("/api/people/remove", {"name": "matus"}),
        ]:
            code, body = api(path, cookie, method="POST", data=data)
            assert code == 409, (path, code, body)
        code, body = api("/api/people/remove", extra=f"-H 'Authorization: Bearer {spare}'",
                         method="POST", data={"name": "nobody"})
        assert code == 404, (code, body)
        assert json.loads(box.succeed(f"cat {PEOPLE}"))["users"]["matus"]["groups"] == [
            "losos-admins", "losos-owner"
        ]

    with subtest("a removed person loses the admin pages and the apps at once"):
        assert person(app("/forgejo/", eva))[0] == "eva"
        code, body = api("/api/people/remove", cookie, method="POST", data={"name": "eva"})
        assert code == 200, (code, body)
        assert "eva" not in json.loads(box.succeed(f"cat {PEOPLE}"))["users"]
        code, body = api("/api/me", eva)
        assert code in (401, 403), (code, body)
        if code == 403:
            assert json.loads(body)["noAccount"] is True, body
        # LosOS Login looks the person up on every request, so the old
        # cookie no longer names anyone to LosOS Git.
        assert person(app("/forgejo/", eva)) == (None, None, None)
        body = json.dumps({"username": "eva", "password": "Eva-Reset-Password-55"})
        code = status(
            f"https://{BOX}/auth/api/firstfactor -H 'Content-Type: application/json' -d '{body}'"
        )
        assert code == "401", code
        # The owner is still there.
        assert api("/api/me", cookie)[0] == 200
  '';
}
