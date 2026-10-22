# nixos-test-vms config for signing in to LosOS Git with LosOS Login
# (modules/app-sign-in.nix), on the app's side. LosOS Cloud reads the same
# headers itself; nix/tests/cloud.nix covers it.
#
# nix/tests/identity.nix proves who nginx names to each app and when. This
# proves LosOS Git (modules/git.nix) does what the names ask, sent the three
# headers on its socket as nginx sends them.
#
# It also runs the call lososd makes to keep each person's name and address
# from LosOS Login in LosOS Git (backend/src/app_names.rs): GitOp::EnsureUser's
# request.
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

  nodes.apps = _: {
    imports = [
      ../../modules/options.nix
      ../../modules/git.nix
    ];
    losos.backend.package = lososPkgs.losos-ctl;
    losos.git.enable = true;
    # The socket's other user. Nothing else here would make the account.
    services.nginx.enable = true;

    environment.systemPackages = [
      pkgs.curl
      pkgs.jq
    ];
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
    apps.wait_for_unit("multi-user.target")
    apps.wait_for_unit("losos-git.service")
    apps.wait_for_open_port(3000)

    def git_me(headers=""):
        """(status, body) of GET /api/v1/user on the socket."""
        out = apps.succeed(f"{GIT} -o /tmp/me -w '%{{http_code}}' {headers} http://git/api/v1/user")
        body = apps.succeed("cat /tmp/me")
        return int(out), (json.loads(body) if out == "200" else body)

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
