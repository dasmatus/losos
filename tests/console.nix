# nixos-test-vms config for the tty1 address banner (modules/console.nix).
#
# The banner is the only thing a person standing at a box with no shell and no
# SSH can read, so "the script draws the right text" is not enough: the unit
# has to win tty1 from getty, and the address has to reach the screen. Those
# are wiring facts that only a booted system can show, so assert them here
# rather than trusting the script exercised on its own.
{ pkgs }:

pkgs.testers.nixosTest {
  name = "losos-console";

  nodes.machine =
    { ... }:
    {
      imports = [
        ../modules/options.nix
        ../modules/console.nix
      ];
      losos.hostName = "consolebox";
    };

  # The same banner on a box the installer put in keyfile mode, which has to
  # say what that costs (wiki/TPM.md). `machine` keeps the default TPM and
  # must not.
  nodes.keyfile =
    { ... }:
    {
      imports = [
        ../modules/options.nix
        ../modules/console.nix
      ];
      losos.hostName = "keyfilebox";
      losos.tpm.enable = false;
    };

  testScript = ''
    start_all()
    machine.wait_for_unit("losos-console.service")

    # tty1 belongs to the banner. If getty also ran there, its login prompt
    # would fight the banner for the screen.
    machine.fail("systemctl is-active getty@tty1.service")
    machine.fail("systemctl is-active autovt@tty1.service")

    # The test network's LAN address (eth1, 192.168.1.0/24) and the mDNS name
    # both reach the screen.
    machine.wait_until_tty_matches("1", r"http://192\.168\.1\.\d+")
    machine.wait_until_tty_matches("1", r"http://consolebox\.local")
    machine.wait_until_tty_matches("1", "LosOS is ready")
    assert "no TPM chip" not in machine.get_tty_text("1")

    keyfile.wait_for_unit("losos-console.service")
    keyfile.wait_until_tty_matches("1", r"http://keyfilebox\.local")
    keyfile.wait_until_tty_matches("1", "This box has no TPM chip")
    keyfile.wait_until_tty_matches("1", "anyone who takes the disk can read your files")

    # The other VTs keep their login prompts.
    machine.succeed("chvt 2")
    machine.wait_until_tty_matches("2", "login:")
  '';
}
