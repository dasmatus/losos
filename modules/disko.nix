# Disk layout: a GPT disk with an ESP and an encrypted btrfs partition
# mounted at /persist, plus a volatile tmpfs root. All persistent state lives
# under /persist and is bind-mounted back onto the tmpfs root by impermanence.
#
# Unlock method is chosen by losos.tpm.enable:
#   true  -> TPM2 (enroll it post-install; see README)
#   false -> random keyfile at /etc/keys/persist-keyfile (injected into initrd)
{
  lib,
  config,
  ...
}:

let
  useTpm = config.losos.tpm.enable;
in
{
  disko.devices = {
    # Volatile root: rebuilt from the closure on every boot.
    nodev."/" = {
      fsType = "tmpfs";
      mountOptions = [ "mode=755" ];
    };

    disk.main = {
      device = config.losos.targetDrive;
      type = "disk";
      content = {
        type = "gpt";
        partitions = {
          ESP = {
            type = "EF00";
            size = "500M";
            content = {
              type = "filesystem";
              format = "vfat";
              mountpoint = "/boot";
              mountOptions = [ "umask=0077" ];
            };
          };

          persist = {
            size = "100%";
            content = {
              type = "luks";
              name = "persist"; # -> /dev/mapper/persist
              # Format-time key:
              #   TPM path  -> prompt for a passphrase at `disko` format time,
              #                then enroll the TPM2 token afterwards.
              #   keyfile   -> use the pre-generated keyfile as the LUKS key,
              #                so the same file unlocks at boot unattended.
              passwordFile = if useTpm then null else "/etc/keys/persist-keyfile";
              settings = {
                allowDiscards = true;
                # initrd keyfile (only for the keyfile path); TPM2 path relies
                # on the enrolled LUKS2 token via tpm2-device=auto. The
                # password fallback is implied by systemd stage 1.
                keyFile = if useTpm then null else "/crypto_keyfile.bin";
                crypttabExtraOpts =
                  if useTpm then [ "tpm2-device=auto" ] else [ ];
              };
              content = {
                type = "filesystem";
                format = "btrfs";
                mountpoint = "/persist";
                mountOptions = [
                  "compress=zstd"
                  "noatime"
                ];
              };
            };
          };
        };
      };
    };
  };
}