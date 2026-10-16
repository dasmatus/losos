# nixos-test-vms config for signing in to LosOS cloud and LosOS Git with
# LosOS Login (modules/app-sign-in.nix), on the apps' side.
#
# nix/tests/identity.nix proves who nginx names to each app and when. This
# proves the apps do what the names ask: the real Nextcloud with user_saml
# set up by the same occ lines the pod's entrypoint runs, and the real
# Forgejo with the same app.ini sections the pod mounts, each sent the three
# headers straight on its own port, as nginx sends them.
#
# Both apps run natively here, on loopback, because that is the cheapest way
# to boot them. A box never runs them this way with sign-in on: native mode
# has no nginx in front to vouch for the headers (modules/services.nix keeps
# user_saml off there).
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
    { config, ... }:
    {
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
      # A oneshot that is inactive once it is done: wait for the boot, then
      # ask how it ended.
      apps.wait_for_unit("multi-user.target")
      apps.wait_until_succeeds("systemctl show -p Result --value nextcloud-setup | grep -qx success")
      apps.wait_for_unit("phpfpm-nextcloud.service")
      apps.wait_for_unit("forgejo.service")
      apps.wait_for_open_port(80)
      apps.wait_for_open_port(3000)

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
    '';
}
