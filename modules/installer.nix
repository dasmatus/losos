# The losos installer: the `losos-install` wrapper around `losos-ctl install`.
#
# The auto-installer is a subcommand of the Rust losos-ctl backend (see
# backend/src/installer.rs). This module packages it as the `losos-install`
# command name the VM tests (nix/tests/install.nix, nix/tests/tpm.nix) and the handbook
# rely on, so the CLI surface is unchanged: `losos-install --emit-target …`,
# `--disko-script …`, `--tpm` / `--no-tpm`, `--drives …`, `--no-install`.
#
# It stays free of any flake coupling (no `self`) so the VM test can import it:
# the losos-ctl derivation is supplied via the `losos.installer.package` option
# (the flake's iso system sets it to self.packages.<system>.losos-ctl; the test
# sets it via nix/packages.nix). The flake the installer builds from is
# cloned over the network at run time (LOSOS_FLAKE_URL, default the public
# GitHub repo) — the ISO needs working network either way, since evaluating
# the flake fetches its nixpkgs input. It clones the release channel the box
# will follow (LOSOS_FLAKE_BRANCH, losos.configRepo.upstream.branch), and the
# default branch where that channel does not exist.
#
# When `losos.installer.autorun` is true (installer ISO only), tty1 runs the
# installer as a service in place of a login prompt, so booting the ISO starts
# it and its firmware-mode menu before any destructive work. Where the medium's
# boot screen draws (modules/splash.nix), the installer runs under it and
# shows its menu, its steps and how it ended on the screen's panel
# (backend/src/install_screen.rs). Elsewhere it prints them on tty1 as text.
# tty2 is reserved for live command logs; tty3 to tty6 log root in to a shell.
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
  # (laying the flake), systemd for systemd-cryptenroll (sealing the disk key
  # to the TPM2 right after the format), clevis for `--tang` (the same key
  # sealed to the TPM2 and a Tang server), curl for `--tang-edge` (asking
  # the edge for the box's Tang key, and sending a problem report), less for
  # showing that report on screen 7 (src/install_report.rs). nixos-install
  # comes from the installer medium's own PATH (--prefix preserves the rest
  # of PATH). cryptsetup/lvm2 mirror the old wrap in case disko reaches for
  # them directly.
  tools = lib.makeBinPath [
    pkgs.less
    pkgs.disko
    pkgs.git
    pkgs.util-linux
    pkgs.coreutils
    pkgs.cryptsetup
    pkgs.lvm2
    pkgs.clevis
    pkgs.curl
    config.systemd.package
  ];

  plymouth = config.boot.plymouth.enable;

  # Where a problem report goes (losos.report.*, modules/problems.nix on the
  # box). The GitHub App's client ID is not among these: the installer reads
  # it from its unit's credential `report-client-id` (below), and a medium
  # without one offers no report. It still names the handbook's page for a
  # failure the handbook explains.
  report = config.losos.report;
  sending = report.enable && report.clientId != null && report.clientId != "";
  reportEnv = [
    "--set LOSOS_RELEASE ${lib.escapeShellArg (import ../nix/version.nix)}"
  ]
  ++ lib.optionals report.enable [
    "--set LOSOS_REPORT_REPOSITORY ${lib.escapeShellArg report.repository}"
    "--set LOSOS_REPORT_LOGIN ${lib.escapeShellArg report.login}"
    "--set LOSOS_REPORT_API ${lib.escapeShellArg report.api}"
  ];

  # GPG keys baked into this medium (losos.access.gpgKeys set on the iso
  # system): every install from it seeds them, as `--gpg-key` would. A stock
  # medium has none and passes nothing.
  seededKeys = config.losos.access.gpgKeys;
  seededKeysFile = pkgs.writeText "losos-access-keys.asc" (lib.concatStringsSep "\n" seededKeys);

  # Two wrappers, because the command and tty1 need opposite exit behavior:
  #
  #   * the CLI command (systemPackages, the VM test, scripts) must exec
  #     losos-ctl so the installer's real exit code reaches the caller — a
  #     trailing `exec bash` here would make machine.succeed (and any `$?`
  #     check) read success out of a failed install;
  #   * tty1's must NOT end when the installer stops, or the console goes
  #     dead with the reason on it. It traps INT (Ctrl-C kills the foreground
  #     losos-ctl, not the wrapper) and then opens a shell under the log. The
  #     service never restarts it: that would run the destructive installer
  #     again.
  losos-install = pkgs.writeShellScriptBin "losos-install" ''
    ${lib.optionalString autorun "export LOSOS_INSTALLER_ISO=1"}
    exec ${ctl}/bin/losos-ctl install "$@"
  '';

  # Plymouth reads tty1's keyboard while it draws, so the installer takes
  # its keys from Plymouth then (LOSOS_SPLASH) and reads nothing from the
  # terminal. A splash that fell back to text cannot host the panel, and
  # would still hold the keyboard, so it goes and the installer runs in
  # text. Afterwards the splash goes too, uncovering tty1's log for the
  # shell under it.
  losos-install-tty1 = pkgs.writeShellScriptBin "losos-install-tty1" ''
    trap : INT
    # tty2 is a dedicated live log console during the destructive install.
    # Keep tty1 for the Plymouth progress panel and its final error/shell.
    if [ -w /dev/tty2 ]; then
      printf '\033c' > /dev/tty2
      printf 'LosOS installer live log (tty1 has the progress screen)\n\n' > /dev/tty2
    fi
    export LOSOS_INSTALLER_ISO=1
    if command -v plymouth >/dev/null && plymouth --ping 2>/dev/null; then
      if [ "$(kbdinfo -C /dev/tty1 getmode 2>/dev/null)" = graphics ]; then
        export LOSOS_SPLASH=1
      else
        plymouth quit --wait
      fi
    fi
    ${ctl}/bin/losos-ctl install "$@"
    if [ -n "''${LOSOS_SPLASH-}" ]; then
      plymouth quit --wait
    fi
    unset LOSOS_SPLASH LOSOS_INSTALLER_ISO
    echo
    echo "The installer has stopped. This is a root shell; restart with: reboot"
    while :; do ${lib.getExe pkgs.bash} -l; done
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
        wrapProgram $out/bin/${binName} --prefix PATH : ${tools} ${lib.concatStringsSep " " reportEnv} \
          --set-default LOSOS_FLAKE_BRANCH ${lib.escapeShellArg config.losos.configRepo.upstream.branch} \
          --set-default LOSOS_REGISTRAR_URL ${lib.escapeShellArg config.losos.proxy.registrarUrl}${
            lib.optionalString (seededKeys != [ ]) " --set LOSOS_INSTALL_GPG_KEYS_FILE ${seededKeysFile}"
          }
      '';

  losos-install-wrapped = wrapWithTools "losos-install" losos-install { };
  losos-install-tty1-wrapped = wrapWithTools "losos-install-tty1" losos-install-tty1 { };
