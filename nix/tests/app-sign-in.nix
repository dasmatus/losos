# nixos-test-vms config for signing in to LosOS Git with LosOS Login
# (modules/app-sign-in.nix), on the app's side. LosOS Cloud reads the same
# headers itself; nix/tests/cloud.nix covers it.
#
# nix/tests/identity.nix proves who nginx names to the app and when. This
# proves the app does what the names ask: the real Forgejo with the same
# app.ini sections the pod mounts, sent the three headers straight on its
# own port, as nginx sends them.
#
# It also runs the call lososd makes to keep each person's name and address
# from LosOS Login in Forgejo (backend/src/app_names.rs):
# ForgejoOp::SetProfile's request.
#
# Forgejo runs natively here, on loopback, because that is the cheapest way
# to boot it. A box never runs it this way with sign-in on: native mode has
# no nginx in front to vouch for the headers.
{ pkgs }:

let
  inherit (pkgs) lib;
  signIn = import ../../modules/app-sign-in.nix { inherit lib; };
  h = signIn.header;
  owner = signIn.ownerAccount;
in

pkgs.testers.nixosTest {
  name = "losos-app-sign-in";

  nodes.apps =
    _:
    {
      services.forgejo = {
        enable = true;
        settings = lib.recursiveUpdate signIn.forgejo {
          server = {
            HTTP_ADDR = "127.0.0.1";
            HTTP_PORT = 3000;
            ROOT_URL = "http://localhost:3000/";
          };
          # As the pod's losos.ini (modules/workloads.nix).
          service.DISABLE_REGISTRATION = true;
          security.INSTALL_LOCK = true;
        };
      };

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

  testScript =
    { nodes, ... }:
    let
      forgejo = nodes.apps.services.forgejo;
    in
    ''
      import json

      FORGEJO = (
          "runuser -u ${forgejo.user} -- env"
          " FORGEJO_WORK_DIR=${forgejo.stateDir}"
          " FORGEJO_CUSTOM=${forgejo.customDir}"
          " HOME=${forgejo.stateDir}"
          " ${lib.getExe forgejo.package}"
      )

      def who(account, name, email):
          return (
              f"-H '${h.user}: {account}' -H '${h.name}: {name}' -H '${h.email}: {email}'"
          )

      start_all()
      apps.wait_for_unit("forgejo.service")
      apps.wait_for_open_port(3000)

      with subtest("Forgejo: the owner arrives as their own account"):
          apps.succeed(
              f"{FORGEJO} admin user create --admin --username ${owner}"
              " --email owner@mattbox.local --password Owner-Long-Password-1 --must-change-password=false"
          )
          page = apps.succeed(
              f"curl -sf {who('${owner}', 'Matus', 'matus@mattbox.local')} http://127.0.0.1:3000/user/settings"
          )
          assert 'name="name" value="${owner}"' in page or "${owner}" in page, page[:400]

      with subtest("Forgejo: a new person is registered and signed in"):
          apps.succeed(
              f"curl -sf -c /tmp/fj -b /tmp/fj {who('eva', 'Eva', 'eva@mattbox.local')}"
              " -o /dev/null http://127.0.0.1:3000/user/settings"
          )
          users = apps.succeed(f"{FORGEJO} admin user list")
          assert "eva" in users, users
          # Not an administrator: there was already a first account.
          admins = apps.succeed(f"{FORGEJO} admin user list --admin")
          assert "eva" not in admins and "${owner}" in admins, admins
          # The session it made keeps her signed in without the header.
          code = apps.succeed(
              "curl -s -o /dev/null -w '%{http_code}' -b /tmp/fj http://127.0.0.1:3000/user/settings"
          )
          assert code == "200", code

      with subtest("Forgejo: no header, no sign-in, and the sign-up form stays shut"):
          code = apps.succeed(
              "curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1:3000/user/settings"
          )
          assert code == "303", code
          page = apps.succeed("curl -s http://127.0.0.1:3000/user/sign_up")
          assert 'name="password"' not in page, "the sign-up form is open"

      with subtest("Forgejo: lososd's request gives an account LosOS Login's name"):
          token = apps.succeed(
              f"{FORGEJO} admin user generate-access-token --username ${owner}"
              " --token-name losos --scopes write:admin,read:user --raw"
          ).strip().splitlines()[-1]
          def patch(login, body):
              return apps.succeed(
                  f"curl -s -o /tmp/patch -w '%{{http_code}}' -X PATCH"
                  f" -H 'Authorization: token {token}' -H 'Content-Type: application/json'"
                  f" -d '{json.dumps(body)}' http://127.0.0.1:3000/api/v1/admin/users/{login}"
              ).strip()
          # ForgejoOp::SetProfile's body. The sign-in header never renames an
          # account Forgejo already has, so this is the only way it changes.
          code = patch("eva", {"login_name": "eva", "source_id": 0,
                               "full_name": "Eva Novak", "email": "eva@example.org"})
          assert code == "200", (code, apps.succeed("cat /tmp/patch"))
          code = patch("${owner}", {"login_name": "${owner}", "source_id": 0, "full_name": "Matus K"})
          assert code == "200", (code, apps.succeed("cat /tmp/patch"))
          for login, name in [("eva", "Eva Novak"), ("${owner}", "Matus K")]:
              user = json.loads(apps.succeed(
                  f"curl -sf -H 'Authorization: token {token}' http://127.0.0.1:3000/api/v1/users/{login}"
              ))
              assert user["full_name"] == name, user
          # The account still signs in with LosOS Login's header.
          code = apps.succeed(
              f"curl -s -o /dev/null -w '%{{http_code}}' {who('eva', 'Eva Novak', 'eva@example.org')}"
              " http://127.0.0.1:3000/user/settings"
          ).strip()
          assert code == "200", code
          # No account yet: 404, left to the person's first sign-in.
          assert patch("nobody", {"login_name": "nobody", "source_id": 0, "full_name": "N"}) == "404"
    '';
}
