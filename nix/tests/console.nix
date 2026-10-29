# nixos-test-vms config for the box's status on tty1 (modules/console.nix and
# modules/splash.nix).
#
# The status is the only thing a person standing at a box with no shell and
# no SSH can read, so "the script draws the right text" is not enough: the
# unit has to win tty1 from getty, the splash has to stay up after boot, and
# the address has to reach the screen. Those are wiring facts that only a
# booted system can show, so assert them here rather than trusting the
# script exercised on its own.
#
# The parts of LosOS the panel lists are stand-ins here: `lososd` and
# `losos-cloud` are units that only sleep, which is all the panel reads of
# them. lososd is then made to crash at every start, as it did under
# MemoryDenyWriteExecute, and the panel has to say so.
#
# `before = true` runs the same boot without the asserts on the parts, to
# take the same pictures from an older console.nix.
{
  pkgs,
  before ? false,
}:

let
  inherit (pkgs) lib;

  # "graphics" while a splash draws on tty1, "text" otherwise.
  mode = "${pkgs.kbd}/bin/kbdinfo -C /dev/tty1 getmode";

  # stage 1 as the box runs it (modules/boot.nix): systemd, which is where
  # the splash starts.
  box =
    { ... }:
    {
      imports = [
        ../../modules/options.nix
        ../../modules/branding.nix
        ../../modules/console.nix
        ../../modules/splash.nix
      ];
      boot.initrd.systemd.enable = true;
      # Plymouth draws in software. Without KVM a single core spends much of
      # the boot drawing, and the test's own console device times out.
      virtualisation.cores = 2;
    };
in
pkgs.testers.nixosTest {
  name = "losos-console";
  enableOCR = true;

  # The default splash, drawing the status under the logo. The graphics
  # driver is in the initrd, as a UEFI machine's firmware framebuffer is, so
  # the splash is up from stage 1. A box the installer put in keyfile mode
  # has to say what that costs (handbook/docs/reference/tpm.md); with that
  # warning the status is longer than one Plymouth message, so it also
  # covers the parts.
  nodes.machine = {
    imports = [ box ];
    losos.hostName = "consolebox";
    losos.tpm.enable = false;
    boot.initrd.kernelModules = [ "bochs" ];
    systemd.services = lib.genAttrs [ "lososd" "losos-cloud" ] (_: {
      wantedBy = [ "multi-user.target" ];
      serviceConfig = {
        ExecStart = "${pkgs.coreutils}/bin/sleep infinity";
        Restart = "always";
        RestartSec = 1;
      };
    });
  };

  # A machine whose graphics driver loads only in stage 2, after Plymouth
  # has settled for text mode: tty1 shows the text banner. It keeps the
  # default TPM, so it must not carry the warning.
  nodes.late = {
    imports = [ box ];
    losos.hostName = "latebox";
  };

  # Keyfile mode with the splash off: tty1 carries the text banner.
  nodes.keyfile = {
    imports = [ box ];
    losos.hostName = "keyfilebox";
    losos.tpm.enable = false;
    losos.splash.enable = false;
  };

  testScript = ''
    start_all()
    # Without KVM the boot outlasts the driver's wait for the guest shell;
    # by the time the banner starts the shell is up.
    machine.wait_for_console_text("Started Show the appliance address on tty1")
    machine.wait_for_unit("losos-console.service")

    # tty1 belongs to the status. If getty also ran there, its login prompt
    # would fight it for the screen.
    machine.fail("systemctl is-active getty@tty1.service")
    machine.fail("systemctl is-active autovt@tty1.service")

    # The splash is still up after boot, and nothing asked it to quit.
    machine.wait_for_unit("multi-user.target")
    machine.succeed("plymouth --ping")
    machine.fail("systemctl is-active plymouth-quit.service")
    machine.succeed("${mode} | grep -qx graphics")

    # The test network's LAN address (eth1, 192.168.1.0/24) and the mDNS
    # name reach the screen through the splash.
    machine.wait_for_text(r"consolebox\.local")
    machine.wait_for_text(r"192\.168\.1\.\d+")
    machine.wait_for_text("is ready")
    machine.wait_for_text("no TPM chip")
    ${lib.optionalString (!before) ''
      machine.wait_for_text("Admin pages")
      machine.wait_for_text("LosOS Cloud")
    ''}
    machine.screenshot("splash-ready")

    # lososd exits at every start and systemd keeps restarting it, as it
    # did when MemoryDenyWriteExecute stopped Wasmtime.
    machine.succeed(
        "mkdir -p /run/systemd/system/lososd.service.d",
        "printf '[Unit]\\nStartLimitIntervalSec=0\\n[Service]\\nExecStart=\\nExecStart=${pkgs.coreutils}/bin/false\\n'"
        " > /run/systemd/system/lososd.service.d/crash.conf",
        "systemctl daemon-reload",
        "systemctl restart lososd.service || true",
    )
    machine.wait_until_succeeds(
        "test $(systemctl show -p NRestarts --value lososd.service) -ge 3", timeout=120
    )
    ${
      if before then
        ''
          machine.sleep(10)
        ''
      else
        ''
          machine.wait_for_text("cannot start its admin pages")
          machine.wait_for_text("not running")
        ''
    }
    machine.screenshot("splash-admin-down")

    # Back to a lososd that stays up, started by hand: the box is ready
    # again.
    machine.succeed(
        "rm /run/systemd/system/lososd.service.d/crash.conf",
        "systemctl daemon-reload",
        "systemctl restart lososd.service",
    )
    ${lib.optionalString (!before) ''
      machine.wait_for_text("is ready")
      machine.screenshot("splash-ready-again")
    ''}

    late.wait_for_console_text("Started Show the appliance address on tty1")
    late.wait_for_unit("losos-console.service")
    late.succeed("plymouth --ping")
    late.succeed("${mode} | grep -qx text")
    late.wait_until_tty_matches("1", r"http://latebox\.local")
    assert "no TPM chip" not in late.get_tty_text("1")

    keyfile.wait_for_console_text("Started Show the appliance address on tty1")
    keyfile.wait_for_unit("losos-console.service")
    keyfile.fail("plymouth --ping")
    keyfile.wait_until_tty_matches("1", r"http://keyfilebox\.local")
    keyfile.wait_until_tty_matches("1", "LosOS v[0-9.]+ is ready")
    keyfile.wait_until_tty_matches("1", "This box has no TPM chip")
    keyfile.wait_until_tty_matches("1", "anyone who takes the disk can read your files")
    ${lib.optionalString (!before) ''
      # No part of LosOS runs on this node, so the text banner lists none.
      assert "[ ok ]" not in keyfile.get_tty_text("1")
    ''}
    keyfile.screenshot("text-banner")

    # The other VTs keep their login prompts.
    machine.succeed("chvt 2")
    machine.wait_until_tty_matches("2", "login:")
  '';
}