in
{
  config = lib.mkIf (ctl != null) {
    environment.systemPackages = [ losos-install-wrapped ];

    # Destructive reinstall medium: booting the ISO starts the installer on tty1.
    # Gated so the VM test (which drives the installer manually) and normal
    # targets never auto-wipe. The other consoles log root in to a shell.
    services.getty.autologinUser = lib.mkIf autorun (lib.mkForce "root");

    # A getty would take tty1 by hanging it up and resetting it, which
    # pulls the terminal from under a running splash; this is the pattern
    # modules/console.nix uses on the installed box.
    systemd.services."getty@tty1".enable = lib.mkIf autorun false;
    systemd.services."autovt@tty1".enable = lib.mkIf autorun false;
    # Reserve tty2 for the installer's live command output; the next VT
    # remains available as a root shell if Wi-Fi or recovery work is needed.
    systemd.services."getty@tty2".enable = lib.mkIf autorun false;
    systemd.services."autovt@tty2".enable = lib.mkIf autorun false;

    systemd.services.losos-installer = lib.mkIf autorun {
      description = "The LosOS installer on tty1";
      wantedBy = [ "multi-user.target" ];
      after = [
        "systemd-user-sessions.service"
        "systemd-vconsole-setup.service"
        "losos-ghcr-cache.service"
      ]
      ++ lib.optional plymouth "plymouth-start.service";
      wants = [ "losos-ghcr-cache.service" ];
      conflicts = [
        "getty@tty1.service"
        "autovt@tty1.service"
      ];
      before = [ "getty.target" ];
      # A switch must never start the destructive installer again.
      restartIfChanged = false;
      path = [ pkgs.kbd ] ++ lib.optional plymouth config.boot.plymouth.package;
      environment.HOME = "/root";
      # The login environment: PATH with nixos-install, the CA bundle for
      # the clone and the flake inputs, TERM.
      script = ''
        source /etc/profile
        export TERM=linux
        exec ${lib.getExe losos-install-tty1-wrapped} "$@"
      '';
      serviceConfig = {
        Type = "idle";
        StandardInput = "tty";
        StandardOutput = "tty";
        StandardError = "tty";
        TTYPath = "/dev/tty1";
        TTYReset = false;
        TTYVHangup = false;
        TTYVTDisallocate = false;
        UtmpIdentifier = "tty1";
        UtmpMode = "user";
        IgnoreSIGPIPE = false;
        # The report client ID, as a credential (src/install_report.rs
        # `client_id`): one passed to the machine (`systemd.set_credential=`
        # on the kernel command line, SMBIOS, a credstore) wins; a medium
        # built with losos.report.clientId (CI's LOSOS_ABRT_VAR, flake.nix)
        # carries it as the fallback. The installer hands it to the box it
        # installs, whose first boot seals it (modules/problems.nix).
        ImportCredential = lib.mkIf report.enable [ "report-client-id" ];
        SetCredential = lib.mkIf sending [ "report-client-id:${report.clientId}" ];
      };
    };
  };
}
