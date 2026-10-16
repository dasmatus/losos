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
#   * a password change and a removed account must take effect at once.
#
# LosOS cloud is a stub: lososd runs in native mode with a `nextcloud-occ`
# that says "installed" and takes any password, which is all the claim asks
# of it. Booting Nextcloud would test Nextcloud.
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
        ];
        losos = {
          hostName = "mattbox";
          backend.package = lososPkgs.losos-ctl;
          admin.ui = stubUi;
          nextcloud.mode = "native";
          forgejo.enable = false;
        };
        systemd.services.lososd.path = lib.mkForce [
          stubOcc
          pkgs.coreutils
        ];
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
        assert matus["groups"] == ["losos-admins"], matus
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

    with subtest("a person in one section gets that section and no more"):
        # Nothing in the admin pages adds people yet, so Eva is written in
        # the way lososd writes the file, hashed by Authelia's own tool, and
        # renamed into place: a file Authelia sees half made is a failed
        # reload.
        mark = journal_mark()
        box.succeed(
            "${pkgs.writeShellScript "add-eva" ''
              set -eu
              hash=$(${pkgs.authelia}/bin/authelia crypto hash generate argon2 \
                --password 'Eva-Long-Password-3' --no-confirm | sed 's/^Digest: //')
              ${pkgs.jq}/bin/jq --arg h "$hash" \
                '.users.eva = {disabled: false, displayname: "Eva", email: "eva@mattbox.local",
                  groups: ["losos-backup"], password: $h}' \
                /var/lib/losos-identity/users.yml > /tmp/users.yml
              install -m 0640 -g authelia-losos /tmp/users.yml /var/lib/losos-identity/.users.tmp
              mv /var/lib/losos-identity/.users.tmp /var/lib/losos-identity/users.yml
            ''}"
        )
        reloaded_since(mark)
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
        audit = box.succeed("grep '\"who\":\"eva\"' /var/lib/losos/audit.log")
        assert '"outcome":"forbidden"' in audit, audit

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

    with subtest("a removed account is refused at once"):
        box.succeed(
            "jq 'del(.users.eva)' /var/lib/losos-identity/users.yml > /tmp/u.yml"
            " && install -m 0640 -g authelia-losos /tmp/u.yml /var/lib/losos-identity/.users.tmp"
            " && mv /var/lib/losos-identity/.users.tmp /var/lib/losos-identity/users.yml"
        )
        code, body = api("/api/me", eva)
        assert code in (401, 403), (code, body)
        if code == 403:
            assert json.loads(body)["noAccount"] is True, body
  '';
}
