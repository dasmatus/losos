# Versioned LosOS component updates via systemd-sysupdate.
#
# The payload contains signed Nix store objects, not a generic system toplevel:
# the latter would overwrite this machine's install-target.nix, overrides.nix,
# TPM/keyfile choice and firmware/disk layout. losos-sysupdate-apply imports
# the signed store objects into /nix/store; nixos-upgrade then evaluates this
# machine's own flake at 03:00 and reuses matching prebuilt outputs.
#
# All downloaded files, extracted bundles and proxy cache entries live under
# /var/lib/losos-sysupdate. modules/impermanence.nix persists /var on the
# encrypted /persist volume, so no update payload is extracted into /tmp,
# /run, or another tmpfs.
{ lib, pkgs, ... }:

let
  stateDir = "/var/lib/losos-sysupdate";
in
{
  systemd.sysupdate.enable = true;

  systemd.sysupdate.transfers."50-losos-components" = {
    Source = {
      Type = "url-file";
      Path = "http://127.0.0.1:8789/";
      MatchPattern = "losos-components_@v.tar.xz";
    };
    Target = {
      Type = "regular-file";
      Path = "${stateDir}/archives";
      MatchPattern = "losos-components_@v.tar";
      Mode = "0600";
      InstancesMax = 2;
    };
    # Transported files are checksummed by systemd-sysupdate. Verify is
    # disabled only because the repo does not yet ship a GPG keypair; the
    # downloaded tar is not extracted by the privileged systemd code. Our
    # stage helper rejects links/traversal/non-regular entries, and Nix
    # verifies the signature on every imported store object.
  };

  systemd.services.losos-sysupdate-proxy = {
    description = "Persistent local proxy for LosOS systemd-sysupdate bundles";
    wantedBy = [ "multi-user.target" ];
    after = [ "network.target" ];
    environment = {
      LOSOS_SYSUPDATE_UPSTREAM = "https://losos.dasmat.us/sysupdate";
      LOSOS_SYSUPDATE_CACHE = "${stateDir}/proxy-cache";
      LOSOS_SYSUPDATE_BIND = "127.0.0.1";
      LOSOS_SYSUPDATE_PORT = "8789";
      SSL_CERT_FILE = "${pkgs.cacert}/etc/ssl/certs/ca-bundle.crt";
    };
    serviceConfig = {
      Type = "simple";
      ExecStart = "${pkgs.python3}/bin/python3 ${../tools/losos-sysupdate-proxy.py}";
      Restart = "on-failure";
      RestartSec = "5s";
      StateDirectory = "losos-sysupdate";
      StateDirectoryMode = "0750";
      ProtectSystem = "strict";
      ProtectHome = true;
      ReadWritePaths = [ stateDir ];
      RestrictAddressFamilies = [ "AF_INET" "AF_INET6" "AF_UNIX" ];
    };
  };

  # The upstream auto-update unit is systemd-sysupdate-update.service (not
  # the D-Bus-backed systemd-sysupdate.service). It downloads archives to
  # /var/lib/losos-sysupdate/archives; only then does our helper validate and
  # extract into a staging directory on the same persistent filesystem.
  systemd.services.systemd-sysupdate-update = {
    wants = [ "losos-sysupdate-proxy.service" ];
    after = [ "losos-sysupdate-proxy.service" ];
    path = [ pkgs.nix pkgs.coreutils pkgs.python3 ];
    serviceConfig.ExecStartPost = [
      "${pkgs.python3}/bin/python3 ${../tools/losos-sysupdate-apply.py}"
    ];
  };

  # The usual NixOS upgrade remains the activation mechanism. Downloading and
  # extracting the update bundle is a prerequisite, but is weakly ordered so
  # offline boxes can still rebuild their checked-in configuration.
  systemd.services.nixos-upgrade = {
    wants = [ "systemd-sysupdate-update.service" ];
    after = [ "systemd-sysupdate-update.service" ];
  };
}
