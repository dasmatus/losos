# nixos-test-vms config for the installer on the medium's boot screen
# (modules/installer.nix, modules/splash.nix, backend/src/install_screen.rs).
#
# Booting the stick runs the installer on tty1. Where the boot screen draws,
# the installer runs under it and its panel is the installer's screen: the
# firmware menu answered with a key, the steps, and how it ended. Where it
# does not, tty1 shows the same in text. Both are wiring facts (who owns
# tty1, who reads the keyboard, what is left on the screen afterwards) that
# only a booted machine shows.
#
# The installer runs through its --disko-script seam, as in tests/install.nix,
# with a script that holds until the test lets it go, so the running step
# stays on the screen long enough to read.
{
  pkgs,
  disko,
}:

let
  mode = "${pkgs.kbd}/bin/kbdinfo -C /dev/tty1 getmode";

  medium =
    { config, pkgs, ... }:
    let
      lososPkgs = import ../flake/packages.nix { inherit pkgs; };
    in
    {
      imports = [
        disko.nixosModules.disko
        ../modules/options.nix
        ../modules/disko.nix
        ../modules/installer.nix
        ../modules/branding.nix
        ../modules/splash.nix
      ];
      losos.installer.package = lososPkgs.losos-ctl;
      losos.installer.autorun = true;
      losos.targetDrives = [ "/dev/vdb" ];
      losos.tpm.enable = false;
      losos.bios = true;
      disko.enableConfig = false;
      services.lvm.enable = true;

      environment.etc."losos/disko-script".source = pkgs.writeShellScript "disko-held" ''
        while [ ! -e /run/losos-go ]; do sleep 1; done
        exec ${config.system.build.diskoScript}
      '';
      systemd.services.losos-installer.scriptArgs = "--disko-script /etc/losos/disko-script --drives /dev/vdb";

      boot.initrd.systemd.enable = true;
      virtualisation = {
        memorySize = 2048;
        # Plymouth draws in software; see tests/console.nix.
        cores = 2;
        emptyDiskImages = [ 2048 ];
      };
    };
in
pkgs.testers.nixosTest {
  name = "losos-installer-screen";
  enableOCR = true;

  # The graphics driver in stage 1, as a UEFI machine's firmware
  # framebuffer is: the installer runs under the splash.
  nodes.splash = {
    imports = [ medium ];
    boot.initrd.kernelModules = [ "bochs" ];
  };

  # The splash off: the installer runs in text on tty1.
  nodes.text = {
    imports = [ medium ];
    losos.splash.enable = false;
  };

  testScript = ''
    def menu_shown(machine, times):
        # The installer notes the menu in tty1's text, under the picture.
        # Answering inside its 30 s matters more than reading it first:
        # OCR under emulation is slower than that.
        def shown(_):
            return machine.get_tty_text("1").count("firmware menu on the boot screen") >= times
        retry(shown)

    splash.start()

    with subtest("the installer owns tty1, under the splash"):
        splash.wait_for_console_text("Started The LosOS installer on tty1")
        splash.wait_for_unit("losos-installer.service")
        splash.fail("systemctl is-active getty@tty1.service")
        splash.succeed("plymouth --ping")
        splash.succeed("${mode} | grep -qx graphics")

    with subtest("the firmware menu is on the panel and takes a key"):
        menu_shown(splash, 1)
        # The panel's opening, slow under emulation, well inside the 30 s.
        splash.sleep(8)
        splash.screenshot("menu")
        splash.send_key("1")
        splash.wait_until_tty_matches("1", "firmware mode: BIOS")

    with subtest("the steps, with the running one marked"):
        splash.wait_for_text("Erasing and encrypting")
        splash.screenshot("progress")
        splash.succeed("touch /run/losos-go")

    with subtest("how it ended, and the log and a shell after a key"):
        splash.wait_for_text("The disks are ready")
        splash.wait_for_text("no TPM chip")
        splash.screenshot("finished")
        splash.succeed("vgs persist-vg")
        splash.send_key("ret")
        splash.wait_until_fails("plymouth --ping")
        splash.succeed("${mode} | grep -qx text")
        splash.wait_until_tty_matches("1", "format\\+mount complete")
        splash.wait_until_tty_matches("1", "This is a root shell")
        splash.screenshot("log")

    with subtest("a refused choice says why on the panel"):
        # UEFI, on a machine that booted the stick in BIOS mode.
        splash.succeed("systemctl restart plymouth-start.service")
        splash.succeed("systemctl restart losos-installer.service")
        menu_shown(splash, 2)
        splash.send_key("2")
        splash.wait_for_text("The install stopped")
        splash.wait_for_text("UEFI")
        splash.screenshot("failed")
        splash.send_key("ret")
        splash.wait_until_fails("plymouth --ping")
        splash.wait_until_tty_matches("1", "cannot install for UEFI")

    with subtest("without the splash, the same on tty1 in text"):
        text.start()
        text.wait_until_tty_matches("1", "Choose the firmware mode")
        text.fail("plymouth --ping")
        text.screenshot("text-menu")
        text.send_chars("1\n")
        text.wait_until_tty_matches("1", "target drives: /dev/vdb")
        text.succeed("touch /run/losos-go")
        text.wait_until_tty_matches("1", "format\\+mount complete")
        text.wait_until_tty_matches("1", "This is a root shell")

    with subtest("the other consoles log root in to a shell"):
        text.succeed("chvt 2")
        text.wait_until_tty_matches("2", "root@")
  '';
}
