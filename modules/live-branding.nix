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
# The pictures are drawn at build time from the brand logo
# (admin-ui/themes/brand/salmon.png) on the dark palette of
# admin-ui/app/src/styles/tokens.css, so replacing the logo file replaces
# them too:
#
#   bios-background.png   800x600, behind the syslinux menu (BIOS boot)
#   uefi-splash.bmp       shown by systemd-stub while the kernel loads
#                         (UEFI boot; modules/secure-boot.nix puts it into
#                         the UKI as its .splash section)
{
  config,
  lib,
  pkgs,
  ...
}:

let
  inherit (config.system.nixos) release label;

  # The everyday salmon. The plate is the admin pages' Halloween logo, picked
  # by the viewer's date; a medium is built once, so it carries the salmon.
  logo = ../admin-ui/themes/brand/salmon.png;

  # tokens.css, dark scheme.
  ground = "#0a0e12";
  ink = "#e2e8ee";
  muted = "#94a2b0";
  accent = "#48b3c0";

  # The logo without its transparent margin, scaled to fit a box of that
  # size and centred in it, so a logo of another shape lands in the same
  # place.
  fit = box: "${logo} -trim +repage -resize ${box} -background none -gravity center -extent ${box}";

  bold = "${pkgs.dejavu_fonts}/share/fonts/truetype/DejaVuSans-Bold.ttf";
  regular = "${pkgs.dejavu_fonts}/share/fonts/truetype/DejaVuSans.ttf";

  art =
    pkgs.runCommand "losos-boot-art"
      {
        nativeBuildInputs = [ pkgs.imagemagick ];
      }
      ''
        mkdir $out

        # The syslinux menu draws its rows over the lower half; the logo
        # and the name sit above them.
        magick -size 800x600 xc:'${ground}' \
          \( ${fit "220x150"} \) -gravity north -geometry +0+52 -composite \
          -font ${bold} -fill '${ink}' -pointsize 46 -annotate +0+218 'LosOS' \
          -font ${regular} -fill '${muted}' -pointsize 17 -annotate +0+282 'installer' \
          -strip PNG24:$out/bios-background.png

        # systemd-stub centres this on a black screen, so the ground is black
        # and nothing frames it.
        magick -size 480x360 xc:black \
          \( ${fit "260x180"} \) -gravity north -geometry +0+44 -composite \
          -font ${bold} -fill '${ink}' -pointsize 46 -annotate +0+246 'LosOS' \
          -type TrueColor BMP3:$out/uefi-splash.bmp
      '';

  # "#rrggbb" as the "r;g;b" a terminal escape wants.
  rgb =
    hex:
    lib.concatMapStringsSep ";" (i: toString (lib.fromHexString (builtins.substring i 2 hex))) [
      1
      3
      5
    ];
in
{
  system.nixos.distroName = "LosOS";
  # ID=losos, and nixpkgs then adds ID_LIKE=nixos.
  system.nixos.distroId = "losos";
  system.nixos.extraOSReleaseArgs = {
    PRETTY_NAME = "LosOS installer, built on NixOS ${release}";
    # The name in systemd's "Welcome to …" line, in the accent colour.
    ANSI_COLOR = "1;38;2;${rgb accent}";
    HOME_URL = "https://github.com/dasmatus/losos";
  };

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
  # "LosOS <version> Installer" from distroName.
  isoImage.splashImage = "${art}/bios-background.png";
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
  system.build.bootSplash = "${art}/uefi-splash.bmp";

  # The banner agetty prints above the login on every console.
  services.getty.greetingLine = ''<<< LosOS installer, built on NixOS ${label} (\m) - \l >>>'';
  services.getty.helpLine = lib.mkForce ''
    This stick installs LosOS and erases every fixed disk in the machine.
    The installer starts by itself on this console.

    To set up a wireless connection, run `nmtui`.
  '';
}
