# The boot pictures of both systems, drawn at build time from the brand logo
# (admin-ui/themes/brand/salmon.png) on the dark palette of
# admin-ui/app/src/styles/tokens.css, so replacing the logo file replaces
# them too. Plain data, imported by modules/branding.nix (which
# modules/live-branding.nix reads it from) and modules/splash.nix:
#
#   installer-background.png  800x600, behind the installer's syslinux menu
#   installer-splash.bmp      shown by systemd-stub while the installer's
#                             kernel loads (modules/secure-boot.nix puts it
#                             into the UKI)
#   grub/                     the GRUB theme an installed box boots with on
#                             a BIOS machine
#   splash-logo.png           the logo alone, for the installed box's
#                             Plymouth theme (modules/splash.nix), which
#                             scales it to the screen
#
# Each picture but the splash logo carries the release tag, so it changes with
# flake/version.nix. The Plymouth theme writes the tag as text instead.
{ pkgs, version }:

let
  # The everyday salmon. The plate is the admin pages' Halloween logo, picked
  # by the viewer's date; a boot picture is drawn once, so it is the salmon.
  logo = ../admin-ui/themes/brand/salmon.png;

  # tokens.css, dark scheme.
  palette = {
    ground = "#0a0e12";
    ink = "#e2e8ee";
    muted = "#94a2b0";
    accent = "#48b3c0";
  };
  inherit (palette)
    ground
    ink
    muted
    accent
    ;

  # The logo without its transparent margin, scaled to fit a box of that
  # size and centred in it, so a logo of another shape lands in the same
  # place.
  fit = box: "${logo} -trim +repage -resize ${box} -background none -gravity center -extent ${box}";

  fonts = "${pkgs.dejavu_fonts}/share/fonts/truetype";
  bold = "${fonts}/DejaVuSans-Bold.ttf";
  regular = "${fonts}/DejaVuSans.ttf";

  # The boot menu in the lower half of the screen, under the logo. GRUB
  # stretches the background to its 1024x768 mode (gfxmodeBios).
  grubTheme = pkgs.writeText "theme.txt" ''
    title-text: ""
    desktop-image: "background.png"
    desktop-color: "${ground}"

    + boot_menu {
      left = 22%
      width = 56%
      top = 50%
      height = 32%
      item_font = "DejaVu Sans Regular 16"
      selected_item_font = "DejaVu Sans Bold 16"
      item_color = "${ink}"
      selected_item_color = "${ground}"
      selected_item_pixmap_style = "select_*.png"
      item_height = 32
      item_padding = 12
      item_spacing = 6
      icon_width = 0
      icon_height = 0
      item_icon_space = 0
      scrollbar = false
    }

    + label {
      left = 0
      width = 100%
      top = 88%
      align = "center"
      id = "__timeout__"
      text = "Starting in %d s"
      color = "${muted}"
      font = "DejaVu Sans Regular 14"
    }
  '';
in
pkgs.runCommand "losos-boot-art"
  {
    nativeBuildInputs = [
      pkgs.imagemagick
      pkgs.grub2
    ];
    passthru = { inherit palette; };
  }
  ''
    mkdir -p $out/grub

    # The syslinux menu draws its rows over the lower half; the logo, the
    # name and the tag sit above them.
    magick -size 800x600 xc:'${ground}' \
      \( ${fit "220x150"} \) -gravity north -geometry +0+52 -composite \
      -font ${bold} -fill '${ink}' -pointsize 46 -annotate +0+218 'LosOS' \
      -font ${regular} -fill '${muted}' -pointsize 17 -annotate +0+282 'installer ${version}' \
      -strip PNG24:$out/installer-background.png

    # systemd-stub centres this on a black screen, so the ground is black
    # and nothing frames it.
    magick -size 480x360 xc:black \
      \( ${fit "260x180"} \) -gravity north -geometry +0+36 -composite \
      -font ${bold} -fill '${ink}' -pointsize 46 -annotate +0+232 'LosOS' \
      -font ${regular} -fill '${muted}' -pointsize 17 -annotate +0+300 '${version}' \
      -type TrueColor BMP3:$out/installer-splash.bmp

    magick -size 1024x768 xc:'${ground}' \
      \( ${fit "280x190"} \) -gravity north -geometry +0+70 -composite \
      -font ${bold} -fill '${ink}' -pointsize 52 -annotate +0+272 'LosOS' \
      -font ${regular} -fill '${muted}' -pointsize 18 -annotate +0+344 '${version}' \
      -strip PNG24:$out/grub/background.png

    # Large enough that the Plymouth theme only ever scales it down.
    magick ${logo} -trim +repage -resize 640x440 -strip PNG32:$out/splash-logo.png

    # The highlight behind the selected entry, as GRUB's nine-piece box.
    for piece in c n s e w ne nw se sw; do
      magick -size 4x4 xc:'${accent}' PNG24:$out/grub/select_$piece.png
    done

    grub-mkfont -s 16 -r 0x20-0x17F -o $out/grub/dejavu-regular-16.pf2 ${regular}
    grub-mkfont -s 14 -r 0x20-0x17F -o $out/grub/dejavu-regular-14.pf2 ${regular}
    grub-mkfont -s 16 -r 0x20-0x17F -o $out/grub/dejavu-bold-16.pf2 ${bold}
    cp ${grubTheme} $out/grub/theme.txt
  ''
