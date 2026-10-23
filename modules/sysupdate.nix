# systemd-sysupdate stages a versioned, prebuilt Nix closure on persistent
# storage. LosOS still runs nixos-rebuild against the local, impure flake:
# drives, firmware, disk-unlock mode, and admin settings are machine-local.
{ config, lib, pkgs, ... }:

let
  stateRoot = "/persist/var/lib/losos-sysupdate";
  exports = "${stateRoot}/exports";
  cacheRoot = "/persist/var/cache/losos-sysupdate-proxy";
  tempRoot = "${stateRoot}/tmp";
  apply = pkgs.writeShellScript "losos-sysupdate-apply" ''
    set -euo pipefail
    umask 077

    state=${lib.escapeShellArg stateRoot}
    target=${lib.escapeShellArg exports}
    marker="$state/imported-version"
    mkdir -p "$target" "$state" "$state/tmp"
    export TMPDIR="$state/tmp" TMP="$state/tmp" TEMP="$state/tmp"

    # A network failure must not strand a box on its current generation.
    # The outer XZ tar is unpacked under /persist; its only member is an inner
    # compressed nix-store export, imported directly from persistent storage.
    if ! ${pkgs.systemd}/bin/systemd-sysupdate update; then
      echo "losos-sysupdate: no new verified bundle available; rebuilding the local flake" >&2
    fi

    candidate_name="$(${pkgs.findutils}/bin/find "$target" -mindepth 1 -maxdepth 1 -type d -name 'losos-os_*' -printf '%f\n' | ${pkgs.coreutils}/bin/sort -V | ${pkgs.coreutils}/bin/tail -n 1)"
    if [ -n "$candidate_name" ]; then
      candidate="$target/$candidate_name/closure.export.xz"
      version="''${candidate_name#losos-os_}"
      imported=""
      if [ -r "$marker" ]; then
        imported="$(cat "$marker")"
      fi
      if [ -f "$candidate" ] && [ "$version" != "$imported" ]; then
        echo "losos-sysupdate: importing prebuilt closure version $version from persistent storage"
        if ${pkgs.xz}/bin/xz --decompress --stdout "$candidate" | ${config.nix.package}/bin/nix-store --import; then
          printf '%s\n' "$version" > "$marker.new"
          mv -f "$marker.new" "$marker"
        else
          echo "losos-sysupdate: closure import failed; the normal Nix build remains the fallback" >&2
          rm -f "$marker.new"
        fi
      fi
    fi

    before_kernel="$(${pkgs.coreutils}/bin/readlink -f /run/current-system/kernel 2>/dev/null || true)"
    before_initrd="$(${pkgs.coreutils}/bin/readlink -f /run/current-system/initrd 2>/dev/null || true)"
    ${config.system.build.nixos-rebuild}/bin/nixos-rebuild switch --flake ${lib.escapeShellArg config.losos.upgradeFlakeUri} --impure
    after_kernel="$(${pkgs.coreutils}/bin/readlink -f /run/current-system/kernel 2>/dev/null || true)"
    after_initrd="$(${pkgs.coreutils}/bin/readlink -f /run/current-system/initrd 2>/dev/null || true)"

    if [ -n "$before_kernel" ] && [ -n "$before_initrd" ] && \
       { [ "$before_kernel" != "$after_kernel" ] || [ "$before_initrd" != "$after_initrd" ]; }; then
      echo "losos-sysupdate: kernel or initrd changed; rebooting into the new generation"
      ${pkgs.systemd}/bin/systemctl reboot
    fi
  '';
in
{
  systemd.sysupdate = {
    enable = true;
    transfers."10-losos-os" = {
      Source = {
        Type = "url-tar";
        Path = "http://127.0.0.1:8786/updates";
        MatchPattern = [ "losos-os_@v.tar.xz" ];
      };
      Target = {
        Type = "directory";
        Path = exports;
        MatchPattern = [ "losos-os_@v" ];
        InstancesMax = 2;
      };
    };
  };

  # The upstream timer would download without applying the local configuration.
  systemd.timers."systemd-sysupdate-update".enable = lib.mkForce false;

  # /var is bind-persisted from /persist. Nix daemon builders otherwise
  # use /tmp by default, which is a tmpfs root on the appliance.
  systemd.services.nix-daemon.environment.TMPDIR = "/persist/var/tmp/losos-nix-build";

  systemd.tmpfiles.rules = [
    "d /persist/var/tmp/losos-nix-build 1777 root root -"
    "d ${exports} 0700 root root -"
    "d ${stateRoot} 0700 root root -"
    "d ${tempRoot} 0700 root root -"
    "d ${cacheRoot} 0700 root root -"
    "d ${cacheRoot}/tmp 0700 root root -"
    "d ${cacheRoot}/home 0700 root root -"
  ];

  systemd.services.losos-sysupdate-proxy = {
    description = "Persistent local proxy for LosOS systemd-sysupdate artifacts";
    wantedBy = [ "multi-user.target" ];
    after = [ "network.target" "local-fs.target" ];
    path = [ pkgs.oras ];
    environment = {
      LOSOS_SYSUPDATE_OCI_REF = "ghcr.io/dasmatus/losos/os-updates:main";
      LOSOS_SYSUPDATE_CACHE = cacheRoot;
      TMPDIR = "${cacheRoot}/tmp";
      TMP = "${cacheRoot}/tmp";
      TEMP = "${cacheRoot}/tmp";
      HOME = "${cacheRoot}/home";
    };
    serviceConfig = {
      Type = "simple";
      ExecStart = "${pkgs.python3}/bin/python3 ${../tools/losos-sysupdate-proxy.py}";
      Restart = "on-failure";
      RestartSec = "5s";
      PrivateTmp = false;
      UMask = "0077";
    };
  };

  systemd.services.losos-sysupdate = {
    description = "Acquire a verified LosOS closure and rebuild the local NixOS appliance";
    wants = [ "network-online.target" ];
    after = [ "network-online.target" "losos-sysupdate-proxy.service" ];
    path = [
      pkgs.coreutils
      pkgs.findutils
      pkgs.xz
      config.nix.package
      config.system.build.nixos-rebuild
      pkgs.systemd
    ];
    environment = {
      TMPDIR = tempRoot;
      TMP = tempRoot;
      TEMP = tempRoot;
    };
    serviceConfig = {
      Type = "oneshot";
      ExecStart = "${apply}";
      TimeoutStartSec = "6h";
      UMask = "0077";
      PrivateTmp = false;
    };
  };

  systemd.timers.losos-sysupdate = {
    description = "Run the LosOS system update each night";
    wantedBy = [ "timers.target" ];
    timerConfig = {
      OnCalendar = "03:00";
      Persistent = true;
      RandomizedDelaySec = "30m";
      Unit = "losos-sysupdate.service";
    };
  };

  system.autoUpgrade.enable = lib.mkForce false;
}
