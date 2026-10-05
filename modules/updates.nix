# Auto-upgrade + scheduled reboot.
#
# Two independent mechanisms:
#   1. system.autoUpgrade rebuilds+activates a new generation from the flake
#      URI, rebooting only when the kernel/initrd/bootloader actually changed.
#   2. A midnight timer that reboots unconditionally — the requested daily
#      "restart at midnight". It runs at 00:07 (off the round minute).
{ pkgs, config, ... }:

{
  system.autoUpgrade = {
    enable = true;
    # Must carry a fragment. `nixos-rebuild --flake <uri>` with no `#attr`
    # resolves nixosConfigurations.$(hostname), and this flake exports only
    # `iso` and `install` — so an unfragmented URI makes every nightly run die
    # with "flake does not provide attribute". losos.upgradeFlakeUri defaults
    # to git+file:///etc/nixos#install.
    flake = config.losos.upgradeFlakeUri;
    # Impure on purpose: flake.nix reads modules/install-target.nix and
    # modules/overrides.nix from /etc/nixos when it can, so a remote
    # upgradeFlakeUri keeps this box's drives, firmware mode, unlock mode and
    # settings instead of evaluating the published defaults. Under the local
    # default URI the files are in-tree anyway and the flag changes nothing.
    # lososd passes the same flag (backend/src/supervisor.rs);
    # tests/invariants.nix pins this one.
    flags = [ "--impure" ];
    dates = "03:00"; # build/upgrade well away from the midnight reboot
    allowReboot = true; # reboot if the upgrade changed kernel/initrd
    randomizedDelaySec = "30m"; # spread load, avoid hammering the flake ref
  };

  # Garbage collection, because the box upgrades itself and nothing else
  # ever deletes a generation. /nix *is* /persist here (impermanence binds
  # /persist/nix over /nix), and with linuxPackages_latest every nightly
  # rebuild that lands a new kernel adds a kernel and an initrd to a 500 MiB
  # ESP as well as a generation to the store. Left alone, the ESP fills in
  # months and the 03:00 switch then fails on a box with no shell to notice
  # from. Two weeks of generations is enough to roll back from the boot menu
  # and little enough to stay inside the ESP; boot.nix caps the menu itself
  # (configurationLimit), which is what actually frees the ESP, since
  # bootloader entries are only removed when their generation is deleted.
  # The hour sits after the upgrade's 03:00 + up to 30 min random delay, so it
  # collects the previous generation rather than racing the build.
  nix.gc = {
    automatic = true;
    dates = "04:30";
    options = "--delete-older-than 14d";
  };

  # Unconditional daily reboot at ~midnight.
  systemd.timers.midnight-reboot = {
    description = "Reboot the system every night just after midnight";
    wantedBy = [ "timers.target" ];
    timerConfig = {
      OnCalendar = "00:07";
      Persistent = true; # catch up if the machine was off at 00:07
      Unit = "midnight-reboot.service";
    };
  };

  systemd.services.midnight-reboot = {
    description = "Reboot the system";
    serviceConfig = {
      Type = "oneshot";
      ExecStart = "${pkgs.systemd}/bin/systemctl reboot";
    };
  };
}
