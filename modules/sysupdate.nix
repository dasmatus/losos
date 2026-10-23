# systemd-sysupdate transports signed, versioned release bundles. The target is
# a directory on the encrypted persistent volume, not /tmp or tmpfs. We then
# activate from the versioned source bundle so per-machine settings survive.
{ config, pkgs, ... }:

let
  updateRoot = "/persist/var/lib/losos-sysupdate";
  versions = "${updateRoot}/versions";
  cache = "/persist/var/cache/losos-sysupdate-proxy";
  certificate = ../nix/keys/secure-boot-db.pem;
  apply = pkgs.writeShellScriptBin "losos-sysupdate-apply" (builtins.readFile ../tools/losos-sysupdate-apply.sh);
in
{
  # The classic NixOS auto-upgrade stays enabled until separately published,
  # signed release bundles are available. The activation service is present
  # for validation/manual use; CI's follow-up change enables its timer and
  # switches off the legacy updater atomically with artifact publication.

  environment.etc."sysupdate.d/50-losos.transfer".text = ''
    [Transfer]
    Verify=no

    [Source]
    Type=url-tar
    Path=http://127.0.0.1:8788/stable
    MatchPattern=losos-update_@v.tar.xz

    [Target]
    Type=directory
    Path=${versions}
    MatchPattern=losos-update_@v
    InstancesMax=2
  '';

  systemd.tmpfiles.rules = [
    "d ${cache} 0755 root root - -"
    "d ${updateRoot} 0755 root root - -"
    "d ${versions} 0755 root root - -"
    "d ${updateRoot}/state 0755 root root - -"
  ];

  systemd.services.losos-sysupdate-proxy = {
    description = "Persistent verified source proxy for systemd-sysupdate";
    wantedBy = [ "multi-user.target" ];
    after = [ "network.target" "persist.mount" ];
    requiresMountsFor = [ cache ];
    path = [ pkgs.openssl ];
    environment = {
      LOSOS_SYSUPDATE_CONFIG_SYNC = if config.losos.configRepo.upstream.enable then "1" else "0";
      LOSOS_SYSUPDATE_CACHE_DIR = cache;
      LOSOS_SYSUPDATE_CERT = toString certificate;
      LOSOS_SYSUPDATE_REPOSITORY = "dasmatus/losos";
      LOSOS_SYSUPDATE_LISTEN = "127.0.0.1";
      LOSOS_SYSUPDATE_PORT = "8788";
    };
    serviceConfig = {
      Type = "simple";
      ExecStart = "${pkgs.python3}/bin/python3 ${../tools/losos-sysupdate-proxy.py}";
      ExecStartPost = "${pkgs.curl}/bin/curl --fail --retry 15 --retry-connrefused --retry-delay 1 --max-time 2 http://127.0.0.1:8788/health";
      Restart = "on-failure";
      RestartSec = "5s";
      UMask = "0022";
      NoNewPrivileges = true;
      ProtectSystem = "strict";
      ProtectHome = true;
      PrivateTmp = true;
      ReadWritePaths = [ cache ];
    };
  };

  systemd.services.losos-sysupdate = {
    description = "Download and activate the newest signed LosOS system update";
    wants = [ "network-online.target" "losos-sysupdate-proxy.service" ];
    requires = [ "losos-sysupdate-proxy.service" ];
    after = [ "network-online.target" "losos-sysupdate-proxy.service" ];
    requiresMountsFor = [ updateRoot versions "/nix" ];
    path = [
      pkgs.systemd
      pkgs.nix
      pkgs.xz
      pkgs.coreutils
      pkgs.findutils
      pkgs.gnused
      pkgs.gnugrep
      config.system.build.nixos-rebuild
    ];
    serviceConfig = {
      Type = "oneshot";
      ExecStart = "${apply}/bin/losos-sysupdate-apply";
      TimeoutStartSec = "6h";
      UMask = "0022";
    };
  };

}
