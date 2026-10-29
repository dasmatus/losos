# Screenshots of a booted box, for PR descriptions and films.
#
# The box is the real front door: nginx, LosOS Login, lososd, LosOS Git, the
# admin pages and the handbook, all built from this tree. LosOS Cloud is a
# stub (lososd in native mode with an `occ` that says "installed"), as in
# nix/tests/identity.nix. A client VM claims the box as Matus, signs in
# through LosOS Login and drives Playwright's Chromium at
# https://mattbox.local (nix/tests/shoot.mjs).
#
# What to shoot is read when the driver runs, not when it is built, so one
# build serves every plan:
#
#   nix build .#checks.x86_64-linux.losos-screens.driver
#   LOSOS_SHOTS_PLAN=plan.json ./result/bin/nixos-test-driver \
#     --no-interactive --output_directory out
#
# The PNGs and manifest.json land in out/shots/. Without a plan it takes the
# overview. The plan's format is at the top of shoot.mjs.
#
# It evaluates on a tree without LosOS Git too (the module is imported when
# it exists), so the same file takes the Before shots on an older commit.
{ pkgs }:

let
  lososPkgs = import ../packages.nix { inherit pkgs; };
  hasGit = builtins.pathExists ../../modules/git.nix;

  browsers = pkgs.playwright-driver.browsers.override {
    withFirefox = false;
    withWebkit = false;
  };
  shooter = pkgs.writeShellScriptBin "losos-shoot" ''
    export PLAYWRIGHT_CORE=${pkgs.playwright-driver}
    export PLAYWRIGHT_BROWSERS_PATH=${browsers}
    export PLAYWRIGHT_SKIP_VALIDATE_HOST_REQUIREMENTS=true
    exec ${pkgs.bun}/bin/bun ${./shoot.mjs} "$@"
  '';

  defaultPlan = builtins.toJSON {
    shots = [
      {
        file = "01-overview.png";
        path = "/";
        title = "the overview";
      }
    ];
  };
in

pkgs.testers.nixosTest {
  name = "losos-screens";

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
          ../../modules/config-repo.nix
        ]
        ++ lib.optional hasGit ../../modules/git.nix;
        losos = {
          hostName = "mattbox";
          backend.package = lososPkgs.losos-ctl;
          admin.ui = lososPkgs.losos-admin-ui;
          admin.handbook = lososPkgs.losos-handbook;
        }
        // lib.optionalAttrs hasGit { git.enable = true; };
        # The claim checks the Host against the kernel's hostname, which the
        # test framework would otherwise set to the node's name.
        networking.hostName = lib.mkForce "mattbox";
        # The configuration repository the installer leaves behind.
        systemd.services.config-tree = {
          wantedBy = [ "multi-user.target" ];
          before = [ "lososd.service" ];
          unitConfig.ConditionPathExists = "!/etc/nixos/.git";
          serviceConfig.Type = "oneshot";
          path = [ pkgs.git ];
          script = ''
            mkdir -p /etc/nixos/modules
            cd /etc/nixos
            printf '{ ... }:\n{\n}\n' > modules/overrides.nix
            git init -q -b main
            git add -A
            git -c user.name=LosOS -c user.email=losos@mattbox.local commit -qm "The box as installed"
          '';
        };
        virtualisation = {
          memorySize = 2048;
          cores = 2;
          vlans = [ 1 ];
        };
      };

    client = _: {
      environment.systemPackages = [
        pkgs.curl
        shooter
      ];
      networking.hosts."192.168.1.1" = [ "mattbox.local" ];
      fonts.packages = [ pkgs.dejavu_fonts ];
      virtualisation = {
        memorySize = 2048;
        cores = 2;
        vlans = [ 1 ];
      };
    };
  };

  testScript = ''
    import json
    import os

    PASSWORD = "Correct-Horse-Battery-9"
    plan_file = os.environ.get("LOSOS_SHOTS_PLAN")
    plan = open(plan_file).read() if plan_file else ${builtins.toJSON defaultPlan}
    json.loads(plan)

    start_all()
    box.wait_for_unit("lososd.service")
    box.wait_for_unit("nginx.service")
    box.wait_for_open_port(443)
    client.wait_for_unit("multi-user.target")

    with subtest("Matus claims the box"):
        body = json.dumps({"name": "matus", "password": PASSWORD})
        client.succeed(
            "curl -skf -X POST https://mattbox.local/api/setup/claim"
            f" -H 'Content-Type: application/json' -d '{body}'"
        )
        box.wait_for_unit("authelia-losos.service")
        box.wait_for_open_port(9091)

    with subtest("the shots"):
        client.succeed("mkdir -p /tmp/shots")
        client.succeed(f"cat > /tmp/plan.json << 'PLAN'\n{plan}\nPLAN")
        status, _ = client.execute(
            "losos-shoot /tmp/plan.json /tmp/shots https://mattbox.local matus"
            f" '{PASSWORD}' >&2",
            timeout=1800,
        )
        # What was taken comes out even when a shot was missed.
        client.copy_from_machine("/tmp/shots", "")
        assert status == 0, "some shots were missed; see the log"
  '';
}
