# nixos-test-vms config for signing in to LosOS cloud and LosOS Git with
# LosOS Login (modules/app-sign-in.nix), on the apps' side.
#
# nix/tests/identity.nix proves who nginx names to each app and when. This
# proves the apps do what the names ask: the real Nextcloud with user_saml
# set up by the same occ lines the pod's entrypoint runs, sent the three
# headers straight on its own port, and LosOS Git (modules/git.nix) sent
# them on its socket, both as nginx sends them.
#
# It also runs the calls lososd makes to keep each person's name and
# address from LosOS Login in both apps (backend/src/app_names.rs): the occ
# steps of setup::plan_person, and GitOp::EnsureUser's request.
#
# Nextcloud runs natively here, on loopback, because that is the cheapest
# way to boot it. A box never runs it this way with sign-in on: native mode
# has no nginx in front to vouch for the headers (modules/services.nix keeps
# user_saml off there).
{ pkgs }:

let
  inherit (pkgs) lib;
  signIn = import ../../modules/app-sign-in.nix { inherit lib; };
  h = signIn.header;
  owner = signIn.ownerAccount;
  lososPkgs = import ../packages.nix { inherit pkgs; };
in

pkgs.testers.nixosTest {
  name = "losos-app-sign-in";

  nodes.apps =
    { config, ... }:
    {
      imports = [
        ../../modules/options.nix
        ../../modules/git.nix
      ];
      losos.backend.package = lososPkgs.losos-ctl;
      losos.git.enable = true;

      services.nextcloud = {
        enable = true;
        package = pkgs.nextcloud34;
        hostName = "localhost";
        config = {
          dbtype = "sqlite";
          adminuser = owner;
          adminpassFile = "${pkgs.writeText "adminpass" "Owner-Long-Password-1"}";
        };
        extraApps = { inherit (pkgs.nextcloud34Packages.apps) user_saml; };
        extraAppsEnable = true;
      };
      # The pod's entrypoint runs these with "yes" (nix/images.nix).
      systemd.services.nextcloud-setup.script = lib.mkAfter (
        signIn.nextcloud.occ "${config.services.nextcloud.occ}/bin/nextcloud-occ" "yes"
      );

      environment.systemPackages = [
        pkgs.curl
        pkgs.jq
      ];
      systemd.services.postgresql.serviceConfig.TimeoutStartSec = "15min";
      virtualisation = {
        memorySize = 2048;
        cores = 2;
      };
    };

  testScript = ''
    import json

    # As nginx reaches LosOS Git: on its socket, as a member of its group.
    GIT = "runuser -u nginx -- curl -s --unix-socket /run/losos-git/http.sock"
    ADMIN_TOKEN = "/var/lib/losos-git/admin-token"

    def who(account, name, email):
        return (
            f"-H '${h.user}: {account}' -H '${h.name}: {name}' -H '${h.email}: {email}'"
        )

    start_all()
    # A oneshot that is inactive once it is done: wait for the boot, then
    # ask how it ended.
    apps.wait_for_unit("multi-user.target")
    apps.wait_until_succeeds("systemctl show -p Result --value nextcloud-setup | grep -qx success")
    # The schema pass setup starts after itself holds SQLite's lock while
    # it runs; a sign-in during it fails with "database is locked".
    apps.wait_until_succeeds(
        "! systemctl list-jobs --no-legend | grep -q nextcloud-update-db"
        " && ! systemctl is-active -q nextcloud-update-db"
    )
    apps.wait_for_unit("phpfpm-nextcloud.service")
    apps.wait_for_unit("losos-git.service")
    apps.wait_for_open_port(80)
    apps.wait_for_open_port(3000)

    def git_me(headers=""):
        """(status, body) of GET /api/v1/user on the socket."""
        out = apps.succeed(f"{GIT} -o /tmp/me -w '%{{http_code}}' {headers} http://git/api/v1/user")
        body = apps.succeed("cat /tmp/me")
        return int(out), (json.loads(body) if out == "200" else body)

    with subtest("Nextcloud: user_saml reads the headers"):
        conf = json.loads(apps.succeed("nextcloud-occ saml:config:get --output=json"))
        assert conf["1"]["general-uid_mapping"] == "${signIn.serverVar h.user}", conf
        assert apps.succeed("nextcloud-occ config:app:get user_saml type").strip() == "environment-variable"

    def nextcloud_sign_in(jar, headers):
        """What a browser does at /login with nginx's headers: follow the
        redirects through user_saml and come back signed in."""
        apps.succeed(f"rm -f {jar}")
        apps.succeed(f"curl -sf -L -c {jar} -b {jar} {headers} -o /dev/null http://localhost/login")
        out = apps.succeed(
            f"curl -sf -b {jar} -H 'OCS-APIRequest: true'"
            " 'http://localhost/ocs/v2.php/cloud/user?format=json'"
        )
        return json.loads(out)["ocs"]["data"]

    with subtest("Nextcloud: the owner arrives as their own account"):
        me = nextcloud_sign_in("/tmp/owner", who("${owner}", "Matus", "matus@mattbox.local"))
        assert me["id"] == "${owner}", me
        # The account that always held their files, not a new one.
        assert "admin" in me["groups"], me

    with subtest("Nextcloud: a new person gets an account of their own"):
        me = nextcloud_sign_in("/tmp/eva", who("eva", "Eva", "eva@mattbox.local"))
        assert me["id"] == "eva" and me["displayname"] == "Eva", me
        assert me["email"] == "eva@mattbox.local", me
        assert "admin" not in me["groups"], me
        info = json.loads(apps.succeed("nextcloud-occ user:info eva --output=json"))
        assert info["backend"] == "user_saml", info

    with subtest("Nextcloud: its own form stays for direct=1"):
        page = apps.succeed("curl -sf 'http://localhost/login?direct=1'")
        assert 'name="password"' in page or "login-form" in page or "body-login" in page, page[:400]

    with subtest("LosOS Git: the owner arrives as their own account"):
        code, me = git_me(who("${owner}", "Matus", "matus@mattbox.local"))
        assert code == 200, (code, me)
        assert (me["login"], me["full_name"], me["email"]) == ("${owner}", "Matus", "matus@mattbox.local"), me

    with subtest("LosOS Git: a new person gets an account, not the box"):
        code, me = git_me(who("eva", "Eva", "eva@mattbox.local"))
        assert code == 200 and me["login"] == "eva" and me["is_admin"] is False, (code, me)
        token = apps.succeed(f"cat {ADMIN_TOKEN}").strip()
        users = json.loads(apps.succeed(
            f"curl -sf -H 'Authorization: token {token}' http://127.0.0.1:3000/api/v1/admin/users"
        ))
        assert {"eva", "${owner}"} <= {u["login"] for u in users}, users

    with subtest("LosOS Git: no header, no sign-in, and the port never reads one"):
        assert git_me()[0] == 401
        code = apps.succeed(
            f"curl -s -o /dev/null -w '%{{http_code}}' {who('eva', 'Eva', 'eva@mattbox.local')}"
            " http://127.0.0.1:3000/api/v1/user"
        )
        assert code == "401", code
        # A name that cannot be an account is refused, not made.
        assert git_me(who("../etc", "x", "x@y"))[0] == 403

    with subtest("Nextcloud: lososd's occ steps give an account LosOS Login's name"):
        # plan_person, natively: is there an account, then the name and the
        # address, each after `--` so a name with a leading dash is a name.
        apps.succeed("nextcloud-occ user:info -- eva")
        apps.fail("nextcloud-occ user:info -- nobody")
        apps.succeed("nextcloud-occ user:setting -- eva settings display_name '-Eva Novak'")
        apps.succeed("nextcloud-occ user:setting -- eva settings email eva@example.org")
        apps.succeed("nextcloud-occ user:setting -- ${owner} settings display_name 'Matus K'")
        info = json.loads(apps.succeed("nextcloud-occ user:info eva --output=json"))
        assert info["display_name"] == "-Eva Novak" and info["email"] == "eva@example.org", info
        info = json.loads(apps.succeed("nextcloud-occ user:info ${owner} --output=json"))
        assert info["display_name"] == "Matus K", info

    with subtest("LosOS Git: lososd's request gives an account LosOS Login's name"):
        token = apps.succeed(f"cat {ADMIN_TOKEN}").strip()
        def ensure(login, body):
            return apps.succeed(
                f"curl -s -o /tmp/put -w '%{{http_code}}' -X PUT"
                f" -H 'Authorization: token {token}' -H 'Content-Type: application/json'"
                f" -d '{json.dumps(body)}' http://127.0.0.1:3000/api/v1/admin/users/{login}"
            ).strip()
        # GitOp::EnsureUser's body.
        code = ensure("eva", {"full_name": "Eva Novak", "email": "eva@example.org"})
        assert code == "200", (code, apps.succeed("cat /tmp/put"))
        # An empty address leaves the one there.
        code = ensure("${owner}", {"full_name": "Matus K", "email": "", "admin": True})
        assert code == "200", (code, apps.succeed("cat /tmp/put"))
        for login, name, email in [("eva", "Eva Novak", "eva@example.org"),
                                   ("${owner}", "Matus K", "matus@mattbox.local")]:
            user = json.loads(apps.succeed(
                f"curl -sf -H 'Authorization: token {token}' http://127.0.0.1:3000/api/v1/users/{login}"
            ))
            assert (user["full_name"], user["email"]) == (name, email), user
        assert user["is_admin"] is True, user
        # The account still signs in with LosOS Login's header.
        code, me = git_me(who("eva", "Eva Novak", "eva@example.org"))
        assert code == 200 and me["login"] == "eva", (code, me)
        # lososd's token acts as a person only when it says so.
        me = json.loads(apps.succeed(
            f"curl -sf -H 'Authorization: token {token}' -H 'X-Losos-As: eva' http://127.0.0.1:3000/api/v1/user"
        ))
        assert me["login"] == "eva", me
  '';
}
