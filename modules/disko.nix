# Disk layout: every drive in losos.targetDrives becomes a GPT disk with one
# LVM physical volume; all PVs feed a single volume group `persist-vg`, whose
# one logical volume `persist` is LUKS-encrypted ext4 mounted at /persist.
# The first drive additionally carries the ESP. The root is a volatile tmpfs,
# rebuilt from the closure every boot; all durable state lives under /persist
# and is bind-mounted back onto the tmpfs root by impermanence.
#
# Pooling drives via LVM (rather than a single disk) means N scavenged disks of
# any size become one logical volume — the "merge them to an array" step the
# installer performs. There is deliberately no RAID level: this is a
# concatenation, so capacity adds up but any single disk failure loses the
# volume; acceptable for a set-and-forget appliance whose data is also
# replicated across the mesh by Longhorn.
#
# The volume is always formatted from the random keyfile the installer
# generates at /etc/keys/persist-keyfile, unattended. What differs by
# losos.tpm.enable is how it is opened at boot:
#   true  -> a TPM2 token the installer enrols right after the format
#            (systemd-cryptenroll, backend/src/installer_io.rs); the keyfile
#            stays only inside /persist as the recovery slot
#   false -> the same keyfile, injected into the initrd (boot.nix)
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
          (lib.optionalAttrs (dev == firstDrive && config.losos.bios) {
            # GRUB's core image has no room between the GPT and the first
            # partition, so a BIOS boot partition holds it. No filesystem and
            # no mountpoint; priority 1 puts it first on the disk.
            boot = {
              type = "EF02";
              size = "1M";
              priority = 1;
            };
          })
          // (lib.optionalAttrs (dev == firstDrive) {
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
    # `persist` consumes all free extents, then carries the LUKS + ext4 stack
    # exactly as the old single-disk layout did — only the device path changes
    # (now /dev/persist-vg/persist, set by disko's lvm_vg type).
    lvm_vg.persist-vg = {
      type = "lvm_vg";
      lvs.persist = {
        # Not 100%FREE, and the missing slice is the feature.
        #
        # The stack below is ext4 inside LUKS inside this LV, which is exactly
        # the arrangement that grows online: lvextend, cryptsetup resize,
        # resize2fs, none of which need /persist unmounted. With every extent
        # already claimed, none of that is reachable — `lvextend` has nothing
        # to take, so the only way to add space is to open the box and add a
        # disk.
        #
        # That matters because /nix *is* /persist here (impermanence binds
        # /persist/nix over /nix), so the store grows with every generation and
        # the 03:00 unattended rebuild is what runs it out of room, on a
        # machine with no shell to notice from. The unallocated remainder is
        # also what an LVM snapshot needs, which is the only cheap rollback
        # this layout can offer.
        #
        # losos.storage.fillPercent = 100 restores the old behaviour.
        size = "${toString config.losos.storage.fillPercent}%FREE";
        content = {
          type = "luks";
          name = "persist"; # -> /dev/mapper/persist
          # Format-time key, in both modes: the keyfile the installer
          # generated. Nothing is ever typed. On the TPM path the installer
          # then enrols the chip from this same file; on the keyfile path the
          # same file unlocks at boot from the initrd.
          passwordFile = "/etc/keys/persist-keyfile";
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
            # password fallback is implied by systemd stage 1. The two must
            # stay exclusive: systemd-cryptsetup given both a key file and
            # tpm2-device= reads the file as a sealed TPM2 blob, not as a
            # LUKS key, so "keyfile first, chip second" is not expressible.
            crypttabExtraOpts = if useTpm then [ "tpm2-device=auto" ] else [ ];
          };
          # ext4, not btrfs, and the choice is load-bearing: fscrypt needs a
          # filesystem that implements it, and btrfs does not (the feature is
          # absent from `mkfs.btrfs -O list-all`; the upstream patches are
          # still unmerged). The shared data domain is protected by an fscrypt
          # policy that is locked whenever losos.sharingMyStorage is off — see
          # docs/superpowers/specs/2026-09-09-k3s-mesh-design.md — so the
          # filesystem has to support it or that whole layer is unavailable.
          #
          # `-O encrypt` must be set at mkfs time; it cannot be enabled later
          # on a mounted filesystem. disko's filesystem type passes extraArgs
          # straight to `mkfs.<format>` before the device.
          #
          # The trade against btrfs is real and deliberate: this loses
          # transparent zstd compression and, more importantly, data
          # checksums. Since the LVM pool is a plain concatenation with no
          # RAID, those checksums were the only thing that *detected* single-
          # disk corruption. Longhorn replication across the mesh now covers
          # that at a different layer.
          content = {
            type = "filesystem";
            format = "ext4";
            extraArgs = [
              "-O"
              "encrypt"
            ];
            mountpoint = "/persist";
            mountOptions = [
              "noatime"
            ];
          };
        };
      };
    };
  };
}
