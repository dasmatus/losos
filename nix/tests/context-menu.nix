# nixos-test-vms config for the right-click menu
# (admin-ui/app/src/components/context-menu.tsx) on a real box: the admin
# page and LosOS Cloud behind the front vhost (modules/containers.nix,
# modules/tls.nix), LosOS Login (modules/identity.nix) and lososd, with the
# owner's files seeded over WebDAV.
#
# The laptop opens the menu where a person would and checks what it holds
# (nix/tests/context-menu-shots.mjs): empty space, an app tile, a file, a
# folder and the space around them, a row in the trash, Shift+F10 on a
# focused row and a long press on a phone; that a text field keeps the
# browser's menu; that F2 and Delete do what the menu says; that nothing
# is refused by the admin CSP. Every state is a picture, light and dark.
#
# `beforeUi`, when given, is the admin UI of the build without the menu: the
# box switches to it in a specialisation and the laptop takes the same
# states again, for the pull request's "before" column.
{
  pkgs,
  beforeUi ? null,
}:

let
  inherit (pkgs) lib;
  lososPkgs = import ../packages.nix { inherit pkgs; };

  python = pkgs.python3.withPackages (ps: [ ps.pillow ]);
  seed = pkgs.writeScriptBin "cloud-seed" ''
    #!${python}/bin/python3
    ${builtins.readFile ./cloud-seed.py}
  '';
  shots = pkgs.writeShellScriptBin "context-menu-shots" ''
    export PLAYWRIGHT_CORE=${pkgs.playwright-driver}/index.mjs
    export PLAYWRIGHT_BROWSERS_PATH=${
      pkgs.playwright-driver.browsers.override {
        withFirefox = false;
        withWebkit = false;
      }
    }
    export PLAYWRIGHT_SKIP_VALIDATE_HOST_REQUIREMENTS=true
    exec ${pkgs.bun}/bin/bun ${./context-menu-shots.mjs} "$@"
  '';
in

pkgs.testers.nixosTest {
  name = "losos-context-menu";

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
      ];
      networking.hostName = "mattbox";
      losos = {
        hostName = "mattbox";
        backend.package = lososPkgs.losos-ctl;
        # The sidebar pins the tile menu offers live in a plugin.
        backend.plugins = lososPkgs.losos-plugins;
        admin.ui = lososPkgs.losos-admin-ui;
        cloud.enable = true;
        forgejo.enable = false;
        mdns.enable = true;
      };
      specialisation = lib.optionalAttrs (beforeUi != null) {
        before.configuration.losos.admin.ui = lib.mkForce beforeUi;
      };
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
        seed
        shots
      ];
      fonts.packages = [ pkgs.dejavu_fonts ];
      virtualisation = {
        memorySize = 3072;
        cores = 2;
        vlans = [ 1 ];
      };
    };
  };

  testScript = ''
    import json

    HOST = "mattbox.local"
    BASE = f"https://{HOST}"
    CA = "/etc/losos-box.pem"
    PASSWORD = "Correct-Horse-Battery-9"

    start_all()
    box.wait_for_unit("nginx.service")
    box.wait_for_unit("lososd.service")
    box.wait_for_unit("losos-cloud.service")
    cert = box.succeed("cat /var/lib/losos-tls/cert.pem")
    laptop.succeed(f"cat > {CA} <<'EOF'\n{cert}EOF")
    laptop.wait_until_succeeds(f"getent hosts {HOST}", timeout=120)

    with subtest("the owner claims the box"):
        laptop.wait_until_succeeds(
            f"curl -sf --cacert {CA} {BASE}/api/setup/claim | jq -e .ready", timeout=600
        )
        out = laptop.succeed(
            f"curl -s --cacert {CA} -w '\\n%{{http_code}}' -X POST {BASE}/api/setup/claim"
            " -H 'Content-Type: application/json'"
            f" -d '{json.dumps({"name": "matus", "password": PASSWORD})}'"
        )
        assert out.rsplit("\n", 1)[1] == "200", out
        box.wait_for_unit("authelia-losos.service")
        box.wait_for_open_port(9091)

    with subtest("the owner's files"):
        laptop.succeed(f"cloud-seed {BASE} matus '{PASSWORD}' {CA}")

    with subtest("the menu, everywhere a person right-clicks"):
        laptop.succeed(f"context-menu-shots {BASE} matus '{PASSWORD}' /tmp/shots/after --ca {CA}")
        laptop.copy_from_machine("/tmp/shots/after", "shots")
        # F2 renamed and Delete trashed a file through the page.
        box.succeed("find /var/lib/losos-cloud -path '*trash*' -name 'old playlist.m3u*' | grep -q .")
        box.fail("find /var/lib/losos-cloud -path '*files*' -name '*playlist.m3u' | grep -q .")
  ''
  + lib.optionalString (beforeUi != null) ''

    with subtest("the same states on the build without the menu"):
        box.succeed("/run/current-system/specialisation/before/bin/switch-to-configuration test")
        laptop.succeed(f"context-menu-shots {BASE} matus '{PASSWORD}' /tmp/shots/before --before --ca {CA}")
        laptop.copy_from_machine("/tmp/shots/before", "shots")
  '';
}
