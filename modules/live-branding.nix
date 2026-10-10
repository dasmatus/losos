# What the installer medium calls itself: LosOS.
#
# Imported by the `iso` system only. The stock installer profile names
# everything after NixOS: the boot menu and its background, the volume label,
# the file name, the console banner, the host name, os-release and the
# "Welcome to" line systemd prints at boot. This module renames each of them.
# The machine underneath is still NixOS, and it says so in the console
# banner and in os-release (`ID_LIKE=nixos`, which nixpkgs adds by itself
# when `distroId` is not "nixos").
#
# Internal names stay as they are, because tools read them: the `nixos`
# user, /etc/nixos, `nixos-install`, `/etc/NIXOS`, the kernel's own
# version string and the boot units nixpkgs names after itself ("NixOS
# Activation").
#
# The shared half, the name, the release tag and the boot pictures, is in
# modules/branding.nix and modules/brand-art.nix.
{
  config,
  lib,
  pkgs,
  ...
}:

let
  inherit (config.system.nixos) release label;
  art = config.system.build.lososBootArt;
in
{
  # ID=losos, and nixpkgs then adds ID_LIKE=nixos. The installed box keeps
  # ID=nixos (modules/branding.nix); nothing ever switches this system.
  system.nixos.distroId = "losos";
  system.nixos.extraOSReleaseArgs.PRETTY_NAME = "LosOS installer ${label}, built on NixOS ${release}";

  # Apart from the installed boxes, which are called `losos` on the LAN.
  networking.hostName = "losos-installer";

  # The label stage 1 finds the medium by (`root=LABEL=`), and the one a
  # desktop shows for the stick. flake/disk-images.nix forces its own for
  # the closure-carrying medium.
  isoImage.volumeID = "LOSOS_INSTALLER";
  # Above iso-image.nix's plain definition, below the mkForce
  # flake/disk-images.nix uses for the closure-carrying medium.
  image.baseName = lib.mkOverride 60 "losos-installer-${label}-${pkgs.stdenv.hostPlatform.system}";

  # BIOS boot menu (syslinux). The entry itself reads
  # "LosOS <tag> Installer" from distroName and the label.
  isoImage.splashImage = "${art}/installer-background.png";
  # VSHIFT moves the menu below the logo, and every *ROW below counts from
  # the shifted top, so 14 and 15 land on screen rows 33 and 34.
  isoImage.syslinuxTheme = ''
    MENU RESOLUTION 800 600
    MENU CLEAR
    MENU ROWS 6
    MENU VSHIFT 19
    MENU CMDLINEROW 13
    MENU TIMEOUTROW 14
    MENU TABMSGROW  15
    MENU HELPMSGROW 16
    MENU HELPMSGENDROW 16
    MENU MARGIN 18

    #                                FG:AARRGGBB  BG:AARRGGBB   shadow
    MENU COLOR BORDER       30;44      #00000000    #00000000   none
    MENU COLOR SCREEN       37;40      #FFE2E8EE    #00000000   none
    MENU COLOR TABMSG       31;40      #FF94A2B0    #00000000   none
    MENU COLOR TIMEOUT      1;37;40    #FFE2E8EE    #00000000   none
    MENU COLOR TIMEOUT_MSG  37;40      #FF94A2B0    #00000000   none
    MENU COLOR CMDMARK      1;36;40    #FF48B3C0    #00000000   none
    MENU COLOR CMDLINE      37;40      #FFE2E8EE    #00000000   none
    MENU COLOR TITLE        1;36;44    #FFE2E8EE    #00000000   none
    MENU COLOR UNSEL        37;44      #FFE2E8EE    #00000000   none
    MENU COLOR HOTKEY       1;37;44    #FFE2E8EE    #00000000   none
    MENU COLOR SEL          7;37;40    #FF0A0E12    #FF48B3C0   none
    MENU COLOR HOTSEL       1;7;37;40  #FF0A0E12    #FF48B3C0   none
  '';

  # UEFI boots the UKI straight away, with no GRUB and so no GRUB theme;
  # without this the medium still carries NixOS's theme in /EFI/BOOT.
  isoImage.grubTheme = null;
  system.build.bootSplash = "${art}/installer-splash.bmp";

  # The banner agetty prints above the login on every console.
  services.getty.greetingLine = ''<<< LosOS installer ${label}, built on NixOS ${release} (\m) - \l >>>'';
  services.getty.helpLine = lib.mkForce ''
    This stick installs LosOS and erases every fixed disk in the machine.
    The installer runs by itself on the first console (Alt+F1).

    To set up a wireless connection, run `nmtui`.
  '';
}
