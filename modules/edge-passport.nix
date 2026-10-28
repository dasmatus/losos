# An open gateway's passport (losos.edge.passport.enable): what lets a box
# check an edge for itself instead of trusting a certificate someone signed
# for it (edge/registrar/src/gateway.rs, backend/access/src/gateway/check.rs).
#
# At boot, before the registrar starts:
#   * losos-gateway-tpm makes the TPM's endorsement key (RSA, the one its
#     maker certified) and an attestation key (ECDSA P-256) under it, and
#     copies the maker's certificate and the firmware's event log beside
#     them in /run/losos-gateway/tpm;
#   * losos-gateway-measure copies the owner's settings to
#     /run/losos-gateway/settings.json and extends PCR 15 with their
#     SHA-256, once per boot. PCR 15 cannot be reset, so the quote shows
#     exactly the settings this boot started with.
# The owner changes them with `POST /passport/settings`, signed with their
# OpenPGP key; the registrar writes /var/lib/losos-gateway/settings.json and
# the machine reboots to measure them.
#
# systemd's own measurements into PCR 11 (boot phases) and PCR 15 (machine
# id, file systems) are switched off: a box compares PCR 11 with the
# published image's unified kernel image and PCR 15 with the settings alone.
{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.losos.edge.passport;
  settingsFile = "/var/lib/losos-gateway/settings.json";
  runDir = "/run/losos-gateway";
  tpmDir = "${runDir}/tpm";
  tcti = "device:/dev/tpm0";
in
{
  config = lib.mkIf (config.losos.edge.enable && cfg.enable) {
    security.tpm2.enable = true;

    systemd.services.losos-gateway-tpm = {
      description = "losos gateway: the TPM's endorsement and attestation keys";
      wantedBy = [ "multi-user.target" ];
      before = [ "losos-registrar.service" ];
      requiredBy = [ "losos-registrar.service" ];
      unitConfig.ConditionPathExists = "/dev/tpm0";
      serviceConfig = {
        Type = "oneshot";
        RemainAfterExit = true;
      };
      environment.TPM2TOOLS_TCTI = tcti;
      path = [
        pkgs.coreutils
        pkgs.tpm2-tools
      ];
      script = ''
        set -eu
        dir=${tpmDir}
        rm -rf "$dir"
        install -d -m 0755 "$dir" "$dir/chain"
        # The raw device has no resource manager: flush what each command
        # leaves loaded, so the per-request commands find room.
        fl() { tpm2_flushcontext -t; tpm2_flushcontext -s; }
        tpm2_createek -G rsa -u "$dir/ek.pub" -c "$dir/ek.ctx" >/dev/null
        fl
        tpm2_createak -C "$dir/ek.ctx" -c "$dir/ak.ctx" -G ecc -g sha256 -s ecdsa \
          -u "$dir/ak.pub" -n "$dir/ak.name" >/dev/null
        fl
        # The maker's certificate for the RSA endorsement key, at the index
        # the TCG EK credential profile names.
        tpm2_nvread 0x01c00002 -o "$dir/ek.der"
        cp /sys/kernel/security/tpm0/binary_bios_measurements "$dir/eventlog.bin"
      '';
    };

    systemd.services.losos-gateway-measure = {
      description = "losos gateway: measure the owner's settings into PCR 15";
      wantedBy = [ "multi-user.target" ];
      after = [ "losos-gateway-tpm.service" ];
      before = [ "losos-registrar.service" ];
      requiredBy = [ "losos-registrar.service" ];
      unitConfig.ConditionPathExists = "/dev/tpm0";
      serviceConfig = {
        Type = "oneshot";
        RemainAfterExit = true;
      };
      environment.TPM2TOOLS_TCTI = tcti;
      path = [
        pkgs.coreutils
        pkgs.tpm2-tools
      ];
      script = ''
        set -eu
        install -d -m 0755 ${runDir}
        # Once per boot: a second extend would leave PCR 15 at a value no
        # box expects.
        [ -e ${runDir}/settings.json ] && exit 0
        [ -s ${settingsFile} ] || exit 0
        cp ${settingsFile} ${runDir}/settings.json.new
        hash=$(sha256sum < ${runDir}/settings.json.new | cut -d' ' -f1)
        tpm2_pcrextend "15:sha256=$hash"
        mv ${runDir}/settings.json.new ${runDir}/settings.json
      '';
    };

    # New settings take effect by measuring them, which only a boot does.
    systemd.paths.losos-gateway-settings = {
      description = "reboot when the owner's settings change";
      wantedBy = [ "multi-user.target" ];
      pathConfig.PathChanged = settingsFile;
    };
    systemd.services.losos-gateway-settings = {
      description = "reboot to measure the owner's new settings";
      serviceConfig = {
        Type = "oneshot";
        ExecStart = "${config.systemd.package}/bin/systemctl reboot";
      };
    };

    systemd.tmpfiles.rules = [ "d /var/lib/losos-gateway 0700 root root - -" ];

    systemd.services.losos-registrar.path = [ pkgs.tpm2-tools ];

    # Measurements a box does not expect (see the header).
    systemd.services.systemd-pcrphase.enable = false;
    systemd.services.systemd-pcrphase-sysinit.enable = false;
    systemd.services.systemd-pcrphase-initrd.enable = false;
    systemd.services.systemd-pcrmachine.enable = false;
    systemd.services."systemd-pcrfs@".enable = false;
    systemd.services.systemd-pcrfs-root.enable = false;
    boot.initrd.systemd.services.systemd-pcrphase-initrd.enable = lib.mkIf config.boot.initrd.systemd.enable false;
  };
}
