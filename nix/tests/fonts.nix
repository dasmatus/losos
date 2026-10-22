# The fontconfig file for a browser a test runs outside a NixOS VM: in the
# test driver, or in a derivation's sandbox (FONTCONFIG_FILE).
#
# pkgs.makeFontsConf lists the font folders and leaves the generic families
# to /etc/fonts/conf.d. There is none in the sandbox, so system-ui and
# sans-serif, the head and tail of the admin UI's --font-ui, fall to the
# first face fontconfig finds, a serif one, and the pictures show the whole
# UI in serif. On a machine that has one, the same test picks that machine's
# fonts instead. This file reads neither the host's folders nor its rules:
# DejaVu only, with every generic family the admin UI's stacks name mapped
# onto it, which is also what a NixOS VM's own fontconfig picks, so a
# picture taken in the driver, in the sandbox or in a VM shows one face.
#
#   FONTCONFIG_FILE = import ./fonts.nix { inherit pkgs; };
#
# `extra` adds font folders (an emoji font, say) after DejaVu.
{
  pkgs,
  extra ? [ ],
}:

let
  folders = [ pkgs.dejavu_fonts ] ++ extra;
  alias = family: face: ''
    <alias binding="same">
      <family>${family}</family>
      <prefer><family>${face}</family></prefer>
    </alias>
  '';
  sans = [
    "sans-serif"
    "system-ui"
    "ui-sans-serif"
    "-apple-system"
    "BlinkMacSystemFont"
    "Segoe UI"
    "Roboto"
    "Helvetica Neue"
    "Helvetica"
    "Arial"
  ];
  mono = [
    "monospace"
    "ui-monospace"
    "SFMono-Regular"
    "SF Mono"
    "Menlo"
    "Consolas"
    "Liberation Mono"
  ];
in
pkgs.writeText "losos-test-fonts.conf" ''
  <?xml version="1.0"?>
  <!DOCTYPE fontconfig SYSTEM "urn:fontconfig:fonts.dtd">
  <fontconfig>
    ${pkgs.lib.concatMapStrings (d: "<dir>${d}</dir>\n") folders}
    <cachedir prefix="xdg">fontconfig</cachedir>
    ${pkgs.lib.concatMapStrings (f: alias f "DejaVu Sans") sans}
    ${pkgs.lib.concatMapStrings (f: alias f "DejaVu Sans Mono") mono}
    ${alias "serif" "DejaVu Serif"}
    <!-- Anything else, a family missing here or a character no named face
         has, ends at the sans face, as fontconfig's own 49-sansserif.conf
         would have it. -->
    <match target="pattern">
      <edit name="family" mode="append_last"><string>DejaVu Sans</string></edit>
    </match>
  </fontconfig>
''
