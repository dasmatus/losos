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
    dates = "03:00"; # build/upgrade well away from the midnight reboot
    allowReboot = true; # reboot if the upgrade changed kernel/initrd
    randomizedDelaySec = "30m"; # spread load, avoid hammering the flake ref
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
