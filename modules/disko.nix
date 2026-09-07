# Disk layout: every drive in losos.targetDrives becomes a GPT disk with one
# LVM physical volume; all PVs feed a single volume group `persist-vg`, whose
# one logical volume `persist` is LUKS-encrypted btrfs mounted at /persist.
# The first drive additionally carries the ESP. The root is a volatile tmpfs,
# rebuilt from the closure every boot; all durable state lives under /persist
# and is bind-mounted back onto the tmpfs root by impermanence.
#
# Pooling drives via LVM (rather than a single disk) means N scavenged disks of
# any size become one logical volume — the "merge them to an array" step the
# installer performs. There is deliberately no RAID level: this is a
# concatenation, so capacity adds up but any single disk failure loses the
# volume; acceptable for a set-and-forget appliance whose data is also
# replicated via Tahoe-LAFS / Nextcloud.
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
  drives = config.losos.targetDrives;
  firstDrive = builtins.head drives;

  # One disko "disk" entry per target drive. Each gets a single GPT partition
  # typed `lvm_pv` feeding `persist-vg`; the first drive also carries the ESP
  # (EFI system partition) so systemd-boot has somewhere to live. disko merges
  # every disk's PV into the one VG declared below.
  mkDisk =
    idx: dev:
    lib.nameValuePair "drive${toString idx}" {
      device = dev;
      type = "disk";
      content = {
        type = "gpt";
        partitions =
          (lib.optionalAttrs (dev == firstDrive) {
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
          })
          // {
            pv = {
              size = "100%";
              content = {
                type = "lvm_pv";
                vg = "persist-vg";
              };
            };
          };
      };
    };
in
{
  assertions = [
    {
      assertion = builtins.length drives > 0;
      message = "losos.targetDrives must list at least one block device.";
    }
  ];

  disko.devices = {
    # Volatile root: rebuilt from the closure on every boot.
    nodev."/" = {
      fsType = "tmpfs";
      mountOptions = [ "mode=755" ];
    };

    # One disk per target drive. builtins.listToAttrs over the indexed list
    # gives { drive0 = ...; drive1 = ...; ... } — disko iterates these and
    # pvcreate's each `pv` partition into the shared VG.
    disk = builtins.listToAttrs (lib.imap0 mkDisk drives);

    # The single volume group spanning every drive's PV. One logical volume
    # `persist` consumes all free extents, then carries the LUKS + btrfs stack
    # exactly as the old single-disk layout did — only the device path changes
    # (now /dev/persist-vg/persist, set by disko's lvm_vg type).
    lvm_vg.persist-vg = {
      type = "lvm_vg";
      lvs.persist = {
        size = "100%FREE";
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
            # NOTE: no `keyFile` here. disko reuses settings.keyFile at
            # *format* time with precedence over passwordFile, and the
            # initrd-only /crypto_keyfile.bin never exists on the installer
            # — luksFormat would die with "Failed to open key file". The
            # boot-side unlock key is declared in boot.nix
            # (boot.initrd.luks.devices.persist.keyFile), next to the
            # boot.initrd.secrets entry that materializes it. TPM2 path
            # relies on the enrolled LUKS2 token via tpm2-device=auto; the
            # password fallback is implied by systemd stage 1.
            crypttabExtraOpts = if useTpm then [ "tpm2-device=auto" ] else [ ];
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
}
