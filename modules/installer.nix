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
# sets it via flake/packages.nix). The flake the installer builds from is
# cloned over the network at run time (LOSOS_FLAKE_URL, default the public
# codeberg repo) — the ISO needs working network either way, since evaluating
# the flake fetches its nixpkgs input.
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

  # Two wrapper variants, because the command and the login shell need
  # opposite exit behavior:
  #
  #   * the CLI command (systemPackages, the VM test, scripts) must exec
  #     losos-ctl so the installer's real exit code reaches the caller — a
  #     trailing `exec bash` here would make machine.succeed (and any `$?`
  #     check) read success out of a failed install;
  #   * the autorun login shell must NOT exit when the installer stops:
  #     getty would respawn the login shell and re-run the destructive
  #     installer in a loop. It traps INT (Ctrl-C kills the foreground
  #     losos-ctl, not the wrapper — a caught trap reverts to default across
  #     the final exec, so the bash it lands in keeps normal Ctrl-C) and
  #     drops to bash. The wrapped copy is only ever root's shell on the
  #     autorun ISO.
  # The two variants MUST ship differently-named binaries. nixpkgs'
  # users-groups module auto-adds every shellPackage used as a user's shell
  # into environment.systemPackages, so both packages land in system.path's
  # buildEnv (built with ignoreCollisions = true): were both named
  # bin/losos-install, one would silently win the collision and serve BOTH
  # roles — concretely the plain-exec CLI won, root's login shell resolved
  # to it through /run/current-system/sw/bin/losos-install, and any exit
  # re-looped the destructive installer via getty again.
  losos-install = pkgs.writeShellScriptBin "losos-install" ''
    exec ${ctl}/bin/losos-ctl install "$@"
  '';

  losos-install-login = pkgs.writeShellScriptBin "losos-install-login" ''
    trap : INT
    ${ctl}/bin/losos-ctl install "$@"
    exec ${lib.getExe pkgs.bash}
  '';

  wrapWithTools =
    binName: inner: extraAttrs:
    pkgs.runCommand binName
      (
        {
          nativeBuildInputs = [ pkgs.makeWrapper ];
          meta.mainProgram = binName;
        }
        // extraAttrs
      )
      ''
        install -Dm755 ${lib.getExe inner} $out/bin/${binName}
        wrapProgram $out/bin/${binName} --prefix PATH : ${tools}
      '';

  losos-install-wrapped = wrapWithTools "losos-install" losos-install { };
  losos-install-shell = wrapWithTools "losos-install-login" losos-install-login {
    # Required by lib.types.shellPackage so it can be root's login shell on
    # the autorun ISO (users.users.root.shell); without it nixpkgs' users
    # module throws "losos-install-login is not a shell package".
    passthru.shellPath = "/bin/losos-install-login";
  };
in
{
  config = lib.mkIf (ctl != null) {
    environment.systemPackages = [ losos-install-wrapped ];

    # Destructive reinstall medium: boot ISO → autologin root → losos-install
    # runs as the login shell. Gated so the VM test (which drives the installer
    # manually) and normal targets never auto-wipe.
    services.getty.autologinUser = lib.mkIf autorun (lib.mkForce "root");
    users.users.root.shell = lib.mkIf autorun losos-install-shell;
  };
}
