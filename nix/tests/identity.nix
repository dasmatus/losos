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
#     in the admin pages and in the apps;
#   * a person's own account: their name and address, the one-time code
#     LosOS Login asks for before a second factor is added (the box has no
#     mail, so lososd hands it out behind their password), an authenticator
#     app added through LosOS Login's own API and then asked for at every
#     sign-in, and an administrator's reset of it;
#   * the two rules an administrator sets, two-factor sign-in and passwords
#     that run out, hold a person to their own account and out of the apps
#     until they comply, and let them go the moment they do.
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
    import base64
    import hashlib
    import hmac
    import json
    import re
    import struct
    import time

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
        assert "argon2" not in body and '"password"' not in body, body
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

    def tunnel(path, host, cookie=None, proto="https", extra=""):
        """A request the way the edge's tunnel delivers it: plain http from
        127.0.0.1, with the browser's scheme in X-Forwarded-Proto."""
        args = (f"-o /tmp/tunnel -w '%{{http_code}} %{{redirect_url}}' {extra}"
                f" -H 'Host: {host}' -H 'X-Forwarded-Proto: {proto}' http://127.0.0.1{path}")
        if cookie:
            args += f" -H 'Cookie: {cookie}'"
        code, _, where = box.succeed(f"curl -s {args}").partition(" ")
        if code != "200":
            return int(code), where
        body = box.succeed("cat /tmp/tunnel")
        try:
            return 200, json.loads(body)
        except ValueError:
            return 200, body

    with subtest("LosOS Login answers through the tunnel, and the apps send people there"):
        public = "cloud.example.org"
        # Nextcloud's own form is gone, direct=1 or not.
        code, where = app("/nextcloud/login?direct=1")
        assert code == 302 and where.startswith(f"https://{BOX}/auth/?rd="), (code, where)
        code, where = app("/nextcloud/login", extra="-d user=notshared -d direct=1")
        assert code == 302 and where.startswith(f"https://{BOX}/auth/"), (code, where)
        # Through the tunnel: LosOS Login, on the public name.
        for page, back in [("/nextcloud/login", "/nextcloud/login"),
                           ("/forgejo/user/login", "/forgejo/user/login")]:
            code, where = tunnel(page, public)
            assert code == 302, (page, code, where)
            assert where == f"https://{public}/auth/?rd=https://{public}{back}", where
        # The portal answers there over the browser's https, with no loop;
        # plain http through the tunnel goes to https, and a LAN client
        # cannot claim https it does not have.
        code, body = tunnel("/auth/", public)
        assert code == 200 and "LosOS" in body, (code, body[:200])
        assert tunnel("/auth/", public, proto="http")[0] == 301
        assert status(f"-H 'X-Forwarded-Proto: https' http://{BOX}/auth/") == "301"
        # The owner adds a domain; LosOS Login takes cookies for it.
        mark = journal_mark()
        box.succeed(
            f"install -d /var/lib/losos-public-names && echo '[\"{public}\", \"bad name\"]'"
            " > /var/lib/losos-public-names/domains.json"
        )
        box.wait_until_succeeds(f"grep -q \"domain: '{public}'\" /run/losos-identity/session.yml")
        box.fail("grep -q 'bad name' /run/losos-identity/session.yml")
        box.wait_until_succeeds(
            f"journalctl -u authelia-losos --after-cursor='{mark}' | grep -q 'Startup complete'",
            timeout=120,
        )
        laptop.wait_until_succeeds(f"curl -sf http://{BOX}/auth/api/health", timeout=60)
        body = json.dumps({"username": "eva", "password": "Eva-Long-Password-3"})
        head = box.succeed(
            f"curl -s -D - -o /dev/null -H 'Host: {public}' -H 'X-Forwarded-Proto: https'"
            f" -H 'Content-Type: application/json' -d '{body}' http://127.0.0.1/auth/api/firstfactor"
        )
        m = re.search(r"(?im)^set-cookie: (losos_session=[^;]+);.*domain={}".format(re.escape(public)), head)
        assert m, head
        outside = m.group(1)
        code, sent = tunnel("/nextcloud/login", public, outside)
        assert code == 200 and sent["headers"].get("x-losos-cloud-user") == "eva", (code, sent)
        code, sent = tunnel("/forgejo/", public, outside)
        assert sent["headers"].get("x-losos-cloud-user") == "eva", sent
        # The admin pages stay on the LAN.
        assert tunnel("/api/state", public, outside)[0] == 403
        # LosOS Login down: a page that waits for it, nobody named.
        box.systemctl("stop losos-identity.path authelia-losos.service")
        code, _ = app("/nextcloud/login", cookie)
        assert code == 503, code
        page = laptop.succeed("cat /tmp/app")
        assert "LosOS Login is starting" in page and 'http-equiv="refresh"' in page, page
        assert tunnel("/forgejo/user/login", public)[0] == 503
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

    # ── LosOS Login: own account, second factors, rules ─────────────────────

    used_steps = {}

    def totp(secret):
        """A code for `secret` by the box's clock, never one used before:
        LosOS Login refuses a code twice, and accepts the next step's."""
        key = base64.b32decode(secret + "=" * (-len(secret) % 8))
        while True:
            step = int(box.succeed("date +%s")) // 30
            want = max(step, used_steps.get(secret, -1) + 1)
            if want <= step + 1:
                break
            time.sleep(10)
        used_steps[secret] = want
        mac = hmac.new(key, struct.pack(">Q", want), hashlib.sha1).digest()
        at = mac[-1] & 0x0F
        n = struct.unpack(">I", mac[at:at + 4])[0] & 0x7FFFFFFF
        return f"{n % 1000000:06d}"

    def authelia(path, cookie, method="POST", data=None):
        """LosOS Login's own API, as its settings page calls it."""
        code, body = api(f"/auth/api{path}", cookie, method=method, data=data)
        return code, (json.loads(body) if body.strip() else {})

    def add_app(cookie, password):
        """Add an authenticator app the way LosOS Login's settings page does:
        a one-time code first, read from the box behind the password."""
        code, body = authelia("/user/session/elevation", cookie)
        assert code == 200, (code, body)
        code, body = api("/api/account/code", cookie, method="POST", data={"password": password})
        assert code == 200, (code, body)
        otc = json.loads(body)["code"]
        assert re.fullmatch(r"[A-Z0-9]{6,32}", otc), otc
        code, body = authelia("/user/session/elevation", cookie, method="PUT", data={"otc": otc})
        assert code == 200 and body["status"] == "OK", (code, body)
        code, body = authelia("/secondfactor/totp/register", cookie, method="PUT",
                              data={"algorithm": "SHA1", "length": 6, "period": 30})
        assert code == 200, (code, body)
        secret = body["data"]["base32_secret"]
        code, body = authelia("/secondfactor/totp/register", cookie, data={"token": totp(secret)})
        assert code == 200 and body["status"] == "OK", (code, body)
        return secret

    def second_factor(cookie, secret):
        """Answer LosOS Login's second step; return the session cookie."""
        body = json.dumps({"token": totp(secret)})
        out = curl(
            f"-D - -o /tmp/2fa -w '%{{http_code}}' https://{BOX}/auth/api/secondfactor/totp"
            f" -H 'Content-Type: application/json' -H 'Cookie: {cookie}' -d '{body}'"
        )
        assert out.endswith("200"), (out, laptop.succeed("cat /tmp/2fa"))
        m = re.search(r"(?im)^set-cookie: (losos_session=[^;]+)", out)
        return m.group(1) if m else cookie

    def groups(name):
        return json.loads(box.succeed(f"cat {PEOPLE}"))["users"][name]["groups"]

    def wait_marks(name, has=(), lacks=()):
        """Until lososd's marks for `name` in the people file say so."""
        cond = " and ".join(
            [f'index("{g}") != null' for g in has] + [f'index("{g}") == null' for g in lacks]
        )
        box.wait_until_succeeds(f"jq -e '.users.{name}.groups | {cond}' {PEOPLE}", timeout=60)

    def eventually(check, timeout=90):
        """Until `check()` holds: LosOS Login reads the people file again
        a moment after lososd writes it."""
        for _ in range(timeout):
            if check():
                return
            time.sleep(1)
        assert check()

    def as_spare(path, method="GET", data=None):
        return api(path, method=method, data=data, extra=f"-H 'Authorization: Bearer {spare}'")

    with subtest("a person with Own account sets their own name and address"):
        mark = journal_mark()
        for name, pw, access in [("oma", "Oma-Long-Password-6", ["losos-account"]),
                                 ("ivo", "Ivo-Long-Password-8", ["losos-backup"])]:
            code, body = api("/api/people", cookie, method="POST", data={
                "name": name, "displayName": name.title(), "email": f"{name}@mattbox.local",
                "password": pw, "groups": access,
            })
            assert code == 200, (code, body)
        reloaded_since(mark)
        oma = sign_in("oma", "Oma-Long-Password-6")
        ivo = sign_in("ivo", "Ivo-Long-Password-8")
        code, body = api("/api/account", oma)
        assert code == 200, (code, body)
        mine = json.loads(body)
        assert mine["name"] == "oma" and mine["email"] == "oma@mattbox.local", mine
        assert mine["factors"] == {"app": False, "passkeys": []} and mine["locked"] == [], mine
        assert mine["rules"] == {"twoFactor": False, "passwordDays": 0}, mine
        assert mine["passwordExpiresAt"] is None and mine["passwordSetAt"] > 0, mine
        code, body = api("/api/account", oma, method="POST",
                         data={"displayName": "Oma Horvathova", "email": "oma@example.org"})
        assert code == 200, (code, body)
        file = json.loads(box.succeed(f"cat {PEOPLE}"))["users"]["oma"]
        assert file["displayname"] == "Oma Horvathova" and file["email"] == "oma@example.org", file
        assert file["groups"] == ["losos-account"], file
        for bad in [{"displayName": "", "email": "oma@example.org"},
                    {"displayName": "Oma", "email": "not an address"},
                    {"displayName": "Oma"}]:
            assert api("/api/account", oma, method="POST", data=bad)[0] == 400, bad
        # Without the access a person has no account page, and the spare
        # key has no account; an administrator has one.
        code, body = api("/api/account", ivo)
        assert code == 403 and json.loads(body)["section"] == "account", (code, body)
        assert as_spare("/api/account")[0] == 409
        assert api("/api/account", cookie)[0] == 200

    with subtest("the one-time code reaches only its owner, behind their password"):
        code, body = api("/api/account/code", oma, method="POST",
                         data={"password": "Oma-Long-Password-6"})
        assert code == 404 and json.loads(body)["noCode"] is True, (code, body)
        code, body = authelia("/user/session/elevation", oma)
        assert code == 200, (code, body)
        code, body = api("/api/account/code", oma, method="POST",
                         data={"password": "Wrong-Password-1234"})
        assert code == 403 and json.loads(body)["wrongPassword"] is True, (code, body)
        # The code was sent to Oma's address, not Matus's.
        code, body = api("/api/account/code", cookie, method="POST", data={"password": PASSWORD})
        assert code == 404, (code, body)
        code, body = api("/api/account/code", oma, method="POST",
                         data={"password": "Oma-Long-Password-6"})
        assert code == 200, (code, body)
        otc = json.loads(body)["code"]
        box.succeed(f"grep -qx '{otc}' /var/lib/authelia-losos/notifications.txt")
        box.succeed("grep -q 'oma@example.org' /var/lib/authelia-losos/notifications.txt")

    with subtest("an authenticator app is asked for at every sign-in once added"):
        # Nobody turns on a rule that would hold them at once.
        code, body = api("/api/people/rules", cookie, method="POST", data={"twoFactor": True})
        assert code == 409 and json.loads(body)["needsOwnFactor"] is True, (code, body)
        matus_app = add_app(cookie, PASSWORD)
        wait_marks("matus", has=["losos-two-factor"])
        assert groups("matus") == ["losos-admins", "losos-owner", "losos-two-factor"]
        # The session that added it signed in with a password alone.
        eventually(lambda: api("/api/state", cookie)[0] == 401)
        cookie = second_factor(cookie, matus_app)
        assert api("/api/state", cookie)[0] == 200
        mine = json.loads(api("/api/account", cookie)[1])
        assert mine["factors"]["app"] is True and mine["locked"] == [], mine
        # A new sign-in needs both steps.
        fresh = sign_in("matus", PASSWORD)
        assert api("/api/state", fresh)[0] == 401
        fresh = second_factor(fresh, matus_app)
        assert api("/api/state", fresh)[0] == 200
        listed = json.loads(api("/api/people", cookie)[1])["people"]
        assert {p["name"]: p["twoFactor"] for p in listed} == {
            "matus": True, "oma": False, "ivo": False
        }, listed

    with subtest("required two-factor sign-in holds people to their own account"):
        code, body = api("/api/people/rules", oma, method="POST", data={"twoFactor": True})
        assert code == 403 and json.loads(body)["section"] == "admin", (code, body)
        code, _ = api("/api/people/rules", cookie, method="POST",
                      data={"twoFactor": True, "passwordDays": 5000})
        assert code == 400, code
        code, body = api("/api/people/rules", cookie, method="POST", data={"twoFactor": True})
        assert code == 200, (code, body)
        wait_marks("oma", has=["losos-locked"])
        wait_marks("ivo", has=["losos-locked"])
        assert "losos-locked" not in groups("matus")
        me = json.loads(api("/api/me", oma)[1])
        assert me["locked"] == ["twoFactor"] and me["sections"] == ["account"], me
        assert me["admin"] is False, me
        code, body = api("/api/state", oma)
        assert code == 403 and json.loads(body)["locked"] is True, (code, body)
        code, body = api("/api/backup", ivo)
        assert code == 403 and json.loads(body)["locked"] is True, (code, body)
        # Their own account is open to them, Own account access or not.
        assert api("/api/account", ivo)[0] == 200
        assert json.loads(api("/api/account", oma)[1])["locked"] == ["twoFactor"]
        # LosOS Login names them to no app, and the apps' sign-in pages say
        # why.
        eventually(lambda: person(app("/forgejo/", oma)) == (None, None, None))
        assert app("/nextcloud/login", oma)[0] == 403
        assert "Finish your account first" in laptop.succeed("cat /tmp/app")
        assert person(app("/forgejo/", cookie))[0] == "notshared"
        listed = json.loads(api("/api/people", cookie)[1])
        assert listed["rules"] == {"twoFactor": True, "passwordDays": 0}, listed
        assert {p["name"]: p["locked"] for p in listed["people"]} == {
            "matus": [], "oma": ["twoFactor"], "ivo": ["twoFactor"]
        }, listed
        # Oma adds an app and is let go.
        oma_app = add_app(oma, "Oma-Long-Password-6")
        wait_marks("oma", has=["losos-two-factor"], lacks=["losos-locked"])
        oma = second_factor(oma, oma_app)
        me = json.loads(api("/api/me", oma)[1])
        assert me["locked"] == [] and me["sections"] == ["account"], me
        assert api("/api/state", oma)[0] == 200
        eventually(lambda: person(app("/forgejo/", oma))[:2] == ("oma", "Oma Horvathova"))

    with subtest("an administrator resets a lost authenticator app"):
        code, _ = api("/api/people/two-factor/reset", oma, method="POST", data={"name": "oma"})
        assert code == 403, code
        code, _ = api("/api/people/two-factor/reset", cookie, method="POST", data={"name": "nobody"})
        assert code == 404, code
        code, body = api("/api/people/two-factor/reset", cookie, method="POST", data={"name": "oma"})
        assert code == 200, (code, body)
        wait_marks("oma", has=["losos-locked"], lacks=["losos-two-factor"])
        assert json.loads(api("/api/me", oma)[1])["locked"] == ["twoFactor"]
        # Root wrote LosOS Login's database; LosOS Login still owns every
        # file of it, and adds the new app.
        box.succeed("test -z \"$(find /var/lib/authelia-losos/ ! -user authelia-losos)\"")
        oma_app = add_app(oma, "Oma-Long-Password-6")
        wait_marks("oma", has=["losos-two-factor"], lacks=["losos-locked"])
        oma = second_factor(oma, oma_app)
        assert api("/api/state", oma)[0] == 200

    with subtest("passwords run out after the days an administrator sets"):
        code, body = api("/api/people/rules", cookie, method="POST",
                         data={"twoFactor": False, "passwordDays": 30})
        assert code == 200, (code, body)
        wait_marks("ivo", lacks=["losos-locked"])
        # Everyone's password was set today, so nobody is held yet.
        assert json.loads(api("/api/me", oma)[1])["locked"] == []
        day = 24 * 60 * 60
        box.succeed(
            f"jq '.oma = (now | floor) - {31 * day}' /var/lib/losos-identity/passwords.json"
            " > /tmp/p.json && install -m 0600 /tmp/p.json /var/lib/losos-identity/passwords.json"
        )
        wait_marks("oma", has=["losos-locked"])
        me = json.loads(api("/api/me", oma)[1])
        assert me["locked"] == ["password"], me
        mine = json.loads(api("/api/account", oma)[1])
        assert mine["passwordExpiresAt"] <= int(box.succeed("date +%s")), mine
        assert api("/api/state", oma)[0] == 403
        # The same password again is no change.
        code, body = api("/api/set-password", oma, method="POST",
                         data={"password": "Oma-Long-Password-6"})
        assert code == 400, (code, body)
        code, body = api("/api/set-password", oma, method="POST",
                         data={"password": "Oma-Fresh-Password-77"})
        assert code == 200, (code, body)
        assert "losos-locked" not in groups("oma")
        me = json.loads(api("/api/me", oma)[1])
        assert me["locked"] == [], me
        mine = json.loads(api("/api/account", oma)[1])
        assert mine["passwordExpiresAt"] - mine["passwordSetAt"] == 30 * day, mine
        # Off again: nobody is held, and the second factors stay.
        code, _ = as_spare("/api/people/rules", method="POST",
                           data={"twoFactor": False, "passwordDays": 0})
        assert code == 200, code
        assert all("losos-locked" not in p["groups"]
                   for p in json.loads(box.succeed(f"cat {PEOPLE}"))["users"].values())
        assert "losos-two-factor" in groups("oma")
  '';
}
