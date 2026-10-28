# Scheduled OS updates live in modules/sysupdate.nix. Keep the persistent
# store collector and the unconditional nightly reboot here.
{ pkgs, ... }:

{
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
