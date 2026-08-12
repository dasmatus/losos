# The losos installer: the `losos-install` wrapper around `losos-ctl install`.
#
# The auto-installer is now a subcommand of the Haskell losos-ctl backend (see
# backend/src/Installer.hs). This module packages it as the `losos-install`
# command name the VM test (tests/install.nix) and the README rely on, so the
# CLI surface is unchanged: `losos-install --emit-target …`, `--disko-script …`,
# `--tpm`, `--drives …`, `--no-install`.
#
# It stays free of any flake coupling (no `self`) so the VM test can import it:
# the losos-ctl derivation is supplied via the `losos.installer.package` option
# (the flake's iso system sets it to self.packages.<system>.losos-ctl; the test
# sets it via flake/packages.nix). The flake source the installer clones at run
# time is baked separately in flake.nix's iso module, where `self` is in scope.
#
# When `losos.installer.autorun` is true (installer ISO only), root's login
# shell becomes `losos-install` and tty1 autologs in as root — so booting the ISO
# runs the installer unattended, the destructive factory-reset / reinstall path.
{
  pkgs,
  lib,
  config,
  ...
}:

let
  ctl = config.losos.installer.package;
  autorun = config.losos.installer.autorun;

  # The tools losos-ctl install shells out to at runtime: disko + git for the
  # full path, util-linux for lsblk (drive detection), coreutils for cp/chmod
  # (laying the flake). nixos-install comes from the installer medium's own PATH
  # (--prefix preserves the rest of PATH). cryptsetup/lvm2 mirror the old wrap
  # in case disko reaches for them directly.
  tools = lib.makeBinPath [
    pkgs.disko
    pkgs.git
    pkgs.util-linux
    pkgs.coreutils
    pkgs.cryptsetup
    pkgs.lvm2
  ];

  # `losos-install` is a thin wrapper that execs `losos-ctl install`. On the
  # autorun ISO it is also root's login shell: after the install completes (or
  # is Ctrl-C'd) it drops to bash instead of exiting, so getty's autologin does
  # not re-loop the installer.
  losos-install = pkgs.writeShellScriptBin "losos-install" ''
    ${ctl}/bin/losos-ctl install "$@"
    exec ${lib.getExe pkgs.bash}
  '';

  losos-install-wrapped =
    pkgs.runCommand "losos-install"
      {
        nativeBuildInputs = [ pkgs.makeWrapper ];
        meta.mainProgram = "losos-install";
      }
      ''
        install -Dm755 ${lib.getExe losos-install} $out/bin/losos-install
        wrapProgram $out/bin/losos-install --prefix PATH : ${tools}
      '';
in
{
  config = lib.mkIf (ctl != null) {
    environment.systemPackages = [ losos-install-wrapped ];

    # Destructive reinstall medium: boot ISO → autologin root → losos-install
    # runs as the login shell. Gated so the VM test (which drives the installer
    # manually) and normal targets never auto-wipe.
    services.getty.autologinUser = lib.mkIf autorun (lib.mkForce "root");
    users.users.root.shell = lib.mkIf autorun losos-install-wrapped;
  };
}
