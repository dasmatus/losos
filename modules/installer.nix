# The minimal losos installer: packages install/losos-install.sh as the
# `losos-install` command on the installer ISO. The flake source it copies to
# a writable work dir (so it can drop in modules/install-target.nix and run
# disko + nixos-install against it) is baked separately in flake.nix's iso
# module, where the flake's own `self` is in scope — keeping this module free
# of any flake coupling so the VM test can import it without `self`.
{
  pkgs,
  lib,
  ...
}:

{
  # Wrap the installer so its PATH has every tool it shells out to: disko,
  # cryptsetup, lvm2, util-linux (lsblk/findmnt/mount), git, gawk, coreutils.
  environment.systemPackages = [
    (pkgs.runCommand "losos-install"
      {
        nativeBuildInputs = [ pkgs.makeWrapper ];
        meta.mainProgram = "losos-install";
      }
      ''
        install -Dm755 ${../install/losos-install.sh} $out/bin/losos-install
        wrapProgram $out/bin/losos-install \
          --prefix PATH : ${
            lib.makeBinPath [
              pkgs.disko
              pkgs.cryptsetup
              pkgs.lvm2
              pkgs.util-linux
              pkgs.git
              pkgs.gawk
              pkgs.coreutils
              pkgs.gnugrep
              pkgs.findutils
            ]
          }
      ''
    )
  ];
}